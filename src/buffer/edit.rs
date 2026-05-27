use super::*;

impl Buffer {
    fn insert_shape_for_text(cursor: Cursor, text: &str) -> InsertShape {
        let line_delta = text.split('\n').count().saturating_sub(1);
        let tail_col = if line_delta == 0 {
            cursor.col + text.len()
        } else {
            text.rsplit('\n').next().map_or(0, |tail| tail.len())
        };

        InsertShape {
            at: cursor,
            line_delta,
            tail_col,
        }
    }

    fn insert_shape_for_char(cursor: Cursor, ch: char) -> InsertShape {
        if ch == '\n' {
            InsertShape {
                at: cursor,
                line_delta: 1,
                tail_col: 0,
            }
        } else {
            InsertShape {
                at: cursor,
                line_delta: 0,
                tail_col: cursor.col + ch.len_utf8(),
            }
        }
    }

    pub fn insert_char(&mut self, cursor: Cursor, ch: char) {
        debug_assert!(
            self.is_valid_cursor(cursor),
            "insert_char called with invalid cursor: {:?}",
            cursor
        );
        let edit = Self::insert_shape_for_char(cursor, ch);
        if let Some(change) = self.lines.insert_char(cursor, ch) {
            self.apply_cache_edits(&[LineEdit::new(change.first_changed_line, change.line_delta)]);
        }

        self.markers.shift_insert(edit);
    }

    pub fn insert_text(&mut self, cursor: Cursor, text: &str) {
        debug_assert!(
            self.is_valid_cursor(cursor),
            "insert_text called with invalid cursor: {:?}",
            cursor
        );
        if text.is_empty() {
            return;
        }

        if let Some(change) = self.insert_text_without_cache_invalidation(cursor, text) {
            self.apply_cache_edits(&[LineEdit::new(change.first_changed_line, change.line_delta)]);
        }
    }

    pub fn remove(&mut self, start: Cursor, end: Cursor) {
        debug_assert!(
            self.is_valid_cursor(start),
            "remove called with invalid start cursor: {:?}",
            start
        );
        debug_assert!(
            self.is_valid_cursor(end),
            "remove called with invalid end cursor: {:?}",
            end
        );
        if start.line > end.line || (start.line == end.line && start.col >= end.col) {
            return;
        }
        if let Some(change) = self.remove_without_cache_invalidation(start, end) {
            self.apply_cache_edits(&[LineEdit::new(change.first_changed_line, change.line_delta)]);
        }
    }

    fn insert_text_without_cache_invalidation(
        &mut self,
        cursor: Cursor,
        text: &str,
    ) -> Option<TextChange> {
        let edit = Self::insert_shape_for_text(cursor, text);
        let change = self.lines.insert_text(cursor, text)?;
        self.markers.shift_insert(edit);
        Some(change)
    }

    fn remove_without_cache_invalidation(
        &mut self,
        start: Cursor,
        end: Cursor,
    ) -> Option<TextChange> {
        let edit = DeleteShape { start, end };
        self.clear_inlay_hints_in_range(start, end);
        let change = self.lines.remove(start, end)?;
        self.markers.shift_delete(edit);
        Some(change)
    }

    /// Applies a completion replacement and any same-buffer edits.
    pub fn apply_completion(
        &mut self,
        range: TextObjectRange,
        replacement: &str,
        cursor_offset: usize,
        additional_text_edits: &[crate::ui::completion::CompletionTextEdit],
    ) -> Option<Cursor> {
        let mut main_start = self.lines.byte_offset_for_cursor(range.start)?;
        let mut main_end = self.lines.byte_offset_for_cursor(range.end)?;

        let mut edits: Vec<(usize, usize, usize, isize, &str)> =
            Vec::with_capacity(additional_text_edits.len() + 1);
        let main_line_delta = replacement.split('\n').count().saturating_sub(1) as isize;
        edits.push((
            main_start,
            main_end,
            range.start.line,
            main_line_delta - (range.end.line as isize - range.start.line as isize),
            replacement,
        ));

        for edit in additional_text_edits {
            let edit_start = self.lines.byte_offset_for_cursor(edit.range.start)?;
            let edit_end = self.lines.byte_offset_for_cursor(edit.range.end)?;
            let edit_line_delta = edit.text.split('\n').count().saturating_sub(1) as isize
                - (edit.range.end.line as isize - edit.range.start.line as isize);

            if edit_end <= main_start {
                let delta = edit.text.len() as isize - (edit_end as isize - edit_start as isize);
                main_start = offset_with_delta(main_start, delta);
                main_end = offset_with_delta(main_end, delta);
            } else if edit_start < main_end {
                return self.apply_completion_main_only(range, replacement, cursor_offset);
            }

            edits.push((
                edit_start,
                edit_end,
                edit.range.start.line,
                edit_line_delta,
                edit.text.as_str(),
            ));
        }

        edits.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
        let cache_edits: Vec<LineEdit> = edits
            .iter()
            .map(|(_, _, line, line_delta, _)| LineEdit::new(*line, *line_delta))
            .collect();

        for (edit_start, edit_end, _, _, text) in edits {
            let start_cursor = self.lines.cursor_for_byte_offset(edit_start)?;
            let end_cursor = self.lines.cursor_for_byte_offset(edit_end)?;
            self.remove_without_cache_invalidation(start_cursor, end_cursor);
            if !text.is_empty() {
                self.insert_text_without_cache_invalidation(start_cursor, text);
            }
        }

        self.apply_cache_edits(&cache_edits);

        let next_cursor = self
            .lines
            .cursor_for_byte_offset(main_start.saturating_add(cursor_offset))?;
        self.warm_syntax_through_with_budget(next_cursor.line, std::time::Duration::from_millis(2));
        self.push_snapshot(next_cursor);
        Some(next_cursor)
    }

