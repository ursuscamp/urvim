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

/// Represents different boundary types for text navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// Word boundary (alphanumeric + underscore)
    Word,
    /// End of word boundary
    WordEnd,
    /// BigWord boundary (non-whitespace)
    BigWord,
    /// End of BigWord boundary
    BigWordEnd,
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
    /// let buf = Buffer::new_from_str("Hello\nWorld");
    /// assert_eq!(buf.line_count(), 2);
    /// ```
    pub fn new_from_str(text: &str) -> Self {
        let lines: Vector<Arc<str>> = if text.is_empty() {
            Vector::unit(Arc::from(""))
        } else {
            text.lines().map(Arc::from).collect::<Vector<_>>()
        };
        Self { lines, path: None }
    }

    #[doc(hidden)]
    #[deprecated(since = "0.1.0", note = "use new_from_str instead")]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self::new_from_str(text)
    }

    pub fn with_path(path: AbsolutePath) -> Self {
        Self {
            lines: Vector::unit(Arc::from("")),
            path: Some(path),
        }
    }

    pub fn from_str_with_path(text: &str, path: AbsolutePath) -> Self {
        let mut buf = Self::new_from_str(text);
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
        self.lines.len() == 1 && self.lines.get(0).is_none_or(|s| s.is_empty())
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
        debug_assert!(
            self.is_valid_cursor(cursor),
            "insert_char called with invalid cursor: {:?}",
            cursor
        );

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

            let new_lines = vec![Arc::from(before), Arc::from(after)];

            let mut left = self.lines.take(line_idx);
            let right = self.lines.skip(line_idx + 1);
            let new: Vector<Arc<str>> = new_lines.into_iter().collect();
            left.append(new);
            left.append(right);
            self.lines = left;
        } else if let Some(line) = self.lines.get(line_idx) {
            let mut new_line = line.to_string();
            new_line.insert(col, ch);
            self.lines = self.lines.update(line_idx, Arc::from(new_line));
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
        debug_assert!(
            self.is_valid_cursor(cursor),
            "insert_text called with invalid cursor: {:?}",
            cursor
        );

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
        debug_assert!(
            self.is_valid_cursor(start),
            "remove called with invalid start cursor: {:?}",
            start
        );
        debug_assert!(
            self.is_valid_cursor(end),
            "remove called with invalid end cursor: {:?}",
            end
        );

        if start.line > end.line || (start.line == end.line && start.col >= end.col) {
            return;
        }

        let start_line = start.line;
        let start_col = start.col;
        let end_line = end.line;
        let end_col = end.col;

        if start_line == end_line {
            if let Some(line) = self.lines.get(start_line) {
                let mut new_line = line.to_string();
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

    /// Deletes the grapheme cluster before the cursor position.
    ///
    /// If the cursor is at the start of a line, joins the current line with the previous line.
    /// Returns the new cursor position after deletion, or None if at document start.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("hello");
    /// let cursor = Cursor::new(0, 3);  // at 'l'
    /// let new_cursor = buf.delete_char_before_cursor(cursor);
    /// assert_eq!(new_cursor, Some(Cursor::new(0, 2)));  // cursor moves back
    /// assert_eq!(buf.as_str(), "helo");
    /// ```
    pub fn delete_char_before_cursor(&mut self, cursor: Cursor) -> Option<Cursor> {
        // If at start of line, join with previous line
        if cursor.col == 0 {
            if cursor.line == 0 {
                // At document start, nothing to delete
                return None;
            }
            // Join current line with previous line
            let current_line = cursor.line;
            let prev_line = current_line - 1;

            // Get content before mutating
            let current_content: String = self
                .lines
                .get(current_line)
                .map_or("", |s| s.as_ref())
                .to_string();
            let prev_content: String = self
                .lines
                .get(prev_line)
                .map_or("", |s| s.as_ref())
                .to_string();
            let prev_content_len = prev_content.len();

            let merged = Arc::from(format!("{}{}", prev_content, current_content));

            // Remove current line and previous line, insert merged
            let mut left = self.lines.take(prev_line);
            let right = self.lines.skip(current_line + 1);
            left.push_back(merged);
            left.append(right);
            self.lines = left;

            // Return cursor at the position where the join happened (end of previous content)
            return Some(Cursor::new(prev_line, prev_content_len));
        }

        // Find the grapheme cluster before the cursor
        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();

        // Find the grapheme that starts before cursor.col
        // grapheme_indices gives us the START of each grapheme
        // We want the LAST grapheme that starts before cursor
        let mut prev_grapheme_start: Option<(usize, usize)> = None;

        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset < cursor.col {
                // This grapheme starts before cursor, remember it (keep updating to get the last one)
                prev_grapheme_start = Some((byte_offset, byte_offset + grapheme.len()));
            } else {
                // This grapheme starts at or after cursor, we're done looking
                break;
            }
        }

        // If we found a grapheme before cursor, delete it
        if let Some((start, end)) = prev_grapheme_start {
            self.remove(
                Cursor::new(cursor.line, start),
                Cursor::new(cursor.line, end),
            );
            return Some(Cursor::new(cursor.line, start));
        }

        // No previous grapheme found (cursor at start of line or other edge case)
        Some(cursor)
    }

    /// Deletes the grapheme cluster at the cursor position.
    ///
    /// If the cursor is at the end of a line, joins the current line with the next line.
    /// Returns the cursor position after deletion (typically the same position), or None if at document end.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("hello");
    /// let cursor = Cursor::new(0, 1);  // at 'e'
    /// let new_cursor = buf.delete_char_at_cursor(cursor);
    /// assert_eq!(new_cursor, Some(Cursor::new(0, 1)));  // cursor stays
    /// assert_eq!(buf.as_str(), "hllo");
    /// ```
    pub fn delete_char_at_cursor(&mut self, cursor: Cursor) -> Option<Cursor> {
        let line_len = self.line_len(cursor.line);

        // If at end of line, join with next line
        if cursor.col >= line_len {
            if cursor.line >= self.lines.len() - 1 {
                // At document end, nothing to delete
                return None;
            }

            // Join current line with next line
            let current_line = cursor.line;
            let next_line = current_line + 1;

            // Get content before mutating
            let current_content: String = self
                .lines
                .get(current_line)
                .map_or("", |s| s.as_ref())
                .to_string();
            let next_content: String = self
                .lines
                .get(next_line)
                .map_or("", |s| s.as_ref())
                .to_string();
            let current_content_len = current_content.len();

            let merged = Arc::from(format!("{}{}", current_content, next_content));

            // Remove current line and next line, insert merged
            let mut left = self.lines.take(current_line);
            let right = self.lines.skip(next_line + 1);
            left.push_back(merged);
            left.append(right);
            self.lines = left;

            // Return cursor at the same visual position (where the join happened)
            return Some(Cursor::new(current_line, current_content_len));
        }

        // Find the grapheme cluster at the cursor
        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();

        // Find the grapheme that starts at or after cursor
        // grapheme_indices gives us the START of each grapheme
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset >= cursor.col {
                // Found a grapheme at or after cursor
                let start = byte_offset;
                let end = byte_offset + grapheme.len();
                self.remove(
                    Cursor::new(cursor.line, start),
                    Cursor::new(cursor.line, end),
                );
                return Some(Cursor::new(cursor.line, start));
            }
        }

        // No grapheme found (shouldn't happen for valid cursor), return current position
        Some(cursor)
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

    /// Join multiple lines starting from the specified line.
    ///
    /// Joins `line_count` lines starting from `start_line`. If `with_space` is true,
    /// inserts a single space between joined lines.
    ///
    /// # Arguments
    ///
    /// * `start_line` - The line index to start joining from (0-indexed)
    /// * `line_count` - Number of lines to join (at least 2 for meaningful join)
    /// * `with_space` - Whether to insert a space between joined lines
    ///
    /// Returns the cursor position at the end of the joined content, or None if
    /// the operation cannot be performed (e.g., not enough lines, invalid start line).
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("hello\nworld");
    /// let cursor = buf.join_lines(0, 2, true);
    /// assert_eq!(cursor, Some(Cursor::new(0, 11))); // "hello world" has 11 chars
    /// assert_eq!(buf.as_str(), "hello world");
    /// ```
    pub fn join_lines(
        &mut self,
        start_line: usize,
        line_count: usize,
        with_space: bool,
    ) -> Option<Cursor> {
        // Validate inputs
        if line_count < 2 {
            return None;
        }

        let total_lines = self.lines.len();
        if start_line >= total_lines {
            return None;
        }

        // Clamp line_count to available lines
        let actual_line_count = (total_lines - start_line).min(line_count);
        if actual_line_count < 2 {
            return None;
        }

        // Collect content from all lines to join
        let mut joined_content = String::new();

        for i in 0..actual_line_count {
            let line_idx = start_line + i;
            if let Some(line) = self.lines.get(line_idx) {
                if i > 0 {
                    // Add space before content of subsequent lines (if with_space is true)
                    if with_space {
                        joined_content.push(' ');
                    }
                }
                joined_content.push_str(line);
            }
        }

        // Get remaining lines after the joined section
        let end_line = start_line + actual_line_count;
        let right = self.lines.skip(end_line);

        // Calculate length before moving
        let joined_len = joined_content.len();

        // Replace the lines
        let mut left = self.lines.take(start_line);
        left.push_back(Arc::from(joined_content));
        left.append(right);
        self.lines = left;

        // Return cursor at end of joined content
        Some(Cursor::new(start_line, joined_len))
    }

    /// Delete multiple lines starting from the specified line.
    ///
    /// Deletes `count` lines starting from `start_line`. The newline characters
    /// between deleted lines are also removed.
    ///
    /// # Arguments
    ///
    /// * `start_line` - The line index to start deleting from (0-indexed)
    /// * `count` - Number of lines to delete
    ///
    /// Returns the cursor position at the start of the next line (or previous line
    /// if deleting the last line), or None if the operation cannot be performed.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("line1\nline2\nline3");
    /// let cursor = buf.delete_lines(0, 1);
    /// assert_eq!(cursor, Some(Cursor::new(0, 0))); // Now at "line2"
    /// assert_eq!(buf.as_str(), "line2\nline3");
    /// ```
    pub fn delete_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();
        
        // Handle empty buffer
        if total_lines == 0 {
            return Some(Cursor::new(0, 0));
        }
        
        // Validate start_line
        if start_line >= total_lines {
            return None;
        }
        
        // Clamp count to available lines
        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(Cursor::new(start_line, 0));
        }
        
        // Calculate end position (line after the last deleted line)
        let end_line = start_line + actual_count;
        
        if end_line >= total_lines {
            // Deleting to end of file
            // Keep lines before start_line
            let mut left = self.lines.take(start_line);
            if left.is_empty() {
                // Was at or past all lines, add empty line
                left.push_back(Arc::from(""));
            }
            self.lines = left;
        } else {
            // Deleting in middle of file
            // Keep lines before start_line and lines after end_line
            let mut left = self.lines.take(start_line);
            let right = self.lines.skip(end_line);
            left.append(right);
            self.lines = left;
        }
        
        // Return new cursor position
        let new_line_count = self.lines.len();
        if new_line_count == 0 {
            Some(Cursor::new(0, 0))
        } else if start_line >= new_line_count {
            Some(Cursor::new(new_line_count - 1, 0))
        } else {
            Some(Cursor::new(start_line, 0))
        }
    }

    /// Changes `count` lines starting from `start_line`.
    /// Deletes the lines and replaces them with a single empty line.
    /// Returns the new cursor position.
    ///
    /// # Arguments
    ///
    /// * `start_line` - The line index to start changing from (0-indexed)
    /// * `count` - The number of lines to change
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("line1\nline2\nline3");
    /// let cursor = buf.change_lines(0, 1);
    /// assert_eq!(cursor, Some(Cursor::new(0, 0)));
    /// assert_eq!(buf.as_str(), "\nline2\nline3");  // line replaced with blank
    /// ```
    pub fn change_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();

        // Handle empty buffer - create one empty line
        if total_lines == 0 {
            self.lines.push_back(Arc::from(""));
            return Some(Cursor::new(0, 0));
        }

        // Validate start_line
        if start_line >= total_lines {
            return None;
        }

        // Clamp count to available lines
        let actual_count = (total_lines - start_line).min(count);
        if actual_count == 0 {
            return Some(Cursor::new(start_line, 0));
        }

        // Delete the lines (similar to delete_lines)
        let end_line = start_line + actual_count;

        if end_line >= total_lines {
            // Changing to end of file - replace deleted lines with one blank line
            let mut left = self.lines.take(start_line);
            // Always add one blank line to replace the changed line(s)
            left.push_back(Arc::from(""));
            self.lines = left;
        } else {
            // Deleting in middle of file
            // Keep lines before start_line, add blank line, then lines after end_line
            let mut left = self.lines.take(start_line);
            left.push_back(Arc::from("")); // Insert blank line
            let right = self.lines.skip(end_line);
            left.append(right);
            self.lines = left;
        }

        // Return new cursor position (at start of blank line)
        Some(Cursor::new(start_line, 0))
    }

    /// Changes text from cursor to end of `count` lines.
    /// Deletes from `start` cursor to end of `count` lines.
    /// Returns the new cursor position at the end of the remaining text on the first line.
    ///
    /// # Arguments
    ///
    /// * `start` - Cursor position to start deletion from (on first line)
    /// * `count` - Number of lines to affect (starting from start.line)
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("hello world");
    /// let cursor = Cursor::new(0, 5);  // after "hello"
    /// let new_cursor = buf.change_to_line_end(cursor, 1);
    /// assert_eq!(new_cursor, Some(Cursor::new(0, 5)));  // at "hello"
    /// assert_eq!(buf.as_str(), "hello");
    /// ```
    pub fn change_to_line_end(&mut self, start: Cursor, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();

        // Handle empty buffer
        if total_lines == 0 {
            return Some(Cursor::new(0, 0));
        }

        // Validate start position
        if start.line >= total_lines {
            return None;
        }

        // Clamp count to available lines
        let actual_count = (total_lines - start.line).min(count);
        if actual_count == 0 {
            return Some(start);
        }

        // Calculate end position: end of line (start.line + actual_count - 1)
        let end_line = start.line + actual_count - 1;
        let end_col = self.line_len(end_line);

        // Create end cursor at end of last line
        let end = Cursor::new(end_line, end_col);

        // Use remove to delete from start to end
        self.remove(start, end);

        // Return cursor at the original start position (which is now at end of truncated text)
        Some(start)
    }

    /// Inserts `count` empty lines AFTER the given line index.
    /// Returns the cursor position at the start of the first inserted line.
    ///
    /// # Arguments
    ///
    /// * `line` - The line index to insert after (0-indexed)
    /// * `count` - The number of empty lines to insert
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("line1\nline2\nline3");
    /// let cursor = buf.insert_lines_after(0, 1);  // Insert after line 1
    /// assert_eq!(cursor, Some(Cursor::new(1, 0))); // At new line 2
    /// assert_eq!(buf.as_str(), "line1\n\nline2\nline3");
    /// ```
    pub fn insert_lines_after(&mut self, line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();
        
        // Handle empty buffer - create first line if inserting into empty
        if total_lines == 0 {
            if count > 0 {
                self.lines.push_back(Arc::from(""));
            }
            return Some(Cursor::new(0, 0));
        }
        
        // Clamp line to valid range (insert after last line means append)
        let insert_after = line.min(total_lines);
        
        // If count is 0, just return cursor at the given line (no insertion)
        if count == 0 {
            return Some(Cursor::new(line, 0));
        }
        
        // Insert the empty lines
        if insert_after >= total_lines {
            // Appending to end of file
            for _ in 0..count {
                self.lines.push_back(Arc::from(""));
            }
            // Cursor at the first inserted line (which is the old total_lines)
            Some(Cursor::new(total_lines, 0))
        } else {
            // Inserting in middle or at beginning
            // take() gets first insert_after + 1 lines, skip() gets the rest
            let mut left = self.lines.take(insert_after + 1);
            let right = self.lines.skip(insert_after + 1);
            for _ in 0..count {
                left.push_back(Arc::from(""));
            }
            left.append(right);
            self.lines = left;
            // Cursor at the first inserted line (which is insert_after + 1)
            Some(Cursor::new(insert_after + 1, 0))
        }
    }

    /// Inserts N empty lines BEFORE the specified line index.
    ///
    /// When line == 0, inserts at the beginning of the buffer.
    /// Returns the cursor position at the first inserted line.
    ///
    /// # Arguments
    ///
    /// * `line` - Line number to insert before (0-indexed)
    /// * `count` - Number of empty lines to insert
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let mut buf = Buffer::from_str("line1\nline2\nline3");
    /// let cursor = buf.insert_lines_before(0, 1);  // Insert before line 1
    /// assert_eq!(cursor, Some(Cursor::new(0, 0))); // At new line 1
    /// assert_eq!(buf.as_str(), "\nline1\nline2\nline3");
    /// ```
    pub fn insert_lines_before(&mut self, line: usize, count: usize) -> Option<Cursor> {
        let total_lines = self.lines.len();

        if total_lines == 0 {
            if count > 0 {
                self.lines.push_back(Arc::from(""));
            }
            return Some(Cursor::new(0, 0));
        }

        if count == 0 {
            return Some(Cursor::new(line, 0));
        }

        if line == 0 {
            for _ in 0..count {
                self.lines.push_front(Arc::from(""));
            }
            Some(Cursor::new(0, 0))
        } else {
            let insert_before = line.saturating_sub(1);
            if insert_before >= total_lines {
                for _ in 0..count {
                    self.lines.push_back(Arc::from(""));
                }
                Some(Cursor::new(total_lines, 0))
            } else {
                let mut left = self.lines.take(insert_before + 1);
                let right = self.lines.skip(insert_before + 1);
                for _ in 0..count {
                    left.push_back(Arc::from(""));
                }
                left.append(right);
                self.lines = left;
                Some(Cursor::new(insert_before + 1, 0))
            }
        }
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

    /// Returns the cursor at the next grapheme cluster.
    ///
    /// If at end of line, wraps to start of next line.
    /// Returns None only if at end of last line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("ab");
    /// let cursor = Cursor::new(0, 0);
    /// let next = buf.next_cursor(cursor);
    /// assert_eq!(next, Some(Cursor::new(0, 1)));
    /// ```
    pub fn next_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        let line_len = self.line_len(cursor.line);

        if cursor.col < line_len {
            // Move within current line
            let line = self.lines.get(cursor.line)?;
            let line_str = line.as_ref();

            // Search only the substring after cursor
            for (relative_offset, _grapheme) in line_str[cursor.col..].grapheme_indices(true) {
                if relative_offset == 0 {
                    continue;
                }
                return Some(Cursor::new(cursor.line, cursor.col + relative_offset));
            }
            // At last grapheme, return end of line
            Some(Cursor::new(cursor.line, line_len))
        } else if cursor.line < self.lines.len() - 1 {
            // Move to start of next line
            Some(Cursor::new(cursor.line + 1, 0))
        } else {
            // At end of last line, stay in place
            None
        }
    }

    /// Returns the cursor at the previous grapheme cluster.
    ///
    /// If at start of line, wraps to end of previous line.
    /// Returns None only if at start of first line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("ab");
    /// let cursor = Cursor::new(0, 1);
    /// let prev = buf.prev_cursor(cursor);
    /// assert_eq!(prev, Some(Cursor::new(0, 0)));
    /// ```
    pub fn prev_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.col > 0 {
            // Move within current line
            let line = self.lines.get(cursor.line)?;
            let line_str = line.as_ref();

            let prefix = &line_str[..cursor.col];

            let last_grapheme_offset = prefix
                .grapheme_indices(true)
                .rev()
                .next()
                .map(|(offset, _)| offset)?;

            Some(Cursor::new(cursor.line, last_grapheme_offset))
        } else if cursor.line > 0 {
            // Move to end of previous line
            let prev_line_len = self.line_len(cursor.line - 1);
            Some(Cursor::new(cursor.line - 1, prev_line_len))
        } else {
            // At start of first line, stay in place
            None
        }
    }

    /// Returns the cursor at the next grapheme cluster in the same line.
    ///
    /// Returns None if cursor is at or past the last grapheme in the line.
    /// Does NOT wrap to the next line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position (must be valid within buffer)
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("a👨‍👩‍👧‍👦b");
    /// let cursor = Cursor::new(0, 0);  // at 'a'
    /// let next = buf.next_cursor_line(cursor);
    /// // Returns cursor at start of emoji (byte ~1)
    /// ```
    pub fn next_cursor_line(&self, cursor: Cursor) -> Option<Cursor> {
        let line_len = self.line_len(cursor.line);

        if cursor.col >= line_len {
            return None;
        }

        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();

        // Search only the substring after cursor, starting from cursor.col
        // grapheme_indices(true) on a substring gives us byte offsets relative to that substring
        // so we need to add cursor.col to get absolute byte offsets
        for (relative_offset, _grapheme) in line_str[cursor.col..].grapheme_indices(true) {
            if relative_offset == 0 {
                // We're at the first grapheme of the substring, which is the current grapheme
                continue;
            }
            return Some(Cursor::new(cursor.line, cursor.col + relative_offset));
        }

        // At last grapheme, return end of line
        Some(Cursor::new(cursor.line, line_len))
    }

    /// Returns the cursor at the previous grapheme cluster in the same line.
    ///
    /// Returns None if cursor is at the first grapheme in the line.
    /// Does NOT wrap to the previous line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position (must be valid within buffer)
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("a👨‍👩‍👧‍👦b");
    /// let cursor = Cursor::new(0, 5);  // after emoji
    /// let prev = buf.prev_cursor_line(cursor);
    /// // Returns cursor at 'a' (byte 0)
    /// ```
    pub fn prev_cursor_line(&self, cursor: Cursor) -> Option<Cursor> {
        if cursor.col == 0 {
            return None;
        }

        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();

        // Search only the substring before cursor using double-ended iterator
        // The grapheme iterator is DoubleEndedIterator, so we can use .rev().next()
        // to efficiently find the last grapheme in the prefix without O(n) iteration
        let prefix = &line_str[..cursor.col];

        // Get the last grapheme's byte offset in the prefix
        // .rev() reverses the iterator (O(1) for double-ended)
        // .next() gets the last element (now first in reversed order)
        let last_grapheme_offset = prefix
            .grapheme_indices(true)
            .rev()
            .next()
            .map(|(offset, _)| offset)?;

        Some(Cursor::new(cursor.line, last_grapheme_offset))
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

    /// Move cursor to the end of the current line (last non-whitespace character).
    ///
    /// If the cursor is already at or beyond the end of the current line, moves to
    /// the end of the next line. If on the last line at the end, returns None.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("hello\nworld");
    /// // In middle of line - move to end
    /// assert_eq!(buf.cursor_end_of_line(Cursor::new(0, 2)), Some(Cursor::new(0, 4)));
    /// // At end of line - move to next line's end
    /// assert_eq!(buf.cursor_end_of_line(Cursor::new(0, 4)), Some(Cursor::new(1, 4)));
    /// // At end of last line - no movement
    /// assert_eq!(buf.cursor_end_of_line(Cursor::new(1, 4)), None);
    /// ```
    pub fn cursor_end_of_line(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }

        let line = self.line_at(cursor.line)?;
        let line_str = line.as_ref();

        // Find last non-whitespace character position on current line
        let mut last_non_ws = None;
        for (idx, grapheme) in line_str.grapheme_indices(true) {
            if !Self::is_whitespace_char(grapheme) {
                last_non_ws = Some(idx);
            }
        }

        let end_pos = last_non_ws.unwrap_or(0);

        // If cursor is before the end position, move to end
        if cursor.col < end_pos {
            return Some(Cursor::new(cursor.line, end_pos));
        }

        // Cursor is at or past end of current line
        // Move to next line if available
        if cursor.line + 1 < total_lines {
            let next_line_idx = cursor.line + 1;
            let next_line_len = self.line_len(next_line_idx);

            if next_line_len > 0 {
                // Find last non-whitespace on next line
                let next_line = self.line_at(next_line_idx)?;
                let next_line_str = next_line.as_ref();

                let mut next_last_non_ws = None;
                for (idx, grapheme) in next_line_str.grapheme_indices(true) {
                    if !Self::is_whitespace_char(grapheme) {
                        next_last_non_ws = Some(idx);
                    }
                }

                return Some(Cursor::new(next_line_idx, next_last_non_ws.unwrap_or(0)));
            } else {
                // Next line is empty, return at position 0
                return Some(Cursor::new(next_line_idx, 0));
            }
        }

        // Already at end of last line - no movement
        None
    }

    /// Move cursor to absolute start of line (column 0).
    ///
    /// If already at column 0, wraps to previous line's column 0.
    /// Returns None if already at column 0 of first line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("  hello\n  world");
    /// // In middle of line - move to column 0
    /// assert_eq!(buf.cursor_start_of_line(Cursor::new(0, 5)), Some(Cursor::new(0, 0)));
    /// // At column 0 on line 1 - wrap to previous line
    /// assert_eq!(buf.cursor_start_of_line(Cursor::new(1, 0)), Some(Cursor::new(0, 0)));
    /// // At column 0 on first line - no movement
    /// assert_eq!(buf.cursor_start_of_line(Cursor::new(0, 0)), None);
    /// ```
    pub fn cursor_start_of_line(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }

        // If not at column 0, move to column 0
        if cursor.col != 0 {
            return Some(Cursor::new(cursor.line, 0));
        }

        // Already at column 0 - wrap to previous line if available
        if cursor.line > 0 {
            return Some(Cursor::new(cursor.line - 1, 0));
        }

        // Already at column 0 of first line - no movement
        None
    }

    /// Move cursor to first non-whitespace character of current line.
    ///
    /// If already at first non-whitespace position, wraps to previous line.
    /// Returns None if already at first non-whitespace of first line.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor};
    ///
    /// let buf = Buffer::from_str("  hello\n  world");
    /// // In middle of line - move to first non-whitespace
    /// assert_eq!(buf.cursor_content_start_of_line(Cursor::new(0, 5)), Some(Cursor::new(0, 2)));
    /// // At first non-whitespace on line 1 - move to previous line's first non-whitespace
    /// assert_eq!(buf.cursor_content_start_of_line(Cursor::new(1, 2)), Some(Cursor::new(0, 2)));
    /// // At first non-whitespace of first line - no movement (no previous line)
    /// assert_eq!(buf.cursor_content_start_of_line(Cursor::new(0, 2)), None);
    /// ```
    pub fn cursor_content_start_of_line(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }

        let line = self.line_at(cursor.line)?;
        let line_str = line.as_ref();

        // Find first non-whitespace character position on current line
        let mut first_non_ws = None;
        for (idx, grapheme) in line_str.grapheme_indices(true) {
            if !Self::is_whitespace_char(grapheme) {
                first_non_ws = Some(idx);
                break;
            }
        }

        let content_start = match first_non_ws {
            Some(pos) => pos,
            None => {
                // Line has no non-whitespace - return at column 0
                if cursor.col > 0 {
                    return Some(Cursor::new(cursor.line, 0));
                }
                // cursor.col == 0 on blank line - continue to wrap logic
                // so we can find the previous line's first non-whitespace
                0
            }
        };

        // If cursor is not at the content start, move to it
        // Otherwise, wrap to previous line
        if cursor.col != content_start {
            return Some(Cursor::new(cursor.line, content_start));
        }

        // Cursor is at content start - wrap to previous line if available
        if cursor.line > 0 {
            let prev_line_idx = cursor.line - 1;
            let prev_line = self.line_at(prev_line_idx)?;
            let prev_line_str = prev_line.as_ref();

            // Find first non-whitespace on previous line
            let mut prev_first_non_ws = None;
            for (idx, grapheme) in prev_line_str.grapheme_indices(true) {
                if !Self::is_whitespace_char(grapheme) {
                    prev_first_non_ws = Some(idx);
                    break;
                }
            }

            if let Some(prev_pos) = prev_first_non_ws {
                return Some(Cursor::new(prev_line_idx, prev_pos));
            } else {
                // Previous line has no non-whitespace - return at column 0
                return Some(Cursor::new(prev_line_idx, 0));
            }
        }

        // Already at first line at content start - no movement
        None
    }

    /// Check if a line is blank (empty or whitespace only).
    ///
    /// # Arguments
    ///
    /// * `line_idx` - Line index to check
    ///
    fn is_blank_line(&self, line_idx: usize) -> bool {
        let line = match self.line_at(line_idx) {
            Some(l) => l,
            None => return false,
        };
        line.chars().all(|c| c.is_whitespace())
    }

    /// Move cursor to the blank line before the previous paragraph.
    ///
    /// A paragraph is a consecutive sequence of non-empty lines.
    /// If on a non-blank line (inside paragraph), finds the blank line BEFORE it.
    /// If on a blank line, finds the blank line before the previous paragraph.
    /// Returns None if no previous blank line/paragraph is found.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position
    ///
    pub fn cursor_paragraph_backward(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }

        let mut line_idx = cursor.line;

        // If on non-blank line (inside paragraph), find blank line BEFORE it
        if !self.is_blank_line(line_idx) {
            while line_idx > 0 && !self.is_blank_line(line_idx) {
                line_idx -= 1;
            }
            // Now at blank line or line 0
            if self.is_blank_line(line_idx) {
                return Some(Cursor::new(line_idx, 0));
            }
            return None; // No blank line found
        }

        // On blank line - find previous blank line
        // Skip blank lines upward
        while line_idx > 0 && self.is_blank_line(line_idx) {
            line_idx -= 1;
        }

        // Skip any non-blank lines (previous paragraph)
        while line_idx > 0 && !self.is_blank_line(line_idx) {
            line_idx -= 1;
        }

        // Now at line 0 or at a blank line
        // If we're at line 0 and it's non-blank, there's no previous blank line
        if line_idx == 0 && !self.is_blank_line(0) {
            return None;
        }

        // If we're still at line 0 and it's blank, we couldn't move to a previous blank line
        if line_idx == 0 {
            return None;
        }

        // line_idx is now at a blank line (not line 0)
        if self.is_blank_line(line_idx) {
            Some(Cursor::new(line_idx, 0))
        } else {
            None // No blank line found
        }
    }

    /// Move cursor to the blank line before the next paragraph.
    ///
    /// A paragraph is a consecutive sequence of non-empty lines.
    /// If on a non-blank line (inside paragraph), finds the blank line AFTER it.
    /// If on a blank line, finds the next blank line.
    /// Returns None if no next blank line/paragraph is found.
    ///
    /// # Arguments
    ///
    /// * `cursor` - Current cursor position
    ///
    pub fn cursor_paragraph_forward(&self, cursor: Cursor) -> Option<Cursor> {
        let total_lines = self.line_count();
        if total_lines == 0 {
            return None;
        }

        let mut line_idx = cursor.line;

        // If on non-blank line (inside paragraph), find blank line AFTER current paragraph
        if !self.is_blank_line(line_idx) {
            while line_idx < total_lines && !self.is_blank_line(line_idx) {
                line_idx += 1;
            }
            // Now at blank line or past EOF
            if line_idx < total_lines && self.is_blank_line(line_idx) {
                return Some(Cursor::new(line_idx, 0));
            }
            return None;
        }

        // On blank line - find next blank line
        while line_idx < total_lines && self.is_blank_line(line_idx) {
            line_idx += 1;
        }

        // Skip non-blank paragraph lines
        while line_idx < total_lines && !self.is_blank_line(line_idx) {
            line_idx += 1;
        }

        // Now at blank line or past EOF
        if line_idx < total_lines && self.is_blank_line(line_idx) {
            Some(Cursor::new(line_idx, 0))
        } else {
            None // No blank line found
        }
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

    fn grapheme_at_byte(&self, line_idx: usize, byte_pos: usize) -> Option<&str> {
        let line = self.lines.get(line_idx)?;
        let line_str = line.as_ref();
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset == byte_pos {
                return Some(grapheme);
            }
        }
        None
    }

    fn prev_grapheme_before_byte(&self, line_idx: usize, byte_pos: usize) -> Option<&str> {
        let line = self.lines.get(line_idx)?;
        let line_str = line.as_ref();
        let mut prev = None;
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset >= byte_pos {
                break;
            }
            prev = Some(grapheme);
        }
        prev
    }

    fn next_grapheme_at_or_after_byte(&self, line_idx: usize, byte_pos: usize) -> Option<&str> {
        let line = self.lines.get(line_idx)?;
        let line_str = line.as_ref();
        for (byte_offset, grapheme) in line_str.grapheme_indices(true) {
            if byte_offset >= byte_pos {
                return Some(grapheme);
            }
        }
        None
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

// ============================================================================
// Boundary methods
// ============================================================================

impl Buffer {
    /// Check if a grapheme is a word character (alphanumeric or underscore).
    pub fn is_word_char(grapheme: &str) -> bool {
        let mut chars = grapheme.chars();
        match chars.next() {
            Some(c) => c.is_alphanumeric() || c == '_',
            None => false,
        }
    }

    /// Check if a grapheme is a whitespace character.
    pub fn is_whitespace_char(grapheme: &str) -> bool {
        let mut chars = grapheme.chars();
        match chars.next() {
            Some(c) => c.is_whitespace(),
            None => false,
        }
    }

    /// Check if a grapheme is a BigWord character (non-whitespace).
    pub fn is_bigword_char(grapheme: &str) -> bool {
        !Self::is_whitespace_char(grapheme)
    }

    /// Check if cursor is at the specified boundary.
    pub fn is_at_boundary(&self, cursor: Cursor, boundary: Boundary) -> bool {
        let line_idx = cursor.line;
        let col = cursor.col;

        let current_grapheme = self.grapheme_at_byte(line_idx, col);
        let prev_grapheme = self.prev_grapheme_before_byte(line_idx, col);
        let next_grapheme = self.next_grapheme_at_or_after_byte(line_idx, col);

        match boundary {
            Boundary::Word => match current_grapheme {
                Some(g) if Self::is_word_char(g) => match prev_grapheme {
                    Some(pg) => !Self::is_word_char(pg),
                    None => true,
                },
                Some(g) if !Self::is_word_char(g) && !Self::is_whitespace_char(g) => {
                    match prev_grapheme {
                        Some(pg) => Self::is_word_char(pg),
                        None => true,
                    }
                }
                _ => false,
            },
            Boundary::WordEnd => match prev_grapheme {
                Some(pg) if Self::is_word_char(pg) => match next_grapheme {
                    Some(ng) => !Self::is_word_char(ng),
                    None => true,
                },
                Some(pg) if !Self::is_word_char(pg) && !Self::is_whitespace_char(pg) => {
                    match next_grapheme {
                        Some(ng) => {
                            Self::is_word_char(ng)
                                || (!Self::is_word_char(ng) && !Self::is_whitespace_char(ng))
                        }
                        None => true,
                    }
                }
                _ => false,
            },
            Boundary::BigWord => match current_grapheme {
                Some(g) if Self::is_bigword_char(g) => match prev_grapheme {
                    Some(pg) => Self::is_whitespace_char(pg),
                    None => true,
                },
                _ => false,
            },
            Boundary::BigWordEnd => match prev_grapheme {
                Some(pg) if Self::is_bigword_char(pg) => match next_grapheme {
                    Some(ng) => Self::is_whitespace_char(ng),
                    None => true,
                },
                _ => false,
            },
        }
    }

    /// Find the next boundary position forward from cursor.
    ///
    /// Returns None if no boundary exists in the forward direction.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor, Boundary};
    ///
    /// let buf = Buffer::from_str("hello world");
    /// let next = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
    /// assert_eq!(next, Some(Cursor::new(0, 6))); // at 'w'
    /// ```
    pub fn next_boundary(&self, cursor: Cursor, boundary: Boundary) -> Option<Cursor> {
        let total_lines = self.line_count();
        let mut line_idx = cursor.line;
        let mut col = cursor.col;

        // If at end of line, move to next line
        if col >= self.line_len(line_idx) {
            line_idx += 1;
            col = 0;
        }

        while line_idx < total_lines {
            let line = match self.line_at(line_idx) {
                Some(l) => l,
                None => break,
            };

            let line_str = line.as_ref();
            let line_len = line_str.len();

            // Skip empty lines
            if line_len == 0 {
                line_idx += 1;
                col = 0;
                continue;
            }

            // Clamp col
            if col >= line_len {
                // Wrapping to new line - first check if we're at start of a word
                // (this handles the case where a line starts with a word without leading whitespace)
                if col == 0 && line_len > 0 {
                    let g = line_str.get(0..).and_then(|s| s.graphemes(true).next());
                    if matches!(g, Some(gg) if Self::is_word_char(gg)) {
                        return Some(Cursor::new(line_idx, 0));
                    }
                }
                line_idx += 1;
                col = 0;
                continue;
            }

            match boundary {
                Boundary::Word => {
                    // Skip to end of current word, then find next word start
                    let mut check_col = col;

                    // Check if we started at a word character
                    let started_at_word_char = if col < line_len {
                        let g = line_str.get(col..).and_then(|s| s.graphemes(true).next());
                        matches!(g, Some(gg) if Self::is_word_char(gg))
                    } else {
                        false
                    };

                    // If we're at a word char, skip to end of it
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_word_char(gg) => {
                                check_col += gg.len();
                            }
                            _ => break,
                        }
                    }

                    // Now we're past the current word (or at the end of line)
                    // Check if the next character is a non-word, non-whitespace character (e.g., "---")
                    // If we came FROM a word, this is a boundary - return the position
                    // If we started at a non-word, skip through and find the next boundary
                    if check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        if let Some(gg) = g
                            && !Self::is_word_char(gg)
                            && !Self::is_whitespace_char(gg)
                        {
                            if started_at_word_char {
                                // We came from a word - this non-word sequence is a separate word
                                // Return the start of it
                                return Some(Cursor::new(line_idx, check_col));
                            } else {
                                // We started at a non-word - skip through the sequence
                                while check_col < line_len {
                                    let g = line_str
                                        .get(check_col..)
                                        .and_then(|s| s.graphemes(true).next());
                                    match g {
                                        Some(gg)
                                            if !Self::is_word_char(gg)
                                                && !Self::is_whitespace_char(gg) =>
                                        {
                                            check_col += gg.len();
                                        }
                                        _ => break,
                                    }
                                }
                            }
                        }
                    }

                    // Skip whitespace to find the next word
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_word_char(gg) => {
                                // Found start of next word - return this position
                                return Some(Cursor::new(line_idx, check_col));
                            }
                            Some(gg) => {
                                check_col += gg.len();
                            }
                            None => break,
                        }
                    }

                    // No more words on this line - wrap to next line
                    // When wrapping, check if next line starts with a word (without leading whitespace)
                    line_idx += 1;
                    col = 0;
                    // Check if the new line starts with a word character
                    if line_idx < total_lines {
                        let next_line = self.line_at(line_idx);
                        if let Some(l) = next_line {
                            let next_line_str = l.as_ref();
                            if !next_line_str.is_empty() {
                                let first_g = next_line_str.graphemes(true).next();
                                if matches!(first_g, Some(g) if Self::is_word_char(g)) {
                                    return Some(Cursor::new(line_idx, 0));
                                }
                            }
                        }
                    }
                    continue;
                }

                Boundary::WordEnd => {
                    // If we're on a word character, go to the end of THIS word
                    // If we're at the last word of the line, wrap to next line
                    // Otherwise, find the end of the next word
                    let mut check_col = col;

                    // Check if we're on a word character
                    let on_word_char = if check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        matches!(g, Some(gg) if Self::is_word_char(gg))
                    } else {
                        false
                    };

                    // Check if we're on a non-word, non-whitespace character (e.g., "---")
                    let on_non_word_non_ws = if check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        matches!(g, Some(gg) if !Self::is_word_char(gg) && !Self::is_whitespace_char(gg))
                    } else {
                        false
                    };

                    if on_non_word_non_ws {
                        // We're at a non-word, non-whitespace char - find its end
                        // This is the end of this "word" (the non-word chars)
                        while check_col < line_len {
                            let g = line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg)
                                    if !Self::is_word_char(gg) && !Self::is_whitespace_char(gg) =>
                                {
                                    check_col += gg.len();
                                }
                                _ => break,
                            }
                        }

                        // If we're at the end of the non-word sequence and it's different from where we started,
                        // return the end of this "word"
                        if check_col > col {
                            // Check if we're at a new position or still at the same position
                            if check_col - 1 > col {
                                return Some(Cursor::new(line_idx, check_col - 1));
                            }
                            // We're at the end of a non-word sequence but were already at its last char
                            // Continue to find the next word end
                        }

                        // Skip any whitespace and find the next word
                        while check_col < line_len {
                            let g = line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg) if Self::is_word_char(gg) => break,
                                Some(gg) if Self::is_whitespace_char(gg) => check_col += gg.len(),
                                Some(gg) => check_col += gg.len(),
                                None => break,
                            }
                        }

                        // Now find the end of that word
                        let mut end_col = check_col;
                        while end_col < line_len {
                            let g = line_str
                                .get(end_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg) if Self::is_word_char(gg) => {
                                    end_col += gg.len();
                                }
                                _ => break,
                            }
                        }
                        if end_col > check_col {
                            return Some(Cursor::new(line_idx, end_col - 1));
                        }
                    } else if on_word_char && check_col < line_len {
                        // We're in a word - find its end
                        let mut at_end_of_line = false;
                        while check_col < line_len {
                            let g = line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg) if Self::is_word_char(gg) => {
                                    // Check if this is the last char of the line
                                    let next_check = check_col + gg.len();
                                    if next_check >= line_len {
                                        at_end_of_line = true;
                                    }
                                    check_col = next_check;
                                }
                                _ => break,
                            }
                        }

                        // If we're NOT at end of line, check if we actually moved forward
                        // past the current word. If we're still at the same position
                        // (meaning we were already at a word end), skip to next word.
                        if !at_end_of_line && check_col > col + 1 {
                            // We moved past at least one character - return end of current word
                            return Some(Cursor::new(line_idx, check_col - 1));
                        } else if !at_end_of_line && check_col == col + 1 {
                            // We were at a word end position - skip whitespace and find next word end
                            // But first, check if we're at a non-word, non-whitespace sequence
                            // If so, that's the end of the next "word" - return it
                            if check_col < line_len {
                                let g = line_str
                                    .get(check_col..)
                                    .and_then(|s| s.graphemes(true).next());
                                if let Some(gg) = g
                                    && !Self::is_word_char(gg)
                                    && !Self::is_whitespace_char(gg)
                                {
                                    // We're at a non-word, non-whitespace sequence
                                    // Find its end and return
                                    let mut end_col = check_col;
                                    while end_col < line_len {
                                        let g = line_str
                                            .get(end_col..)
                                            .and_then(|s| s.graphemes(true).next());
                                        match g {
                                            Some(gg)
                                                if !Self::is_word_char(gg)
                                                    && !Self::is_whitespace_char(gg) =>
                                            {
                                                end_col += gg.len();
                                            }
                                            _ => break,
                                        }
                                    }
                                    if end_col > check_col {
                                        return Some(Cursor::new(line_idx, end_col - 1));
                                    }
                                }
                            }

                            // Find next word start (skip whitespace only)
                            while check_col < line_len {
                                let g = line_str
                                    .get(check_col..)
                                    .and_then(|s| s.graphemes(true).next());
                                match g {
                                    Some(gg) if Self::is_word_char(gg) => break,
                                    Some(gg) if Self::is_whitespace_char(gg) => {
                                        check_col += gg.len()
                                    }
                                    Some(gg) => {
                                        // Hit a non-word, non-whitespace - we've already handled this above
                                        // This shouldn't be reached
                                        let _ = gg; // suppress unused warning
                                        break;
                                    }
                                    None => break,
                                }
                            }
                            // Now find the end of that next word
                            let mut end_col = check_col;
                            while end_col < line_len {
                                let g = line_str
                                    .get(end_col..)
                                    .and_then(|s| s.graphemes(true).next());
                                match g {
                                    Some(gg) if Self::is_word_char(gg) => {
                                        end_col += gg.len();
                                    }
                                    _ => break,
                                }
                            }
                            if end_col > check_col {
                                return Some(Cursor::new(line_idx, end_col - 1));
                            }
                        }
                    }

                    // Either not on a word, or at end of line - wrap to next line
                    // Check if next line starts with a word and find its end
                    line_idx += 1;
                    let _col = 0;

                    // Find word on next line and return its end
                    while line_idx < total_lines {
                        let next_line = match self.line_at(line_idx) {
                            Some(l) => l,
                            None => break,
                        };
                        let next_line_str = next_line.as_ref();
                        let next_line_len = next_line_str.len();

                        if next_line_len == 0 {
                            line_idx += 1;
                            continue;
                        }

                        // Find start of word on this line
                        let mut check_col = 0;
                        while check_col < next_line_len {
                            let g = next_line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next());
                            match g {
                                Some(gg) if Self::is_word_char(gg) => {
                                    // Found word start - find its end
                                    let mut end_col = check_col;
                                    while end_col < next_line_len {
                                        let gg = next_line_str
                                            .get(end_col..)
                                            .and_then(|s| s.graphemes(true).next());
                                        match gg {
                                            Some(gc) if Self::is_word_char(gc) => {
                                                end_col += gc.len();
                                            }
                                            _ => break,
                                        }
                                    }
                                    // Return position at end of word (not after)
                                    if end_col > check_col {
                                        return Some(Cursor::new(line_idx, end_col - 1));
                                    }
                                }
                                Some(gg) => {
                                    check_col += gg.len();
                                }
                                None => break,
                            }
                        }

                        // No word found on this line, continue to next
                        line_idx += 1;
                    }
                }
                Boundary::BigWord => {
                    // First, skip to end of current bigword if we're in one
                    let mut check_col = col;
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                check_col += gg.len();
                            }
                            _ => break,
                        }
                    }
                    // Now skip whitespace to find next bigword
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                // Found next bigword start
                                return Some(Cursor::new(line_idx, check_col));
                            }
                            Some(gg) => {
                                check_col += gg.len();
                            }
                            None => break,
                        }
                    }

                    // No more bigwords on this line - wrap to next line
                    // When wrapping, check if next line starts with a bigword (non-whitespace)
                    // If it starts with whitespace, skip it and find the first bigword
                    line_idx += 1;
                    let _col = 0;

                    while line_idx < total_lines {
                        let next_line = match self.line_at(line_idx) {
                            Some(l) => l,
                            None => break,
                        };
                        let next_line_str = next_line.as_ref();
                        let next_line_len = next_line_str.len();
                        if next_line_len == 0 {
                            line_idx += 1;
                            continue;
                        }

                        // Check if first char is a bigword char (non-whitespace)
                        let first_g = next_line_str.graphemes(true).next();
                        if matches!(first_g, Some(g) if Self::is_bigword_char(g)) {
                            // Line starts with a bigword - return position 0
                            return Some(Cursor::new(line_idx, 0));
                        } else {
                            // Line starts with whitespace - skip it and find first bigword
                            let mut check_col = 0;
                            while check_col < next_line_len {
                                let g = next_line_str
                                    .get(check_col..)
                                    .and_then(|s| s.graphemes(true).next());
                                match g {
                                    Some(gg) if Self::is_bigword_char(gg) => {
                                        // Found first bigword on this line
                                        return Some(Cursor::new(line_idx, check_col));
                                    }
                                    Some(gg) => {
                                        check_col += gg.len();
                                    }
                                    None => break,
                                }
                            }
                            // No bigword found on this line, continue to next
                            line_idx += 1;
                        }
                    }
                }
                Boundary::BigWordEnd => {
                    // Find end of current bigword, then find end of next bigword
                    let mut check_col = col;
                    let start_col = col;

                    // First, skip to end of current bigword if we're in one
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                check_col += gg.len();
                            }
                            _ => break,
                        }
                    }

                    // After first while, check_col is at end of current word or past it
                    // If we moved forward past the starting position, check what comes after
                    if check_col > start_col {
                        let after_current = if check_col < line_len {
                            line_str
                                .get(check_col..)
                                .and_then(|s| s.graphemes(true).next())
                        } else {
                            None
                        };

                        match after_current {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                // Another word right after - continue to find it
                            }
                            Some(gg) if Self::is_whitespace_char(gg) => {
                                // Whitespace after - if we moved to a NEW position (not same as start),
                                // return end of current word. But if we're at same position as start
                                // (e.g., single char), find next word instead.
                                if check_col - 1 > start_col {
                                    return Some(Cursor::new(line_idx, check_col - 1));
                                }
                                // Fall through to find next word
                            }
                            None => {
                                // End of line - don't return here, fall through to wrap
                            }
                            _ => {}
                        }
                    }

                    // Try to find next word on current line (skip whitespace, find word)
                    // Track original position to know if we found whitespace
                    let pre_whitespace_col = check_col;

                    // Skip whitespace to find next bigword
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => break,
                            Some(gg) => check_col += gg.len(),
                            None => break,
                        }
                    }

                    // Now at start of next bigword, find its end
                    while check_col < line_len {
                        let g = line_str
                            .get(check_col..)
                            .and_then(|s| s.graphemes(true).next());
                        match g {
                            Some(gg) if Self::is_bigword_char(gg) => {
                                check_col += gg.len();
                            }
                            _ => break,
                        }
                    }

                    // Return position AT last character (not after)
                    // Only return if we found a next word (check_col advanced past pre_whitespace_col)
                    // AND we moved forward from start
                    let found_next_word = check_col > pre_whitespace_col && check_col > start_col;
                    // Special case: if we started at position 0 (wrapped from previous line) and found a word
                    let started_at_zero = start_col == 0 && check_col > 0;

                    if (found_next_word || started_at_zero) && check_col <= line_len + 1 {
                        return Some(Cursor::new(line_idx, check_col - 1));
                    }
                    // No next word found on this line - fall through to wrap to next line
                }
            }

            // Move to next line
            line_idx += 1;
            col = 0;
        }

        None
    }

    /// Find the previous boundary position backward from cursor.
    ///
    /// Returns None if no boundary exists in the backward direction.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::{Buffer, Cursor, Boundary};
    ///
    /// let buf = Buffer::from_str("hello world");
    /// let prev = buf.prev_boundary(Cursor::new(0, 6), Boundary::Word);
    /// assert_eq!(prev, Some(Cursor::new(0, 0))); // at 'h'
    /// ```
    pub fn prev_boundary(&self, cursor: Cursor, boundary: Boundary) -> Option<Cursor> {
        let mut line_idx = cursor.line;
        let mut col = cursor.col;

        // If at start of line, move to end of previous line
        if col == 0 {
            if line_idx == 0 {
                return None;
            }
            line_idx -= 1;
            col = self.line_len(line_idx);
        }

        loop {
            if line_idx >= self.line_count() {
                if line_idx == 0 {
                    return None;
                }
                line_idx -= 1;
                col = self.line_len(line_idx);
                continue;
            }

            let line = self.line_at(line_idx)?;

            let line_str = line.as_ref();
            let line_len = line_str.len();

            if line_len == 0 {
                if line_idx == 0 {
                    return None;
                }
                line_idx -= 1;
                col = self.line_len(line_idx);
                continue;
            }

            // Clamp col
            if col > line_len {
                col = line_len;
            }

            // Scan backward looking for boundary
            let mut check_col = col;
            while check_col > 0 {
                // Move back one grapheme
                let mut prev_offset = 0;
                let mut found = false;
                for (byte_offset, _g) in line_str.grapheme_indices(true) {
                    if byte_offset >= check_col {
                        break;
                    }
                    prev_offset = byte_offset;
                    found = true;
                }
                if !found {
                    break;
                }
                check_col = prev_offset;

                // Check if this position is a boundary (not the starting position)
                if check_col < col {
                    let check_cursor = Cursor::new(line_idx, check_col);
                    if self.is_at_boundary(check_cursor, boundary) {
                        return Some(check_cursor);
                    }
                }
            }

            // Try previous line
            if line_idx == 0 {
                return None;
            }
            line_idx -= 1;
            col = self.line_len(line_idx);
        }
    }

    /// Find the next occurrence of a character forward from cursor position.
    /// Searches from cursor.col + 1 onwards within the same line.
    /// Returns the cursor position ON the found character.
    /// If count > 1, finds the count-th occurrence.
    /// Returns None if character not found.
    pub fn find_char_forward(
        &self,
        cursor: Cursor,
        target: char,
        count: usize,
    ) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let line_str = line.as_ref();

        // Start searching from cursor.col + 1 grapheme forward
        // Use grapheme_indices to properly handle Unicode grapheme clusters
        let start_col = cursor.col + 1;

        // Collect all occurrences of target character from start_col onwards
        // grapheme_indices(true) gives us byte offsets of each grapheme cluster start
        let mut occurrences: Vec<usize> = Vec::new();
        for (grapheme_idx, grapheme) in line_str.grapheme_indices(true) {
            if grapheme_idx >= start_col {
                // Check if the grapheme starts with the target character
                if grapheme.starts_with(target) {
                    occurrences.push(grapheme_idx);
                }
            }
        }

        // Return the count-th occurrence (1-indexed, so count-1)
        let target_idx = occurrences.get(count.saturating_sub(1))?;
        Some(Cursor::new(line_idx, *target_idx))
    }

    /// Find the previous occurrence of a character backward from cursor position.
    /// Searches from cursor.col - 1 backwards within the same line.
    /// Returns the cursor position ON the found character.
    /// If count > 1, finds the count-th previous occurrence.
    /// Returns None if character not found.
    pub fn find_char_backward(
        &self,
        cursor: Cursor,
        target: char,
        count: usize,
    ) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let line_str = line.as_ref();

        // When searching backward, we need to find characters BEFORE cursor position.
        // If cursor is ON the target character, we should skip it and find the previous one.
        // The issue: when cursor is at position P which is the start of a grapheme,
        // searching from P-1 with idx <= P-1 might not include P-1 if P-1 is the target.
        //
        // For "helllo" with cursor on 3rd 'l' at position 3:
        // - We want to find 2nd 'l' at position 2
        // - Search from position 2 (cursor.col - 1), include idx >= 2 that match
        //
        // For "hello" with cursor on 1st 'l' at position 2:
        // - We want to find nothing (no 'l' before position 2)
        // - Search from position 1, but only idx < 2 that match (excludes idx 2)
        //
        // The key insight: search with idx < cursor.col, which includes all positions
        // strictly before the cursor. Then take the last 'count' occurrences.

        // Collect all occurrences of target character with idx < cursor.col
        // Use grapheme_indices(true) to properly handle Unicode grapheme clusters
        let occurrences: Vec<usize> = line_str
            .grapheme_indices(true)
            .filter(|&(idx, grapheme)| idx < cursor.col && grapheme.starts_with(target))
            .map(|(idx, _)| idx)
            .collect();

        // Get the count-th occurrence from the end (rightmost = first when going backward)
        // occurrences is in ascending order, so rightmost is at len-1
        let target_idx = occurrences.len().saturating_sub(count);
        let idx = *occurrences.get(target_idx)?;
        Some(Cursor::new(line_idx, idx))
    }
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

    // next_cursor tests

    #[test]
    fn test_next_cursor_ascii() {
        let buf = Buffer::from_str("hello");

        assert_eq!(buf.next_cursor(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        assert_eq!(buf.next_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 2)));
        assert_eq!(buf.next_cursor(Cursor::new(0, 4)), Some(Cursor::new(0, 5)));
        assert_eq!(buf.next_cursor(Cursor::new(0, 5)), None); // at end of line, last line
    }

    #[test]
    fn test_next_cursor_multibyte() {
        let buf = Buffer::from_str("aβc"); // 'β' is 2 bytes

        assert_eq!(buf.next_cursor(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        assert_eq!(buf.next_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 3))); // jump over β
        assert_eq!(buf.next_cursor(Cursor::new(0, 3)), Some(Cursor::new(0, 4)));
    }

    #[test]
    fn test_next_cursor_emoji() {
        let buf = Buffer::from_str("a😀c"); // emoji is 4 bytes

        assert_eq!(buf.next_cursor(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        assert_eq!(buf.next_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 5))); // jump over emoji
        assert_eq!(buf.next_cursor(Cursor::new(0, 5)), Some(Cursor::new(0, 6)));
    }

    #[test]
    fn test_next_cursor_across_newline() {
        let buf = Buffer::from_str("ab\ncd");

        // "ab" has byte len 2, "cd" has byte len 2
        assert_eq!(buf.next_cursor(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        assert_eq!(buf.next_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 2)));
        assert_eq!(buf.next_cursor(Cursor::new(0, 2)), Some(Cursor::new(1, 0))); // cross newline
        assert_eq!(buf.next_cursor(Cursor::new(1, 0)), Some(Cursor::new(1, 1)));
        // At col 2 (end of "cd"), moving right goes past end -> None
        assert_eq!(buf.next_cursor(Cursor::new(1, 2)), None);
    }

    #[test]
    fn test_next_cursor_at_end_of_last_line() {
        let buf = Buffer::from_str("ab\ncd");

        // At end of last line, moving right stays in place (returns None)
        assert_eq!(buf.next_cursor(Cursor::new(1, 2)), None);
    }

    // prev_cursor tests

    #[test]
    fn test_prev_cursor_ascii() {
        let buf = Buffer::from_str("hello");

        assert_eq!(buf.prev_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
        assert_eq!(buf.prev_cursor(Cursor::new(0, 5)), Some(Cursor::new(0, 4)));
        assert_eq!(buf.prev_cursor(Cursor::new(0, 0)), None); // at start
    }

    #[test]
    fn test_prev_cursor_multibyte() {
        let buf = Buffer::from_str("aβc"); // 'β' is 2 bytes

        assert_eq!(buf.prev_cursor(Cursor::new(0, 3)), Some(Cursor::new(0, 1))); // jump over β
        assert_eq!(buf.prev_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
    }

    #[test]
    fn test_prev_cursor_emoji() {
        let buf = Buffer::from_str("a😀c"); // emoji is 4 bytes

        assert_eq!(buf.prev_cursor(Cursor::new(0, 5)), Some(Cursor::new(0, 1))); // jump over emoji
        assert_eq!(buf.prev_cursor(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
    }

    #[test]
    fn test_prev_cursor_across_newline() {
        let buf = Buffer::from_str("ab\ncd");

        assert_eq!(buf.prev_cursor(Cursor::new(1, 0)), Some(Cursor::new(0, 2))); // cross newline
        assert_eq!(buf.prev_cursor(Cursor::new(0, 2)), Some(Cursor::new(0, 1)));
    }

    #[test]
    fn test_prev_cursor_at_start() {
        let buf = Buffer::from_str("ab");

        assert_eq!(buf.prev_cursor(Cursor::new(0, 0)), None);
    }

    // next_cursor_line tests

    #[test]
    fn test_next_cursor_line_ascii() {
        let buf = Buffer::from_str("hello");

        // At start, next is col 1
        assert_eq!(buf.next_cursor_line(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        // At middle
        assert_eq!(buf.next_cursor_line(Cursor::new(0, 2)), Some(Cursor::new(0, 3)));
        // At last char, next is end of line
        assert_eq!(buf.next_cursor_line(Cursor::new(0, 4)), Some(Cursor::new(0, 5)));
        // At end of line, None
        assert_eq!(buf.next_cursor_line(Cursor::new(0, 5)), None);
    }

    #[test]
    fn test_next_cursor_line_emoji() {
        let buf = Buffer::from_str("a😀b"); // emoji is 4 bytes

        // At 'a', next is start of emoji
        assert_eq!(buf.next_cursor_line(Cursor::new(0, 0)), Some(Cursor::new(0, 1)));
        // At emoji, next is 'b'
        assert_eq!(buf.next_cursor_line(Cursor::new(0, 1)), Some(Cursor::new(0, 5)));
        // At 'b', next is end of line
        assert_eq!(buf.next_cursor_line(Cursor::new(0, 5)), Some(Cursor::new(0, 6)));
        // At end of line, None
        assert_eq!(buf.next_cursor_line(Cursor::new(0, 6)), None);
    }

    #[test]
    fn test_next_cursor_line_at_end_of_line() {
        let buf = Buffer::from_str("hello");

        // At end of line, returns None (doesn't wrap to next line)
        assert_eq!(buf.next_cursor_line(Cursor::new(0, 5)), None);
    }

    // prev_cursor_line tests

    #[test]
    fn test_prev_cursor_line_ascii() {
        let buf = Buffer::from_str("hello");

        // At col 1, prev is col 0
        assert_eq!(buf.prev_cursor_line(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
        // At col 3, prev is col 2
        assert_eq!(buf.prev_cursor_line(Cursor::new(0, 3)), Some(Cursor::new(0, 2)));
        // At start, None
        assert_eq!(buf.prev_cursor_line(Cursor::new(0, 0)), None);
    }

    #[test]
    fn test_prev_cursor_line_emoji() {
        let buf = Buffer::from_str("a😀b"); // emoji is 4 bytes

        // At emoji start, prev is 'a'
        assert_eq!(buf.prev_cursor_line(Cursor::new(0, 1)), Some(Cursor::new(0, 0)));
        // At 'b', prev is emoji start
        assert_eq!(buf.prev_cursor_line(Cursor::new(0, 5)), Some(Cursor::new(0, 1)));
        // At start, None
        assert_eq!(buf.prev_cursor_line(Cursor::new(0, 0)), None);
    }

    #[test]
    fn test_prev_cursor_line_at_start_of_line() {
        let buf = Buffer::from_str("hello");

        // At start of line, returns None (doesn't wrap to prev line)
        assert_eq!(buf.prev_cursor_line(Cursor::new(0, 0)), None);
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

    // cursor_end_of_line tests

    #[test]
    fn test_cursor_end_of_line_middle_of_line() {
        let buf = Buffer::from_str("hello");

        // In middle of line, move to end
        assert_eq!(
            buf.cursor_end_of_line(Cursor::new(0, 2)),
            Some(Cursor::new(0, 4))
        );
    }

    #[test]
    fn test_cursor_end_of_line_at_end_wraps() {
        let buf = Buffer::from_str("hello\nworld");

        // At end of line, wraps to next line's end
        assert_eq!(
            buf.cursor_end_of_line(Cursor::new(0, 4)),
            Some(Cursor::new(1, 4))
        );
    }

    #[test]
    fn test_cursor_end_of_line_at_end_of_last_line() {
        let buf = Buffer::from_str("hello\nworld");

        // At end of last line, no movement
        assert_eq!(buf.cursor_end_of_line(Cursor::new(1, 4)), None);
    }

    #[test]
    fn test_cursor_end_of_line_empty_buffer() {
        let buf = Buffer::new();

        // Empty buffer, no movement
        assert_eq!(buf.cursor_end_of_line(Cursor::new(0, 0)), None);
    }

    #[test]
    fn test_cursor_end_of_line_empty_line() {
        let buf = Buffer::from_str("hello\n\nworld");

        // Empty line in middle, wrap to next line (empty line)
        assert_eq!(
            buf.cursor_end_of_line(Cursor::new(0, 4)),
            Some(Cursor::new(1, 0))
        );
    }

    #[test]
    fn test_cursor_end_of_line_with_trailing_whitespace() {
        let buf = Buffer::from_str("hello   ");

        // Should move to last non-whitespace character
        assert_eq!(
            buf.cursor_end_of_line(Cursor::new(0, 0)),
            Some(Cursor::new(0, 4))
        );
    }

    #[test]
    fn test_cursor_end_of_line_with_wide_characters() {
        let buf = Buffer::from_str("hello😀world");

        // "hello" (5 bytes) + "😀" (4 bytes) = 9 bytes, then "world" (5 bytes) = 14 bytes total
        // Last char 'd' is at byte 13
        assert_eq!(
            buf.cursor_end_of_line(Cursor::new(0, 0)),
            Some(Cursor::new(0, 13))
        );
    }

    // cursor_start_of_line tests

    #[test]
    fn test_cursor_start_of_line_middle_of_line() {
        let buf = Buffer::from_str("  hello");

        // In middle of line - move to column 0
        assert_eq!(
            buf.cursor_start_of_line(Cursor::new(0, 5)),
            Some(Cursor::new(0, 0))
        );
    }

    #[test]
    fn test_cursor_start_of_line_at_column_zero_wraps() {
        let buf = Buffer::from_str("  hello\n  world");

        // At column 0 on line 1 - wrap to previous line
        assert_eq!(
            buf.cursor_start_of_line(Cursor::new(1, 0)),
            Some(Cursor::new(0, 0))
        );
    }

    #[test]
    fn test_cursor_start_of_line_at_first_line_no_wrap() {
        let buf = Buffer::from_str("  hello");

        // At column 0 on first line - no movement
        assert_eq!(buf.cursor_start_of_line(Cursor::new(0, 0)), None);
    }

    #[test]
    fn test_cursor_start_of_line_empty_buffer() {
        let buf = Buffer::from_str("");

        // Empty buffer - no movement
        assert_eq!(buf.cursor_start_of_line(Cursor::new(0, 0)), None);
    }

    // cursor_content_start_of_line tests

    #[test]
    fn test_cursor_content_start_of_line_middle_of_line() {
        let buf = Buffer::from_str("  hello");

        // In middle of line - move to first non-whitespace
        assert_eq!(
            buf.cursor_content_start_of_line(Cursor::new(0, 5)),
            Some(Cursor::new(0, 2))
        );
    }

    #[test]
    fn test_cursor_content_start_of_line_at_first_non_ws() {
        let buf = Buffer::from_str("  hello\n  world");

        // At first non-whitespace on line 1 - wrap to previous line (line 0)
        assert_eq!(
            buf.cursor_content_start_of_line(Cursor::new(1, 2)),
            Some(Cursor::new(0, 2))
        );
    }

    #[test]
    fn test_cursor_content_start_of_line_at_first_line_no_wrap() {
        let buf = Buffer::from_str("  hello");

        // At first non-whitespace of first line - no movement
        assert_eq!(buf.cursor_content_start_of_line(Cursor::new(0, 2)), None);
    }

    #[test]
    fn test_cursor_content_start_of_line_no_leading_whitespace() {
        let buf = Buffer::from_str("hello");

        // No leading whitespace - already at first non-whitespace
        assert_eq!(buf.cursor_content_start_of_line(Cursor::new(0, 0)), None);
    }

    #[test]
    fn test_cursor_content_start_of_line_empty_buffer() {
        let buf = Buffer::from_str("");

        // Empty buffer - no movement
        assert_eq!(buf.cursor_content_start_of_line(Cursor::new(0, 0)), None);
    }

    #[test]
    fn test_cursor_content_start_of_line_empty_line() {
        let buf = Buffer::from_str("  \nhello");

        // At first non-whitespace on line 1 - wrap to previous line which is empty
        // Previous line has no non-whitespace, so move to column 0
        assert_eq!(
            buf.cursor_content_start_of_line(Cursor::new(1, 0)),
            Some(Cursor::new(0, 0))
        );
    }

    #[test]
    fn test_cursor_content_start_of_line_with_wide_characters() {
        let buf = Buffer::from_str("  hello😀world");

        // First non-whitespace is 'h' at byte 2
        assert_eq!(
            buf.cursor_content_start_of_line(Cursor::new(0, 5)),
            Some(Cursor::new(0, 2))
        );
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

        let cursor_after_first_char = buf.next_cursor(Cursor::new(0, 0));
        assert_eq!(cursor_after_first_char, Some(Cursor::new(0, 3)));

        if let Some(cursor) = cursor_after_first_char {
            buf.insert_char(cursor, 'X');
        }

        assert_eq!(buf.as_str(), "日X本語");

        let cursor_after_insert = buf.next_cursor(cursor_after_first_char.unwrap());
        assert_eq!(cursor_after_insert, Some(Cursor::new(0, 4)));
    }

    #[test]
    fn test_insert_between_emoji_via_cursor_movement() {
        let mut buf = Buffer::from_str("😀😀");

        assert_eq!(buf.as_str(), "😀😀");
        assert_eq!(buf.line_len(0), 8);

        let cursor_after_first_emoji = buf.next_cursor(Cursor::new(0, 0));
        assert_eq!(cursor_after_first_emoji, Some(Cursor::new(0, 4)));

        if let Some(cursor) = cursor_after_first_emoji {
            buf.insert_char(cursor, 'X');
        }

        assert_eq!(buf.as_str(), "😀X😀");

        let cursor_after_insert = buf.next_cursor(cursor_after_first_emoji.unwrap());
        assert_eq!(cursor_after_insert, Some(Cursor::new(0, 5)));
    }

    #[test]
    fn test_insert_mid_emoji_via_cursor_movement() {
        let mut buf = Buffer::from_str("a😀b");

        assert_eq!(buf.as_str(), "a😀b");

        let cursor_after_emoji = buf.next_cursor(Cursor::new(0, 1));
        assert_eq!(cursor_after_emoji, Some(Cursor::new(0, 5)));

        if let Some(cursor) = cursor_after_emoji {
            buf.insert_char(cursor, 'X');
        }

        assert_eq!(buf.as_str(), "a😀Xb");
    }

    // Boundary motion tests

    #[test]
    fn test_word_forward_at_last_word() {
        // "hello world" - cursor at 'd' (last char), w should go to start of next word
        let buf = Buffer::from_str("hello world");
        // At position 10 ('d'), w should go to... wait, there's no next line, so should wrap or stay
        // Actually "hello world\nmore" - at 'd' in "world", w should go to 'm' in "more"
        let buf2 = Buffer::from_str("hello world\nmore");
        let result = buf2.next_boundary(Cursor::new(0, 10), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(1, 0))); // wraps to 'm' on line 1
    }

    #[test]
    fn test_word_forward_wrap_no_leading_whitespace() {
        // "hello\nworld" - at 'd' in "hello", w should go to 'w' on line 1 (first word)
        let buf = Buffer::from_str("hello\nworld");
        let result = buf.next_boundary(Cursor::new(0, 4), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(1, 0))); // wraps to 'w' on line 1
    }

    #[test]
    fn test_word_forward_at_word_end() {
        // "hello world" - at 'h' (start of "hello"), w should go to 'w' (start of "world")
        // This is NOT a wrapping case - it's moving to the next word on the same line
        let buf = Buffer::from_str("hello world");
        let result = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(0, 6))); // 'w'
    }

    #[test]
    fn test_word_forward_at_word_end_with_nonword() {
        // "hello---world" - at 'o' (position 4, end of "hello")
        // w should go to position 5 (first "-")
        // e should go to position 7 (end of "---")
        let buf = Buffer::from_str("hello---world");

        let result_w = buf.next_boundary(Cursor::new(0, 4), Boundary::Word);
        assert_eq!(
            result_w,
            Some(Cursor::new(0, 5)),
            "w should go to first '-'"
        );

        let result_e = buf.next_boundary(Cursor::new(0, 4), Boundary::WordEnd);
        assert_eq!(result_e, Some(Cursor::new(0, 7)), "e should go to last '-'");
    }

    #[test]
    fn test_word_end_at_nonword_sequence_end() {
        // "hello---world" - at last '-' (position 7), e should go to end of "world" (position 12)
        let buf = Buffer::from_str("hello---world");

        let result = buf.next_boundary(Cursor::new(0, 7), Boundary::WordEnd);
        assert_eq!(
            result,
            Some(Cursor::new(0, 12)),
            "e should go to end of 'world'"
        );
    }

    #[test]
    fn test_word_end_at_word_start() {
        // "hello world" - at 'h', e should go to 'o' (end of "hello")
        let buf = Buffer::from_str("hello world");
        let result = buf.next_boundary(Cursor::new(0, 0), Boundary::WordEnd);
        assert_eq!(result, Some(Cursor::new(0, 4))); // 'o'
    }

    #[test]
    fn test_word_end_at_word_end() {
        // "hello world" - at 'o' (end of "hello"), e should go to 'd' (end of "world")
        let buf = Buffer::from_str("hello world");
        let result = buf.next_boundary(Cursor::new(0, 4), Boundary::WordEnd);
        assert_eq!(result, Some(Cursor::new(0, 10))); // 'd'
    }

    #[test]
    fn test_word_end_at_last_char_wraps() {
        // "hello world\nfoo" - at 'd' in "world", e should wrap to 'o' in "foo"
        let buf = Buffer::from_str("hello world\nfoo");
        let result = buf.next_boundary(Cursor::new(0, 10), Boundary::WordEnd);
        assert_eq!(result, Some(Cursor::new(1, 2))); // 'o' in "foo"
    }

    #[test]
    fn test_bigword_forward_wrap_no_leading_whitespace() {
        // "hello\nworld" - at end of line 0, W should go to 'w' on line 1 (first word)
        let buf = Buffer::from_str("hello\nworld");
        let result = buf.next_boundary(Cursor::new(0, 4), Boundary::BigWord);
        assert_eq!(result, Some(Cursor::new(1, 0))); // wraps to 'w' on line 1
    }

    #[test]
    fn test_bigword_forward_wrap_with_leading_whitespace() {
        // "hello\n  world" - at end of line 0, W should skip the leading spaces and go to 'w' on line 1
        let buf = Buffer::from_str("hello\n  world");
        let result = buf.next_boundary(Cursor::new(0, 4), Boundary::BigWord);
        assert_eq!(result, Some(Cursor::new(1, 2))); // wraps to 'w' on line 1 (skipping 2 spaces)
    }

    // Non-word boundary tests (bug fix for "hello---world" case)

    #[test]
    fn test_word_forward_with_nonword_chars() {
        // "hello---world" - at 'h', w should go to first '-' (position 5)
        let buf = Buffer::from_str("hello---world");
        let result = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(0, 5))); // first '-'
    }

    #[test]
    fn test_word_forward_at_nonword_boundary() {
        // "hello---world" - at first '-' (position 5), w should go to 'w' of "world" (position 8)
        let buf = Buffer::from_str("hello---world");
        let result = buf.next_boundary(Cursor::new(0, 5), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(0, 8))); // first 'w' of "world"
    }

    #[test]
    fn test_word_forward_multiple_nonword_chars() {
        // "a...b" - at 'a', w should go to first '.' (position 1)
        let buf = Buffer::from_str("a...b");
        let result = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(0, 1))); // first '.'
    }

    #[test]
    fn test_word_forward_nonword_at_start() {
        // "...hello" - at first '.' (position 0), w should go to 'h' (position 3)
        let buf = Buffer::from_str("...hello");
        let result = buf.next_boundary(Cursor::new(0, 0), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(0, 3))); // 'h'
    }

    #[test]
    fn test_word_end_with_nonword_chars() {
        // "hello---world" - at 'h', e should go to 'o' (end of "hello")
        let buf = Buffer::from_str("hello---world");
        let result = buf.next_boundary(Cursor::new(0, 0), Boundary::WordEnd);
        assert_eq!(result, Some(Cursor::new(0, 4))); // 'o' (end of "hello")
    }

    #[test]
    fn test_word_end_at_nonword_boundary() {
        // "hello---world" - at first '-' (position 5), e should go to last '-' (position 7)
        let buf = Buffer::from_str("hello---world");
        let result = buf.next_boundary(Cursor::new(0, 5), Boundary::WordEnd);
        assert_eq!(result, Some(Cursor::new(0, 7))); // last '-' (end of "---")
    }

    #[test]
    fn test_word_backward_with_nonword_chars() {
        // "hello---world" - at 'd' (position 11), b should go to start of "world" (position 8)
        // This matches Vim behavior - b goes to start of current/previous word
        let buf = Buffer::from_str("hello---world");
        let result = buf.prev_boundary(Cursor::new(0, 11), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(0, 8))); // start of "world"
    }

    #[test]
    fn test_word_backward_at_nonword_boundary() {
        // "hello---world" - at first '-' (position 5), b should go to 'h' (position 0)
        let buf = Buffer::from_str("hello---world");
        let result = buf.prev_boundary(Cursor::new(0, 5), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(0, 0))); // 'h'
    }

    #[test]
    fn test_word_backward_at_word_boundary_after_nonword() {
        // "hello---world" - at first 'w' of "world" (position 8), b should go to first '-' (position 5)
        let buf = Buffer::from_str("hello---world");
        let result = buf.prev_boundary(Cursor::new(0, 8), Boundary::Word);
        assert_eq!(result, Some(Cursor::new(0, 5))); // first '-'
    }

    // BigWordEnd wrap test (bug fix for E key at end of line)

    #[test]
    fn test_bigword_end_at_end_of_word_wraps_to_next_line() {
        // "hello\nworld" - at end of line 0 (position 4), E should wrap to end of "world" on line 1
        let buf = Buffer::from_str("hello\nworld");
        let result = buf.next_boundary(Cursor::new(0, 4), Boundary::BigWordEnd);
        assert_eq!(result, Some(Cursor::new(1, 4))); // end of "world" on line 1
    }

    #[test]
    fn test_bigword_end_in_middle_of_word() {
        // "hello world" - at position 2 ('l'), E should go to end of "hello" (position 4)
        let buf = Buffer::from_str("hello world");
        let result = buf.next_boundary(Cursor::new(0, 2), Boundary::BigWordEnd);
        assert_eq!(result, Some(Cursor::new(0, 4))); // end of "hello"
    }

    #[test]
    fn test_bigword_end_at_last_char_with_next_word() {
        // "hello world" - at last char of "hello" (position 4), E should go to end of "world" (position 10)
        let buf = Buffer::from_str("hello world");
        let result = buf.next_boundary(Cursor::new(0, 4), Boundary::BigWordEnd);
        assert_eq!(result, Some(Cursor::new(0, 10))); // end of "world"
    }

    // Delete character tests

    #[test]
    fn test_delete_char_before_cursor_in_middle() {
        let mut buf = Buffer::from_str("hello");
        let cursor = Cursor::new(0, 3); // at 'l'
        let new_cursor = buf.delete_char_before_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 2))); // cursor moves back
        assert_eq!(buf.as_str(), "helo");
    }

    #[test]
    fn test_delete_char_before_cursor_at_start() {
        let mut buf = Buffer::from_str("hello");
        let cursor = Cursor::new(0, 0); // at start
        let new_cursor = buf.delete_char_before_cursor(cursor);
        assert_eq!(new_cursor, None); // nothing to delete
        assert_eq!(buf.as_str(), "hello");
    }

    #[test]
    fn test_delete_char_before_cursor_at_doc_start() {
        let mut buf = Buffer::from_str("hello");
        let cursor = Cursor::new(0, 0);
        let new_cursor = buf.delete_char_before_cursor(cursor);
        assert_eq!(new_cursor, None);
    }

    #[test]
    fn test_delete_char_before_cursor_joins_lines() {
        let mut buf = Buffer::from_str("hello\nworld");
        let cursor = Cursor::new(1, 0); // at start of line 1
        let new_cursor = buf.delete_char_before_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 5))); // at end of "hello"
        assert_eq!(buf.as_str(), "helloworld");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_delete_char_before_cursor_unicode() {
        // "héllo" - 'é' is a single grapheme (2 bytes: é = [0xc3, 0xa9])
        // Byte layout: h(0), é(1-2), l(3), l(4), o(5)
        // Cursor at byte 3 (first 'l'), should delete 'é' (bytes 1-2)
        let mut buf = Buffer::from_str("héllo");
        let cursor = Cursor::new(0, 3); // at first 'l' (byte 3)
        let new_cursor = buf.delete_char_before_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor at start of 'é'
        assert_eq!(buf.as_str(), "hllo"); // "é" removed as single unit
    }

    #[test]
    fn test_delete_char_before_cursor_emoji() {
        // "a👍b" - emoji is 4 bytes, single grapheme
        // Byte layout: a(0), 👍(1-4), b(5)
        let mut buf = Buffer::from_str("a👍b");
        let cursor = Cursor::new(0, 5); // at 'b' (byte 5)
        let new_cursor = buf.delete_char_before_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor at 'a'
        assert_eq!(buf.as_str(), "ab"); // "👍" removed as single unit
    }

    #[test]
    fn test_delete_char_at_cursor_in_middle() {
        let mut buf = Buffer::from_str("hello");
        let cursor = Cursor::new(0, 1); // at 'e'
        let new_cursor = buf.delete_char_at_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor stays
        assert_eq!(buf.as_str(), "hllo");
    }

    #[test]
    fn test_delete_char_at_cursor_at_end() {
        let mut buf = Buffer::from_str("hello");
        let cursor = Cursor::new(0, 5); // at end
        let new_cursor = buf.delete_char_at_cursor(cursor);
        assert_eq!(new_cursor, None); // nothing to delete at end
        assert_eq!(buf.as_str(), "hello");
    }

    #[test]
    fn test_delete_char_at_cursor_at_doc_end() {
        let mut buf = Buffer::from_str("hello");
        let cursor = Cursor::new(0, 5); // at end of single line
        let new_cursor = buf.delete_char_at_cursor(cursor);
        assert_eq!(new_cursor, None);
    }

    #[test]
    fn test_delete_char_at_cursor_joins_lines() {
        let mut buf = Buffer::from_str("hello\nworld");
        let cursor = Cursor::new(0, 5); // at end of line 0
        let new_cursor = buf.delete_char_at_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 5))); // cursor stays at end of first line
        assert_eq!(buf.as_str(), "helloworld");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_delete_char_at_cursor_unicode() {
        // "héllo" - 'é' is a single grapheme (2 bytes)
        // Byte layout: h(0), é(1-2), l(3), l(4), o(5)
        // Cursor at byte 0 (at 'h'), should delete 'h'
        let mut buf = Buffer::from_str("héllo");
        let cursor = Cursor::new(0, 0); // at 'h' (byte 0)
        let new_cursor = buf.delete_char_at_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 0))); // cursor stays at start
        assert_eq!(buf.as_str(), "éllo"); // "h" removed
    }

    #[test]
    fn test_delete_char_at_cursor_emoji() {
        // "a👍b" - emoji is 4 bytes, single grapheme
        let mut buf = Buffer::from_str("a👍b");
        let cursor = Cursor::new(0, 1); // at emoji
        let new_cursor = buf.delete_char_at_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor stays at position
        assert_eq!(buf.as_str(), "ab"); // "👍" removed as single unit
    }

    #[test]
    fn test_delete_char_at_cursor_last_line_joins_next() {
        // When at end of last line, should try to join with next line (but none exists)
        let mut buf = Buffer::from_str("hello\nworld");
        let cursor = Cursor::new(1, 5); // at end of line 1 (last line)
        let new_cursor = buf.delete_char_at_cursor(cursor);
        assert_eq!(new_cursor, None); // nothing to join with
        assert_eq!(buf.as_str(), "hello\nworld");
    }

    #[test]
    fn test_delete_char_at_cursor_not_at_end_joins_next() {
        // When in middle of line, delete just removes character (no line join)
        let mut buf = Buffer::from_str("ab\ncd");
        let cursor = Cursor::new(0, 1); // at 'b' (not at end which is col 2)
        let new_cursor = buf.delete_char_at_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor stays
        assert_eq!(buf.as_str(), "a\ncd"); // 'b' removed, lines not joined
    }

    #[test]
    fn test_insert_mode_delete_at_position_1() {
        // Simulate insert mode: cursor at position 1 in "abc"
        // This is between 'a' (pos 0) and 'b' (pos 1)
        // Delete should remove 'b' (at cursor), cursor stays at 1
        let mut buf = Buffer::from_str("abc");
        let cursor = Cursor::new(0, 1);
        let new_cursor = buf.delete_char_at_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 1))); // cursor stays at position 1
        assert_eq!(buf.as_str(), "ac"); // 'b' removed
    }

    #[test]
    fn test_insert_mode_backspace_at_position_1() {
        // Simulate insert mode: cursor at position 1 in "abc"
        // This is between 'a' (pos 0) and 'b' (pos 1)
        // Backspace should remove 'a' (before cursor), cursor moves to 0
        let mut buf = Buffer::from_str("abc");
        let cursor = Cursor::new(0, 1);
        let new_cursor = buf.delete_char_before_cursor(cursor);
        assert_eq!(new_cursor, Some(Cursor::new(0, 0))); // cursor moves back to position 0
        assert_eq!(buf.as_str(), "bc"); // 'a' removed
    }

    // Join lines tests

    #[test]
    fn test_join_lines_with_space() {
        let mut buf = Buffer::from_str("hello\nworld");
        let cursor = buf.join_lines(0, 2, true);
        assert_eq!(cursor, Some(Cursor::new(0, 11))); // "hello world" has 11 chars
        assert_eq!(buf.as_str(), "hello world");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_join_lines_without_space() {
        let mut buf = Buffer::from_str("hello\nworld");
        let cursor = buf.join_lines(0, 2, false);
        assert_eq!(cursor, Some(Cursor::new(0, 10))); // "helloworld" has 10 chars
        assert_eq!(buf.as_str(), "helloworld");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_join_lines_multiple_with_space() {
        let mut buf = Buffer::from_str("a\nb\nc\nd");
        let cursor = buf.join_lines(0, 4, true);
        assert_eq!(cursor, Some(Cursor::new(0, 7))); // "a b c d" has 7 chars
        assert_eq!(buf.as_str(), "a b c d");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_join_lines_multiple_without_space() {
        let mut buf = Buffer::from_str("a\nb\nc\nd");
        let cursor = buf.join_lines(0, 4, false);
        assert_eq!(cursor, Some(Cursor::new(0, 4))); // "abcd" has 4 chars
        assert_eq!(buf.as_str(), "abcd");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_join_lines_on_last_line_returns_none() {
        let mut buf = Buffer::from_str("hello\nworld");
        let cursor = buf.join_lines(1, 2, true); // Try to join from last line
        assert_eq!(cursor, None);
        assert_eq!(buf.as_str(), "hello\nworld"); // Unchanged
    }

    #[test]
    fn test_join_lines_insufficient_lines() {
        let mut buf = Buffer::from_str("hello\nworld");
        let cursor = buf.join_lines(0, 5, true); // Only 2 lines available
        assert_eq!(cursor, Some(Cursor::new(0, 11))); // Still joins the 2 lines
        assert_eq!(buf.as_str(), "hello world");
    }

    #[test]
    fn test_join_lines_with_empty_line() {
        let mut buf = Buffer::from_str("hello\n\nworld");
        // Join all 3 lines (hello, empty, world) with space
        let cursor = buf.join_lines(0, 3, true);
        assert_eq!(cursor, Some(Cursor::new(0, 12))); // "hello  world" (2 spaces) has 12 chars
        assert_eq!(buf.as_str(), "hello  world");
    }

    #[test]
    fn test_join_lines_invalid_start_line() {
        let mut buf = Buffer::from_str("hello\nworld");
        let cursor = buf.join_lines(5, 2, true);
        assert_eq!(cursor, None);
    }

    #[test]
    fn test_join_lines_count_one_returns_none() {
        let mut buf = Buffer::from_str("hello\nworld");
        let cursor = buf.join_lines(0, 1, true); // line_count < 2
        assert_eq!(cursor, None);
    }

    #[test]
    fn test_delete_lines_single_line() {
        let mut buf = Buffer::from_str("line1\nline2\nline3");
        let cursor = buf.delete_lines(0, 1);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "line2\nline3");
    }

    #[test]
    fn test_delete_lines_multiple_lines() {
        let mut buf = Buffer::from_str("line1\nline2\nline3\nline4");
        let cursor = buf.delete_lines(0, 2);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "line3\nline4");
    }

    #[test]
    fn test_delete_lines_from_middle() {
        let mut buf = Buffer::from_str("line1\nline2\nline3\nline4\nline5");
        let cursor = buf.delete_lines(1, 2);
        assert_eq!(cursor, Some(Cursor::new(1, 0)));
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.as_str(), "line1\nline4\nline5");
    }

    #[test]
    fn test_delete_lines_from_last_line() {
        let mut buf = Buffer::from_str("line1\nline2\nline3");
        let cursor = buf.delete_lines(2, 1);
        assert_eq!(cursor, Some(Cursor::new(1, 0)));
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "line1\nline2");
    }

    #[test]
    fn test_delete_lines_only_line() {
        let mut buf = Buffer::from_str("only line");
        let cursor = buf.delete_lines(0, 1);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.as_str(), "");
    }

    #[test]
    fn test_delete_lines_count_exceeds_available() {
        let mut buf = Buffer::from_str("line1\nline2\nline3");
        let cursor = buf.delete_lines(1, 10); // Only 2 lines from index 1
        assert_eq!(cursor, Some(Cursor::new(0, 0))); // Only line1 remains, at index 0
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.as_str(), "line1");
    }

    #[test]
    fn test_delete_lines_invalid_start_line() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.delete_lines(5, 1);
        assert_eq!(cursor, None);
    }

    #[test]
    fn test_delete_lines_empty_buffer() {
        let mut buf = Buffer::new();
        let cursor = buf.delete_lines(0, 1);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.as_str(), "");
    }

    #[test]
    fn test_change_lines_single_line() {
        let mut buf = Buffer::from_str("hello\nworld\ntest");
        let cursor = buf.change_lines(0, 1);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.as_str(), "\nworld\ntest");  // First line replaced with blank
    }

    #[test]
    fn test_change_lines_multiple_lines() {
        let mut buf = Buffer::from_str("line1\nline2\nline3\nline4");
        let cursor = buf.change_lines(0, 2);  // Change 2 lines, leave 1 blank
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.as_str(), "\nline3\nline4");  // 2 lines replaced with 1 blank
    }

    #[test]
    fn test_change_lines_from_middle() {
        let mut buf = Buffer::from_str("line1\nline2\nline3\nline4");
        let cursor = buf.change_lines(1, 1);  // Change line 2
        assert_eq!(cursor, Some(Cursor::new(1, 0)));
        assert_eq!(buf.line_count(), 4);
        assert_eq!(buf.as_str(), "line1\n\nline3\nline4");  // line2 replaced with blank
    }

    #[test]
    fn test_change_lines_from_last_line() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.change_lines(1, 1);  // Change last line
        assert_eq!(cursor, Some(Cursor::new(1, 0)));
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "line1\n");  // last line replaced with blank
    }

    #[test]
    fn test_change_lines_only_line() {
        let mut buf = Buffer::from_str("only line");
        let cursor = buf.change_lines(0, 1);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.as_str(), "");  // Line replaced with blank
    }

    #[test]
    fn test_change_lines_count_exceeds_available() {
        let mut buf = Buffer::from_str("line1\nline2\nline3");
        let cursor = buf.change_lines(0, 5);  // Try to change 5 lines, only 3 exist
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 1);  // Should leave 1 blank line
        assert_eq!(buf.as_str(), "");  // All 3 lines replaced with 1 blank
    }

    #[test]
    fn test_change_lines_invalid_start_line() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.change_lines(5, 1);  // Start beyond available lines
        assert_eq!(cursor, None);
        assert_eq!(buf.as_str(), "line1\nline2");  // No change
    }

    #[test]
    fn test_change_lines_empty_buffer() {
        let mut buf = Buffer::new();
        let cursor = buf.change_lines(0, 1);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 1);
        assert_eq!(buf.as_str(), "");
    }

    // Tests for change_to_line_end

    #[test]
    fn test_change_to_line_end_middle_of_line() {
        let mut buf = Buffer::from_str("hello world");
        // Cursor after "hello" (position 5)
        let cursor = buf.change_to_line_end(Cursor::new(0, 5), 1);
        assert_eq!(cursor, Some(Cursor::new(0, 5)));
        assert_eq!(buf.as_str(), "hello");
    }

    #[test]
    fn test_change_to_line_end_at_start_of_line() {
        let mut buf = Buffer::from_str("hello world");
        // Cursor at position 0 (before "h")
        let cursor = buf.change_to_line_end(Cursor::new(0, 0), 1);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.as_str(), "");
    }

    #[test]
    fn test_change_to_line_end_at_end_of_line() {
        let mut buf = Buffer::from_str("hello");
        // Cursor at end of line (position 5)
        let cursor = buf.change_to_line_end(Cursor::new(0, 5), 1);
        assert_eq!(cursor, Some(Cursor::new(0, 5)));
        assert_eq!(buf.as_str(), "hello");  // No change
    }

    #[test]
    fn test_change_to_line_end_multiple_lines() {
        let mut buf = Buffer::from_str("hello world\nsecond line\nthird line");
        // Cursor after "hello" (position 5 on line 0), count=2 means lines 0 and 1
        // Delete from (0,5) to end of line 1
        // Result: "hello" + "third line" (line 2 remains)
        let cursor = buf.change_to_line_end(Cursor::new(0, 5), 2);
        assert_eq!(cursor, Some(Cursor::new(0, 5)));
        assert_eq!(buf.as_str(), "hello\nthird line");
    }

    #[test]
    fn test_change_to_line_end_count_exceeds_available() {
        let mut buf = Buffer::from_str("line1\nline2");
        // 10C on 2-line buffer should clamp to 2
        let cursor = buf.change_to_line_end(Cursor::new(0, 3), 10);
        assert_eq!(cursor, Some(Cursor::new(0, 3)));
        assert_eq!(buf.as_str(), "lin");
    }

    #[test]
    fn test_change_to_line_end_invalid_start_line() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.change_to_line_end(Cursor::new(5, 0), 1);
        assert_eq!(cursor, None);
        assert_eq!(buf.as_str(), "line1\nline2");  // No change
    }

    #[test]
    fn test_change_to_line_end_empty_buffer() {
        let mut buf = Buffer::new();
        let cursor = buf.change_to_line_end(Cursor::new(0, 0), 1);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.as_str(), "");
    }

    #[test]
    fn test_change_to_line_end_zero_count() {
        let mut buf = Buffer::from_str("hello world");
        let cursor = buf.change_to_line_end(Cursor::new(0, 5), 0);
        assert_eq!(cursor, Some(Cursor::new(0, 5)));
        assert_eq!(buf.as_str(), "hello world");  // No change
    }

    #[test]
    fn test_insert_lines_after_first_line() {
        let mut buf = Buffer::from_str("line1\nline2\nline3");
        let cursor = buf.insert_lines_after(0, 1);  // Insert after line 1
        assert_eq!(cursor, Some(Cursor::new(1, 0))); // At new line 2
        assert_eq!(buf.line_count(), 4);
        assert_eq!(buf.as_str(), "line1\n\nline2\nline3");
    }

    #[test]
    fn test_insert_lines_after_middle_line() {
        let mut buf = Buffer::from_str("line1\nline2\nline3");
        let cursor = buf.insert_lines_after(1, 1);  // Insert after line 2
        assert_eq!(cursor, Some(Cursor::new(2, 0))); // At new line 3
        assert_eq!(buf.line_count(), 4);
        assert_eq!(buf.as_str(), "line1\nline2\n\nline3");
    }

    #[test]
    fn test_insert_lines_after_last_line() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.insert_lines_after(1, 1);  // Insert after last line
        assert_eq!(cursor, Some(Cursor::new(2, 0))); // At new line 3 (index 2)
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.as_str(), "line1\nline2\n");
    }

    #[test]
    fn test_insert_lines_after_empty_buffer() {
        let mut buf = Buffer::new();  // Creates buffer with 1 empty line
        let cursor = buf.insert_lines_after(0, 1);
        assert_eq!(cursor, Some(Cursor::new(1, 0)));  // Cursor at new line (index 1)
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "\n");  // Two empty lines
    }

    #[test]
    fn test_insert_lines_after_multiple_lines() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.insert_lines_after(0, 3);  // Insert 3 lines after line 1
        assert_eq!(cursor, Some(Cursor::new(1, 0))); // At first inserted line
        assert_eq!(buf.line_count(), 5);
        assert_eq!(buf.as_str(), "line1\n\n\n\nline2");
    }

    #[test]
    fn test_insert_lines_after_zero_count() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.insert_lines_after(0, 0);  // No lines to insert
        assert_eq!(cursor, Some(Cursor::new(0, 0))); // Cursor at line 0
        assert_eq!(buf.line_count(), 2);  // No change
        assert_eq!(buf.as_str(), "line1\nline2");
    }

    #[test]
    fn test_insert_lines_after_count_exceeds() {
        let mut buf = Buffer::from_str("line1\nline2");
        // Insert after line 5 (beyond available), should append
        let cursor = buf.insert_lines_after(5, 2);
        assert_eq!(cursor, Some(Cursor::new(2, 0))); // At first inserted line
        assert_eq!(buf.line_count(), 4);
        assert_eq!(buf.as_str(), "line1\nline2\n\n");
    }

    #[test]
    fn test_insert_lines_before_first_line() {
        let mut buf = Buffer::from_str("line1\nline2\nline3");
        let cursor = buf.insert_lines_before(0, 1);  // Insert before line 1
        assert_eq!(cursor, Some(Cursor::new(0, 0))); // At new line 1
        assert_eq!(buf.line_count(), 4);
        assert_eq!(buf.as_str(), "\nline1\nline2\nline3");
    }

    #[test]
    fn test_insert_lines_before_middle_line() {
        let mut buf = Buffer::from_str("line1\nline2\nline3");
        let cursor = buf.insert_lines_before(1, 1);  // Insert before line 2
        assert_eq!(cursor, Some(Cursor::new(1, 0))); // At new line 2
        assert_eq!(buf.line_count(), 4);
        assert_eq!(buf.as_str(), "line1\n\nline2\nline3");
    }

    #[test]
    fn test_insert_lines_before_last_line() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.insert_lines_before(1, 1);  // Insert before line 2
        assert_eq!(cursor, Some(Cursor::new(1, 0))); // At new line 2
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.as_str(), "line1\n\nline2");
    }

    #[test]
    fn test_insert_lines_before_empty_buffer() {
        let mut buf = Buffer::new();
        let cursor = buf.insert_lines_before(0, 1);
        assert_eq!(cursor, Some(Cursor::new(0, 0)));
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "\n");
    }

    #[test]
    fn test_insert_lines_before_multiple_lines() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.insert_lines_before(0, 3);  // Insert 3 lines before line 1
        assert_eq!(cursor, Some(Cursor::new(0, 0))); // At first inserted line
        assert_eq!(buf.line_count(), 5);
        assert_eq!(buf.as_str(), "\n\n\nline1\nline2");
    }

    #[test]
    fn test_insert_lines_before_zero_count() {
        let mut buf = Buffer::from_str("line1\nline2");
        let cursor = buf.insert_lines_before(0, 0);  // No lines to insert
        assert_eq!(cursor, Some(Cursor::new(0, 0))); // Cursor at line 0
        assert_eq!(buf.line_count(), 2);
        assert_eq!(buf.as_str(), "line1\nline2");
    }

    #[test]
    fn test_insert_lines_before_count_exceeds() {
        let mut buf = Buffer::from_str("line1\nline2");
        // Insert before line 5 (beyond available), should append
        let cursor = buf.insert_lines_before(5, 2);
        assert_eq!(cursor, Some(Cursor::new(2, 0))); // At first inserted line
        assert_eq!(buf.line_count(), 4);
        assert_eq!(buf.as_str(), "line1\nline2\n\n");
    }

    // Paragraph motion tests

    #[test]
    fn test_is_blank_line() {
        let buf = Buffer::from_str("hello\n\n  \n\tworld\n");
        // Line 0: "hello" - not blank
        assert!(!buf.is_blank_line(0));
        // Line 1: "" - blank (empty between \n\n)
        assert!(buf.is_blank_line(1));
        // Line 2: "  " - blank (spaces only)
        assert!(buf.is_blank_line(2));
        // Line 3: "\tworld" - NOT blank (tab + "world" has non-whitespace)
        assert!(!buf.is_blank_line(3));
        // Note: trailing \n does NOT create an empty line in Rust's lines()
    }

    #[test]
    fn test_is_blank_line_out_of_bounds() {
        let buf = Buffer::from_str("hello");
        // Out of bounds should return false
        assert!(!buf.is_blank_line(5));
        assert!(!buf.is_blank_line(100));
    }

    #[test]
    fn test_cursor_paragraph_backward_from_paragraph() {
        // Buffer:
        // 0: "Para 1 line 1"
        // 1: "" (blank)
        // 2: "Para 2 line 1"
        // 3: "Para 2 line 2"
        // 4: "" (blank)
        // 5: "Para 3 line 1"
        let buf = Buffer::from_str("Para 1 line 1\n\nPara 2 line 1\nPara 2 line 2\n\nPara 3 line 1");

        // From middle of Para 2 (line 2), should find blank line before it (line 1)
        let cursor = Cursor::new(2, 5);
        let result = buf.cursor_paragraph_backward(cursor);
        assert_eq!(result, Some(Cursor::new(1, 0)));

        // From line 3 (Para 2 line 2), should find blank line before Para 2 (line 1)
        let cursor = Cursor::new(3, 5);
        let result = buf.cursor_paragraph_backward(cursor);
        assert_eq!(result, Some(Cursor::new(1, 0)));

        // From Para 1 (line 0), should find no previous blank line
        let cursor = Cursor::new(0, 5);
        let result = buf.cursor_paragraph_backward(cursor);
        assert_eq!(result, None);
    }

    #[test]
    fn test_cursor_paragraph_backward_from_blank_line() {
        // Buffer:
        // 0: "Para 1 line 1"
        // 1: "" (blank)
        // 2: "Para 2 line 1"
        // 3: "Para 2 line 2"
        // 4: "" (blank)
        // 5: "Para 3 line 1"
        let buf = Buffer::from_str("Para 1 line 1\n\nPara 2 line 1\nPara 2 line 2\n\nPara 3 line 1");

        // From blank line 1, should find blank line before Para 1 (none, returns None)
        let cursor = Cursor::new(1, 0);
        let result = buf.cursor_paragraph_backward(cursor);
        assert_eq!(result, None);

        // From blank line 4, should find blank line before Para 2 (line 1)
        let cursor = Cursor::new(4, 0);
        let result = buf.cursor_paragraph_backward(cursor);
        assert_eq!(result, Some(Cursor::new(1, 0)));
    }

    #[test]
    fn test_cursor_paragraph_backward_multiple_paragraphs() {
        // Buffer:
        // 0: "Para 1"
        // 1: "" (blank)
        // 2: "Para 2"
        // 3: "" (blank)
        // 4: "Para 3"
        let buf = Buffer::from_str("Para 1\n\nPara 2\n\nPara 3");

        // From Para 3 (line 4), one paragraph backward
        let cursor = Cursor::new(4, 0);
        let result = buf.cursor_paragraph_backward(cursor);
        assert_eq!(result, Some(Cursor::new(3, 0)));

        // From Para 2 (line 2), one paragraph backward
        let cursor = Cursor::new(2, 0);
        let result = buf.cursor_paragraph_backward(cursor);
        assert_eq!(result, Some(Cursor::new(1, 0)));

        // From Para 1 (line 0), no previous paragraph
        let cursor = Cursor::new(0, 0);
        let result = buf.cursor_paragraph_backward(cursor);
        assert_eq!(result, None);
    }

    #[test]
    fn test_cursor_paragraph_forward_from_paragraph() {
        // Buffer:
        // 0: "Para 1 line 1"
        // 1: "" (blank)
        // 2: "Para 2 line 1"
        // 3: "Para 2 line 2"
        // 4: "" (blank)
        // 5: "Para 3 line 1"
        let buf = Buffer::from_str("Para 1 line 1\n\nPara 2 line 1\nPara 2 line 2\n\nPara 3 line 1");

        // From Para 1 (line 0), should find blank line after Para 1 (line 1)
        let cursor = Cursor::new(0, 5);
        let result = buf.cursor_paragraph_forward(cursor);
        assert_eq!(result, Some(Cursor::new(1, 0)));

        // From Para 2 line 1 (line 2), should find blank line after Para 2 (line 4)
        let cursor = Cursor::new(2, 5);
        let result = buf.cursor_paragraph_forward(cursor);
        assert_eq!(result, Some(Cursor::new(4, 0)));
    }

    #[test]
    fn test_cursor_paragraph_forward_from_blank_line() {
        // Buffer:
        // 0: "Para 1 line 1"
        // 1: "" (blank)
        // 2: "Para 2 line 1"
        // 3: "Para 2 line 2"
        // 4: "" (blank)
        // 5: "Para 3 line 1"
        let buf = Buffer::from_str("Para 1 line 1\n\nPara 2 line 1\nPara 2 line 2\n\nPara 3 line 1");

        // From blank line 1, should find blank line after Para 2 (line 4)
        let cursor = Cursor::new(1, 0);
        let result = buf.cursor_paragraph_forward(cursor);
        assert_eq!(result, Some(Cursor::new(4, 0)));

        // From blank line 4, should find no next paragraph (None)
        let cursor = Cursor::new(4, 0);
        let result = buf.cursor_paragraph_forward(cursor);
        assert_eq!(result, None);
    }

    #[test]
    fn test_cursor_paragraph_forward_multiple_paragraphs() {
        // Buffer:
        // 0: "Para 1"
        // 1: "" (blank)
        // 2: "Para 2"
        // 3: "" (blank)
        // 4: "Para 3"
        let buf = Buffer::from_str("Para 1\n\nPara 2\n\nPara 3");

        // From Para 1 (line 0), one paragraph forward
        let cursor = Cursor::new(0, 0);
        let result = buf.cursor_paragraph_forward(cursor);
        assert_eq!(result, Some(Cursor::new(1, 0)));

        // From Para 2 (line 2), one paragraph forward
        let cursor = Cursor::new(2, 0);
        let result = buf.cursor_paragraph_forward(cursor);
        assert_eq!(result, Some(Cursor::new(3, 0)));

        // From Para 3 (line 4), no next paragraph
        let cursor = Cursor::new(4, 0);
        let result = buf.cursor_paragraph_forward(cursor);
        assert_eq!(result, None);
    }

    #[test]
    fn test_cursor_paragraph_whitespace_only_lines() {
        // Buffer with whitespace-only lines treated as blank
        // 0: "Para 1"
        // 1: "   " (spaces - blank)
        // 2: "Para 2"
        let buf = Buffer::from_str("Para 1\n   \nPara 2");

        // From Para 1, should find blank line after it (line 1)
        let cursor = Cursor::new(0, 0);
        let result = buf.cursor_paragraph_forward(cursor);
        assert_eq!(result, Some(Cursor::new(1, 0)));

        // From Para 2, backward should find blank line before it (line 1)
        let cursor = Cursor::new(2, 0);
        let result = buf.cursor_paragraph_backward(cursor);
        assert_eq!(result, Some(Cursor::new(1, 0)));
    }

    #[test]
    fn test_cursor_paragraph_empty_buffer() {
        let buf = Buffer::new();
        let cursor = Cursor::new(0, 0);
        assert_eq!(buf.cursor_paragraph_backward(cursor), None);
        assert_eq!(buf.cursor_paragraph_forward(cursor), None);
    }

    #[test]
    fn test_cursor_paragraph_single_line() {
        // Buffer with only one line (not blank)
        let buf = Buffer::from_str("Single line");

        let cursor = Cursor::new(0, 5);
        assert_eq!(buf.cursor_paragraph_backward(cursor), None);
        assert_eq!(buf.cursor_paragraph_forward(cursor), None);
    }
}
