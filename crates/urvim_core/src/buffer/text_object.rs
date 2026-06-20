use super::*;
type GraphemePredicate = fn(&str) -> bool;

impl Buffer {
    pub fn get_inner_big_word_range(&self, cursor: Cursor) -> Option<TextObjectRange> {
        self.get_inner_range_for(cursor, Self::is_bigword_char)
    }

    pub fn get_inner_big_word_range_with_count(
        &self,
        cursor: Cursor,
        count: usize,
    ) -> Option<TextObjectRange> {
        self.expand_text_object_count(cursor, count, Self::get_inner_big_word_range)
    }

    pub fn get_around_big_word_range(&self, cursor: Cursor) -> Option<TextObjectRange> {
        self.get_around_range_for(
            cursor,
            Self::is_bigword_char,
            Self::get_inner_big_word_range,
        )
    }

    pub fn get_around_big_word_range_with_count(
        &self,
        cursor: Cursor,
        count: usize,
    ) -> Option<TextObjectRange> {
        self.expand_text_object_count(cursor, count, Self::get_around_big_word_range)
    }

    pub fn get_inner_word_range(&self, cursor: Cursor) -> Option<TextObjectRange> {
        self.get_inner_range_for(cursor, Self::is_word_char)
    }

    pub fn get_inner_word_range_with_count(
        &self,
        cursor: Cursor,
        count: usize,
    ) -> Option<TextObjectRange> {
        self.expand_text_object_count(cursor, count, Self::get_inner_word_range)
    }

    pub fn get_around_word_range(&self, cursor: Cursor) -> Option<TextObjectRange> {
        self.get_around_range_for(cursor, Self::is_word_char, Self::get_inner_word_range)
    }

    pub fn get_around_word_range_with_count(
        &self,
        cursor: Cursor,
        count: usize,
    ) -> Option<TextObjectRange> {
        self.expand_text_object_count(cursor, count, Self::get_around_word_range)
    }

    fn get_inner_range_for(
        &self,
        cursor: Cursor,
        token_predicate: GraphemePredicate,
    ) -> Option<TextObjectRange> {
        let line = self.line_at(cursor.line)?;
        let cursor_grapheme = self.grapheme_at_byte(cursor.line, cursor.col);
        let region_predicate = if cursor_grapheme.as_deref().is_none_or(token_predicate) {
            token_predicate
        } else {
            Self::is_whitespace_char
        };
        let start = self.expand_left_matching(line, cursor.col, region_predicate);
        let end = self.expand_right_matching(line, cursor.col, region_predicate);
        Some(TextObjectRange {
            start: Cursor::new(cursor.line, start),
            end: Cursor::new(cursor.line, end),
        })
    }

    fn get_around_range_for(
        &self,
        cursor: Cursor,
        token_predicate: GraphemePredicate,
        inner_resolver: fn(&Self, Cursor) -> Option<TextObjectRange>,
    ) -> Option<TextObjectRange> {
        let line = self.line_at(cursor.line)?;
        let cursor_grapheme = self.grapheme_at_byte(cursor.line, cursor.col);

        if cursor_grapheme.as_deref().is_none_or(token_predicate) {
            // Around-text-objects on a token are just the inner selection plus
            // any immediately trailing whitespace.
            let inner = inner_resolver(self, cursor)?;
            let end = self.expand_right_matching(line, inner.end.col, Self::is_whitespace_char);
            return Some(TextObjectRange {
                start: inner.start,
                end: Cursor::new(cursor.line, end),
            });
        }

        // On whitespace, include the whole separator block and then the next
        // logical object after it. For `aw`, punctuation is a single object;
        // for `aW`, punctuation is part of the following BigWord.
        let whitespace_start =
            self.expand_left_matching(line, cursor.col, Self::is_whitespace_char);
        let whitespace_end = self.expand_right_matching(line, cursor.col, Self::is_whitespace_char);
        let end = self.end_after_whitespace_and_object(line, whitespace_end, token_predicate);
        Some(TextObjectRange {
            start: Cursor::new(cursor.line, whitespace_start),
            end: Cursor::new(cursor.line, end.max(whitespace_end)),
        })
    }

    fn expand_text_object_count(
        &self,
        cursor: Cursor,
        count: usize,
        resolver: fn(&Self, Cursor) -> Option<TextObjectRange>,
    ) -> Option<TextObjectRange> {
        let mut range = resolver(self, cursor)?;
        // Counts are expanded by resolving the next object from the first
        // non-whitespace cursor after the current range. That keeps count logic
        // separate from the object-specific scanning rules.
        for _ in 1..count {
            let next_cursor = self.next_non_whitespace_cursor(range.end)?;
            let next_range = resolver(self, next_cursor)?;
            range.end = next_range.end;
        }
        Some(range)
    }

    fn expand_left_matching(
        &self,
        line: impl TextRef,
        start_col: usize,
        predicate: GraphemePredicate,
    ) -> usize {
        let mut start = start_col;
        let mut col = start_col;
        while col > 0 {
            let prev_col = self.prev_grapheme_start(&line, col);
            if prev_col == col {
                break;
            }
            col = prev_col;
            if matches!(line.grapheme_at(col), Some(g) if predicate(g.as_str())) {
                start = col;
            } else {
                break;
            }
        }
        start
    }

    fn expand_right_matching(
        &self,
        line: impl TextRef,
        start_col: usize,
        predicate: GraphemePredicate,
    ) -> usize {
        let mut end = start_col;
        let mut col = start_col;
        while col < line.len() {
            if let Some(g) = line.grapheme_at(col) {
                if predicate(g.as_str()) {
                    end = col + g.len();
                    col = end;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        end
    }

    fn end_after_whitespace_and_object(
        &self,
        line: impl TextRef,
        start_col: usize,
        token_predicate: GraphemePredicate,
    ) -> usize {
        match line.grapheme_at(start_col) {
            Some(g) if token_predicate(g.as_str()) => {
                self.expand_right_matching(line, start_col, token_predicate)
            }
            Some(g) => start_col + g.len(),
            None => start_col,
        }
    }

    pub(super) fn prev_grapheme_start(&self, line: impl TextRef, byte_offset: usize) -> usize {
        if byte_offset == 0 {
            return 0;
        }
        line.previous_grapheme(byte_offset)
            .map(|grapheme| grapheme.byte_idx())
            .unwrap_or(0)
    }

    pub(super) fn next_non_whitespace_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        let line = self.line_at(cursor.line)?;
        for grapheme in line.graphemes() {
            if grapheme.byte_idx() < cursor.col {
                continue;
            }
            if !Self::is_whitespace_char(grapheme.as_str()) {
                return Some(Cursor::new(cursor.line, grapheme.byte_idx()));
            }
        }
        None
    }
}
