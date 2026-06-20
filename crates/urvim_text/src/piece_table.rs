use crate::{
    Cursor, TextChange, TextEncoding, TextPosition, TextRange, TextRef, TextSnapshot, TextStorage,
    byte_index_to_position_character, position_character_to_byte_index,
};
use std::fmt;
use std::sync::Arc;

/// Piece-table text storage implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceTable {
    inner: Arc<PieceTableInner>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PieceTableInner {
    original: Arc<str>,
    add: String,
    lines: Vec<Line>,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Line {
    pieces: Vec<Piece>,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Piece {
    source: PieceSource,
    start: usize,
    len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PieceSource {
    Original,
    Add,
}

/// Non-owning reference into `PieceTable`.
#[derive(Debug, Clone, Copy)]
pub struct PieceTableRef<'a> {
    text: &'a PieceTable,
    start: Cursor,
    end: Cursor,
}

impl PieceTable {
    /// Creates empty text with one empty line.
    pub fn new_empty() -> Self {
        <Self as TextStorage>::new_empty()
    }

    /// Creates text from full contents.
    pub fn from_text(text: &str) -> Self {
        <Self as TextStorage>::from_text(text)
    }

    /// Returns one line without its trailing newline.
    pub fn get(&self, line: usize) -> Option<PieceTableRef<'_>> {
        self.line(line)
    }

    /// Iterates lines without trailing newlines.
    pub fn iter(&self) -> impl Iterator<Item = PieceTableRef<'_>> + '_ {
        self.lines()
    }

    /// Replaces one line.
    pub fn update(&self, line: usize, text: Arc<str>) -> Self {
        let mut next = self.clone();
        if line < next.line_count() {
            let inner = Arc::make_mut(&mut next.inner);
            let new_line = line_from_text(&mut inner.add, text.as_ref());
            inner.lines[line] = new_line;
            inner.len = compute_len(&inner.lines);
        }
        next
    }

    /// Returns the number of bytes in the logical text.
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

    /// Returns the byte length of a line.
    pub fn line_len(&self, line: usize) -> usize {
        self.inner.lines.get(line).map_or(0, |line| line.len)
    }

    /// Returns true when there are no lines in the backing vector.
    pub fn is_vector_empty(&self) -> bool {
        self.inner.lines.is_empty()
    }

    fn inner_mut(&mut self) -> &mut PieceTableInner {
        Arc::make_mut(&mut self.inner)
    }

    fn valid_cursor(&self, cursor: Cursor) -> bool {
        self.line(cursor.line)
            .is_some_and(|line| cursor.col <= line.len() && line.is_char_boundary(cursor.col))
    }

    fn line_is_char_boundary(&self, line_idx: usize, col: usize) -> bool {
        let Some(line) = self.inner.lines.get(line_idx) else {
            return false;
        };
        if col > line.len {
            return false;
        }

        let mut offset = 0usize;
        for piece in &line.pieces {
            let piece_end = offset + piece.len;
            if col < piece_end {
                let text = self.piece_text(piece);
                return text.is_char_boundary(col - offset);
            }
            if col == piece_end {
                return true;
            }
            offset = piece_end;
        }

        col == offset
    }

    fn piece_text<'a>(&'a self, piece: &Piece) -> &'a str {
        match piece.source {
            PieceSource::Original => &self.inner.original[piece.start..piece.start + piece.len],
            PieceSource::Add => &self.inner.add[piece.start..piece.start + piece.len],
        }
    }

    fn eof_cursor(&self) -> Cursor {
        let line = self.line_count().saturating_sub(1);
        Cursor::new(line, self.line_len(line))
    }

    fn split_line_pieces(line: &Line, col: usize) -> (Line, Line) {
        let mut left = Line::empty();
        let mut right = Line::empty();
        let mut offset = 0usize;

        for piece in &line.pieces {
            let piece_end = offset + piece.len;
            if piece_end <= col {
                left.push(piece.clone());
            } else if offset >= col {
                right.push(piece.clone());
            } else {
                let split = col - offset;
                if split > 0 {
                    left.push(piece.slice(0, split));
                }
                if split < piece.len {
                    right.push(piece.slice(split, piece.len - split));
                }
            }
            offset = piece_end;
        }

        (left, right)
    }

    fn merge_line_pieces(line: &mut Line) {
        let mut merged: Vec<Piece> = Vec::with_capacity(line.pieces.len());
        for piece in line.pieces.drain(..) {
            if piece.len == 0 {
                continue;
            }
            if let Some(last) = merged.last_mut()
                && last.source == piece.source
                && last.start + last.len == piece.start
            {
                last.len += piece.len;
                continue;
            }
            merged.push(piece);
        }
        line.len = merged.iter().map(|piece| piece.len).sum();
        line.pieces = merged;
    }

    fn append_text_lines(inner: &mut PieceTableInner, text: &str) -> Vec<Line> {
        let add_start = inner.add.len();
        inner.add.push_str(text);

        let mut lines = Vec::new();
        let mut offset = add_start;
        let parts = text.split('\n').collect::<Vec<_>>();
        for (idx, part) in parts.iter().enumerate() {
            let mut line = Line::empty();
            if !part.is_empty() {
                line.push(Piece {
                    source: PieceSource::Add,
                    start: offset,
                    len: part.len(),
                });
            }
            line.len = part.len();
            lines.push(line);
            offset += part.len();
            if idx + 1 < parts.len() {
                offset += 1;
            }
        }

        lines
    }

    fn insert_newline(&mut self, cursor: Cursor) -> Option<TextChange> {
        if !self.valid_cursor(cursor) {
            return None;
        }

        let inner = self.inner_mut();
        let old_line_count = inner.lines.len();
        let line = inner.lines.get(cursor.line)?.clone();
        let (left, right) = Self::split_line_pieces(&line, cursor.col);
        inner.lines[cursor.line] = left;
        inner.lines.insert(cursor.line + 1, right);
        inner.len = compute_len(&inner.lines);
        Some(TextChange::new(
            cursor.line,
            old_line_count,
            inner.lines.len(),
        ))
    }
}

