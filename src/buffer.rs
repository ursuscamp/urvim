//! Text buffer module backed by Vector<Arc<str>>.
//!
//! This module provides the `Buffer` type, a text buffer implementation built on
//! top of imbl's Vector with Arc<str> for each line. The buffer supports
//! efficient text manipulation with proper Unicode handling including grapheme
//! clusters, combining characters, and emoji.
//!
//! # Features
//!
//! - **Efficient text editing**: Insert, remove, and modify text at any position
//! - **Unicode support**: Full support for grapheme clusters, combining characters,
//!   emoji, CJK characters, and other Unicode text
//! - **Line-based operations**: Navigate and manipulate text by lines
//! - **File I/O**: Load from and save to files
//! - **Display width calculation**: Calculate visual width of text for terminal display
//!
//! # Example
//!
//! ```
//! use urvim::buffer::{Buffer, Cursor};
//!
//! // Create a new buffer
//! let mut buf = Buffer::new();
//!
//! // Insert text
//! buf.insert_text(Cursor::new(0, 0), "Hello, 世界! 😀");
//!
//! // Get line count
//! println!("Lines: {}", buf.line_count());
//!
//! // Get a specific line
//! if let Some(line) = buf.line_at(0) {
//!     println!("Line content: {}", line);
//! }
//! ```

use crate::path::AbsolutePath;
use imbl::Vector;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Cursor position in the buffer.
///
/// Line and column (byte position within line).
/// Column can be from 0 to line byte length (inclusive, meaning cursor is at end of line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}

