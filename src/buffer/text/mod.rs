//! Text storage abstractions for buffers.

use super::{Cursor, TextObjectRange};
use imbl::Vector;
use std::fmt;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// Encoding used when converting between byte cursors and protocol positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    /// UTF-8 byte offsets.
    Utf8,
    /// UTF-16 code-unit offsets.
    Utf16,
}

/// Protocol-neutral text position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    /// Zero-based line index.
    pub line: usize,
    /// Encoding-dependent character offset within the line.
    pub character: usize,
}

/// Protocol-neutral encoded text range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    /// Start position, inclusive.
    pub start: TextPosition,
    /// End position, exclusive.
    pub end: TextPosition,
}

/// Structural result of a text mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextChange {
    /// First line whose contents may have changed.
    pub first_changed_line: usize,
    /// Net change in line count.
    pub line_delta: isize,
}

impl TextChange {
    fn new(first_changed_line: usize, old_line_count: usize, new_line_count: usize) -> Self {
        Self {
            first_changed_line,
            line_delta: new_line_count as isize - old_line_count as isize,
        }
    }
}

/// Non-owning view over text that may be backed by multiple chunks.
pub trait TextRef {
    /// Returns the byte length of the referenced text.
    fn len(&self) -> usize;

    /// Returns true when the referenced text is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterates contiguous chunks that make up this text reference.
    fn chunks(&self) -> impl Iterator<Item = &str> + '_;

    /// Returns this text as a borrowed contiguous slice when the backing storage supports it.
    fn contiguous_text(&self) -> Option<&str> {
        None
    }

    /// Writes this text into caller-provided scratch storage.
    fn write_to_string(&self, scratch: &mut String) {
        scratch.clear();
        scratch.reserve(self.len());
        for chunk in self.chunks() {
            scratch.push_str(chunk);
        }
    }

    /// Returns contiguous text, reusing scratch storage when materialization is required.
    fn contiguous_text_with_scratch<'a>(&'a self, scratch: &'a mut String) -> &'a str {
        if let Some(text) = self.contiguous_text() {
            return text;
        }
        self.write_to_string(scratch);
        scratch.as_str()
    }

    /// Returns true when the byte index is on a UTF-8 character boundary.
    fn is_char_boundary(&self, byte_idx: usize) -> bool {
        chunk_byte_boundary(self, byte_idx)
    }

    /// Returns the character starting at the byte index.
    fn char_at(&self, byte_idx: usize) -> Option<char> {
        if byte_idx >= self.len() || !self.is_char_boundary(byte_idx) {
            return None;
        }

        let mut bytes = 0usize;
        for chunk in self.chunks() {
            let chunk_end = bytes + chunk.len();
            if byte_idx < chunk_end {
                return chunk[byte_idx - bytes..].chars().next();
            }
            bytes = chunk_end;
        }
        None
    }

    /// Returns the character before the byte index and its starting byte offset.
    fn previous_char(&self, byte_idx: usize) -> Option<(usize, char)> {
        if byte_idx == 0 || byte_idx > self.len() || !self.is_char_boundary(byte_idx) {
            return None;
        }

        let mut previous = None;
        for (idx, ch) in self.char_indices() {
            if idx >= byte_idx {
                break;
            }
            previous = Some((idx, ch));
        }
        previous
    }

    /// Returns the character at or after the byte index and its starting byte offset.
    fn next_char(&self, byte_idx: usize) -> Option<(usize, char)> {
        if byte_idx > self.len() || !self.is_char_boundary(byte_idx) {
            return None;
        }

        self.char_indices().find(|(idx, _)| *idx >= byte_idx)
    }

    /// Writes the byte range into caller-provided scratch storage.
    fn write_range_to_string(&self, start: usize, end: usize, scratch: &mut String) -> Option<()> {
        if start > end
            || end > self.len()
            || !self.is_char_boundary(start)
            || !self.is_char_boundary(end)
        {
            return None;
        }

        scratch.clear();
        scratch.reserve(end - start);
        let mut bytes = 0usize;
        for chunk in self.chunks() {
            let chunk_end = bytes + chunk.len();
            if chunk_end <= start {
                bytes = chunk_end;
                continue;
            }
            if bytes >= end {
                break;
            }

            let local_start = start.saturating_sub(bytes);
            let local_end = (end.min(chunk_end)) - bytes;
            scratch.push_str(chunk.get(local_start..local_end)?);
            bytes = chunk_end;
        }
        Some(())
    }

    /// Materializes the byte range as owned text.
    fn range_text(&self, start: usize, end: usize) -> Option<String> {
        let mut text = String::new();
        self.write_range_to_string(start, end, &mut text)?;
        Some(text)
    }

    /// Returns true when the byte range starts with the provided prefix.
    fn range_starts_with(&self, start: usize, end: usize, prefix: &str) -> Option<bool> {
        if start > end
            || end > self.len()
            || !self.is_char_boundary(start)
            || !self.is_char_boundary(end)
        {
            return None;
        }
        if prefix.len() > end - start {
            return Some(false);
        }

        let mut prefix_offset = 0usize;
        let mut bytes = 0usize;
        for chunk in self.chunks() {
            let chunk_end = bytes + chunk.len();
            if chunk_end <= start {
                bytes = chunk_end;
                continue;
            }
            if bytes >= end || prefix_offset == prefix.len() {
                break;
            }

            let local_start = start.saturating_sub(bytes);
            let local_end = (end.min(chunk_end)) - bytes;
            let range_chunk = chunk.get(local_start..local_end)?;
            let remaining_prefix = &prefix[prefix_offset..];
            let compare_len = range_chunk.len().min(remaining_prefix.len());
            if range_chunk.as_bytes()[..compare_len] != remaining_prefix.as_bytes()[..compare_len] {
                return Some(false);
            }
            prefix_offset += compare_len;
            bytes = chunk_end;
        }

        Some(prefix_offset == prefix.len())
    }

