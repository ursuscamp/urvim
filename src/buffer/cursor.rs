use super::*;

impl Buffer {
    /// Returns the character at the cursor, if the cursor is positioned on a character.
    pub fn char_at_cursor(&self, cursor: Cursor) -> Option<char> {
        let line = self.line_at(cursor.line)?;
        line.get(cursor.col..)?.chars().next()
    }

    /// Returns the character immediately before the cursor, if one exists.
    pub fn char_before_cursor(&self, cursor: Cursor) -> Option<char> {
        if cursor.col == 0 {
            return None;
        }
        let line = self.line_at(cursor.line)?;
        line.get(..cursor.col)?.chars().next_back()
    }

    pub fn line_len(&self, line_idx: usize) -> usize {
        self.lines.get(line_idx).map_or(0, |s| s.len())
    }

    pub fn is_valid_cursor(&self, cursor: Cursor) -> bool {
        if cursor.line >= self.lines.len() {
            return false;
        }
        let line_len = self.line_len(cursor.line);
        if cursor.col > line_len {
            return false;
        }
        if cursor.col == line_len {
            return true;
        }
        if let Some(line) = self.lines.get(cursor.line) {
            line.is_char_boundary(cursor.col)
        } else {
            false
        }
    }

    pub fn next_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        let line_len = self.line_len(cursor.line);
        if cursor.col < line_len {
            let line = self.lines.get(cursor.line)?;
            let line_str = line.as_ref();
            for (relative_offset, _grapheme) in line_str[cursor.col..].grapheme_indices(true) {
                if relative_offset == 0 {
                    continue;
                }
                return Some(Cursor::new(cursor.line, cursor.col + relative_offset));
            }
            Some(Cursor::new(cursor.line, line_len))
        } else if cursor.line < self.lines.len() - 1 {
            Some(Cursor::new(cursor.line + 1, 0))
        } else {
            None
        }
    }

    pub fn prev_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.col > 0 {
            let line = self.lines.get(cursor.line)?;
            let line_str = line.as_ref();
            let prefix = &line_str[..cursor.col];
            let last_grapheme_offset = prefix
                .grapheme_indices(true)
                .next_back()
                .map(|(offset, _)| offset)?;
            Some(Cursor::new(cursor.line, last_grapheme_offset))
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
        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();
        for (relative_offset, _grapheme) in line_str[cursor.col..].grapheme_indices(true) {
            if relative_offset == 0 {
                continue;
            }
            return Some(Cursor::new(cursor.line, cursor.col + relative_offset));
        }
        Some(Cursor::new(cursor.line, line_len))
    }

    pub fn prev_cursor_line(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.col == 0 {
            return None;
        }
        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();
        let prefix = &line_str[..cursor.col];
        let last_grapheme_offset = prefix
            .grapheme_indices(true)
            .next_back()
            .map(|(offset, _)| offset)?;
        Some(Cursor::new(cursor.line, last_grapheme_offset))
    }

    pub fn cursor_down(&self, cursor: Cursor, visual_col: usize) -> Option<Cursor> {
        if cursor.line >= self.lines.len() - 1 {
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
        line.grapheme_indices(true)
            .find(|(_, grapheme)| !Self::is_whitespace_char(grapheme))
            .map(|(idx, _)| idx)
    }

    pub(super) fn last_non_whitespace_col(&self, line_idx: usize) -> Option<usize> {
        let line = self.line_at(line_idx)?;
        line.grapheme_indices(true)
            .filter(|(_, grapheme)| !Self::is_whitespace_char(grapheme))
            .map(|(idx, _)| idx)
            .next_back()
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
        line.chars().all(|c| c.is_whitespace())
    }

    pub fn cursor_paragraph_backward(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }
        let mut line_idx = cursor.line;
        if !self.is_blank_line(line_idx) {
            while line_idx > 0 && !self.is_blank_line(line_idx) {
                line_idx -= 1;
            }
            if self.is_blank_line(line_idx) {
                return Some(Cursor::new(line_idx, 0));
            }
            return None;
        }
        while line_idx > 0 && self.is_blank_line(line_idx) {
            line_idx -= 1;
        }
        while line_idx > 0 && !self.is_blank_line(line_idx) {
            line_idx -= 1;
        }
        if line_idx == 0 && !self.is_blank_line(0) {
            return None;
        }
        if line_idx == 0 {
            return None;
        }
        if self.is_blank_line(line_idx) {
            Some(Cursor::new(line_idx, 0))
        } else {
            None
        }
    }

    pub fn cursor_paragraph_forward(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }
        let mut line_idx = cursor.line;
        if !self.is_blank_line(line_idx) {
            while line_idx < total_lines && !self.is_blank_line(line_idx) {
                line_idx += 1;
            }
            if line_idx < total_lines && self.is_blank_line(line_idx) {
                return Some(Cursor::new(line_idx, 0));
            }
            return None;
        }
        while line_idx < total_lines && self.is_blank_line(line_idx) {
            line_idx += 1;
        }
        while line_idx < total_lines && !self.is_blank_line(line_idx) {
            line_idx += 1;
        }
        if line_idx < total_lines && self.is_blank_line(line_idx) {
            Some(Cursor::new(line_idx, 0))
        } else {
            None
        }
    }

    pub fn visual_col_at(&self, cursor: Cursor) -> usize {
        let line = match self.lines.get(cursor.line) {
            Some(l) => l.as_ref(),
            None => return 0,
        };
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
        let line = match self.lines.get(line_idx) {
            Some(l) => l.as_ref(),
            None => return 0,
        };
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
        let line = match self.lines.get(line_idx) {
            Some(l) => l.as_ref(),
            None => return 0,
        };

        display_width_at(line, 0, configured_tab_width())
    }

    pub(super) fn grapheme_at_byte(&self, line_idx: usize, byte_pos: usize) -> Option<&str> {
        let line = self.lines.get(line_idx)?;
        let line_str = line.as_ref();
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset == byte_pos {
                return Some(grapheme);
            }
        }
        None
    }

    pub(super) fn prev_grapheme_before_byte(
        &self,
        line_idx: usize,
        byte_pos: usize,
    ) -> Option<&str> {
        let line = self.lines.get(line_idx)?;
        let line_str = line.as_ref();
        let mut prev = None;
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset >= byte_pos {
                break;
            }
            prev = Some(grapheme);
        }
        prev
    }

    pub(super) fn next_grapheme_at_or_after_byte(
        &self,
        line_idx: usize,
        byte_pos: usize,
    ) -> Option<&str> {
        let line = self.lines.get(line_idx)?;
        let line_str = line.as_ref();
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset >= byte_pos {
                return Some(grapheme);
            }
        }
        None
    }
}