impl Cursor {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// A text buffer backed by a Vector of Arc<str> lines.
///
/// Buffer provides efficient text editing with proper Unicode support.
/// Each line is stored as an Arc<str> without trailing newline characters.
/// Newlines exist implicitly between lines.
///
/// # Example
///
/// ```
/// use urvim::buffer::{Buffer, Cursor};
///
/// let mut buf = Buffer::from_str("Hello, World!");
/// buf.insert_text(Cursor::new(0, 7), "Beautiful ");
/// assert_eq!(buf.as_str(), "Hello, Beautiful World!");
/// ```
#[derive(Debug, Clone)]
pub struct Buffer {
    lines: Vector<Arc<str>>,
    path: Option<AbsolutePath>,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    /// Creates a new empty buffer.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::new();
    /// assert!(buf.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            lines: Vector::unit(Arc::from("")),
            path: None,
        }
    }

    /// Creates a buffer from a string slice.
    ///
    /// The text is split into lines by newline characters.
    /// Each line is stored WITHOUT its trailing newline.
    ///
    /// # Arguments
    ///
    /// * `text` - The initial text content
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Hello\nWorld");
    /// assert_eq!(buf.line_count(), 2);
    /// ```
    pub fn from_str(text: &str) -> Self {
        let lines: Vector<Arc<str>> = if text.is_empty() {
            Vector::unit(Arc::from(""))
        } else {
            text.lines().map(Arc::from).collect::<Vector<_>>()
        };
        Self { lines, path: None }
    }

    pub fn with_path(path: AbsolutePath) -> Self {
        Self {
            lines: Vector::unit(Arc::from("")),
            path: Some(path),
        }
    }

    pub fn from_str_with_path(text: &str, path: AbsolutePath) -> Self {
        let mut buf = Self::from_str(text);
        buf.path = Some(path);
        buf
    }

    /// Loads a buffer from a file.
    ///
    /// Reads the entire file contents into the buffer.
    /// The file is expected to be valid UTF-8.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to load
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not valid UTF-8.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use urvim::buffer::Buffer;
    /// use std::path::Path;
    ///
    /// let buf = Buffer::load_from_file(Path::new("example.txt")).unwrap();
    /// ```
    pub fn load_from_file(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let abs_path = AbsolutePath::from_path(path);
        Ok(Self::from_str_with_path(&contents, abs_path.unwrap()))
    }

    /// Saves the buffer contents to a file.
    ///
    /// Writes the entire buffer content to the specified file,
    /// overwriting any existing content.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to save to
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    /// use std::path::Path;
    ///
    /// let buf = Buffer::from_str("Hello, World!");
    /// buf.save_to_file(Path::new("output.txt")).unwrap();
    /// ```
    pub fn save_to_file(&self, path: &Path) -> io::Result<()> {
        let mut file = File::create(path)?;
        file.write_all(self.as_str().as_bytes())?;
        Ok(())
    }

    /// Returns the number of characters in the buffer.
    ///
    /// This counts all characters across all lines (excluding newlines between lines).
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Hello");
    /// assert_eq!(buf.len(), 5);
    /// ```
    pub fn len(&self) -> usize {
        self.lines.iter().map(|s| s.len()).sum::<usize>() + self.lines.len().saturating_sub(1)
    }

    /// Returns true if the buffer is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::new();
    /// assert!(buf.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines.get(0).map_or(true, |s| s.is_empty())
    }

    pub fn path(&self) -> Option<&AbsolutePath> {
        self.path.as_ref()
    }

    pub fn set_path(&mut self, path: AbsolutePath) {
        self.path = Some(path);
    }

    pub fn file_name(&self) -> Option<&std::ffi::OsStr> {
        self.path.as_ref().and_then(|p| p.file_name())
    }

    /// Gets the line at the specified index.
    ///
    /// Lines are 0-indexed. Each line does NOT include a trailing newline.
    ///
    /// # Arguments
    ///
    /// * `line_idx` - Line number (0-indexed)
    ///
    /// Returns None if the line index is out of bounds.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Line 1\nLine 2\n");
    /// let line = buf.line_at(0);
    /// assert!(line.is_some());
    /// ```
    pub fn line_at(&self, line_idx: usize) -> Option<&Arc<str>> {
        self.lines.get(line_idx)
    }

    /// Returns the number of lines in the buffer.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Line 1\nLine 2\nLine 3");
    /// assert_eq!(buf.line_count(), 3);
    /// ```
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns the buffer contents as a String.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Hello");
    /// assert_eq!(buf.as_str(), "Hello");
    /// ```
    pub fn as_str(&self) -> String {
        if self.lines.is_empty() {
            return String::new();
        }
        let mut result = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                result.push('\n');
            }
            result.push_str(line);
        }
        result
    }

    /// Inserts a single character at the specified position.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Cursor position (line and byte index) to insert at
    /// * `ch` - Character to insert
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("Hello");
    /// buf.insert_char(Cursor::new(0, 5), '!');
    /// assert_eq!(buf.as_str(), "Hello!");
    /// ```
    pub fn insert_char(&mut self, cursor: Cursor, ch: char) {
        let line_idx = cursor.line;
        let col = cursor.col;

        if ch == '\n' {
            let before = if let Some(line) = self.lines.get(line_idx) {
                line[..col].to_string()
            } else {
                String::new()
            };

            let after = if let Some(line) = self.lines.get(line_idx) {
                line[col..].to_string()
            } else {
                String::new()
            };

            let mut new_lines = Vec::new();
            new_lines.push(Arc::from(before));
            new_lines.push(Arc::from(after));

            let mut left = self.lines.take(line_idx);
            let right = self.lines.skip(line_idx + 1);
            let new: Vector<Arc<str>> = new_lines.into_iter().collect();
            left.append(new);
            left.append(right);
            self.lines = left;
        } else {
            if let Some(line) = self.lines.get(line_idx) {
                let mut new_line = (&*line).to_string();
                new_line.insert(col, ch);
                self.lines = self.lines.update(line_idx, Arc::from(new_line));
            }
        }
    }

    /// Inserts text at the specified position.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Cursor position (line and byte index) to insert at
    /// * `text` - Text to insert
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("Hello");
    /// buf.insert_text(Cursor::new(0, 5), " World");
    /// assert_eq!(buf.as_str(), "Hello World");
    /// ```
    pub fn insert_text(&mut self, mut cursor: Cursor, text: &str) {
        for ch in text.chars() {
            self.insert_char(cursor, ch);
            if ch == '\n' {
                cursor = Cursor::new(cursor.line + 1, 0);
            } else {
                cursor = Cursor::new(cursor.line, cursor.col + ch.len_utf8());
            }
        }
    }

    /// Removes a range of characters from the buffer.
    ///
    /// # Arguments
    ///
    /// * `start` - Start cursor position (inclusive)
    /// * `end` - End cursor position (exclusive)
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("Hello, World!");
    /// buf.remove(Cursor::new(0, 5), Cursor::new(0, 12));  // Remove ", World"
    /// assert_eq!(buf.as_str(), "Hello!");
    /// ```
    pub fn remove(&mut self, start: Cursor, end: Cursor) {
        if start.line > end.line || (start.line == end.line && start.col >= end.col) {
            return;
        }

        let start_line = start.line;
        let start_col = start.col;
        let end_line = end.line;
        let end_col = end.col;

        if start_line == end_line {
            if let Some(line) = self.lines.get(start_line) {
                let mut new_line = (&*line).to_string();
                new_line.drain(start_col..end_col);
                self.lines = self.lines.update(start_line, Arc::from(new_line));
            }
        } else {
            let before = if let Some(line) = self.lines.get(start_line) {
                line[..start_col].to_string()
            } else {
                String::new()
            };

            let after = if let Some(line) = self.lines.get(end_line) {
                line[end_col..].to_string()
            } else {
                String::new()
            };

            let merged = Arc::from(format!("{}{}", before, after));

            let mut left = self.lines.take(start_line);
            let right = self.lines.skip(end_line + 1);
            left.push_back(merged);
            left.append(right);
            self.lines = left;
        }
    }

    /// Returns the byte length of a line.
    ///
    /// # Arguments
    ///
    /// * `line_idx` - Line index (0-based)
    ///
    /// Returns 0 if line doesn't exist.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("hello\nworld");
    /// assert_eq!(buf.line_len(0), 5);
    /// assert_eq!(buf.line_len(1), 5);
    /// ```
    pub fn line_len(&self, line_idx: usize) -> usize {
        self.lines.get(line_idx).map_or(0, |s| s.len())
    }

    /// Checks if a cursor position is valid.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Cursor position to check
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("hello");
    /// assert!(buf.is_valid_cursor(Cursor::new(0, 0)));
    /// assert!(buf.is_valid_cursor(Cursor::new(0, 5)));  // at end of line
    /// assert!(!buf.is_valid_cursor(Cursor::new(0, 6))); // beyond line
    /// assert!(!buf.is_valid_cursor(Cursor::new(1, 0))); // beyond last line
    /// ```
    pub fn is_valid_cursor(&self, cursor: Cursor) -> bool {
        if cursor.line >= self.lines.len() {
            return false;
        }
        let line_len = self.line_len(cursor.line);
        if cursor.col > line_len {
            return false;
        }
        if cursor.col == line_len {
            return true;
        }
        if let Some(line) = self.lines.get(cursor.line) {
            line.is_char_boundary(cursor.col)
        } else {
            false
        }
    }

    /// Moves cursor right by one grapheme.
    ///
    /// Returns None if cursor is at end of last line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Starting cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("ab");
    /// let cursor = Cursor::new(0, 0);
    /// let next = buf.cursor_right(cursor);
    /// assert_eq!(next, Some(Cursor::new(0, 1)));
    /// ```
    pub fn cursor_right(&self, cursor: Cursor) -> Option<Cursor> {
        let line_len = self.line_len(cursor.line);

        if cursor.col < line_len {
            // Move within current line
            let line = self.lines.get(cursor.line)?;
            let line_str = line.as_ref();

            // Find next grapheme (skip current if at boundary)
            for (byte_offset, _grapheme) in line_str.grapheme_indices(true) {
                if byte_offset > cursor.col {
                    return Some(Cursor::new(cursor.line, byte_offset));
                }
            }
            // At last grapheme, move to end of line
            return Some(Cursor::new(cursor.line, line_len));
        } else if cursor.line < self.lines.len() - 1 {
            // Move to start of next line
            return Some(Cursor::new(cursor.line + 1, 0));
        } else {
            // At end of last line, stay in place
            return None;
        }
    }

    /// Moves cursor left by one grapheme.
    ///
    /// Returns None if cursor is at start of first line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Starting cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("ab");
    /// let cursor = Cursor::new(0, 1);
    /// let prev = buf.cursor_left(cursor);
    /// assert_eq!(prev, Some(Cursor::new(0, 0)));
    /// ```
    pub fn cursor_left(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.col > 0 {
            // Move within current line
            let line = self.lines.get(cursor.line)?;
            let line_str = line.as_ref();

            // Find previous grapheme start
            let mut prev_offset = 0;
            for (byte_offset, _grapheme) in line_str.grapheme_indices(true) {
                if byte_offset >= cursor.col {
                    return Some(Cursor::new(cursor.line, prev_offset));
                }
                prev_offset = byte_offset;
            }
            // Should not reach here if cursor.col > 0 and <= line_len
            return Some(Cursor::new(cursor.line, prev_offset));
        } else if cursor.line > 0 {
            // Move to end of previous line
            let prev_line_len = self.line_len(cursor.line - 1);
            return Some(Cursor::new(cursor.line - 1, prev_line_len));
        } else {
            // At start of first line, stay in place
            return None;
        }
    }

    /// Moves cursor down to the next line, preserving visual column.
    ///
    /// Returns None if cursor is on the last line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Starting cursor position
    /// * `visual_col` - Target visual column to preserve
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("ab\ncd");
    /// let cursor = Cursor::new(0, 1);
    /// let down = buf.cursor_down(cursor, 1);
    /// assert_eq!(down, Some(Cursor::new(1, 1)));
    /// ```
    pub fn cursor_down(&self, cursor: Cursor, visual_col: usize) -> Option<Cursor> {
        if cursor.line >= self.lines.len() - 1 {
            return None;
        }

        let next_line = cursor.line + 1;
        let target_col = self.byte_pos_at_visual_col(next_line, visual_col);

        Some(Cursor::new(next_line, target_col))
    }

    /// Moves cursor up to the previous line, preserving visual column.
    ///
    /// Returns None if cursor is on the first line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Starting cursor position
    /// * `visual_col` - Target visual column to preserve
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("ab\ncd");
    /// let cursor = Cursor::new(1, 1);
    /// let up = buf.cursor_up(cursor, 1);
    /// assert_eq!(up, Some(Cursor::new(0, 1)));
    /// ```
    pub fn cursor_up(&self, cursor: Cursor, visual_col: usize) -> Option<Cursor> {
        if cursor.line == 0 {
            return None;
        }

        let prev_line = cursor.line - 1;
        let target_col = self.byte_pos_at_visual_col(prev_line, visual_col);

        Some(Cursor::new(prev_line, target_col))
    }

    /// Returns the visual column (display width) at the cursor position.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("a😀c");
    /// assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
    /// assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
    /// assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 3);
    /// ```
    pub fn visual_col_at(&self, cursor: Cursor) -> usize {
        let line = match self.lines.get(cursor.line) {
            Some(l) => l.as_ref(),
            None => return 0,
        };

        let mut visual_col = 0;
        let mut byte_offset = 0;

        for grapheme in line.graphemes(true) {
            if byte_offset >= cursor.col {
                break;
            }
            visual_col += grapheme_width(grapheme);
            byte_offset += grapheme.len();
        }

        visual_col
    }

    /// Returns the byte position at the given visual column.
    ///
    /// If visual column is beyond end of line, returns line byte length.
    ///
    /// # Arguments
    ///
    /// * `line_idx` - Line index
    /// * `visual_col` - Target visual column
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("a😀c");
    /// assert_eq!(buf.byte_pos_at_visual_col(0, 0), 0);
    /// assert_eq!(buf.byte_pos_at_visual_col(0, 1), 1);
    /// assert_eq!(buf.byte_pos_at_visual_col(0, 2), 1);  // middle of emoji
    /// assert_eq!(buf.byte_pos_at_visual_col(0, 3), 5);
    /// ```
    pub fn byte_pos_at_visual_col(&self, line_idx: usize, visual_col: usize) -> usize {
        let line = match self.lines.get(line_idx) {
            Some(l) => l.as_ref(),
            None => return 0,
        };

        let mut current_visual = 0;
        let mut byte_offset = 0;

        for grapheme in line.graphemes(true) {
            let gwidth = grapheme_width(grapheme);
            if current_visual + gwidth > visual_col {
                // Stop at this grapheme
                return byte_offset;
            }
            current_visual += gwidth;
            byte_offset += grapheme.len();
        }

        // Beyond all graphemes, return end of line
        line.len()
    }
}