    /// Iterates character indices relative to this text reference.
    fn char_indices(&self) -> impl Iterator<Item = (usize, char)> + '_ {
        let mut base = 0usize;
        self.chunks().flat_map(move |chunk| {
            let chunk_base = base;
            base += chunk.len();
            chunk
                .char_indices()
                .map(move |(local_idx, ch)| (chunk_base + local_idx, ch))
        })
    }

    /// Iterates Unicode grapheme clusters with byte indices relative to this text reference.
    fn graphemes(&self) -> TextGraphemes<'_> {
        TextGraphemes::new(self)
    }

    /// Returns the grapheme starting at the byte index.
    fn grapheme_at(&self, byte_idx: usize) -> Option<TextGrapheme<'_>> {
        self.graphemes()
            .find(|grapheme| grapheme.byte_idx() == byte_idx)
    }

    /// Returns the grapheme before the byte index.
    fn previous_grapheme(&self, byte_idx: usize) -> Option<TextGrapheme<'_>> {
        if byte_idx == 0 || byte_idx > self.len() || !self.is_char_boundary(byte_idx) {
            return None;
        }
        let mut previous = None;
        for grapheme in self.graphemes() {
            if grapheme.byte_idx() >= byte_idx {
                break;
            }
            previous = Some(grapheme);
        }
        previous
    }

    /// Returns the grapheme at or after the byte index.
    fn next_grapheme(&self, byte_idx: usize) -> Option<TextGrapheme<'_>> {
        if byte_idx > self.len() || !self.is_char_boundary(byte_idx) {
            return None;
        }
        self.graphemes()
            .find(|grapheme| grapheme.byte_idx() >= byte_idx)
    }

    /// Materializes this reference as owned contiguous text.
    fn to_text(&self) -> String {
        let mut text = String::with_capacity(self.len());
        for chunk in self.chunks() {
            text.push_str(chunk);
        }
        text
    }
}

/// Grapheme cluster with its byte position in a `TextRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextGrapheme<'a> {
    /// Grapheme borrowed directly from a contiguous source chunk.
    Borrowed {
        /// Byte index where the grapheme starts.
        byte_idx: usize,
        /// Grapheme cluster text.
        text: &'a str,
    },
    /// Grapheme materialized because it spans chunk boundaries.
    Owned {
        /// Byte index where the grapheme starts.
        byte_idx: usize,
        /// Grapheme cluster text.
        text: String,
    },
}

impl TextGrapheme<'_> {
    /// Returns the byte index where the grapheme starts.
    pub fn byte_idx(&self) -> usize {
        match self {
            Self::Borrowed { byte_idx, .. } | Self::Owned { byte_idx, .. } => *byte_idx,
        }
    }

    /// Returns the grapheme byte length.
    pub fn len(&self) -> usize {
        self.as_str().len()
    }

    /// Returns true when this grapheme is empty.
    pub fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }

    /// Returns the grapheme as a string slice.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Borrowed { text, .. } => text,
            Self::Owned { text, .. } => text.as_str(),
        }
    }

    /// Converts this grapheme into owned text.
    pub fn into_owned(self) -> String {
        match self {
            Self::Borrowed { text, .. } => text.to_string(),
            Self::Owned { text, .. } => text,
        }
    }
}

impl<T: TextRef + ?Sized> TextRef for &T {
    fn len(&self) -> usize {
        (**self).len()
    }

    fn chunks(&self) -> impl Iterator<Item = &str> + '_ {
        (**self).chunks()
    }

    fn contiguous_text(&self) -> Option<&str> {
        (**self).contiguous_text()
    }
}

/// Streaming grapheme iterator over a `TextRef`.
pub struct TextGraphemes<'a> {
    chunks: std::vec::IntoIter<(&'a str, usize)>,
    pending: std::vec::IntoIter<TextGrapheme<'a>>,
    carry: String,
    carry_start: usize,
    finished: bool,
}

impl<'a> TextGraphemes<'a> {
    fn new<T: TextRef + ?Sized>(text: &'a T) -> Self {
        let mut byte_base = 0usize;
        let chunks = text
            .chunks()
            .map(|chunk| {
                let chunk_base = byte_base;
                byte_base += chunk.len();
                (chunk, chunk_base)
            })
            .collect::<Vec<_>>()
            .into_iter();

        Self {
            chunks,
            pending: Vec::new().into_iter(),
            carry: String::new(),
            carry_start: 0,
            finished: false,
        }
    }

    fn refill_pending(&mut self) {
        while self.pending.as_slice().is_empty() && !self.finished {
            if let Some((chunk, chunk_start)) = self.chunks.next() {
                self.push_chunk(chunk, chunk_start);
            } else {
                self.finish_carry();
            }
        }
    }