impl<'a> PieceTableRef<'a> {
    fn new(text: &'a PieceTable, start: Cursor, end: Cursor) -> Self {
        Self { text, start, end }
    }

    /// Returns this reference as a contiguous string slice when possible.
    pub fn as_str(&self) -> Option<&str> {
        if self.start.line != self.end.line {
            return None;
        }
        let line = self.text.inner.lines.get(self.start.line)?;
        let mut offset = 0usize;
        for piece in &line.pieces {
            let piece_end = offset + piece.len;
            if self.start.col >= offset && self.end.col <= piece_end {
                let text = self.text.piece_text(piece);
                return text.get(self.start.col - offset..self.end.col - offset);
            }
            offset = piece_end;
        }
        None
    }
}

impl TextRef for PieceTableRef<'_> {
    fn len(&self) -> usize {
        if self.start.line == self.end.line {
            return self.end.col.saturating_sub(self.start.col);
        }

        let mut len = 0usize;
        if let Some(line) = self.text.inner.lines.get(self.start.line) {
            len = len.saturating_add(line.len.saturating_sub(self.start.col));
        }
        for line_idx in self.start.line + 1..self.end.line {
            len = len.saturating_add(1);
            len = len.saturating_add(self.text.line_len(line_idx));
        }
        len = len.saturating_add(1);
        len.saturating_add(self.end.col)
    }

    fn chunks(&self) -> impl Iterator<Item = &str> + '_ {
        PieceTableRefChunks::new(*self)
    }

    fn contiguous_text(&self) -> Option<&str> {
        self.as_str()
    }
}

impl fmt::Display for PieceTableRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.chunks() {
            f.write_str(chunk)?;
        }
        Ok(())
    }
}

impl TextSnapshot for PieceTable {
    type Ref<'a> = PieceTableRef<'a>;

