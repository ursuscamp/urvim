use super::*;

impl Buffer {
    pub fn find_char_forward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let line_str = line.as_ref();
        let start_col = cursor.col + 1;
        let mut occurrences: Vec<usize> = Vec::new();
        for (grapheme_idx, grapheme) in line_str.grapheme_indices(true) {
            if grapheme_idx >= start_col && grapheme.starts_with(target) {
                occurrences.push(grapheme_idx);
            }
        }
        let target_idx = occurrences.get(count.saturating_sub(1))?;
        Some(Cursor::new(line_idx, *target_idx))
    }

    pub fn find_char_backward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let line_str = line.as_ref();
        let occurrences: Vec<usize> = line_str
            .grapheme_indices(true)
            .filter(|&(idx, grapheme)| idx < cursor.col && grapheme.starts_with(target))
            .map(|(idx, _)| idx)
            .collect();
        let target_idx = occurrences.len().saturating_sub(count);
        let idx = *occurrences.get(target_idx)?;
        Some(Cursor::new(line_idx, idx))
    }

    /// Moves to the character just before the next forward match.
    pub fn find_till_forward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let search_cursor = self.till_forward_search_cursor(cursor);
        let found = self.find_char_forward(search_cursor, target, count)?;
        Some(self.prev_cursor_line(found).unwrap_or(found))
    }

    /// Moves to the character just after the next backward match.
    pub fn find_till_backward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let search_cursor = Cursor::new(cursor.line, cursor.col.saturating_sub(1));
        let found = self.find_char_backward(search_cursor, target, count)?;
        Some(
            self.next_cursor_line(found)
                .unwrap_or_else(|| Cursor::new(found.line, self.line_len(found.line))),
        )
    }

    fn till_forward_search_cursor(&self, cursor: Cursor) -> Cursor {
        let Some(line) = self.line_at(cursor.line) else {
            return cursor;
        };
        let line_str = line.as_ref();
        if cursor.col == 0 {
            return Cursor::new(cursor.line, 0);
        }

        let mut col = cursor.col;
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset >= cursor.col {
                col = byte_offset + grapheme.len();
                break;
            }
        }
        Cursor::new(cursor.line, col)
    }
}