    fn push_chunk(&mut self, chunk: &'a str, chunk_start: usize) {
        if chunk.is_empty() {
            return;
        }

        let mut emitted = Vec::new();
        let mut skip_until = 0usize;

        if !self.carry.is_empty() {
            let mut joined = String::with_capacity(self.carry.len() + chunk.len());
            joined.push_str(&self.carry);
            joined.push_str(chunk);

            let first_len = joined
                .graphemes(true)
                .next()
                .map(str::len)
                .unwrap_or(joined.len());
            let carry_len = self.carry.len();
            let text = joined[..first_len].to_string();
            emitted.push(TextGrapheme::Owned {
                byte_idx: self.carry_start,
                text,
            });
            skip_until = first_len.saturating_sub(carry_len);
            self.carry.clear();
        }

        let remaining = &chunk[skip_until..];
        let remaining_base = chunk_start + skip_until;
        let mut boundaries = remaining
            .grapheme_indices(true)
            .map(|(idx, grapheme)| (idx, grapheme))
            .collect::<Vec<_>>();

        if let Some((last_idx, last_grapheme)) = boundaries.pop() {
            for (local_idx, grapheme) in boundaries {
                emitted.push(TextGrapheme::Borrowed {
                    byte_idx: remaining_base + local_idx,
                    text: grapheme,
                });
            }
            self.carry_start = remaining_base + last_idx;
            self.carry.clear();
            self.carry.push_str(last_grapheme);
        }

        self.pending = emitted.into_iter();
    }

    fn finish_carry(&mut self) {
        self.finished = true;
        let emitted = if self.carry.is_empty() {
            Vec::new()
        } else {
            vec![TextGrapheme::Owned {
                byte_idx: self.carry_start,
                text: std::mem::take(&mut self.carry),
            }]
        };
        self.carry.clear();
        self.pending = emitted.into_iter();
    }
}

impl<'a> Iterator for TextGraphemes<'a> {
    type Item = TextGrapheme<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.refill_pending();
        self.pending.next()
    }
}

/// Read-only snapshot behavior for buffer text.
pub trait TextSnapshot: Clone + PartialEq {
    /// Reference type returned for any section of text.
    type Ref<'a>: TextRef + 'a
    where
        Self: 'a;

    /// Returns the byte length including implicit newlines between lines.
    fn len(&self) -> usize;

    /// Returns true when the text has one empty line.
    fn is_empty(&self) -> bool;

    /// Returns the number of lines.
    fn line_count(&self) -> usize;

    /// Returns one line without its trailing newline.
    fn line(&self, line: usize) -> Option<Self::Ref<'_>>;

    /// Iterates lines without trailing newlines.
    fn lines(&self) -> impl Iterator<Item = Self::Ref<'_>> + '_;

    /// Returns text in a cursor range.
    fn range(&self, start: Cursor, end: Cursor) -> Option<Self::Ref<'_>>;

    /// Returns the whole text.
    fn text(&self) -> Self::Ref<'_>;

    /// Converts a cursor to a full-text byte offset.
    fn byte_offset_for_cursor(&self, cursor: Cursor) -> Option<usize>;

    /// Converts a full-text byte offset to a cursor.
    fn cursor_for_byte_offset(&self, offset: usize) -> Option<Cursor>;

    /// Converts a byte cursor to an encoded position.
    fn position_for_cursor(&self, cursor: Cursor, encoding: TextEncoding) -> Option<TextPosition>;

    /// Converts an encoded position to a byte cursor.
    fn cursor_for_position(&self, position: TextPosition, encoding: TextEncoding)
    -> Option<Cursor>;

    /// Converts a byte cursor range to an encoded range.
    fn range_for_cursors(
        &self,
        start: Cursor,
        end: Cursor,
        encoding: TextEncoding,
    ) -> Option<TextRange> {
        Some(TextRange {
            start: self.position_for_cursor(start, encoding)?,
            end: self.position_for_cursor(end, encoding)?,
        })
    }

    /// Converts an encoded range to a byte cursor range.
    fn cursors_for_range(
        &self,
        range: TextRange,
        encoding: TextEncoding,
    ) -> Option<TextObjectRange> {
        Some(TextObjectRange {
            start: self.cursor_for_position(range.start, encoding)?,
            end: self.cursor_for_position(range.end, encoding)?,
        })
    }

    /// Converts a line span to an encoded range.
    fn line_range_for_lines(
        &self,
        start_line: usize,
        end_line: usize,
        encoding: TextEncoding,
    ) -> Option<TextRange>;
}

/// Editable text storage behavior.
pub trait TextStorage: TextSnapshot {
    /// Creates empty text with one empty line.
    fn new_empty() -> Self;

    /// Creates text from full contents.
    fn from_text(text: &str) -> Self;

    /// Replaces the full text.
    fn replace_text(&mut self, text: &str);

    /// Inserts one character.
    fn insert_char(&mut self, cursor: Cursor, ch: char) -> Option<TextChange>;

    /// Inserts text.
    fn insert_text(&mut self, cursor: Cursor, text: &str) -> Option<TextChange>;

    /// Removes a cursor range.
    fn remove(&mut self, start: Cursor, end: Cursor) -> Option<TextChange>;

    /// Deletes whole lines.
    fn delete_lines(&mut self, start_line: usize, count: usize) -> Option<TextChange>;

