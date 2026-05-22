use super::*;

impl Buffer {
    /// Returns the character at the cursor, if the cursor is positioned on a character.
    pub fn char_at_cursor(&self, cursor: Cursor) -> Option<char> {
        let line = self.line_at(cursor.line)?;
        line.char_at(cursor.col)
    }

    /// Returns the character immediately before the cursor, if one exists.
    pub fn char_before_cursor(&self, cursor: Cursor) -> Option<char> {
        if cursor.col == 0 {
            return None;
        }
        let line = self.line_at(cursor.line)?;
        line.previous_char(cursor.col).map(|(_, ch)| ch)
    }

    pub fn line_len(&self, line_idx: usize) -> usize {
        self.lines.line_len(line_idx)
    }

    pub fn is_valid_cursor(&self, cursor: Cursor) -> bool {
        if cursor.line >= self.lines.line_count() {
            return false;
        }
        let line_len = self.line_len(cursor.line);
        if cursor.col > line_len {
            return false;
        }
        if cursor.col == line_len {
            return true;
        }
        self.line_at(cursor.line)
            .is_some_and(|line| line.is_char_boundary(cursor.col))
    }

    /// Normalizes a cursor so it lands on a valid grapheme boundary in the current buffer.
    pub fn sync_cursor(&self, cursor: Cursor) -> Cursor {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return Cursor::new(0, 0);
        }

        let line_idx = cursor.line.min(total_lines - 1);
        let line_len = self.line_len(line_idx);
        let col = cursor.col.min(line_len);
        let cursor = Cursor::new(line_idx, col);
        if self.is_valid_cursor(cursor) {
            return cursor;
        }

        let Some(line) = self.line_at(line_idx) else {
            return cursor;
        };
        let mut previous_boundary = 0usize;

        for grapheme in line.graphemes() {
            let grapheme_idx = grapheme.byte_idx();
            let next_boundary = grapheme_idx + grapheme.len();
            if col <= grapheme_idx {
                return Cursor::new(line_idx, grapheme_idx);
            }
            if col < next_boundary {
                let left_distance = col - grapheme_idx;
                let right_distance = next_boundary - col;
                let synced_col = if right_distance < left_distance {
                    next_boundary
                } else {
                    grapheme_idx
                };
                return Cursor::new(line_idx, synced_col);
            }

            previous_boundary = next_boundary;
        }

