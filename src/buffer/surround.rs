use super::*;
use crate::editor::{BracketKind, DelimiterFamily, QuoteKind};

impl Buffer {
    /// Adds a delimiter pair around `range`.
    ///
    /// Returns the cursor position to keep after a successful mutation,
    /// or `None` when the range is empty or invalid.
    pub fn add_surround(
        &mut self,
        range: TextObjectRange,
        delimiter: DelimiterFamily,
    ) -> Option<Cursor> {
        if range.start.line > range.end.line
            || (range.start.line == range.end.line && range.start.col >= range.end.col)
            || !self.is_valid_cursor(range.start)
            || !self.is_valid_cursor(range.end)
        {
            return None;
        }

        let closing = delimiter.closing_delimiter().to_string();
        let opening = delimiter.opening_delimiter().to_string();
        self.insert_text(range.end, &closing);
        self.insert_text(range.start, &opening);
        Some(range.start)
    }

    /// Adds delimiter lines around a line range.
    ///
    /// The closing delimiter is inserted first so `start_line` still refers to
    /// the original first selected line when the opening delimiter is inserted.
    /// Returns the inserted opening delimiter position on success.
    pub fn add_linewise_surround(
        &mut self,
        start_line: usize,
        count: usize,
        delimiter: DelimiterFamily,
    ) -> Option<Cursor> {
        let line_count = self.line_count();
        if line_count == 0 || start_line >= line_count || count == 0 {
            return None;
        }

        let actual_count = (line_count - start_line).min(count);
        let end_line = start_line + actual_count - 1;
        let close_cursor = Cursor::new(end_line, self.line_len(end_line));
        let closing = format!("\n{}", delimiter.closing_delimiter());
        self.insert_text(close_cursor, &closing);

        let open_cursor = Cursor::new(start_line, 0);
        let opening = format!("{}\n", delimiter.opening_delimiter());
        self.insert_text(open_cursor, &opening);
        Some(open_cursor)
    }

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