    /// Replaces whole lines with one empty line.
    fn change_lines(&mut self, start_line: usize, count: usize) -> Option<TextChange>;

    /// Inserts blank lines after a line.
    fn insert_blank_lines_after(&mut self, line: usize, count: usize) -> Option<TextChange>;

    /// Inserts blank lines before a line.
    fn insert_blank_lines_before(&mut self, line: usize, count: usize) -> Option<TextChange>;

    /// Pastes linewise content.
    fn paste_linewise<I>(&mut self, line: usize, lines: I, after: bool) -> Option<TextChange>
    where
        I: IntoIterator,
        I::Item: AsRef<str>;

    /// Joins adjacent lines.
    fn join_lines(
        &mut self,
        start_line: usize,
        line_count: usize,
        with_space: bool,
    ) -> Option<(Cursor, TextChange)>;
}

/// Current line-oriented text storage implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineText {
    lines: Vector<Arc<str>>,
}

impl LineText {
    /// Creates empty text with one empty line.
    pub fn new_empty() -> Self {
        <Self as TextStorage>::new_empty()
    }

    /// Creates text from full contents.
    pub fn from_text(text: &str) -> Self {
        <Self as TextStorage>::from_text(text)
    }

    /// Returns the line byte length.
    pub fn line_len(&self, line: usize) -> usize {
        self.lines.get(line).map_or(0, |line| line.len())
    }

    /// Returns one line without its trailing newline.
    pub fn get(&self, line: usize) -> Option<LineTextRef<'_>> {
        self.line(line)
    }

    /// Iterates lines without trailing newlines.
    pub fn iter(&self) -> impl Iterator<Item = LineTextRef<'_>> + '_ {
        self.lines()
    }

    /// Replaces one line.
    pub fn update(&self, line: usize, text: Arc<str>) -> Self {
        let mut next = self.clone();
        if line < next.lines.len() {
            next.lines = next.lines.update(line, text);
        }
        next
    }

    /// Returns the number of lines.
    pub fn len(&self) -> usize {
        <Self as TextSnapshot>::len(self)
    }

    /// Returns true when the text has one empty line.
    pub fn is_empty(&self) -> bool {
        <Self as TextSnapshot>::is_empty(self)
    }

    /// Returns the number of lines.
    pub fn line_count(&self) -> usize {
        <Self as TextSnapshot>::line_count(self)
    }

    /// Returns true when there are no lines.
    pub fn is_vector_empty(&self) -> bool {
        self.lines.is_empty()
    }

    fn line_str(&self, line: usize) -> Option<&str> {
        self.lines.get(line).map(|line| line.as_ref())
    }

    fn eof_cursor(&self) -> Cursor {
        let line = self.lines.len().saturating_sub(1);
        Cursor::new(line, self.line_len(line))
    }

    fn valid_cursor(&self, cursor: Cursor) -> bool {
        self.line_str(cursor.line)
            .is_some_and(|line| cursor.col <= line.len() && line.is_char_boundary(cursor.col))
    }
}

/// Non-owning reference into `LineText`.
#[derive(Debug, Clone, Copy)]
pub struct LineTextRef<'a> {
    text: &'a LineText,
    start: Cursor,
    end: Cursor,
}

impl<'a> LineTextRef<'a> {
    fn new(text: &'a LineText, start: Cursor, end: Cursor) -> Self {
        Self { text, start, end }
    }

    /// Returns this reference as a contiguous string slice when possible.
    pub fn as_str(&self) -> Option<&'a str> {
        if self.start.line != self.end.line {
            return None;
        }
        self.text
            .line_str(self.start.line)?
            .get(self.start.col..self.end.col)
    }
}

impl TextRef for LineTextRef<'_> {
    fn len(&self) -> usize {
        if self.start.line == self.end.line {
            return self.end.col.saturating_sub(self.start.col);
        }

        let mut len = 0usize;
        if let Some(line) = self.text.line_str(self.start.line) {
            len = len.saturating_add(line.len().saturating_sub(self.start.col));
        }
        for line_idx in self.start.line + 1..self.end.line {
            len = len.saturating_add(1);
            len = len.saturating_add(self.text.line_len(line_idx));
        }
        len = len.saturating_add(1);
        len.saturating_add(self.end.col)
    }

    fn chunks(&self) -> impl Iterator<Item = &str> + '_ {
        LineTextRefChunks {
            reference: *self,
            next_line: self.start.line,
            emit_newline_before_next_line: false,
            done: false,
        }
    }

    fn contiguous_text(&self) -> Option<&str> {
        self.as_str()
    }
}

impl fmt::Display for LineTextRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.chunks() {
            f.write_str(chunk)?;
        }
        Ok(())
    }
}

struct LineTextRefChunks<'a> {
    reference: LineTextRef<'a>,
    next_line: usize,
    emit_newline_before_next_line: bool,
    done: bool,
}

impl<'a> Iterator for LineTextRefChunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if self.emit_newline_before_next_line {
            self.emit_newline_before_next_line = false;
            return Some("\n");
        }

        let start = self.reference.start;
        let end = self.reference.end;
        if self.next_line > end.line {
            self.done = true;
            return None;
        }

        let line_idx = self.next_line;
        self.next_line += 1;
        if line_idx == end.line {
            self.done = true;
        } else {
            self.emit_newline_before_next_line = true;
        }

        let line = self.reference.text.line_str(line_idx)?;
        let start_col = if line_idx == start.line { start.col } else { 0 };
        let end_col = if line_idx == end.line {
            end.col
        } else {
            line.len()
        };
        line.get(start_col..end_col)
    }
}

