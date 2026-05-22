use super::*;

impl Buffer {
    pub fn find_char_forward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let start_col = cursor.col + 1;
        let mut occurrences: Vec<usize> = Vec::new();
        for grapheme in line.graphemes() {
            if grapheme.byte_idx() >= start_col && grapheme.as_str().starts_with(target) {
                occurrences.push(grapheme.byte_idx());
            }
        }
        let target_idx = occurrences.get(count.saturating_sub(1))?;
        Some(Cursor::new(line_idx, *target_idx))
    }

    pub fn find_char_backward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let occurrences: Vec<usize> = line
            .graphemes()
            .filter(|grapheme| {
                grapheme.byte_idx() < cursor.col && grapheme.as_str().starts_with(target)
            })
            .map(|grapheme| grapheme.byte_idx())
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
        if cursor.col == 0 {
            return Cursor::new(cursor.line, 0);
        }

        let mut col = cursor.col;
        for grapheme in line.graphemes() {
            if grapheme.byte_idx() >= cursor.col {
                col = grapheme.byte_idx() + grapheme.len();
                break;
            }
        }
        Cursor::new(cursor.line, col)
    }
}