    /// Applies a batch of text edits and updates cache state incrementally.
    pub fn apply_text_edits(&mut self, edits: &[(Cursor, Cursor, String)]) -> bool {
        if edits.is_empty() {
            return true;
        }

        let mut edits = edits.to_vec();
        edits.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));

        let cache_edits: Vec<LineEdit> = edits
            .iter()
            .map(|(start, end, text)| {
                let line_delta = text.split('\n').count().saturating_sub(1) as isize
                    - (end.line as isize - start.line as isize);
                LineEdit::new(start.line, line_delta)
            })
            .collect();

        for (start, end, text) in &edits {
            self.remove_without_cache_invalidation(*start, *end);
            if !text.is_empty() {
                self.insert_text_without_cache_invalidation(*start, text.as_str());
            }
        }

        self.apply_cache_edits(&cache_edits);
        self.push_snapshot(self.current_cursor());
        true
    }

    fn apply_completion_main_only(
        &mut self,
        range: TextObjectRange,
        replacement: &str,
        cursor_offset: usize,
    ) -> Option<Cursor> {
        let start = range.start;
        let end = range.end;
        self.remove(start, end);
        self.insert_text(start, replacement);
        let start_offset = self.lines.byte_offset_for_cursor(start)?;
        let next_cursor = self
            .lines
            .cursor_for_byte_offset(start_offset.saturating_add(cursor_offset))?;
        self.push_snapshot(next_cursor);
        Some(next_cursor)
    }

    /// Returns the exact text covered by a characterwise range.
    pub fn text_in_range(&self, start: Cursor, end: Cursor) -> Option<String> {
        if start.line > end.line || (start.line == end.line && start.col >= end.col) {
            return Some(String::new());
        }
        if !self.is_valid_cursor(start) || !self.is_valid_cursor(end) {
            return None;
        }

        if start.line == end.line {
            return self.line_at(start.line)?.range_text(start.col, end.col);
        }
        self.lines.range(start, end).map(|text| text.to_text())
    }

    /// Returns the exact text covered by a whole-line range.
    pub fn text_in_lines(&self, start_line: usize, count: usize) -> Option<String> {
        let total_lines = self.line_count();
        if start_line >= total_lines {
            return None;
        }
        if count == 0 {
            return Some(String::new());
        }

        let actual_count = (total_lines - start_line).min(count);
        let mut text = String::new();
        for line_idx in start_line..start_line + actual_count {
            if line_idx > start_line {
                text.push('\n');
            }
            let line = self.line_at(line_idx)?;
            for chunk in line.chunks() {
                text.push_str(chunk);
            }
        }
        Some(text)
    }

    pub fn delete_char_before_cursor(&mut self, cursor: Cursor) -> Option<Cursor> {
        if cursor.col == 0 {
            if cursor.line == 0 {
                return None;
            }
            let current_line = cursor.line;
            let prev_line = current_line - 1;
            let prev_content_len = self.line_len(prev_line);
            let end = Cursor::new(current_line, 0);
            if let Some(change) = self
                .lines
                .remove(Cursor::new(prev_line, prev_content_len), end)
            {
                self.invalidate_syntax_from_with_line_delta(
                    change.first_changed_line,
                    change.line_delta,
                );
            }
            return Some(Cursor::new(prev_line, prev_content_len));
        }

        let line = self.line_at(cursor.line)?;
        let mut scratch = String::new();
        let line_str = line.contiguous_text_with_scratch(&mut scratch);
        let mut prev_grapheme_start: Option<(usize, usize)> = None;
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset < cursor.col {
                prev_grapheme_start = Some((byte_offset, byte_offset + grapheme.len()));
            } else {
                break;
            }
        }
        if let Some((start, end)) = prev_grapheme_start {
            self.remove(
                Cursor::new(cursor.line, start),
                Cursor::new(cursor.line, end),
            );
            return Some(Cursor::new(cursor.line, start));
        }
        Some(cursor)
    }

    pub fn delete_char_at_cursor(&mut self, cursor: Cursor) -> Option<Cursor> {
        let line_len = self.line_len(cursor.line);
        if cursor.col >= line_len {
            if cursor.line >= self.lines.line_count() - 1 {
                return None;
            }
            let current_line = cursor.line;
            let next_line = current_line + 1;
            let current_content_len = self.line_len(current_line);
            if let Some(change) = self.lines.remove(
                Cursor::new(current_line, current_content_len),
                Cursor::new(next_line, 0),
            ) {
                self.invalidate_syntax_from_with_line_delta(
                    change.first_changed_line,
                    change.line_delta,
                );
            }
            return Some(Cursor::new(current_line, current_content_len));
        }

        let line = self.line_at(cursor.line)?;
        let mut scratch = String::new();
        let line_str = line.contiguous_text_with_scratch(&mut scratch);
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset >= cursor.col {
                let start = byte_offset;
                let end = byte_offset + grapheme.len();
                self.remove(
                    Cursor::new(cursor.line, start),
                    Cursor::new(cursor.line, end),
                );
                self.invalidate_syntax_from(cursor.line);
                return Some(Cursor::new(cursor.line, start));
            }
        }
        Some(cursor)
    }

    pub fn join_lines(
        &mut self,
        start_line: usize,
        line_count: usize,
        with_space: bool,
    ) -> Option<Cursor> {
        if line_count < 2 {
            return None;
        }
        let (cursor, change) = self.lines.join_lines(start_line, line_count, with_space)?;
        self.invalidate_syntax_from_with_line_delta(change.first_changed_line, change.line_delta);
        Some(cursor)
    }

    pub fn delete_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.line_count();
        if start_line >= total_lines {
            return None;
        }
        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(Cursor::new(start_line, 0));
        }
        let change = self.lines.delete_lines(start_line, actual_count)?;
        self.markers.delete_lines(start_line, actual_count);
        self.invalidate_syntax_from_with_line_delta(change.first_changed_line, change.line_delta);
        let new_line_count = self.lines.line_count();
        if start_line >= new_line_count {
            Some(Cursor::new(new_line_count - 1, 0))
        } else {
            Some(Cursor::new(start_line, 0))
        }
    }

    pub fn change_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.line_count();
        if start_line >= total_lines {
            return None;
        }
        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(Cursor::new(start_line, 0));
        }
        let change = self.lines.change_lines(start_line, actual_count)?;
        self.invalidate_syntax_from_with_line_delta(change.first_changed_line, change.line_delta);
        Some(Cursor::new(start_line, 0))
    }

    pub fn change_to_line_end(&mut self, start: Cursor, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.line_count();
        if start.line >= total_lines {
            return None;
        }
        let actual_count = (total_lines - start.line).min(count);
        if actual_count == 0 {
            return Some(start);
        }
        let end_line = start.line + actual_count - 1;
        let end_col = self.line_len(end_line);
        let end = Cursor::new(end_line, end_col);
        self.remove(start, end);
        Some(start)
    }

    pub fn insert_lines_after(&mut self, line: usize, count: usize) -> Option<Cursor> {
        if count == 0 {
            return Some(Cursor::new(line, 0));
        }
        let change = self.lines.insert_blank_lines_after(line, count)?;
        self.invalidate_syntax_from_with_line_delta(change.first_changed_line, change.line_delta);
        self.markers.insert_lines(change.first_changed_line, count);
        Some(Cursor::new(change.first_changed_line, 0))
    }

    pub fn insert_lines_before(&mut self, line: usize, count: usize) -> Option<Cursor> {
        if count == 0 {
            return Some(Cursor::new(line, 0));
        }
        let change = self.lines.insert_blank_lines_before(line, count)?;
        self.invalidate_syntax_from_with_line_delta(change.first_changed_line, change.line_delta);
        self.markers.insert_lines(change.first_changed_line, count);
        Some(Cursor::new(change.first_changed_line, 0))
    }

    pub fn delete_range(&mut self, range: TextObjectRange) -> Option<Cursor> {
        let start = range.start;
        let end = range.end;
        if start.line > end.line || (start.line == end.line && start.col >= end.col) {
            return Some(start);
        }
        self.remove(start, end);
        Some(start)
    }

    pub fn paste_linewise_content(
        &mut self,
        line: usize,
        content_lines: &[Arc<str>],
        after: bool,
    ) -> Option<Cursor> {
        let total_lines = self.lines.line_count();
        let content_len = content_lines.len();
        if content_len == 0 {
            return Some(Cursor::new(line, 0));
        }

        let insert_at = if after {
            (line + 1).min(total_lines)
        } else {
            line.min(total_lines)
        };

        let change = self.lines.paste_linewise(
            line,
            content_lines.iter().map(|line| line.as_ref()),
            after,
        )?;

        self.markers.insert_lines(insert_at, content_len);

        self.invalidate_syntax_from_with_line_delta(change.first_changed_line, change.line_delta);
        Some(Cursor::new(insert_at, 0))
    }
}

fn offset_with_delta(offset: usize, delta: isize) -> usize {
    if delta.is_negative() {
        offset.saturating_sub(delta.unsigned_abs())
    } else {
        offset.saturating_add(delta as usize)
    }
}