    fn len(&self) -> usize {
        self.inner.len
    }

    fn is_empty(&self) -> bool {
        self.inner.lines.len() == 1 && self.inner.lines.first().is_none_or(|line| line.len == 0)
    }

    fn line_count(&self) -> usize {
        self.inner.lines.len()
    }

    fn line(&self, line: usize) -> Option<Self::Ref<'_>> {
        let line_ref = self.inner.lines.get(line)?;
        Some(PieceTableRef::new(
            self,
            Cursor::new(line, 0),
            Cursor::new(line, line_ref.len),
        ))
    }

    fn lines(&self) -> impl Iterator<Item = Self::Ref<'_>> + '_ {
        (0..self.line_count()).filter_map(|line| self.line(line))
    }

    fn range(&self, start: Cursor, end: Cursor) -> Option<Self::Ref<'_>> {
        if start > end || !self.valid_cursor(start) || !self.valid_cursor(end) {
            return None;
        }
        Some(PieceTableRef::new(self, start, end))
    }

    fn text(&self) -> Self::Ref<'_> {
        PieceTableRef::new(self, Cursor::new(0, 0), self.eof_cursor())
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
                    .line_is_char_boundary(line_idx, col)
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

impl TextStorage for PieceTable {
    fn new_empty() -> Self {
        Self {
            inner: Arc::new(PieceTableInner {
                original: Arc::from(""),
                add: String::new(),
                lines: vec![Line::empty()],
                len: 0,
            }),
        }
    }

    fn from_text(text: &str) -> Self {
        if text.is_empty() {
            return Self::new_empty();
        }

        let mut original = String::new();
        let mut lines = Vec::new();
        let mut offset = 0usize;

        for part in text.lines() {
            let mut line = Line::empty();
            if !part.is_empty() {
                line.push(Piece {
                    source: PieceSource::Original,
                    start: offset,
                    len: part.len(),
                });
            }
            line.len = part.len();
            original.push_str(part);
            lines.push(line);
            offset += part.len();
        }

        let len = compute_len(&lines);
        Self {
            inner: Arc::new(PieceTableInner {
                original: Arc::from(original),
                add: String::new(),
                lines,
                len,
            }),
        }
    }

    fn replace_text(&mut self, text: &str) {
        *self = Self::from_text(text);
    }

    fn insert_char(&mut self, cursor: Cursor, ch: char) -> Option<TextChange> {
        if ch == '\n' {
            return self.insert_newline(cursor);
        }
        let mut encoded = [0u8; 4];
        let text = ch.encode_utf8(&mut encoded);
        self.insert_text(cursor, text)
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

        let inner = self.inner_mut();
        let segments = Self::append_text_lines(inner, text);
        let old_line_count = inner.lines.len();

        let line = inner.lines.get(cursor.line)?.clone();
        let (left, right) = Self::split_line_pieces(&line, cursor.col);

        if segments.len() == 1 {
            let mut new_line = left;
            new_line.pieces.extend(segments[0].pieces.clone());
            new_line.pieces.extend(right.pieces);
            new_line.len = new_line.pieces.iter().map(|piece| piece.len).sum();
            Self::merge_line_pieces(&mut new_line);
            inner.lines[cursor.line] = new_line;
        } else {
            let mut new_lines = Vec::with_capacity(segments.len());

            let mut first_line = left;
            first_line.pieces.extend(segments[0].pieces.clone());
            first_line.len = first_line.pieces.iter().map(|piece| piece.len).sum();
            Self::merge_line_pieces(&mut first_line);
            new_lines.push(first_line);

            for segment in &segments[1..segments.len() - 1] {
                let mut line = segment.clone();
                Self::merge_line_pieces(&mut line);
                new_lines.push(line);
            }

            let mut last_line = segments.last().cloned().unwrap_or_else(Line::empty);
            last_line.pieces.extend(right.pieces);
            last_line.len = last_line.pieces.iter().map(|piece| piece.len).sum();
            Self::merge_line_pieces(&mut last_line);
            new_lines.push(last_line);

            inner.lines.splice(cursor.line..=cursor.line, new_lines);
        }

        inner.len = compute_len(&inner.lines);
        Some(TextChange::new(
            cursor.line,
            old_line_count,
            inner.lines.len(),
        ))
    }