impl TextSnapshot for LineText {
    type Ref<'a> = LineTextRef<'a>;

    fn len(&self) -> usize {
        self.lines.iter().map(|line| line.len()).sum::<usize>() + self.lines.len().saturating_sub(1)
    }

    fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines.get(0).is_none_or(|line| line.is_empty())
    }

    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn line(&self, line: usize) -> Option<Self::Ref<'_>> {
        let text = self.line_str(line)?;
        Some(LineTextRef::new(
            self,
            Cursor::new(line, 0),
            Cursor::new(line, text.len()),
        ))
    }

    fn lines(&self) -> impl Iterator<Item = Self::Ref<'_>> + '_ {
        (0..self.line_count()).filter_map(|line| self.line(line))
    }

    fn range(&self, start: Cursor, end: Cursor) -> Option<Self::Ref<'_>> {
        if start > end || !self.valid_cursor(start) || !self.valid_cursor(end) {
            return None;
        }
        Some(LineTextRef::new(self, start, end))
    }

    fn text(&self) -> Self::Ref<'_> {
        LineTextRef::new(self, Cursor::new(0, 0), self.eof_cursor())
    }

    fn byte_offset_for_cursor(&self, cursor: Cursor) -> Option<usize> {
        if !self.valid_cursor(cursor) {
            return None;
        }

        let mut offset = 0usize;
        for line_idx in 0..cursor.line {
            offset = offset.saturating_add(self.line_len(line_idx));
            offset = offset.saturating_add(1);
        }
        Some(offset.saturating_add(cursor.col))
    }

    fn cursor_for_byte_offset(&self, offset: usize) -> Option<Cursor> {
        let mut current = 0usize;
        for line_idx in 0..self.line_count() {
            let line_len = self.line_len(line_idx);
            if offset <= current + line_len {
                let col = offset - current;
                return self
                    .valid_cursor(Cursor::new(line_idx, col))
                    .then_some(Cursor::new(line_idx, col));
            }
            current = current.saturating_add(line_len);
            if line_idx + 1 < self.line_count() {
                if offset == current {
                    return None;
                }
                current = current.saturating_add(1);
            }
        }
        None
    }

    fn position_for_cursor(&self, cursor: Cursor, encoding: TextEncoding) -> Option<TextPosition> {
        let line = self.line(cursor.line)?;
        let character = byte_index_to_position_character(&line, cursor.col, encoding)?;
        Some(TextPosition {
            line: cursor.line,
            character,
        })
    }

    fn cursor_for_position(
        &self,
        position: TextPosition,
        encoding: TextEncoding,
    ) -> Option<Cursor> {
        let line = self.line(position.line)?;
        let col = position_character_to_byte_index(&line, position.character, encoding)?;
        Some(Cursor::new(position.line, col))
    }

    fn line_range_for_lines(
        &self,
        start_line: usize,
        end_line: usize,
        encoding: TextEncoding,
    ) -> Option<TextRange> {
        if start_line >= end_line || start_line >= self.line_count() {
            return None;
        }

        let start = self.position_for_cursor(Cursor::new(start_line, 0), encoding)?;
        let end_line_index = end_line.min(self.line_count());
        let end = if end_line_index < self.line_count() {
            self.position_for_cursor(Cursor::new(end_line_index, 0), encoding)?
        } else {
            let last_line = self.line_count().saturating_sub(1);
            self.position_for_cursor(Cursor::new(last_line, self.line_len(last_line)), encoding)?
        };
        Some(TextRange { start, end })
    }
}

impl TextStorage for LineText {
    fn new_empty() -> Self {
        Self {
            lines: Vector::unit(Arc::from("")),
        }
    }

    fn from_text(text: &str) -> Self {
        if text.is_empty() {
            return Self::new_empty();
        }
        Self {
            lines: text.lines().map(Arc::from).collect(),
        }
    }

    fn replace_text(&mut self, text: &str) {
        *self = Self::from_text(text);
    }

    fn insert_char(&mut self, cursor: Cursor, ch: char) -> Option<TextChange> {
        if !self.valid_cursor(cursor) {
            return None;
        }

        let old_line_count = self.line_count();
        if ch == '\n' {
            let line = self.line_str(cursor.line)?;
            let before = line[..cursor.col].to_string();
            let after = line[cursor.col..].to_string();
            let mut left = self.lines.take(cursor.line);
            let right = self.lines.skip(cursor.line + 1);
            left.push_back(Arc::from(before));
            left.push_back(Arc::from(after));
            left.append(right);
            self.lines = left;
        } else {
            let line = self.line_str(cursor.line)?;
            let mut new_line = line.to_string();
            new_line.insert(cursor.col, ch);
            self.lines = self.lines.update(cursor.line, Arc::from(new_line));
        }
        Some(TextChange::new(
            cursor.line,
            old_line_count,
            self.line_count(),
        ))
    }

