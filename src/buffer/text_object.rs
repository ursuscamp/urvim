use super::*;

impl Buffer {
    pub fn get_inner_word_range(&self, cursor: Cursor) -> Option<TextObjectRange> {
        let line = self.line_at(cursor.line)?;
        let line_str = line.as_ref();
        let line_len = line_str.len();
        let cursor_grapheme = self.grapheme_at_byte(cursor.line, cursor.col);

        if cursor_grapheme.is_none_or(Self::is_word_char) {
            let mut word_start = cursor.col;
            let mut word_end = cursor.col;
            let mut col = cursor.col;
            while col > 0 {
                let prev_col = self.prev_grapheme_start(line_str, col);
                if prev_col == col {
                    break;
                }
                col = prev_col;
                if let Some(g) = line_str.get(col..).and_then(|s| s.graphemes(true).next()) {
                    if Self::is_word_char(g) {
                        word_start = col;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            col = cursor.col;
            while col < line_len {
                let next_grapheme = line_str.get(col..).and_then(|s| s.graphemes(true).next());
                if let Some(g) = next_grapheme {
                    if Self::is_word_char(g) {
                        word_end = col + g.len();
                        col = word_end;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            Some(TextObjectRange {
                start: Cursor::new(cursor.line, word_start),
                end: Cursor::new(cursor.line, word_end),
            })
        } else {
            let mut ws_start = cursor.col;
            let mut ws_end = cursor.col;
            let mut col = cursor.col;
            while col > 0 {
                let prev_col = self.prev_grapheme_start(line_str, col);
                if prev_col == col {
                    break;
                }
                col = prev_col;
                if let Some(g) = line_str.get(col..).and_then(|s| s.graphemes(true).next()) {
                    if Self::is_whitespace_char(g) {
                        ws_start = col;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            col = cursor.col;
            while col < line_len {
                let next_grapheme = line_str.get(col..).and_then(|s| s.graphemes(true).next());
                if let Some(g) = next_grapheme {
                    if Self::is_whitespace_char(g) {
                        ws_end = col + g.len();
                        col = ws_end;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            Some(TextObjectRange {
                start: Cursor::new(cursor.line, ws_start),
                end: Cursor::new(cursor.line, ws_end),
            })
        }
    }

    pub fn get_inner_word_range_with_count(
        &self,
        cursor: Cursor,
        count: usize,
    ) -> Option<TextObjectRange> {
        let mut range = self.get_inner_word_range(cursor)?;
        for _ in 1..count {
            let next_cursor = self.next_non_whitespace_cursor(range.end)?;
            let next_range = self.get_inner_word_range(next_cursor)?;
            range.end = next_range.end;
        }
        Some(range)
    }

    pub fn get_around_word_range(&self, cursor: Cursor) -> Option<TextObjectRange> {
        let line = self.line_at(cursor.line)?;
        let line_str = line.as_ref();
        let line_len = line_str.len();
        let cursor_grapheme = self.grapheme_at_byte(cursor.line, cursor.col);
        if cursor_grapheme.is_none_or(Self::is_word_char) {
            let inner = self.get_inner_word_range(cursor)?;
            let mut end_col = inner.end.col;
            let mut col = inner.end.col;
            while col < line_len {
                let next_grapheme = line_str.get(col..).and_then(|s| s.graphemes(true).next());
                if let Some(g) = next_grapheme {
                    if Self::is_whitespace_char(g) {
                        end_col = col + g.len();
                        col = end_col;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            Some(TextObjectRange {
                start: inner.start,
                end: Cursor::new(cursor.line, end_col),
            })
        } else {
            let mut ws_start = cursor.col;
            let mut ws_end = cursor.col;
            let mut word_end_col = cursor.col;
            let mut col = cursor.col;
            while col > 0 {
                let prev_col = self.prev_grapheme_start(line_str, col);
                if prev_col == col {
                    break;
                }
                col = prev_col;
                if let Some(g) = line_str.get(col..).and_then(|s| s.graphemes(true).next()) {
                    if Self::is_whitespace_char(g) {
                        ws_start = col;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            col = cursor.col;
            while col < line_len {
                let next_grapheme = line_str.get(col..).and_then(|s| s.graphemes(true).next());
                if let Some(g) = next_grapheme {
                    if Self::is_whitespace_char(g) {
                        ws_end = col + g.len();
                        col = ws_end;
                    } else if Self::is_word_char(g) {
                        word_end_col = col;
                        while word_end_col < line_len {
                            let word_grapheme = line_str
                                .get(word_end_col..)
                                .and_then(|s| s.graphemes(true).next());
                            if let Some(wg) = word_grapheme {
                                if Self::is_word_char(wg) {
                                    word_end_col += wg.len();
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        break;
                    } else {
                        word_end_col = col + g.len();
                        break;
                    }
                } else {
                    break;
                }
            }
            let end_col = if word_end_col > ws_end {
                word_end_col
            } else {
                ws_end
            };
            Some(TextObjectRange {
                start: Cursor::new(cursor.line, ws_start),
                end: Cursor::new(cursor.line, end_col),
            })
        }
    }

    pub fn get_around_word_range_with_count(
        &self,
        cursor: Cursor,
        count: usize,
    ) -> Option<TextObjectRange> {
        let mut range = self.get_around_word_range(cursor)?;
        for _ in 1..count {
            let next_cursor = self.next_non_whitespace_cursor(range.end)?;
            let next_range = self.get_around_word_range(next_cursor)?;
            range.end = next_range.end;
        }
        Some(range)
    }

    pub(super) fn prev_grapheme_start(&self, line: &str, byte_offset: usize) -> usize {
        if byte_offset == 0 {
            return 0;
        }
        let mut prev_offset = 0;
        for (byte_pos, _grapheme) in line.grapheme_indices(true) {
            if byte_pos >= byte_offset {
                break;
            }
            prev_offset = byte_pos;
        }
        prev_offset
    }

    pub(super) fn next_non_whitespace_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        let line = self.line_at(cursor.line)?;
        let line_str = line.as_ref();
        let mut col = cursor.col;
        while col < line_str.len() {
            let grapheme = line_str.get(col..).and_then(|s| s.graphemes(true).next())?;
            if !Self::is_whitespace_char(grapheme) {
                return Some(Cursor::new(cursor.line, col));
            }
            col += grapheme.len();
        }
        None
    }
}