    fn remove(&mut self, start: Cursor, end: Cursor) -> Option<TextChange> {
        if start > end || start == end || !self.valid_cursor(start) || !self.valid_cursor(end) {
            return None;
        }

        let inner = self.inner_mut();
        let old_line_count = inner.lines.len();

        if start.line == end.line {
            let line = inner.lines.get(start.line)?.clone();
            let (left, mid_right) = Self::split_line_pieces(&line, start.col);
            let (_, right) = Self::split_line_pieces(&mid_right, end.col.saturating_sub(start.col));
            let mut new_line = left;
            new_line.pieces.extend(right.pieces);
            new_line.len = new_line.pieces.iter().map(|piece| piece.len).sum();
            Self::merge_line_pieces(&mut new_line);
            inner.lines[start.line] = new_line;
        } else {
            let start_line = inner.lines.get(start.line)?.clone();
            let end_line = inner.lines.get(end.line)?.clone();
            let (left, _) = Self::split_line_pieces(&start_line, start.col);
            let (_, right) = Self::split_line_pieces(&end_line, end.col);

            let mut merged_line = left;
            merged_line.pieces.extend(right.pieces);
            merged_line.len = merged_line.pieces.iter().map(|piece| piece.len).sum();
            Self::merge_line_pieces(&mut merged_line);

            inner
                .lines
                .splice(start.line..=end.line, std::iter::once(merged_line));
        }

        if inner.lines.is_empty() {
            inner.lines.push(Line::empty());
        }

        inner.len = compute_len(&inner.lines);
        Some(TextChange::new(
            start.line,
            old_line_count,
            inner.lines.len(),
        ))
    }

    fn delete_lines(&mut self, start_line: usize, count: usize) -> Option<TextChange> {
        let inner = self.inner_mut();
        let total_lines = inner.lines.len();
        if start_line >= total_lines {
            return None;
        }

        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(TextChange::new(start_line, total_lines, total_lines));
        }

        let end_line = start_line + actual_count;
        if end_line >= total_lines {
            let mut prefix = inner.lines.drain(..start_line).collect::<Vec<_>>();
            if prefix.is_empty() {
                prefix.push(Line::empty());
            }
            inner.lines = prefix;
        } else {
            inner.lines.drain(start_line..end_line);
            if inner.lines.is_empty() {
                inner.lines.push(Line::empty());
            }
        }