    fn insert_text(&mut self, cursor: Cursor, text: &str) -> Option<TextChange> {
        if text.is_empty() {
            return Some(TextChange::new(
                cursor.line,
                self.line_count(),
                self.line_count(),
            ));
        }
        if !self.valid_cursor(cursor) {
            return None;
        }

        let line = self.line_str(cursor.line)?;
        let old_line_count = self.line_count();
        let before = line[..cursor.col].to_string();
        let after = line[cursor.col..].to_string();
        let mut inserted_parts = text.split('\n');
        let first_part = inserted_parts.next().unwrap_or_default();
        let mut new_lines: Vec<Arc<str>> = Vec::new();
        new_lines.push(Arc::from(format!("{}{}", before, first_part)));

        for part in inserted_parts {
            new_lines.push(Arc::from(part));
        }

        if new_lines.len() == 1 {
            let mut new_line = new_lines.pop()?.to_string();
            new_line.push_str(&after);
            self.lines = self.lines.update(cursor.line, Arc::from(new_line));
        } else {
            if let Some(last_line) = new_lines.last_mut() {
                let mut merged = last_line.to_string();
                merged.push_str(&after);
                *last_line = Arc::from(merged);
            }

            let mut left = self.lines.take(cursor.line);
            let right = self.lines.skip(cursor.line + 1);
            left.append(new_lines.into_iter().collect());
            left.append(right);
            self.lines = left;
        }

        Some(TextChange::new(
            cursor.line,
            old_line_count,
            self.line_count(),
        ))
    }

    fn remove(&mut self, start: Cursor, end: Cursor) -> Option<TextChange> {
        if start > end || start == end || !self.valid_cursor(start) || !self.valid_cursor(end) {
            return None;
        }

        let old_line_count = self.line_count();
        if start.line == end.line {
            let line = self.line_str(start.line)?;
            let mut new_line = line.to_string();
            new_line.drain(start.col..end.col);
            self.lines = self.lines.update(start.line, Arc::from(new_line));
        } else {
            let before = self.line_str(start.line)?[..start.col].to_string();
            let after = self.line_str(end.line)?[end.col..].to_string();
            let mut left = self.lines.take(start.line);
            let right = self.lines.skip(end.line + 1);
            left.push_back(Arc::from(format!("{}{}", before, after)));
            left.append(right);
            self.lines = left;
        }

        Some(TextChange::new(
            start.line,
            old_line_count,
            self.line_count(),
        ))
    }

    fn delete_lines(&mut self, start_line: usize, count: usize) -> Option<TextChange> {
        let total_lines = self.line_count();
        if start_line >= total_lines {
            return None;
        }
        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(TextChange::new(start_line, total_lines, total_lines));
        }

        let end_line = start_line + actual_count;
        if end_line >= total_lines {
            let mut left = self.lines.take(start_line);
            if left.is_empty() {
                left.push_back(Arc::from(""));
            }
            self.lines = left;
        } else {
            let mut left = self.lines.take(start_line);
            let right = self.lines.skip(end_line);
            left.append(right);
            self.lines = left;
        }
        Some(TextChange::new(start_line, total_lines, self.line_count()))
    }

    fn change_lines(&mut self, start_line: usize, count: usize) -> Option<TextChange> {
        let total_lines = self.line_count();
        if start_line >= total_lines {
            return None;
        }
        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(TextChange::new(start_line, total_lines, total_lines));
        }

        let end_line = start_line + actual_count;
        if end_line >= total_lines {
            let mut left = self.lines.take(start_line);
            left.push_back(Arc::from(""));
            self.lines = left;
        } else {
            let mut left = self.lines.take(start_line);
            left.push_back(Arc::from(""));
            let right = self.lines.skip(end_line);
            left.append(right);
            self.lines = left;
        }
        Some(TextChange::new(start_line, total_lines, self.line_count()))
    }

    fn insert_blank_lines_after(&mut self, line: usize, count: usize) -> Option<TextChange> {
        let total_lines = self.line_count();
        if count == 0 {
            return Some(TextChange::new(line, total_lines, total_lines));
        }
        let insert_at = if line >= total_lines {
            total_lines
        } else {
            line + 1
        };
        insert_blank_lines_at(&mut self.lines, insert_at, count);
        Some(TextChange::new(insert_at, total_lines, self.line_count()))
    }

    fn insert_blank_lines_before(&mut self, line: usize, count: usize) -> Option<TextChange> {
        let total_lines = self.line_count();
        if count == 0 {
            return Some(TextChange::new(line, total_lines, total_lines));
        }
        let insert_at = line.min(total_lines);
        insert_blank_lines_at(&mut self.lines, insert_at, count);
        Some(TextChange::new(insert_at, total_lines, self.line_count()))
    }

    fn paste_linewise<I>(&mut self, line: usize, lines: I, after: bool) -> Option<TextChange>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let content: Vec<Arc<str>> = lines
            .into_iter()
            .map(|line| Arc::from(line.as_ref()))
            .collect();
        if content.is_empty() {
            return Some(TextChange::new(line, self.line_count(), self.line_count()));
        }

        let total_lines = self.line_count();
        let insert_at = if after {
            (line + 1).min(total_lines)
        } else {
            line.min(total_lines)
        };
        insert_lines_at(&mut self.lines, insert_at, content);
        Some(TextChange::new(insert_at, total_lines, self.line_count()))
    }

    fn join_lines(
        &mut self,
        start_line: usize,
        line_count: usize,
        with_space: bool,
    ) -> Option<(Cursor, TextChange)> {
        if line_count < 2 || start_line >= self.line_count() {
            return None;
        }
        let total_lines = self.line_count();
        let actual_line_count = (total_lines - start_line).min(line_count);
        if actual_line_count < 2 {
            return None;
        }

        let mut joined_content = String::new();
        for i in 0..actual_line_count {
            if i > 0 && with_space {
                joined_content.push(' ');
            }
            joined_content.push_str(self.line_str(start_line + i)?);
        }

        let joined_len = joined_content.len();
        let end_line = start_line + actual_line_count;
        let mut left = self.lines.take(start_line);
        let right = self.lines.skip(end_line);
        left.push_back(Arc::from(joined_content));
        left.append(right);
        self.lines = left;
        let change = TextChange::new(start_line, total_lines, self.line_count());
        Some((Cursor::new(start_line, joined_len), change))
    }
}

