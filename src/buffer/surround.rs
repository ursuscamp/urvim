use super::*;
use crate::editor::{BracketKind, DelimiterFamily, QuoteKind};

impl Buffer {
    /// Replaces the nearest resolved surrounding pair around `cursor`.
    ///
    /// Returns the cursor position to keep after a successful mutation,
    /// or `None` when no valid pair is resolved or the request is a no-op.
    pub fn replace_surround(
        &mut self,
        cursor: Cursor,
        target: DelimiterFamily,
        replacement: DelimiterFamily,
    ) -> Option<Cursor> {
        if target == replacement {
            return None;
        }

        let pair = self.find_surround_pair(cursor, target)?;
        self.replace_delimiter_at(pair.close, replacement.closing_delimiter())?;
        self.replace_delimiter_at(pair.open, replacement.opening_delimiter())?;
        Some(pair.open)
    }

    /// Deletes the nearest resolved surrounding pair around `cursor`.
    ///
    /// Returns the cursor position to keep after a successful mutation,
    /// or `None` when no valid pair is resolved.
    pub fn delete_surround(&mut self, cursor: Cursor, target: DelimiterFamily) -> Option<Cursor> {
        let pair = self.find_surround_pair(cursor, target)?;
        self.delete_delimiter_at(pair.close)?;
        self.delete_delimiter_at(pair.open)?;
        Some(pair.open)
    }

    fn find_surround_pair(&self, cursor: Cursor, target: DelimiterFamily) -> Option<SurroundPair> {
        let range = match target {
            DelimiterFamily::Paren => {
                self.get_around_bracket_range_with_count(cursor, BracketKind::Paren, 1)?
            }
            DelimiterFamily::Square => {
                self.get_around_bracket_range_with_count(cursor, BracketKind::Square, 1)?
            }
            DelimiterFamily::Curly => {
                self.get_around_bracket_range_with_count(cursor, BracketKind::Curly, 1)?
            }
            DelimiterFamily::Angle => {
                self.get_around_bracket_range_with_count(cursor, BracketKind::Angle, 1)?
            }
            DelimiterFamily::DoubleQuote => {
                self.get_around_quote_range_with_count(cursor, QuoteKind::Double, 1)?
            }
            DelimiterFamily::SingleQuote => {
                self.get_around_quote_range_with_count(cursor, QuoteKind::Single, 1)?
            }
            DelimiterFamily::Backtick => {
                self.get_around_quote_range_with_count(cursor, QuoteKind::Backtick, 1)?
            }
        };

        let close = self.prev_cursor(range.end)?;
        Some(SurroundPair {
            open: range.start,
            close,
        })
    }

    fn replace_delimiter_at(&mut self, cursor: Cursor, replacement: char) -> Option<()> {
        self.delete_delimiter_at(cursor)?;
        let replacement_text = replacement.to_string();
        self.insert_text(cursor, &replacement_text);
        Some(())
    }

    fn delete_delimiter_at(&mut self, cursor: Cursor) -> Option<()> {
        let end = self.next_cursor(cursor)?;
        self.remove(cursor, end);
        Some(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SurroundPair {
    open: Cursor,
    close: Cursor,
}