        Cursor::new(line_idx, previous_boundary.min(line_len))
    }

    pub fn next_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        let line_len = self.line_len(cursor.line);
        if cursor.col < line_len {
            let line = self.line_at(cursor.line)?;
            for grapheme in line.graphemes() {
                if grapheme.byte_idx() <= cursor.col {
                    continue;
                }
                return Some(Cursor::new(cursor.line, grapheme.byte_idx()));
            }
            Some(Cursor::new(cursor.line, line_len))
        } else if cursor.line < self.lines.line_count() - 1 {
            Some(Cursor::new(cursor.line + 1, 0))
        } else {
            None
        }
    }

    pub fn prev_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.col > 0 {
            let line = self.line_at(cursor.line)?;
            let previous = line.previous_grapheme(cursor.col)?;
            Some(Cursor::new(cursor.line, previous.byte_idx()))
        } else if cursor.line > 0 {
            let prev_line_len = self.line_len(cursor.line - 1);
            Some(Cursor::new(cursor.line - 1, prev_line_len))
        } else {
            None
        }
    }

    pub fn next_cursor_line(&self, cursor: Cursor) -> Option<Cursor> {
        let line_len = self.line_len(cursor.line);
        if cursor.col >= line_len {
            return None;
        }
        let line = self.line_at(cursor.line)?;
        for grapheme in line.graphemes() {
            if grapheme.byte_idx() <= cursor.col {
                continue;
            }
            return Some(Cursor::new(cursor.line, grapheme.byte_idx()));
        }
        Some(Cursor::new(cursor.line, line_len))
    }

    pub fn prev_cursor_line(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.col == 0 {
            return None;
        }
        let line = self.line_at(cursor.line)?;
        let previous = line.previous_grapheme(cursor.col)?;
        Some(Cursor::new(cursor.line, previous.byte_idx()))
    }

    pub fn cursor_down(&self, cursor: Cursor, visual_col: usize) -> Option<Cursor> {
        if cursor.line >= self.lines.line_count() - 1 {
            return None;
        }
        let next_line = cursor.line + 1;
        let target_col = self.byte_pos_at_visual_col(next_line, visual_col);
        Some(Cursor::new(next_line, target_col))
    }

    pub fn cursor_up(&self, cursor: Cursor, visual_col: usize) -> Option<Cursor> {
        if cursor.line == 0 {
            return None;
        }
        let prev_line = cursor.line - 1;
        let target_col = self.byte_pos_at_visual_col(prev_line, visual_col);
        Some(Cursor::new(prev_line, target_col))
    }

    pub(super) fn first_non_whitespace_col(&self, line_idx: usize) -> Option<usize> {
        let line = self.line_at(line_idx)?;
        line.graphemes()
            .find(|grapheme| !Self::is_whitespace_char(grapheme.as_str()))
            .map(|grapheme| grapheme.byte_idx())
    }

    pub(super) fn last_non_whitespace_col(&self, line_idx: usize) -> Option<usize> {
        let line = self.line_at(line_idx)?;
        line.graphemes()
            .filter(|grapheme| !Self::is_whitespace_char(grapheme.as_str()))
            .map(|grapheme| grapheme.byte_idx())
            .last()
    }

    pub fn cursor_end_of_line(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }
        let end_pos = self.last_non_whitespace_col(cursor.line).unwrap_or(0);
        if cursor.col < end_pos {
            return Some(Cursor::new(cursor.line, end_pos));
        }
        if cursor.line + 1 < total_lines {
            let next_line_idx = cursor.line + 1;
            let next_line_len = self.line_len(next_line_idx);
            if next_line_len > 0 {
                return Some(Cursor::new(
                    next_line_idx,
                    self.last_non_whitespace_col(next_line_idx).unwrap_or(0),
                ));
            }
            return Some(Cursor::new(next_line_idx, 0));
        }
        None
    }

    pub fn cursor_start_of_line(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }
        if cursor.col != 0 {
            return Some(Cursor::new(cursor.line, 0));
        }
        if cursor.line > 0 {
            return Some(Cursor::new(cursor.line - 1, 0));
        }
        None
    }

    pub fn cursor_content_start_of_line(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }
        let content_start = match self.first_non_whitespace_col(cursor.line) {
            Some(pos) => pos,
            None => {
                if cursor.col > 0 {
                    return Some(Cursor::new(cursor.line, 0));
                }
                0
            }
        };
        if cursor.col != content_start {
            return Some(Cursor::new(cursor.line, content_start));
        }
        if cursor.line > 0 {
            let prev_line_idx = cursor.line - 1;
            if let Some(prev_pos) = self.first_non_whitespace_col(prev_line_idx) {
                return Some(Cursor::new(prev_line_idx, prev_pos));
            }
            return Some(Cursor::new(prev_line_idx, 0));
        }
        None
    }

    pub(super) fn is_blank_line(&self, line_idx: usize) -> bool {
        let line = match self.line_at(line_idx) {
            Some(l) => l,
            None => return false,
        };
        line.char_indices().all(|(_, c)| c.is_whitespace())
    }

    fn file_start_cursor(&self) -> Cursor {
        Cursor::new(0, 0)
    }

    fn file_end_cursor(&self) -> Cursor {
        let last_line = self.line_count().saturating_sub(1);
        Cursor::new(last_line, self.line_len(last_line))
    }

    pub fn cursor_paragraph_backward(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }
        if total_lines == 1 && self.is_blank_line(0) {
            return None;
        }

        let mut line_idx = cursor.line.min(total_lines - 1);

        // If the cursor starts on a blank run, skip to the first non-blank line above it.
        while line_idx > 0 && self.is_blank_line(line_idx) {
            line_idx -= 1;
        }

        // Walk upward through the paragraph until we find the preceding blank line.
        while line_idx > 0 && !self.is_blank_line(line_idx) {
            line_idx -= 1;
        }

        if self.is_blank_line(line_idx) {
            if (0..line_idx).any(|idx| !self.is_blank_line(idx)) {
                Some(Cursor::new(line_idx, 0))
            } else {
                Some(self.file_start_cursor())
            }
        } else {
            Some(self.file_start_cursor())
        }
    }

    pub fn cursor_paragraph_forward(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }
        if total_lines == 1 && self.is_blank_line(0) {
            return None;
        }

        let mut line_idx = cursor.line.min(total_lines - 1);

        // If the cursor starts on a blank run, skip to the first non-blank line below it.
        while line_idx + 1 < total_lines && self.is_blank_line(line_idx) {
            line_idx += 1;
        }

        // Walk downward through the paragraph until we find the following blank line.
        while line_idx < total_lines && !self.is_blank_line(line_idx) {
            line_idx += 1;
        }

        if line_idx < total_lines {
            if ((line_idx + 1)..total_lines).any(|idx| !self.is_blank_line(idx)) {
                Some(Cursor::new(line_idx, 0))
            } else {
                Some(self.file_end_cursor())
            }
        } else {
            Some(self.file_end_cursor())
        }
    }

    pub fn visual_col_at(&self, cursor: Cursor) -> usize {
        let line = match self.line_at(cursor.line) {
            Some(l) => l,
            None => return 0,
        };
        let mut scratch = String::new();
        let line = line.contiguous_text_with_scratch(&mut scratch);
        let tab_width = configured_tab_width();
        let mut visual_col = 0;
        let mut byte_offset = 0;
        for grapheme in line.graphemes(true) {
            if byte_offset >= cursor.col {
                break;
            }
            visual_col += display_grapheme_width(grapheme, visual_col, tab_width);
            byte_offset += grapheme.len();
        }
        visual_col
    }

    pub fn byte_pos_at_visual_col(&self, line_idx: usize, visual_col: usize) -> usize {
        let line = match self.line_at(line_idx) {
            Some(l) => l,
            None => return 0,
        };
        let mut scratch = String::new();
        let line = line.contiguous_text_with_scratch(&mut scratch);
        let tab_width = configured_tab_width();
        let mut current_visual = 0;
        let mut byte_offset = 0;
        for grapheme in line.graphemes(true) {
            let gwidth = display_grapheme_width(grapheme, current_visual, tab_width);
            if current_visual + gwidth > visual_col {
                return byte_offset;
            }
            current_visual += gwidth;
            byte_offset += grapheme.len();
        }
        line.len()
    }

    /// Returns the visual width of the specified line using the configured tab width.
    pub fn visual_line_width(&self, line_idx: usize) -> usize {
        let line = match self.line_at(line_idx) {
            Some(l) => l,
            None => return 0,
        };
        let mut scratch = String::new();
        let line = line.contiguous_text_with_scratch(&mut scratch);

        display_width_at(line, 0, configured_tab_width())
    }

    pub(super) fn grapheme_at_byte(&self, line_idx: usize, byte_pos: usize) -> Option<String> {
        let line = self.line_at(line_idx)?;
        line.grapheme_at(byte_pos)
            .map(|grapheme| grapheme.into_owned())
    }

    pub(super) fn prev_grapheme_before_byte(
        &self,
        line_idx: usize,
        byte_pos: usize,
    ) -> Option<String> {
        let line = self.line_at(line_idx)?;
        line.previous_grapheme(byte_pos)
            .map(|grapheme| grapheme.into_owned())
    }

    pub(super) fn next_grapheme_at_or_after_byte(
        &self,
        line_idx: usize,
        byte_pos: usize,
    ) -> Option<String> {
        let line = self.line_at(line_idx)?;
        line.next_grapheme(byte_pos)
            .map(|grapheme| grapheme.into_owned())
    }
}