        inner.len = compute_len(&inner.lines);
        Some(TextChange::new(start_line, total_lines, inner.lines.len()))
    }

    fn change_lines(&mut self, start_line: usize, count: usize) -> Option<TextChange> {
        let inner = self.inner_mut();
        let total_lines = inner.lines.len();
        if start_line >= total_lines {
            return None;
        }

        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(TextChange::new(start_line, total_lines, total_lines));
        }

        let end_line = start_line + actual_count;
        if end_line >= total_lines {
            let mut prefix = inner.lines.drain(..start_line).collect::<Vec<_>>();
            prefix.push(Line::empty());
            inner.lines = prefix;
        } else {
            inner.lines.drain(start_line..end_line);
            inner.lines.insert(start_line, Line::empty());
        }

        inner.len = compute_len(&inner.lines);
        Some(TextChange::new(start_line, total_lines, inner.lines.len()))
    }

    fn insert_blank_lines_after(&mut self, line: usize, count: usize) -> Option<TextChange> {
        let inner = self.inner_mut();
        let total_lines = inner.lines.len();
        if count == 0 {
            return Some(TextChange::new(line, total_lines, total_lines));
        }

        let insert_at = if line >= total_lines {
            total_lines
        } else {
            line + 1
        };
        inner.lines.splice(
            insert_at..insert_at,
            std::iter::repeat_with(Line::empty).take(count),
        );
        inner.len = compute_len(&inner.lines);
        Some(TextChange::new(insert_at, total_lines, inner.lines.len()))
    }

    fn insert_blank_lines_before(&mut self, line: usize, count: usize) -> Option<TextChange> {
        let inner = self.inner_mut();
        let total_lines = inner.lines.len();
        if count == 0 {
            return Some(TextChange::new(line, total_lines, total_lines));
        }

        let insert_at = line.min(total_lines);
        inner.lines.splice(
            insert_at..insert_at,
            std::iter::repeat_with(Line::empty).take(count),
        );
        inner.len = compute_len(&inner.lines);
        Some(TextChange::new(insert_at, total_lines, inner.lines.len()))
    }

    fn paste_linewise<I>(&mut self, line: usize, lines: I, after: bool) -> Option<TextChange>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let contents: Vec<String> = lines
            .into_iter()
            .map(|line| line.as_ref().to_owned())
            .collect();
        if contents.is_empty() {
            return Some(TextChange::new(line, self.line_count(), self.line_count()));
        }

        let inner = self.inner_mut();
        let total_lines = inner.lines.len();
        let insert_at = if after {
            (line + 1).min(total_lines)
        } else {
            line.min(total_lines)
        };
        let mut new_lines = Vec::with_capacity(contents.len());
        for content in contents {
            new_lines.push(line_from_text(&mut inner.add, &content));
        }
        inner.lines.splice(insert_at..insert_at, new_lines);
        inner.len = compute_len(&inner.lines);
        Some(TextChange::new(insert_at, total_lines, inner.lines.len()))
    }

    fn join_lines(
        &mut self,
        start_line: usize,
        line_count: usize,
        with_space: bool,
    ) -> Option<(Cursor, TextChange)> {
        let inner = self.inner_mut();
        if line_count < 2 || start_line >= inner.lines.len() {
            return None;
        }

        let total_lines = inner.lines.len();
        let actual_line_count = (total_lines - start_line).min(line_count);
        if actual_line_count < 2 {
            return None;
        }

        let mut joined = Line::empty();
        for idx in 0..actual_line_count {
            if idx > 0 && with_space {
                let start = inner.add.len();
                inner.add.push(' ');
                joined.push(Piece {
                    source: PieceSource::Add,
                    start,
                    len: 1,
                });
                joined.len += 1;
            }
            let line = inner.lines.get(start_line + idx)?.clone();
            joined.pieces.extend(line.pieces);
            joined.len += line.len;
        }

        Self::merge_line_pieces(&mut joined);
        let joined_len = joined.len;
        inner.lines.splice(
            start_line..start_line + actual_line_count,
            std::iter::once(joined),
        );
        inner.len = compute_len(&inner.lines);
        let change = TextChange::new(start_line, total_lines, inner.lines.len());
        Some((Cursor::new(start_line, joined_len), change))
    }
}

struct LineRangeChunks<'a> {
    table: &'a PieceTable,
    line_idx: usize,
    start_col: usize,
    end_col: usize,
    piece_idx: usize,
    piece_offset: usize,
}

impl<'a> LineRangeChunks<'a> {
    fn new(
        table: &'a PieceTable,
        line_idx: usize,
        start_col: usize,
        end_col: usize,
    ) -> Option<Self> {
        table.inner.lines.get(line_idx)?;
        Some(Self {
            table,
            line_idx,
            start_col,
            end_col,
            piece_idx: 0,
            piece_offset: 0,
        })
    }
}

impl<'a> Iterator for LineRangeChunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.table.inner.lines.get(self.line_idx)?;
        while self.piece_idx < line.pieces.len() {
            let piece = &line.pieces[self.piece_idx];
            let piece_start = self.piece_offset;
            let piece_end = piece_start + piece.len;

            if piece_end <= self.start_col {
                self.piece_idx += 1;
                self.piece_offset = piece_end;
                continue;
            }
            if piece_start >= self.end_col {
                return None;
            }

            let text = self.table.piece_text(piece);
            let local_start = self.start_col.saturating_sub(piece_start);
            let local_end = (self.end_col.min(piece_end)) - piece_start;
            self.piece_idx += 1;
            self.piece_offset = piece_end;
            return text.get(local_start..local_end);
        }

        None
    }
}

