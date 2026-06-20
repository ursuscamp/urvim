use super::*;
use crate::editor::QuoteKind;
use std::cmp::Reverse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QuotePair {
    open: Cursor,
    close: Cursor,
}

impl Buffer {
    /// Resolves the range inside a matching quote pair.
    pub fn get_inner_quote_range(
        &self,
        cursor: Cursor,
        kind: QuoteKind,
    ) -> Option<TextObjectRange> {
        self.get_inner_quote_range_with_count(cursor, kind, 1)
    }

    /// Resolves the range around a matching quote pair.
    pub fn get_around_quote_range(
        &self,
        cursor: Cursor,
        kind: QuoteKind,
    ) -> Option<TextObjectRange> {
        self.get_around_quote_range_with_count(cursor, kind, 1)
    }

    /// Resolves the range inside matching quote pairs, expanding outward for counts.
    pub fn get_inner_quote_range_with_count(
        &self,
        cursor: Cursor,
        kind: QuoteKind,
        count: usize,
    ) -> Option<TextObjectRange> {
        self.get_quote_range_with_count(cursor, kind, count, false)
    }

    /// Resolves the range around matching quote pairs, expanding outward for counts.
    pub fn get_around_quote_range_with_count(
        &self,
        cursor: Cursor,
        kind: QuoteKind,
        count: usize,
    ) -> Option<TextObjectRange> {
        self.get_quote_range_with_count(cursor, kind, count, true)
    }

    fn get_quote_range_with_count(
        &self,
        cursor: Cursor,
        kind: QuoteKind,
        count: usize,
        around: bool,
    ) -> Option<TextObjectRange> {
        if count == 0 {
            return None;
        }

        let mut pair = self.find_quote_pair(cursor, kind)?;
        for _ in 1..count {
            pair = self.find_enclosing_quote_pair(pair, kind)?;
        }

        let start = if around {
            pair.open
        } else {
            self.next_cursor(pair.open)?
        };
        let end = if around {
            self.next_cursor(pair.close)?
        } else {
            pair.close
        };

        Some(TextObjectRange { start, end })
    }

    fn find_quote_pair(&self, cursor: Cursor, kind: QuoteKind) -> Option<QuotePair> {
        let pairs = self.collect_quote_pairs(kind);
        if pairs.is_empty() {
            return None;
        }

        if let Some(pair) = pairs
            .iter()
            .copied()
            .filter(|pair| self.quote_pair_covers_cursor(*pair, cursor))
            .max_by_key(|pair| {
                (
                    pair.open.line,
                    pair.open.col,
                    Reverse((pair.close.line, pair.close.col)),
                )
            })
        {
            return Some(pair);
        }

        pairs
            .iter()
            .copied()
            .filter(|pair| {
                pair.open.line == cursor.line
                    && Self::compare_cursor_positions(pair.open, cursor) != std::cmp::Ordering::Less
            })
            .min_by_key(|pair| (pair.open.line, pair.open.col))
    }

    fn find_enclosing_quote_pair(&self, inner: QuotePair, kind: QuoteKind) -> Option<QuotePair> {
        let pairs = self.collect_quote_pairs(kind);
        pairs
            .into_iter()
            .filter(|pair| {
                Self::compare_cursor_positions(pair.open, inner.open) == std::cmp::Ordering::Less
                    && Self::compare_cursor_positions(pair.close, inner.close)
                        == std::cmp::Ordering::Greater
            })
            .max_by_key(|pair| {
                (
                    pair.open.line,
                    pair.open.col,
                    Reverse((pair.close.line, pair.close.col)),
                )
            })
    }

    fn collect_quote_pairs(&self, kind: QuoteKind) -> Vec<QuotePair> {
        let mut open: Option<Cursor> = None;
        let mut pairs = Vec::new();
        let delimiter = kind.delimiter();

        for line_idx in 0..self.line_count() {
            let Some(line) = self.line_at(line_idx) else {
                continue;
            };
            for (byte_idx, ch) in line.char_indices() {
                if ch != delimiter || Self::is_escaped_quote(&line, byte_idx) {
                    continue;
                }

                let cursor = Cursor::new(line_idx, byte_idx);
                if let Some(open_cursor) = open.take() {
                    pairs.push(QuotePair {
                        open: open_cursor,
                        close: cursor,
                    });
                } else {
                    open = Some(cursor);
                }
            }
        }

        pairs
    }

    fn quote_pair_covers_cursor(&self, pair: QuotePair, cursor: Cursor) -> bool {
        Self::compare_cursor_positions(pair.open, cursor) != std::cmp::Ordering::Greater
            && Self::compare_cursor_positions(cursor, pair.close) != std::cmp::Ordering::Greater
    }

    fn is_escaped_quote(line: &impl TextRef, byte_idx: usize) -> bool {
        let mut slashes = 0usize;
        let mut idx = byte_idx;

        while let Some((prev_idx, ch)) = line.previous_char(idx) {
            if ch != '\\' {
                break;
            }
            slashes += 1;
            idx = prev_idx;
        }

        slashes % 2 == 1
    }

    fn compare_cursor_positions(a: Cursor, b: Cursor) -> std::cmp::Ordering {
        (a.line, a.col).cmp(&(b.line, b.col))
    }
}