/// Calculates the display width of a single character.
///
/// Uses Unicode Annex #11 rules for character width.
/// Returns 0 for control characters, 1 for narrow characters,
/// and 2 for wide characters (CJK, emoji, etc.).
///
/// # Example
///
/// ```
/// use urvim::buffer::char_width;
///
/// assert_eq!(char_width('a'), 1);
/// assert_eq!(char_width('中'), 2);
/// assert_eq!(char_width('😀'), 2);
/// ```
pub fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// Calculates the display width of a string.
///
/// Uses Unicode Annex #11 rules for character width.
/// This counts the total display width of all characters.
///
/// # Example
///
/// ```
/// use urvim::buffer::str_width;
///
/// assert_eq!(str_width("hello"), 5);
/// assert_eq!(str_width("你好"), 4);
/// ```
pub fn str_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Calculates the display width of a grapheme cluster.
///
/// # Example
///
/// ```
/// use urvim::buffer::grapheme_width;
///
/// assert_eq!(grapheme_width("a"), 1);
/// assert_eq!(grapheme_width("😀"), 2);
/// ```
pub fn grapheme_width(grapheme: &str) -> usize {
    UnicodeWidthStr::width(grapheme)
}

/// Converts a character index to a byte index.
///
/// # Arguments
///
/// * `char_idx` - Character position in the text
/// * `text` - The text to index into
///
/// # Example
///
/// ```
/// use urvim::buffer::to_byte_index;
///
/// let text = "aβc";
/// // 'a' = byte 0, 'β' = bytes 1-2, 'c' = byte 3
/// assert_eq!(to_byte_index(2, text), 3);
/// ```
pub fn to_byte_index(char_idx: usize, text: &str) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buf = Buffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.as_str(), "");
    }

    #[test]
    fn test_from_str() {
        let buf = Buffer::from_str("hello");
        assert!(!buf.is_empty());
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.as_str(), "hello");
    }

    #[test]
    fn test_from_str_multiline() {
        let buf = Buffer::from_str("hello\nworld");
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "hello\nworld");
    }

    #[test]
    fn test_from_str_trailing_newline() {
        let buf = Buffer::from_str("hello\n");
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.as_str(), "hello");
    }

    #[test]
    fn test_insert_char() {
        let mut buf = Buffer::from_str("hello");
        buf.insert_char(Cursor::new(0, 5), '!');
        assert_eq!(buf.as_str(), "hello!");
    }

    #[test]
    fn test_insert_text() {
        let mut buf = Buffer::from_str("hello");
        buf.insert_text(Cursor::new(0, 5), " world");
        assert_eq!(buf.as_str(), "hello world");
    }

    #[test]
    fn test_insert_at_beginning() {
        let mut buf = Buffer::from_str("world");
        buf.insert_text(Cursor::new(0, 0), "hello ");
        assert_eq!(buf.as_str(), "hello world");
    }

    #[test]
    fn test_insert_in_middle() {
        let mut buf = Buffer::from_str("hello");
        buf.insert_text(Cursor::new(0, 2), "XX");
        assert_eq!(buf.as_str(), "heXXllo");
    }

    #[test]
    fn test_insert_with_newline() {
        let mut buf = Buffer::from_str("hello");
        buf.insert_text(Cursor::new(0, 2), "X\nY");
        assert_eq!(buf.as_str(), "heX\nYllo");
        assert_eq!(buf.line_count(), 2);
    }

    #[test]
    fn test_remove() {
        let mut buf = Buffer::from_str("hello world");
        buf.remove(Cursor::new(0, 5), Cursor::new(0, 11));
        assert_eq!(buf.as_str(), "hello");
    }

    #[test]
    fn test_remove_from_beginning() {
        let mut buf = Buffer::from_str("hello");
        buf.remove(Cursor::new(0, 0), Cursor::new(0, 2));
        assert_eq!(buf.as_str(), "llo");
    }

    #[test]
    fn test_remove_multiline() {
        let mut buf = Buffer::from_str("hello\nworld");
        buf.remove(Cursor::new(0, 2), Cursor::new(1, 2));
        assert_eq!(buf.as_str(), "herld");
    }

    #[test]
    fn test_line_count() {
        let buf = Buffer::from_str("line1\nline2\nline3");
        assert_eq!(buf.line_count(), 3);
    }

    #[test]
    fn test_line_count_single_line() {
        let buf = Buffer::from_str("hello");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_line_count_empty() {
        let buf = Buffer::new();
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_line_at() {
        let buf = Buffer::from_str("line1\nline2\nline3");
        assert_eq!(buf.line_at(0).map(|s| s.as_ref() as &str), Some("line1"));
        assert_eq!(buf.line_at(1).map(|s| s.as_ref() as &str), Some("line2"));
        assert_eq!(buf.line_at(2).map(|s| s.as_ref() as &str), Some("line3"));
    }

    #[test]
    fn test_line_at_out_of_bounds() {
        let buf = Buffer::from_str("hello");
        assert!(buf.line_at(1).is_none());
    }

    #[test]
    fn test_line_grapheme_len() {
        let buf = Buffer::from_str("a😀c\n");
        assert_eq!(buf.line_at(0).map(|s| str_width(s.as_ref())), Some(4));
    }

    #[test]
    fn test_save_and_load() {
        let buf = Buffer::from_str("hello world");
        buf.save_to_file(std::path::Path::new("/tmp/test_buffer.txt"))
            .unwrap();

        let loaded = Buffer::load_from_file(std::path::Path::new("/tmp/test_buffer.txt")).unwrap();
        assert_eq!(loaded.as_str(), "hello world");

        std::fs::remove_file("/tmp/test_buffer.txt").ok();
    }

    #[test]
    fn test_multiline_with_empty_lines() {
        let buf = Buffer::from_str("a\n\nb");
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.line_at(0).map(|s| s.as_ref() as &str), Some("a"));
        assert_eq!(buf.line_at(1).map(|s| s.as_ref() as &str), Some(""));
        assert_eq!(buf.line_at(2).map(|s| s.as_ref() as &str), Some("b"));
    }

    #[test]
    fn test_remove_all() {
        let mut buf = Buffer::from_str("hello");
        buf.remove(Cursor::new(0, 0), Cursor::new(0, 5));
        assert!(buf.is_empty());
        assert_eq!(buf.as_str(), "");
    }

    #[test]
    fn test_insert_into_empty() {
        let mut buf = Buffer::new();
        buf.insert_text(Cursor::new(0, 0), "test");
        assert_eq!(buf.as_str(), "test");
    }

    #[test]
    fn test_line_with_tab() {
        let buf = Buffer::from_str("a\tb");
        assert_eq!(buf.line_at(0).map(|s| s.len()), Some(3));
    }

    #[test]
    fn test_char_width_ascii() {
        assert_eq!(char_width('a'), 1);
        assert_eq!(char_width('z'), 1);
    }

    #[test]
    fn test_char_width_cjk() {
        assert_eq!(char_width('中'), 2);
        assert_eq!(char_width('日'), 2);
    }

    #[test]
    fn test_char_width_narrow() {
        assert_eq!(char_width('\t'), 0);
    }

    #[test]
    fn test_str_width() {
        assert_eq!(str_width("hello"), 5);
        assert_eq!(str_width("helło"), 5);
        assert_eq!(str_width("你好"), 4);
        assert_eq!(str_width("😀"), 2);
    }

    #[test]
    fn test_grapheme_width() {
        assert_eq!(grapheme_width("a"), 1);
        assert_eq!(grapheme_width("😀"), 2);
        assert_eq!(grapheme_width("中"), 2);
    }

    #[test]
    fn test_visual_col_at() {
        let buf = Buffer::from_str("a😀c");
        assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 3);
    }

    #[test]
    fn test_buffer_len() {
        let buf = Buffer::from_str("abc\ndef");
        assert_eq!(buf.len(), 7); // 3 + 1 + 3
    }

    // Cursor tests

    #[test]
    fn test_cursor_new() {
        let cursor = Cursor::new(0, 0);
        assert_eq!(cursor.line, 0);
        assert_eq!(cursor.col, 0);
    }

    #[test]
    fn test_cursor_default() {
        let cursor = Cursor::default();
        assert_eq!(cursor, Cursor::new(0, 0));
    }

    #[test]
    fn test_cursor_partial_eq() {
        let c1 = Cursor::new(0, 5);
        let c2 = Cursor::new(0, 5);
        let c3 = Cursor::new(1, 5);
        assert_eq!(c1, c2);
        assert_ne!(c1, c3);
    }

    #[test]
    fn test_is_valid_cursor() {
        let buf = Buffer::from_str("hello");
        assert!(buf.is_valid_cursor(Cursor::new(0, 0)));
        assert!(buf.is_valid_cursor(Cursor::new(0, 3)));
        assert!(buf.is_valid_cursor(Cursor::new(0, 5))); // at end
        assert!(!buf.is_valid_cursor(Cursor::new(0, 6))); // beyond line
        assert!(!buf.is_valid_cursor(Cursor::new(1, 0))); // beyond last line
    }

    #[test]
    fn test_is_valid_cursor_multiline() {
        let buf = Buffer::from_str("hello\nworld");
        assert!(buf.is_valid_cursor(Cursor::new(0, 0)));
        assert!(buf.is_valid_cursor(Cursor::new(0, 5)));
        assert!(buf.is_valid_cursor(Cursor::new(1, 0)));
        assert!(buf.is_valid_cursor(Cursor::new(1, 5)));
        assert!(!buf.is_valid_cursor(Cursor::new(1, 6)));
        assert!(!buf.is_valid_cursor(Cursor::new(2, 0)));
    }

    // cursor_right tests

    #[test]
    fn test_cursor_right_ascii() {
        let buf = Buffer::from_str("hello");

        assert_eq!(buf.cursor_right(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        assert_eq!(buf.cursor_right(Cursor::new(0, 1)), Some(Cursor::new(0, 2)));
        assert_eq!(buf.cursor_right(Cursor::new(0, 4)), Some(Cursor::new(0, 5)));
        assert_eq!(buf.cursor_right(Cursor::new(0, 5)), None); // at end of line, last line
    }

    #[test]
    fn test_cursor_right_multibyte() {
        let buf = Buffer::from_str("aβc"); // 'β' is 2 bytes

        assert_eq!(buf.cursor_right(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        assert_eq!(buf.cursor_right(Cursor::new(0, 1)), Some(Cursor::new(0, 3))); // jump over β
        assert_eq!(buf.cursor_right(Cursor::new(0, 3)), Some(Cursor::new(0, 4)));
    }

    #[test]
    fn test_cursor_right_emoji() {
        let buf = Buffer::from_str("a😀c"); // emoji is 4 bytes

        assert_eq!(buf.cursor_right(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        assert_eq!(buf.cursor_right(Cursor::new(0, 1)), Some(Cursor::new(0, 5))); // jump over emoji
        assert_eq!(buf.cursor_right(Cursor::new(0, 5)), Some(Cursor::new(0, 6)));
    }

    #[test]
    fn test_cursor_right_across_newline() {
        let buf = Buffer::from_str("ab\ncd");

        // "ab" has byte len 2, "cd" has byte len 2
        assert_eq!(buf.cursor_right(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        assert_eq!(buf.cursor_right(Cursor::new(0, 1)), Some(Cursor::new(0, 2)));
        assert_eq!(buf.cursor_right(Cursor::new(0, 2)), Some(Cursor::new(1, 0))); // cross newline
        assert_eq!(buf.cursor_right(Cursor::new(1, 0)), Some(Cursor::new(1, 1)));
        // At col 2 (end of "cd"), moving right goes past end -> None
        assert_eq!(buf.cursor_right(Cursor::new(1, 2)), None);
    }

    #[test]
    fn test_cursor_right_at_end_of_last_line() {
        let buf = Buffer::from_str("ab\ncd");

        // At end of last line, moving right stays in place (returns None)
        assert_eq!(buf.cursor_right(Cursor::new(1, 2)), None);
    }

    // cursor_left tests

    #[test]
    fn test_cursor_left_ascii() {
        let buf = Buffer::from_str("hello");

        assert_eq!(buf.cursor_left(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
        assert_eq!(buf.cursor_left(Cursor::new(0, 5)), Some(Cursor::new(0, 4)));
        assert_eq!(buf.cursor_left(Cursor::new(0, 0)), None); // at start
    }

    #[test]
    fn test_cursor_left_multibyte() {
        let buf = Buffer::from_str("aβc"); // 'β' is 2 bytes

        assert_eq!(buf.cursor_left(Cursor::new(0, 3)), Some(Cursor::new(0, 1))); // jump over β
        assert_eq!(buf.cursor_left(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
    }

    #[test]
    fn test_cursor_left_emoji() {
        let buf = Buffer::from_str("a😀c"); // emoji is 4 bytes

        assert_eq!(buf.cursor_left(Cursor::new(0, 5)), Some(Cursor::new(0, 1))); // jump over emoji
        assert_eq!(buf.cursor_left(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
    }

    #[test]
    fn test_cursor_left_across_newline() {
        let buf = Buffer::from_str("ab\ncd");

        assert_eq!(buf.cursor_left(Cursor::new(1, 0)), Some(Cursor::new(0, 2))); // cross newline
        assert_eq!(buf.cursor_left(Cursor::new(0, 2)), Some(Cursor::new(0, 1)));
    }

    #[test]
    fn test_cursor_left_at_start() {
        let buf = Buffer::from_str("ab");

        assert_eq!(buf.cursor_left(Cursor::new(0, 0)), None);
    }

    // cursor_down tests

    #[test]
    fn test_cursor_down_preserves_visual_col() {
        let buf = Buffer::from_str("ab\ncd");

        assert_eq!(
            buf.cursor_down(Cursor::new(0, 0), 0),
            Some(Cursor::new(1, 0))
        );
        assert_eq!(
            buf.cursor_down(Cursor::new(0, 1), 1),
            Some(Cursor::new(1, 1))
        );
        assert_eq!(
            buf.cursor_down(Cursor::new(0, 2), 2),
            Some(Cursor::new(1, 2))
        );
    }

    #[test]
    fn test_cursor_down_with_emoji() {
        let buf = Buffer::from_str("a😀\nb");

        // a😀 has visual width 3 (1 + 2), b has visual width 1
        // visual col 1 should map to byte 1 (after 'a')
        assert_eq!(
            buf.cursor_down(Cursor::new(0, 0), 0),
            Some(Cursor::new(1, 0))
        );
        assert_eq!(
            buf.cursor_down(Cursor::new(0, 1), 1),
            Some(Cursor::new(1, 1))
        ); // after 'a'
           // visual col 2 would be in middle of emoji, should clamp to end of next line
        assert_eq!(
            buf.cursor_down(Cursor::new(0, 5), 3),
            Some(Cursor::new(1, 1))
        ); // end of "b"
    }

    #[test]
    fn test_cursor_down_short_line_clamps() {
        let buf = Buffer::from_str("ab\nc");

        // Line 0 has "ab" (2 chars), Line 1 has "c" (1 char)
        // From col 2 on line 0, going down should clamp to col 1 (end of line 1)
        assert_eq!(
            buf.cursor_down(Cursor::new(0, 2), 2),
            Some(Cursor::new(1, 1))
        );
    }

    #[test]
    fn test_cursor_down_at_last_line() {
        let buf = Buffer::from_str("ab\ncd");

        // At last line, should return None
        assert_eq!(buf.cursor_down(Cursor::new(1, 0), 0), None);
    }

    // cursor_up tests

    #[test]
    fn test_cursor_up_preserves_visual_col() {
        let buf = Buffer::from_str("ab\ncd");

        assert_eq!(buf.cursor_up(Cursor::new(1, 0), 0), Some(Cursor::new(0, 0)));
        assert_eq!(buf.cursor_up(Cursor::new(1, 1), 1), Some(Cursor::new(0, 1)));
        assert_eq!(buf.cursor_up(Cursor::new(1, 2), 2), Some(Cursor::new(0, 2)));
    }

    #[test]
    fn test_cursor_up_with_emoji() {
        let buf = Buffer::from_str("a\nb😀");

        // Going up from line 1 should preserve visual column
        assert_eq!(buf.cursor_up(Cursor::new(1, 0), 0), Some(Cursor::new(0, 0)));
        assert_eq!(buf.cursor_up(Cursor::new(1, 1), 1), Some(Cursor::new(0, 1)));
    }

    #[test]
    fn test_cursor_up_short_line_clamps() {
        let buf = Buffer::from_str("ab\nc");

        // Line 0 has "ab" (2 chars), Line 1 has "c" (1 char)
        // From col 1 on line 1, going up should stay at col 1
        assert_eq!(buf.cursor_up(Cursor::new(1, 1), 1), Some(Cursor::new(0, 1)));
    }

    #[test]
    fn test_cursor_up_at_first_line() {
        let buf = Buffer::from_str("ab\ncd");

        // At first line, should return None
        assert_eq!(buf.cursor_up(Cursor::new(0, 0), 0), None);
    }

    // visual_col_at tests

    #[test]
    fn test_visual_col_at_cursor() {
        let buf = Buffer::from_str("a😀c");

        assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 3);
    }

    #[test]
    fn test_visual_col_at_multiline() {
        let buf = Buffer::from_str("ab\ncd");

        assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 2)), 2);
        assert_eq!(buf.visual_col_at(Cursor::new(1, 0)), 0);
    }

    // byte_pos_at_visual_col tests

    #[test]
    fn test_byte_pos_at_visual_col() {
        let buf = Buffer::from_str("a😀c");

        assert_eq!(buf.byte_pos_at_visual_col(0, 0), 0);
        assert_eq!(buf.byte_pos_at_visual_col(0, 1), 1);
        assert_eq!(buf.byte_pos_at_visual_col(0, 2), 1); // middle of emoji
        assert_eq!(buf.byte_pos_at_visual_col(0, 3), 5);
        assert_eq!(buf.byte_pos_at_visual_col(0, 10), 6); // beyond line
    }

    // line_len tests

    #[test]
    fn test_line_len() {
        let buf = Buffer::from_str("hello\nworld");

        assert_eq!(buf.line_len(0), 5);
        assert_eq!(buf.line_len(1), 5);
    }

    #[test]
    fn test_line_len_out_of_bounds() {
        let buf = Buffer::from_str("hello");

        assert_eq!(buf.line_len(1), 0);
    }

    #[test]
    fn test_insert_char_ascii_cursor_mapping() {
        let mut buf = Buffer::new();
        let cursor = Cursor::new(0, 0);

        buf.insert_char(cursor, 'h');
        buf.insert_char(Cursor::new(0, 1), 'e');
        buf.insert_char(Cursor::new(0, 2), 'l');
        buf.insert_char(Cursor::new(0, 3), 'l');
        buf.insert_char(Cursor::new(0, 4), 'o');

        assert_eq!(buf.as_str(), "hello");
        assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 2)), 2);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 3)), 3);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 4)), 4);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 5);
    }

    #[test]
    fn test_insert_char_wide_cursor_mapping() {
        let mut buf = Buffer::new();
        let cursor = Cursor::new(0, 0);

        buf.insert_char(cursor, '日');
        buf.insert_char(Cursor::new(0, 3), '本');
        buf.insert_char(Cursor::new(0, 6), '語');

        assert_eq!(buf.as_str(), "日本語");
        assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 3)), 2);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 6)), 4);
    }

    #[test]
    fn test_insert_char_emoji_cursor_mapping() {
        let mut buf = Buffer::new();
        let cursor = Cursor::new(0, 0);

        buf.insert_char(cursor, 'a');
        buf.insert_char(Cursor::new(0, 1), '😀');
        buf.insert_char(Cursor::new(0, 5), 'b');

        assert_eq!(buf.as_str(), "a😀b");
        assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 3);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 6)), 4);
    }

    #[test]
    fn test_insert_newline_cursor_mapping() {
        let mut buf = Buffer::from_str("hello");
        let cursor = Cursor::new(0, 5);

        buf.insert_char(cursor, '\n');

        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "hello\n");
        assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 5);
        assert_eq!(buf.visual_col_at(Cursor::new(1, 0)), 0);
    }

    #[test]
    fn test_insert_newline_mid_line_cursor_mapping() {
        let mut buf = Buffer::from_str("hello");
        let cursor = Cursor::new(0, 2);

        buf.insert_char(cursor, '\n');

        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "he\nllo");
        assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 2)), 2);
        assert_eq!(buf.visual_col_at(Cursor::new(1, 0)), 0);
        assert_eq!(buf.visual_col_at(Cursor::new(1, 3)), 3);
    }

    #[test]
    fn test_insert_mixed_ascii_wide_cursor_mapping() {
        let mut buf = Buffer::new();

        buf.insert_char(Cursor::new(0, 0), 'a');
        buf.insert_char(Cursor::new(0, 1), '日');
        buf.insert_char(Cursor::new(0, 4), 'b');
        buf.insert_char(Cursor::new(0, 5), '本');
        buf.insert_char(Cursor::new(0, 8), 'c');

        assert_eq!(buf.as_str(), "a日b本c");
        assert_eq!(buf.visual_col_at(Cursor::new(0, 0)), 0);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 1)), 1);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 4)), 3);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 5)), 4);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 8)), 6);
        assert_eq!(buf.visual_col_at(Cursor::new(0, 9)), 7);
    }

    #[test]
    fn test_insert_between_wide_chars_via_cursor_movement() {
        let mut buf = Buffer::from_str("日本語");

        assert_eq!(buf.as_str(), "日本語");
        assert_eq!(buf.line_len(0), 9);

        let cursor_after_first_char = buf.cursor_right(Cursor::new(0, 0));
        assert_eq!(cursor_after_first_char, Some(Cursor::new(0, 3)));

        if let Some(cursor) = cursor_after_first_char {
            buf.insert_char(cursor, 'X');
        }

        assert_eq!(buf.as_str(), "日X本語");

        let cursor_after_insert = buf.cursor_right(cursor_after_first_char.unwrap());
        assert_eq!(cursor_after_insert, Some(Cursor::new(0, 4)));
    }

    #[test]
    fn test_insert_between_emoji_via_cursor_movement() {
        let mut buf = Buffer::from_str("😀😀");

        assert_eq!(buf.as_str(), "😀😀");
        assert_eq!(buf.line_len(0), 8);

        let cursor_after_first_emoji = buf.cursor_right(Cursor::new(0, 0));
        assert_eq!(cursor_after_first_emoji, Some(Cursor::new(0, 4)));

        if let Some(cursor) = cursor_after_first_emoji {
            buf.insert_char(cursor, 'X');
        }

        assert_eq!(buf.as_str(), "😀X😀");

        let cursor_after_insert = buf.cursor_right(cursor_after_first_emoji.unwrap());
        assert_eq!(cursor_after_insert, Some(Cursor::new(0, 5)));
    }

    #[test]
    fn test_insert_mid_emoji_via_cursor_movement() {
        let mut buf = Buffer::from_str("a😀b");

        assert_eq!(buf.as_str(), "a😀b");

        let cursor_after_emoji = buf.cursor_right(Cursor::new(0, 1));
        assert_eq!(cursor_after_emoji, Some(Cursor::new(0, 5)));

        if let Some(cursor) = cursor_after_emoji {
            buf.insert_char(cursor, 'X');
        }

        assert_eq!(buf.as_str(), "a😀Xb");
    }
}