struct PieceTableRefChunks<'a> {
    text: PieceTableRef<'a>,
    current_line: usize,
    current_chunks: Option<LineRangeChunks<'a>>,
    pending_newline: bool,
    done: bool,
}

impl<'a> PieceTableRefChunks<'a> {
    fn new(text: PieceTableRef<'a>) -> Self {
        let current_chunks = LineRangeChunks::new(
            text.text,
            text.start.line,
            text.start.col,
            if text.start.line == text.end.line {
                text.end.col
            } else {
                text.text.line_len(text.start.line)
            },
        );
        Self {
            text,
            current_line: text.start.line,
            current_chunks,
            pending_newline: false,
            done: false,
        }
    }
}

impl<'a> Iterator for PieceTableRefChunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.done {
                return None;
            }

            if self.pending_newline {
                self.pending_newline = false;
                return Some("\n");
            }

            if let Some(chunks) = self.current_chunks.as_mut()
                && let Some(chunk) = chunks.next()
            {
                return Some(chunk);
            }

            if self.current_line == self.text.end.line {
                self.done = true;
                return None;
            }

            self.current_line += 1;
            let end_col = if self.current_line == self.text.end.line {
                self.text.end.col
            } else {
                self.text.text.line_len(self.current_line)
            };
            self.current_chunks =
                LineRangeChunks::new(self.text.text, self.current_line, 0, end_col);
            self.pending_newline = true;
        }
    }
}

impl Line {
    fn empty() -> Self {
        Self {
            pieces: Vec::new(),
            len: 0,
        }
    }

    fn push(&mut self, piece: Piece) {
        self.len += piece.len;
        self.pieces.push(piece);
    }
}

impl Piece {
    fn slice(&self, start: usize, len: usize) -> Self {
        Self {
            source: self.source,
            start: self.start + start,
            len,
        }
    }
}

fn line_from_text(add: &mut String, text: &str) -> Line {
    let mut line = Line::empty();
    if text.is_empty() {
        return line;
    }

    let start = add.len();
    add.push_str(text);
    line.push(Piece {
        source: PieceSource::Add,
        start,
        len: text.len(),
    });
    line
}

fn compute_len(lines: &[Line]) -> usize {
    lines.iter().map(|line| line.len).sum::<usize>() + lines.len().saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_has_one_empty_line() {
        let piece_table = PieceTable::new_empty();

        assert!(piece_table.is_empty());
        assert_eq!(piece_table.line_count(), 1);
        assert_eq!(piece_table.text().to_text(), "");
    }

    #[test]
    fn from_text_splits_lines_without_trailing_newlines() {
        let piece_table = PieceTable::from_text("alpha\nbeta\ngamma");

        assert_eq!(piece_table.line_count(), 3);
        assert_eq!(piece_table.len(), "alpha\nbeta\ngamma".len());
        assert_eq!(piece_table.text().to_text(), "alpha\nbeta\ngamma");
        assert_eq!(piece_table.line(1).expect("line").to_text(), "beta");
    }

    #[test]
    fn insert_and_remove_update_logical_text() {
        let mut piece_table = PieceTable::from_text("hello\nworld");

        assert!(piece_table.insert_char(Cursor::new(0, 5), '!').is_some());
        assert_eq!(piece_table.text().to_text(), "hello!\nworld");

        assert!(piece_table.insert_char(Cursor::new(1, 0), '\n').is_some());
        assert_eq!(piece_table.text().to_text(), "hello!\n\nworld");

        assert!(
            piece_table
                .remove(Cursor::new(0, 3), Cursor::new(2, 1))
                .is_some()
        );
        assert_eq!(piece_table.text().to_text(), "helorld");
    }
}