fn insert_blank_lines_at(lines: &mut Vector<Arc<str>>, insert_at: usize, count: usize) {
    let blanks = std::iter::repeat_with(|| Arc::from(""))
        .take(count)
        .collect::<Vec<_>>();
    insert_lines_at(lines, insert_at, blanks);
}

fn insert_lines_at(lines: &mut Vector<Arc<str>>, insert_at: usize, new_lines: Vec<Arc<str>>) {
    if insert_at >= lines.len() {
        for line in new_lines {
            lines.push_back(line);
        }
        return;
    }

    let mut left = lines.take(insert_at);
    let right = lines.skip(insert_at);
    left.append(new_lines.into_iter().collect());
    left.append(right);
    *lines = left;
}

fn byte_index_to_position_character(
    text: &impl TextRef,
    byte_index: usize,
    encoding: TextEncoding,
) -> Option<usize> {
    if byte_index > text.len() {
        return None;
    }

    match encoding {
        TextEncoding::Utf8 => chunk_byte_boundary(text, byte_index).then_some(byte_index),
        TextEncoding::Utf16 => {
            let mut bytes = 0usize;
            let mut units = 0usize;
            for chunk in text.chunks() {
                if byte_index < bytes + chunk.len() {
                    let local = byte_index - bytes;
                    if !chunk.is_char_boundary(local) {
                        return None;
                    }
                    for ch in chunk[..local].chars() {
                        units += ch.len_utf16();
                    }
                    return Some(units);
                }
                if byte_index == bytes + chunk.len() {
                    for ch in chunk.chars() {
                        units += ch.len_utf16();
                    }
                    return Some(units);
                }
                bytes += chunk.len();
                for ch in chunk.chars() {
                    units += ch.len_utf16();
                }
            }
            (byte_index == bytes).then_some(units)
        }
    }
}

fn position_character_to_byte_index(
    text: &impl TextRef,
    character: usize,
    encoding: TextEncoding,
) -> Option<usize> {
    match encoding {
        TextEncoding::Utf8 => chunk_byte_boundary(text, character).then_some(character),
        TextEncoding::Utf16 => {
            let mut bytes = 0usize;
            let mut units = 0usize;
            for chunk in text.chunks() {
                for (local_byte, ch) in chunk.char_indices() {
                    if units == character {
                        return Some(bytes + local_byte);
                    }
                    let next_units = units + ch.len_utf16();
                    if character > units && character < next_units {
                        return None;
                    }
                    units = next_units;
                }
                bytes += chunk.len();
            }
            (units == character).then_some(bytes)
        }
    }
}

