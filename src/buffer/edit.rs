use super::*;

impl Buffer {
    pub fn insert_char(&mut self, cursor: Cursor, ch: char) {
        debug_assert!(
            self.is_valid_cursor(cursor),
            "insert_char called with invalid cursor: {:?}",
            cursor
        );
        let line_idx = cursor.line;
        let col = cursor.col;

        if ch == '\n' {
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
            self.invalidate_syntax_from(line_idx);
        } else if let Some(line) = self.lines.get(line_idx) {
            let mut new_line = line.to_string();
            new_line.insert(col, ch);
            self.lines = self.lines.update(line_idx, Arc::from(new_line));
            self.invalidate_syntax_from(line_idx);
        }
    }

    pub fn insert_text(&mut self, mut cursor: Cursor, text: &str) {
        debug_assert!(
            self.is_valid_cursor(cursor),
            "insert_text called with invalid cursor: {:?}",
            cursor
        );
        for ch in text.chars() {
            self.insert_char(cursor, ch);
            if ch == '\n' {
                cursor = Cursor::new(cursor.line + 1, 0);
            } else {
                cursor = Cursor::new(cursor.line, cursor.col + ch.len_utf8());
            }
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
            self.invalidate_syntax_from(start_line);
        }
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
            let merged = Arc::from(format!("{}{}", prev_content, current_content));
            let mut left = self.lines.take(prev_line);
            let right = self.lines.skip(current_line + 1);
            left.push_back(merged);
            left.append(right);
            self.lines = left;
            self.invalidate_syntax_from(prev_line);
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
            let merged = Arc::from(format!("{}{}", current_content, next_content));
            let mut left = self.lines.take(current_line);
            let right = self.lines.skip(next_line + 1);
            left.push_back(merged);
            left.append(right);
            self.lines = left;
            self.invalidate_syntax_from(current_line);
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
        self.invalidate_syntax_from(start_line);
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
        self.invalidate_syntax_from(start_line);
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
            self.invalidate_syntax_from(0);
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
        self.invalidate_syntax_from(start_line);
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
            self.invalidate_syntax_from(0);
            return Some(Cursor::new(0, 0));
        }
        let insert_after = line.min(total_lines);
        if count == 0 {
            return Some(Cursor::new(line, 0));
        }
        if insert_after >= total_lines {
            for _ in 0..count {
                self.lines.push_back(Arc::from(""));
            }
            self.invalidate_syntax_from(total_lines);
            Some(Cursor::new(total_lines, 0))
        } else {
            let mut left = self.lines.take(insert_after + 1);
            let right = self.lines.skip(insert_after + 1);
            for _ in 0..count {
                left.push_back(Arc::from(""));
            }
            left.append(right);
            self.lines = left;
            self.invalidate_syntax_from(insert_after + 1);
            Some(Cursor::new(insert_after + 1, 0))
        }
    }

    pub fn insert_lines_before(&mut self, line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();
        if total_lines == 0 {
            if count > 0 {
                self.lines.push_back(Arc::from(""));
            }
            self.invalidate_syntax_from(0);
            return Some(Cursor::new(0, 0));
        }
        if count == 0 {
            return Some(Cursor::new(line, 0));
        }
        if line == 0 {
            for _ in 0..count {
                self.lines.push_front(Arc::from(""));
            }
            self.invalidate_syntax_from(0);
            Some(Cursor::new(0, 0))
        } else {
            let insert_before = line.saturating_sub(1);
            if insert_before >= total_lines {
                for _ in 0..count {
                    self.lines.push_back(Arc::from(""));
                }
                self.invalidate_syntax_from(total_lines);
                Some(Cursor::new(total_lines, 0))
            } else {
                let mut left = self.lines.take(insert_before + 1);
                let right = self.lines.skip(insert_before + 1);
                for _ in 0..count {
                    left.push_back(Arc::from(""));
                }
                left.append(right);
                self.lines = left;
                self.invalidate_syntax_from(line);
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
}
