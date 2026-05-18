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
        let line_idx = cursor.line;
        let col = cursor.col;

        if ch == '\n' {
            let old_line_count = self.lines.len();
            let before = if let Some(line) = self.lines.get(line_idx) {
                line[..col].to_string()
            } else {
                String::new()
            };
            let after = if let Some(line) = self.lines.get(line_idx) {
                line[col..].to_string()
            } else {
                String::new()
            };
            let new_lines = vec![Arc::from(before), Arc::from(after)];
            let mut left = self.lines.take(line_idx);
            let right = self.lines.skip(line_idx + 1);
            let new: Vector<Arc<str>> = new_lines.into_iter().collect();
            left.append(new);
            left.append(right);
            self.lines = left;
            let line_delta = self.lines.len() as isize - old_line_count as isize;
            self.invalidate_syntax_from_with_line_delta(line_idx, line_delta);
        } else if let Some(line) = self.lines.get(line_idx) {
            let mut new_line = line.to_string();
            new_line.insert(col, ch);
            self.lines = self.lines.update(line_idx, Arc::from(new_line));
            self.invalidate_syntax_from(line_idx);
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

        let edit = Self::insert_shape_for_text(cursor, text);

        let Some(line) = self.lines.get(cursor.line) else {
            return;
        };
        let old_line_count = self.lines.len();
        let before = line[..cursor.col].to_string();
        let after = line[cursor.col..].to_string();
        let mut inserted_parts = text.split('\n');
        let first_part = inserted_parts.next().unwrap_or_default();
        let mut new_lines: Vec<Arc<str>> = Vec::new();
        new_lines.push(Arc::from(format!("{}{}", before, first_part)));

        for part in inserted_parts {
            new_lines.push(Arc::from(part));
        }

        if new_lines.len() == 1 {
            let mut new_line = new_lines
                .pop()
                .expect("single inserted line should be present")
                .to_string();
            new_line.push_str(&after);
            self.lines = self.lines.update(cursor.line, Arc::from(new_line));
            self.invalidate_syntax_from(cursor.line);
            self.markers.shift_insert(edit);
            return;
        }

        if let Some(last_line) = new_lines.last_mut() {
            let mut merged = last_line.to_string();
            merged.push_str(&after);
            *last_line = Arc::from(merged);
        }

        let mut left = self.lines.take(cursor.line);
        let right = self.lines.skip(cursor.line + 1);
        let inserted: Vector<Arc<str>> = new_lines.into_iter().collect();
        left.append(inserted);
        left.append(right);
        self.lines = left;
        let line_delta = self.lines.len() as isize - old_line_count as isize;
        self.invalidate_syntax_from_with_line_delta(cursor.line, line_delta);

        self.markers.shift_insert(edit);
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
        let edit = DeleteShape { start, end };
        self.clear_inlay_hints_in_range(start, end);
        let start_line = start.line;
        let start_col = start.col;
        let end_line = end.line;
        let end_col = end.col;

        if start_line == end_line {
            if let Some(line) = self.lines.get(start_line) {
                let mut new_line = line.to_string();
                new_line.drain(start_col..end_col);
                self.lines = self.lines.update(start_line, Arc::from(new_line));
                self.invalidate_syntax_from(start_line);
            }
        } else {
            let old_line_count = self.lines.len();
            let before = if let Some(line) = self.lines.get(start_line) {
                line[..start_col].to_string()
            } else {
                String::new()
            };
            let after = if let Some(line) = self.lines.get(end_line) {
                line[end_col..].to_string()
            } else {
                String::new()
            };
            let merged = Arc::from(format!("{}{}", before, after));
            let mut left = self.lines.take(start_line);
            let right = self.lines.skip(end_line + 1);
            left.push_back(merged);
            left.append(right);
            self.lines = left;
            let line_delta = self.lines.len() as isize - old_line_count as isize;
            self.invalidate_syntax_from_with_line_delta(start_line, line_delta);
        }

        self.markers.shift_delete(edit);
    }

    /// Applies a completion replacement and any same-buffer edits.
    pub fn apply_completion(
        &mut self,
        range: TextObjectRange,
        replacement: &str,
        cursor_offset: usize,
        additional_text_edits: &[crate::ui::completion::CompletionTextEdit],
    ) -> Option<Cursor> {
        let mut main_start = completion_range_to_byte_offset(&self.lines, range.start)?;
        let mut main_end = completion_range_to_byte_offset(&self.lines, range.end)?;

        let mut edits: Vec<(usize, usize, &str)> =
            Vec::with_capacity(additional_text_edits.len() + 1);
        edits.push((main_start, main_end, replacement));

        for edit in additional_text_edits {
            let edit_start = completion_range_to_byte_offset(&self.lines, edit.range.start)?;
            let edit_end = completion_range_to_byte_offset(&self.lines, edit.range.end)?;

            if edit_end <= main_start {
                let delta = edit.text.len() as isize - (edit_end as isize - edit_start as isize);
                main_start = offset_with_delta(main_start, delta);
                main_end = offset_with_delta(main_end, delta);
            } else if edit_start < main_end {
                return self.apply_completion_main_only(range, replacement, cursor_offset);
            }

            edits.push((edit_start, edit_end, edit.text.as_str()));
        }

        edits.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));

        for (edit_start, edit_end, text) in edits {
            let start_cursor = completion_cursor_from_byte_offset(&self.lines, edit_start)?;
            let end_cursor = completion_cursor_from_byte_offset(&self.lines, edit_end)?;
            self.remove(start_cursor, end_cursor);
            self.insert_text(start_cursor, text);
        }

        let next_cursor = completion_cursor_from_byte_offset(
            &self.lines,
            main_start.saturating_add(cursor_offset),
        )?;
        self.push_snapshot(next_cursor);
        Some(next_cursor)
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
        let start_offset = completion_range_to_byte_offset(&self.lines, start)?;
        let next_cursor = completion_cursor_from_byte_offset(
            &self.lines,
            start_offset.saturating_add(cursor_offset),
        )?;
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
            let line = self.line_at(start.line)?;
            return Some(line[start.col..end.col].to_string());
        }

        let mut text = String::new();
        let first_line = self.line_at(start.line)?;
        text.push_str(&first_line[start.col..]);
        for line_idx in start.line + 1..end.line {
            text.push('\n');
            text.push_str(self.line_at(line_idx)?);
        }
        text.push('\n');
        let last_line = self.line_at(end.line)?;
        text.push_str(&last_line[..end.col]);
        Some(text)
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
            text.push_str(self.line_at(line_idx)?);
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
            let current_content: String = self
                .lines
                .get(current_line)
                .map_or("", |s| s.as_ref())
                .to_string();
            let prev_content: String = self
                .lines
                .get(prev_line)
                .map_or("", |s| s.as_ref())
                .to_string();
            let prev_content_len = prev_content.len();
            let old_line_count = self.lines.len();
            let merged = Arc::from(format!("{}{}", prev_content, current_content));
            let mut left = self.lines.take(prev_line);
            let right = self.lines.skip(current_line + 1);
            left.push_back(merged);
            left.append(right);
            self.lines = left;
            let line_delta = self.lines.len() as isize - old_line_count as isize;
            self.invalidate_syntax_from_with_line_delta(prev_line, line_delta);
            return Some(Cursor::new(prev_line, prev_content_len));
        }

        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();
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
            if cursor.line >= self.lines.len() - 1 {
                return None;
            }
            let current_line = cursor.line;
            let next_line = current_line + 1;
            let current_content: String = self
                .lines
                .get(current_line)
                .map_or("", |s| s.as_ref())
                .to_string();
            let next_content: String = self
                .lines
                .get(next_line)
                .map_or("", |s| s.as_ref())
                .to_string();
            let current_content_len = current_content.len();
            let old_line_count = self.lines.len();
            let merged = Arc::from(format!("{}{}", current_content, next_content));
            let mut left = self.lines.take(current_line);
            let right = self.lines.skip(next_line + 1);
            left.push_back(merged);
            left.append(right);
            self.lines = left;
            let line_delta = self.lines.len() as isize - old_line_count as isize;
            self.invalidate_syntax_from_with_line_delta(current_line, line_delta);
            return Some(Cursor::new(current_line, current_content_len));
        }

        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();
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
        let total_lines = self.lines.len();
        if start_line >= total_lines {
            return None;
        }
        let actual_line_count = (total_lines - start_line).min(line_count);
        if actual_line_count < 2 {
            return None;
        }
        let old_line_count = self.lines.len();
        let mut joined_content = String::new();
        for i in 0..actual_line_count {
            let line_idx = start_line + i;
            if let Some(line) = self.lines.get(line_idx) {
                if i > 0 && with_space {
                    joined_content.push(' ');
                }
                joined_content.push_str(line);
            }
        }
        let end_line = start_line + actual_line_count;
        let right = self.lines.skip(end_line);
        let joined_len = joined_content.len();
        let mut left = self.lines.take(start_line);
        left.push_back(Arc::from(joined_content));
        left.append(right);
        self.lines = left;
        let line_delta = self.lines.len() as isize - old_line_count as isize;
        self.invalidate_syntax_from_with_line_delta(start_line, line_delta);
        Some(Cursor::new(start_line, joined_len))
    }

    pub fn delete_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();
        if total_lines == 0 {
            return Some(Cursor::new(0, 0));
        }
        if start_line >= total_lines {
            return None;
        }
        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(Cursor::new(start_line, 0));
        }
        let old_line_count = self.lines.len();
        let end_line = start_line + actual_count;
        if end_line >= total_lines {
            let mut left = self.lines.take(start_line);
            if left.is_empty() {
                left.push_back(Arc::from(""));
            }
            self.lines = left;
        } else {
            let mut left = self.lines.take(start_line);
            let right = self.lines.skip(end_line);
            left.append(right);
            self.lines = left;
        }
        self.markers.delete_lines(start_line, actual_count);
        let line_delta = self.lines.len() as isize - old_line_count as isize;
        self.invalidate_syntax_from_with_line_delta(start_line, line_delta);
        let new_line_count = self.lines.len();
        if new_line_count == 0 {
            Some(Cursor::new(0, 0))
        } else if start_line >= new_line_count {
            Some(Cursor::new(new_line_count - 1, 0))
        } else {
            Some(Cursor::new(start_line, 0))
        }
    }

    pub fn change_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();
        if total_lines == 0 {
            self.lines.push_back(Arc::from(""));
            self.invalidate_syntax_from_with_line_delta(
                0,
                self.lines.len() as isize - total_lines as isize,
            );
            return Some(Cursor::new(0, 0));
        }
        if start_line >= total_lines {
            return None;
        }
        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(Cursor::new(start_line, 0));
        }
        let end_line = start_line + actual_count;
        let old_line_count = self.lines.len();
        if end_line >= total_lines {
            let mut left = self.lines.take(start_line);
            left.push_back(Arc::from(""));
            self.lines = left;
        } else {
            let mut left = self.lines.take(start_line);
            left.push_back(Arc::from(""));
            let right = self.lines.skip(end_line);
            left.append(right);
            self.lines = left;
        }
        let line_delta = self.lines.len() as isize - old_line_count as isize;
        self.invalidate_syntax_from_with_line_delta(start_line, line_delta);
        Some(Cursor::new(start_line, 0))
    }

    pub fn change_to_line_end(&mut self, start: Cursor, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();
        if total_lines == 0 {
            return Some(Cursor::new(0, 0));
        }
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
        let total_lines = self.lines.len();
        if total_lines == 0 {
            if count > 0 {
                self.lines.push_back(Arc::from(""));
            }
            self.invalidate_syntax_from_with_line_delta(
                0,
                self.lines.len() as isize - total_lines as isize,
            );
            self.markers.insert_lines(0, count);
            return Some(Cursor::new(0, 0));
        }
        let insert_after = line.min(total_lines);
        if count == 0 {
            return Some(Cursor::new(line, 0));
        }
        let old_line_count = self.lines.len();
        if insert_after >= total_lines {
            for _ in 0..count {
                self.lines.push_back(Arc::from(""));
            }
            self.invalidate_syntax_from_with_line_delta(
                total_lines,
                self.lines.len() as isize - old_line_count as isize,
            );
            self.markers.insert_lines(total_lines, count);
            Some(Cursor::new(total_lines, 0))
        } else {
            let mut left = self.lines.take(insert_after + 1);
            let right = self.lines.skip(insert_after + 1);
            for _ in 0..count {
                left.push_back(Arc::from(""));
            }
            left.append(right);
            self.lines = left;
            self.invalidate_syntax_from_with_line_delta(
                insert_after + 1,
                self.lines.len() as isize - old_line_count as isize,
            );
            self.markers.insert_lines(insert_after + 1, count);
            Some(Cursor::new(insert_after + 1, 0))
        }
    }

    pub fn insert_lines_before(&mut self, line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();
        if total_lines == 0 {
            if count > 0 {
                self.lines.push_back(Arc::from(""));
            }
            self.invalidate_syntax_from_with_line_delta(
                0,
                self.lines.len() as isize - total_lines as isize,
            );
            self.markers.insert_lines(0, count);
            return Some(Cursor::new(0, 0));
        }
        if count == 0 {
            return Some(Cursor::new(line, 0));
        }
        let old_line_count = self.lines.len();
        if line == 0 {
            for _ in 0..count {
                self.lines.push_front(Arc::from(""));
            }
            self.invalidate_syntax_from_with_line_delta(
                0,
                self.lines.len() as isize - old_line_count as isize,
            );
            self.markers.insert_lines(0, count);
            Some(Cursor::new(0, 0))
        } else {
            let insert_before = line.saturating_sub(1);
            if insert_before >= total_lines {
                for _ in 0..count {
                    self.lines.push_back(Arc::from(""));
                }
                self.invalidate_syntax_from_with_line_delta(
                    total_lines,
                    self.lines.len() as isize - old_line_count as isize,
                );
                self.markers.insert_lines(total_lines, count);
                Some(Cursor::new(total_lines, 0))
            } else {
                let mut left = self.lines.take(insert_before + 1);
                let right = self.lines.skip(insert_before + 1);
                for _ in 0..count {
                    left.push_back(Arc::from(""));
                }
                left.append(right);
                self.lines = left;
                self.invalidate_syntax_from_with_line_delta(
                    line,
                    self.lines.len() as isize - old_line_count as isize,
                );
                self.markers.insert_lines(line, count);
                Some(Cursor::new(insert_before + 1, 0))
            }
        }
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
        let total_lines = self.lines.len();
        let content_len = content_lines.len();
        if content_len == 0 {
            return Some(Cursor::new(line, 0));
        }

        let insert_at = if after {
            (line + 1).min(total_lines)
        } else {
            line.min(total_lines)
        };

        let old_line_count = self.lines.len();

        if total_lines == 0 {
            for content in content_lines {
                self.lines.push_back(content.clone());
            }
        } else if insert_at >= total_lines {
            for content in content_lines {
                self.lines.push_back(content.clone());
            }
        } else {
            let mut left = self.lines.take(insert_at);
            let right = self.lines.skip(insert_at);
            for content in content_lines {
                left.push_back(content.clone());
            }
            left.append(right);
            self.lines = left;
        }

        self.markers.insert_lines(insert_at, content_len);

        let line_delta = self.lines.len() as isize - old_line_count as isize;
        self.invalidate_syntax_from_with_line_delta(insert_at, line_delta);
        Some(Cursor::new(insert_at, 0))
    }
}

fn completion_range_to_byte_offset(lines: &Vector<Arc<str>>, cursor: Cursor) -> Option<usize> {
    let mut offset = 0usize;

    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx == cursor.line {
            return (cursor.col <= line.len()).then_some(offset + cursor.col);
        }

        offset = offset.saturating_add(line.len());
        if line_idx + 1 < lines.len() {
            offset = offset.saturating_add(1);
        }
    }

    None
}

fn completion_cursor_from_byte_offset(lines: &Vector<Arc<str>>, offset: usize) -> Option<Cursor> {
    let mut current = 0usize;

    for (line_idx, line) in lines.iter().enumerate() {
        if offset <= current + line.len() {
            return Some(Cursor::new(line_idx, offset - current));
        }

        current = current.saturating_add(line.len());
        if line_idx + 1 < lines.len() {
            current = current.saturating_add(1);
        }
    }

    None
}

fn offset_with_delta(offset: usize, delta: isize) -> usize {
    if delta.is_negative() {
        offset.saturating_sub(delta.unsigned_abs())
    } else {
        offset.saturating_add(delta as usize)
    }
}