fn chunk_byte_boundary<T: TextRef + ?Sized>(text: &T, byte_index: usize) -> bool {
    if byte_index > text.len() {
        return false;
    }

    let mut bytes = 0usize;
    for chunk in text.chunks() {
        if byte_index < bytes + chunk.len() {
            return chunk.is_char_boundary(byte_index - bytes);
        }
        if byte_index == bytes + chunk.len() {
            return true;
        }
        bytes += chunk.len();
    }
    byte_index == bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ChunkedTextRef<'a> {
        chunks: Vec<&'a str>,
    }

    impl<'a> ChunkedTextRef<'a> {
        fn new(chunks: Vec<&'a str>) -> Self {
            Self { chunks }
        }
    }

    impl TextRef for ChunkedTextRef<'_> {
        fn len(&self) -> usize {
            self.chunks.iter().map(|chunk| chunk.len()).sum()
        }

        fn chunks(&self) -> impl Iterator<Item = &str> + '_ {
            self.chunks.iter().copied()
        }
    }

    #[test]
    fn empty_text_has_one_empty_line() {
        let text = LineText::new_empty();

        assert_eq!(text.line_count(), 1);
        assert!(text.is_empty());
        assert_eq!(text.text().to_text(), "");
    }

    #[test]
    fn range_spanning_lines_includes_newlines() {
        let text = LineText::from_text("alpha\nbeta\ngamma");
        let range = text
            .range(Cursor::new(0, 2), Cursor::new(2, 3))
            .expect("range");

        assert_eq!(
            range.chunks().collect::<Vec<_>>(),
            vec!["pha", "\n", "beta", "\n", "gam"]
        );
        assert_eq!(range.to_text(), "pha\nbeta\ngam");
    }

    #[test]
    fn byte_offsets_convert_to_cursors() {
        let text = LineText::from_text("ab\ncde");

        assert_eq!(text.byte_offset_for_cursor(Cursor::new(1, 2)), Some(5));
        assert_eq!(text.cursor_for_byte_offset(5), Some(Cursor::new(1, 2)));
    }

    #[test]
    fn utf16_position_conversion_handles_non_bmp() {
        let text = LineText::from_text("a𝄞b");

        assert_eq!(
            text.position_for_cursor(Cursor::new(0, "a𝄞".len()), TextEncoding::Utf16),
            Some(TextPosition {
                line: 0,
                character: 3
            })
        );
        assert_eq!(
            text.cursor_for_position(
                TextPosition {
                    line: 0,
                    character: 2
                },
                TextEncoding::Utf16,
            ),
            None
        );
    }

    #[test]
    fn contiguous_text_with_scratch_borrows_contiguous_line_refs() {
        let text = LineText::from_text("alpha");
        let line = text.line(0).expect("line");
        let mut scratch = String::from("preserve allocation");

        let borrowed = line.contiguous_text_with_scratch(&mut scratch);

        assert_eq!(borrowed, "alpha");
        assert_eq!(scratch, "preserve allocation");
    }

    #[test]
    fn contiguous_text_with_scratch_materializes_chunked_refs() {
        let text = ChunkedTextRef::new(vec!["al", "pha"]);
        let mut scratch = String::from("old");

        let materialized = text.contiguous_text_with_scratch(&mut scratch);

        assert_eq!(materialized, "alpha");
        assert_eq!(scratch, "alpha");
    }

    #[test]
    fn chunked_char_helpers_work_across_chunk_boundaries() {
        let text = ChunkedTextRef::new(vec!["a", "é", "𝄞", "z"]);
        let e_start = "a".len();
        let symbol_start = "aé".len();
        let z_start = "aé𝄞".len();

        assert!(text.is_char_boundary(0));
        assert!(text.is_char_boundary(e_start));
        assert!(text.is_char_boundary(symbol_start));
        assert!(text.is_char_boundary(z_start));
        assert!(text.is_char_boundary(text.len()));
        assert!(!text.is_char_boundary(e_start + 1));
        assert!(!text.is_char_boundary(symbol_start + 1));

        assert_eq!(text.char_at(e_start), Some('é'));
        assert_eq!(text.previous_char(z_start), Some((symbol_start, '𝄞')));
        assert_eq!(text.next_char(e_start), Some((e_start, 'é')));
        assert_eq!(text.next_char(e_start + 1), None);
    }

    #[test]
    fn chunked_graphemes_merge_clusters_across_chunk_boundaries() {
        let text = ChunkedTextRef::new(vec!["a", "e", "\u{301}", "🇺", "🇸", "z"]);

        let graphemes = text
            .graphemes()
            .map(|grapheme| (grapheme.byte_idx(), grapheme.into_owned()))
            .collect::<Vec<_>>();

        assert_eq!(
            graphemes,
            vec![
                (0, String::from("a")),
                (1, String::from("e\u{301}")),
                ("ae\u{301}".len(), String::from("🇺🇸")),
                ("ae\u{301}🇺🇸".len(), String::from("z")),
            ]
        );
        assert_eq!(
            text.grapheme_at(1).map(|g| g.into_owned()),
            Some(String::from("e\u{301}"))
        );
        assert_eq!(
            text.previous_grapheme("ae\u{301}🇺🇸".len())
                .map(|g| g.into_owned()),
            Some(String::from("🇺🇸"))
        );
        assert_eq!(
            text.next_grapheme("ae\u{301}".len())
                .map(|g| g.into_owned()),
            Some(String::from("🇺🇸"))
        );
    }

    #[test]
    fn chunk_contained_graphemes_are_borrowed() {
        let text = ChunkedTextRef::new(vec!["abc", "def"]);
        let graphemes = text.graphemes().collect::<Vec<_>>();

        assert!(matches!(graphemes[0], TextGrapheme::Borrowed { .. }));
        assert!(matches!(graphemes[1], TextGrapheme::Borrowed { .. }));
        assert!(matches!(graphemes[2], TextGrapheme::Owned { .. }));
        assert!(matches!(graphemes[3], TextGrapheme::Borrowed { .. }));
        assert!(matches!(graphemes[4], TextGrapheme::Borrowed { .. }));
        assert!(matches!(graphemes[5], TextGrapheme::Owned { .. }));
    }

    #[test]
    fn range_text_materializes_chunked_ranges() {
        let text = ChunkedTextRef::new(vec!["al", "pha", " be", "ta"]);

        assert_eq!(text.range_text(1, 8), Some("lpha be".to_string()));
        assert_eq!(text.range_text(1, 100), None);
    }

    #[test]
    fn range_starts_with_checks_chunked_prefixes_without_materializing() {
        let text = ChunkedTextRef::new(vec!["../", "src", "/main.rs"]);

        assert_eq!(text.range_starts_with(0, text.len(), "../"), Some(true));
        assert_eq!(text.range_starts_with(0, text.len(), "./"), Some(false));
        assert_eq!(text.range_starts_with(3, text.len(), "src"), Some(true));
        assert_eq!(
            text.range_starts_with(3, text.len(), "src/main"),
            Some(true)
        );
        assert_eq!(text.range_starts_with(3, 5, "src"), Some(false));
        assert_eq!(text.range_starts_with(100, text.len(), "src"), None);
    }
}
