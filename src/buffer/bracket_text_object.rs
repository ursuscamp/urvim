use super::*;
use crate::editor::BracketKind;
use std::cmp::Reverse;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BracketPair {
    open: Cursor,
    close: Cursor,
}

impl Buffer {
    /// Resolves the range inside a matching bracket pair.
    pub fn get_inner_bracket_range(
        &self,
        cursor: Cursor,
        kind: BracketKind,
    ) -> Option<TextObjectRange> {
        self.get_inner_bracket_range_with_count(cursor, kind, 1)
    }

    /// Resolves the range around a matching bracket pair.
    pub fn get_around_bracket_range(
        &self,
        cursor: Cursor,
        kind: BracketKind,
    ) -> Option<TextObjectRange> {
        self.get_around_bracket_range_with_count(cursor, kind, 1)
    }

    /// Resolves the range inside matching bracket pairs, expanding outward for counts.
    pub fn get_inner_bracket_range_with_count(
        &self,
        cursor: Cursor,
        kind: BracketKind,
        count: usize,
    ) -> Option<TextObjectRange> {
        self.get_bracket_range_with_count(cursor, kind, count, false)
    }

    /// Resolves the range around matching bracket pairs, expanding outward for counts.
    pub fn get_around_bracket_range_with_count(
        &self,
        cursor: Cursor,
        kind: BracketKind,
        count: usize,
    ) -> Option<TextObjectRange> {
        self.get_bracket_range_with_count(cursor, kind, count, true)
    }

    fn get_bracket_range_with_count(
        &self,
        cursor: Cursor,
        kind: BracketKind,
        count: usize,
        around: bool,
    ) -> Option<TextObjectRange> {
        if count == 0 {
            return None;
        }

        let mut pair = self.find_bracket_pair(cursor, kind)?;
        for _ in 1..count {
            pair = self.find_enclosing_bracket_pair(pair, kind)?;
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

    fn find_bracket_pair(&self, cursor: Cursor, kind: BracketKind) -> Option<BracketPair> {
        let mut pairs = self.collect_bracket_pairs(kind);
        if pairs.is_empty() {
            return None;
        }
        pairs.sort_by_key(|pair| {
            (
                pair.open.line,
                pair.open.col,
                Reverse((pair.close.line, pair.close.col)),
            )
        });

        if let Some(pair) = pairs
            .iter()
            .copied()
            .filter(|pair| self.bracket_pair_covers_cursor(*pair, cursor))
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

        pairs.iter().copied().find(|pair| {
            pair.open.line == cursor.line
                && Self::compare_cursor(pair.open, cursor) != std::cmp::Ordering::Less
        })
    }

    fn find_enclosing_bracket_pair(
        &self,
        inner: BracketPair,
        kind: BracketKind,
    ) -> Option<BracketPair> {
        let mut pairs = self.collect_bracket_pairs(kind);
        pairs.sort_by_key(|pair| {
            (
                pair.open.line,
                pair.open.col,
                Reverse((pair.close.line, pair.close.col)),
            )
        });
        pairs
            .into_iter()
            .filter(|pair| {
                Self::compare_cursor(pair.open, inner.open) == std::cmp::Ordering::Less
                    && Self::compare_cursor(pair.close, inner.close) == std::cmp::Ordering::Greater
            })
            .max_by_key(|pair| {
                (
                    pair.open.line,
                    pair.open.col,
                    Reverse((pair.close.line, pair.close.col)),
                )
            })
    }

    fn collect_bracket_pairs(&self, kind: BracketKind) -> Vec<BracketPair> {
        let mut stack: Vec<Cursor> = Vec::new();
        let mut pairs = Vec::new();

        for line_idx in 0..self.line_count() {
            let Some(line) = self.line_at(line_idx) else {
                continue;
            };
            for (col, grapheme) in line.grapheme_indices(true) {
                let Some(ch) = grapheme.chars().next() else {
                    continue;
                };
                let cursor = Cursor::new(line_idx, col);
                if kind.matches_opening(ch) {
                    stack.push(cursor);
                } else if kind.matches_closing(ch) {
                    if let Some(open) = stack.pop() {
                        pairs.push(BracketPair {
                            open,
                            close: cursor,
                        });
                    }
                }
            }
        }

        pairs
    }

    fn bracket_pair_covers_cursor(&self, pair: BracketPair, cursor: Cursor) -> bool {
        Self::compare_cursor(pair.open, cursor) != std::cmp::Ordering::Greater
            && Self::compare_cursor(cursor, pair.close) != std::cmp::Ordering::Greater
    }

    fn compare_cursor(a: Cursor, b: Cursor) -> std::cmp::Ordering {
        (a.line, a.col).cmp(&(b.line, b.col))
    }
}
