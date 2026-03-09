//! Text buffer module backed by ropey.
//!
//! This module provides the `Buffer` type, a text buffer implementation built on
//! top of [ropey](https://github.com/cessen/ropey) - a fast and robust UTF-8
//! text rope library. The buffer supports efficient text manipulation with proper
//! Unicode handling including grapheme clusters, combining characters, and emoji.
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
//! use urvim::buffer::Buffer;
//!
//! // Create a new buffer
//! let mut buf = Buffer::new();
//!
//! // Insert text
//! buf.insert_text(0, "Hello, 世界! 😀");
//!
//! // Get line count
//! println!("Lines: {}", buf.line_count());
//!
//! // Get a specific line
//! if let Some(line) = buf.get_line(0) {
//!     println!("Line content: {}", line.as_str());
//!     println!("Line width: {}", line.grapheme_len());
//! }
//! ```

use crate::path::AbsolutePath;
use ropey::Rope;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// A text buffer backed by a rope data structure.
///
/// Buffer provides efficient text editing with proper Unicode support.
/// It wraps [ropey](https://github.com/cessen/ropey) to provide:
/// - O(log n) insertion and deletion
/// - Efficient line and character indexing
/// - Full Unicode support including grapheme clusters
///
/// # Example
///
/// ```
/// use urvim::buffer::Buffer;
///
/// let mut buf = Buffer::from_str("Hello, World!");
/// buf.insert_text(7, "Beautiful ");
/// assert_eq!(buf.as_str(), "Hello, Beautiful World!");
/// ```
#[derive(Debug, Clone)]
pub struct Buffer {
    rope: Rope,
    path: Option<AbsolutePath>,
}

/// A reference to a line in the buffer.
///
/// Line provides methods to access line content and calculate display widths.
/// The line does not include the trailing newline character (if present).
///
/// # Example
///
/// ```
/// use urvim::buffer::Buffer;
///
/// let buf = Buffer::from_str("Hello\nWorld\n");
/// let line = buf.get_line(0).unwrap();
/// assert_eq!(line.as_str(), "Hello");
/// assert_eq!(line.len(), 5);
/// ```
#[derive(Debug, Clone)]
pub struct Line<'a> {
    rope: &'a Rope,
    line_idx: usize,
}

/// An iterator over grapheme clusters in the buffer.
///
/// Each item is a tuple of (byte_index, grapheme_string) where:
/// - `byte_index` is the byte offset of the grapheme in the text
/// - `grapheme_string` is the grapheme cluster as a String
///
/// # Example
///
/// ```
/// use urvim::buffer::Buffer;
///
/// let buf = Buffer::from_str("a😀c");
/// for (byte_idx, grapheme) in buf.grapheme_indices() {
///     println!("{}: {}", byte_idx, grapheme);
/// }
/// // Output:
/// // 0: a
/// // 1: 😀
/// // 5: c
/// ```
#[derive(Debug, Clone)]
pub struct GraphemeIndices<'a> {
    rope: &'a Rope,
    cursor: GraphemeCursor,
    current_byte: usize,
}

/// An iterator over bytes in the buffer.
///
/// # Example
///
/// ```
/// use urvim::buffer::Buffer;
///
/// let buf = Buffer::from_str("abc");
/// let bytes: Vec<u8> = buf.bytes().collect();
/// assert_eq!(bytes, vec![b'a', b'b', b'c']);
/// ```
#[derive(Debug, Clone)]
pub struct Bytes<'a> {
    rope: &'a Rope,
    char_idx: usize,
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
            rope: Rope::new(),
            path: None,
        }
    }

    /// Creates a buffer from a string slice.
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
    /// let buf = Buffer::from_str("Hello, World!");
    /// assert_eq!(buf.len(), 13);
    /// ```
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from(text),
            path: None,
        }
    }

    pub fn with_path(path: AbsolutePath) -> Self {
        Self {
            rope: Rope::new(),
            path: Some(path),
        }
    }

    pub fn from_str_with_path(text: &str, path: AbsolutePath) -> Self {
        Self {
            rope: Rope::from(text),
            path: Some(path),
        }
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
        Ok(Self {
            rope: Rope::from(contents),
            path: abs_path,
        })
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
    /// This is the total character count, not byte count or display width.
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
        self.rope.len_chars()
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
        self.rope.len_chars() == 0
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

    /// Inserts a single character at the specified position.
    ///
    /// # Arguments
    ///
    /// * `index` - Character position to insert at (0-indexed)
    /// * `ch` - Character to insert
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let mut buf = Buffer::from_str("Hello");
    /// buf.insert_char(5, '!');
    /// assert_eq!(buf.as_str(), "Hello!");
    /// ```
    pub fn insert_char(&mut self, index: usize, ch: char) {
        self.rope.insert_char(index, ch);
    }

    /// Inserts text at the specified position.
    ///
    /// # Arguments
    ///
    /// * `index` - Character position to insert at (0-indexed)
    /// * `text` - Text to insert
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let mut buf = Buffer::from_str("Hello");
    /// buf.insert_text(5, " World");
    /// assert_eq!(buf.as_str(), "Hello World");
    /// ```
    pub fn insert_text(&mut self, index: usize, text: &str) {
        self.rope.insert(index, text);
    }

    /// Removes a range of characters from the buffer.
    ///
    /// # Arguments
    ///
    /// * `start` - Start position of the range (inclusive)
    /// * `end` - End position of the range (exclusive)
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let mut buf = Buffer::from_str("Hello, World!");
    /// buf.remove(5, 12);  // Remove ", World"
    /// assert_eq!(buf.as_str(), "Hello!");
    /// ```
    pub fn remove(&mut self, start: usize, end: usize) {
        self.rope.remove(start..end);
    }

    /// Gets the character at the specified position.
    ///
    /// # Arguments
    ///
    /// * `index` - Character position (0-indexed)
    ///
    /// Returns `None` if the position is out of bounds.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Hello");
    /// assert_eq!(buf.get_char_at(0), Some('H'));
    /// assert_eq!(buf.get_char_at(5), None);
    /// ```
    pub fn get_char_at(&self, index: usize) -> Option<char> {
        self.rope.get_char(index)
    }

    /// Gets the grapheme cluster at the specified byte position.
    ///
    /// Note: This takes a byte index, not a character index.
    ///
    /// # Arguments
    ///
    /// * `byte_idx` - Byte position in the text (0-indexed)
    ///
    /// Returns `None` if the position is out of bounds.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("a😀c");
    /// assert_eq!(buf.get_grapheme_at(0).map(|s| s.as_str()), Some(Some("a")));
    /// assert_eq!(buf.get_grapheme_at(1).map(|s| s.as_str()), Some(Some("😀")));
    /// ```
    pub fn get_grapheme_at(&self, byte_idx: usize) -> Option<ropey::RopeSlice<'_>> {
        if byte_idx >= self.rope.len_bytes() {
            return None;
        }

        let mut cursor = GraphemeCursor::new(0, self.rope.len_bytes(), true);
        let mut current_byte = 0;

        while current_byte < self.rope.len_bytes() {
            let (chunk, chunk_byte_idx, _, _) = self.rope.chunk_at_byte(current_byte);
            let chunk_start = chunk_byte_idx;

            match cursor.next_boundary(chunk, chunk_start) {
                Ok(Some(end_byte)) => {
                    let end_byte = end_byte.min(self.rope.len_bytes());
                    if current_byte == byte_idx {
                        let start_char = self.rope.byte_to_char(current_byte);
                        let end_char = self.rope.byte_to_char(end_byte);
                        return Some(self.rope.slice(start_char..end_char));
                    }
                    current_byte = end_byte;
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
        None
    }

    /// Gets a line by its line number.
    ///
    /// Lines are 0-indexed. The returned Line does not include
    /// the trailing newline character.
    ///
    /// # Arguments
    ///
    /// * `line_num` - Line number (0-indexed)
    ///
    /// Returns `None` if the line number is out of bounds.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Line 1\nLine 2\n");
    /// let line = buf.get_line(0);
    /// assert!(line.is_some());
    /// ```
    pub fn get_line(&self, line_num: usize) -> Option<Line<'_>> {
        if line_num >= self.line_count() {
            None
        } else {
            Some(Line {
                rope: &self.rope,
                line_idx: line_num,
            })
        }
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
        self.rope.len_lines()
    }

    /// Converts a line number to a character position.
    ///
    /// Returns the character index at the start of the given line.
    ///
    /// # Arguments
    ///
    /// * `line` - Line number (0-indexed)
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("abc\ndef\nghi");
    /// assert_eq!(buf.line_to_char(1), 4);
    /// ```
    pub fn line_to_char(&self, line: usize) -> usize {
        self.rope.line_to_char(line)
    }

    /// Converts a character position to (line, column).
    ///
    /// # Arguments
    ///
    /// * `char_idx` - Character index (0-indexed)
    ///
    /// Returns a tuple of (line, column), both 0-indexed.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("abc\ndef\nghi");
    /// assert_eq!(buf.char_to_line_char(5), (1, 1));
    /// ```
    pub fn char_to_line_char(&self, char_idx: usize) -> (usize, usize) {
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        let col = char_idx - line_start;
        (line, col)
    }

    /// Returns an iterator over grapheme clusters in the buffer.
    ///
    /// Each item is (byte_index, grapheme_string).
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("a😀c");
    /// let indices: Vec<_> = buf.grapheme_indices().collect();
    /// assert_eq!(indices.len(), 3);
    /// ```
    pub fn grapheme_indices(&self) -> GraphemeIndices<'_> {
        GraphemeIndices {
            rope: &self.rope,
            cursor: GraphemeCursor::new(0, self.rope.len_bytes(), true),
            current_byte: 0,
        }
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
        self.rope.to_string()
    }

    /// Returns an iterator over bytes in the buffer.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("abc");
    /// let bytes: Vec<_> = buf.bytes().collect();
    /// assert_eq!(bytes, vec![b'a', b'b', b'c']);
    /// ```
    pub fn bytes(&self) -> Bytes<'_> {
        Bytes {
            rope: &self.rope,
            char_idx: 0,
        }
    }

    /// Checks if the character at the given position is a newline.
    ///
    /// # Arguments
    ///
    /// * `char_idx` - Character position (0-indexed)
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("ab\ncd");
    /// assert!(!buf.is_newline(0));
    /// assert!(buf.is_newline(2));
    /// ```
    pub fn is_newline(&self, char_idx: usize) -> bool {
        self.rope.get_char(char_idx).map_or(false, |ch| ch == '\n')
    }

    /// Finds the previous newline character before the given byte position.
    ///
    /// # Arguments
    ///
    /// * `from_byte` - Byte position to search from (searches positions < from_byte)
    ///
    /// Returns the byte index of the newline, or `None` if no newline exists.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("ab\ncd");
    /// assert_eq!(buf.prev_newline(4), Some(2));
    /// assert_eq!(buf.prev_newline(1), None);
    /// ```
    pub fn prev_newline(&self, from_byte: usize) -> Option<usize> {
        let text = self.rope.to_string();
        let bytes = text.as_bytes();

        if from_byte == 0 {
            return None;
        }

        let search_end = from_byte.saturating_sub(1);

        for i in (0..=search_end).rev() {
            if bytes[i] == b'\n' {
                return Some(i);
            }
        }
        None
    }

    /// Finds the next newline character at or after the given position.
    ///
    /// # Arguments
    ///
    /// * `from` - Character position to start searching from
    ///
    /// Returns the character index of the newline, or `None` if no newline exists.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("ab\ncd");
    /// assert_eq!(buf.next_newline(0), Some(2));
    /// assert_eq!(buf.next_newline(3), None);
    /// ```
    pub fn next_newline(&self, from: usize) -> Option<usize> {
        let text = self.rope.to_string();
        let chars: Vec<char> = text.chars().collect();

        for i in from..chars.len() {
            if chars[i] == '\n' {
                return Some(i);
            }
        }
        None
    }

    /// Gets the line containing the given character position.
    ///
    /// # Arguments
    ///
    /// * `char_idx` - Character position (0-indexed)
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Line 1\nLine 2");
    /// let line = buf.line_containing_char(7);
    /// assert!(line.is_some());
    /// ```
    pub fn line_containing_char(&self, char_idx: usize) -> Option<Line<'_>> {
        let line_idx = self.rope.char_to_line(char_idx);
        self.get_line(line_idx)
    }

    /// Gets the chunk of text containing the given character position.
    ///
    /// This is useful for working with ropey's internal chunking.
    ///
    /// # Arguments
    ///
    /// * `char_idx` - Character position (0-indexed)
    ///
    /// Returns a string slice of the containing chunk, or `None` if out of bounds.
    pub fn get_chunk_at(&self, char_idx: usize) -> Option<&str> {
        self.rope.get_chunk_at_char(char_idx).map(|(s, _, _, _)| s)
    }
}

impl<'a> Line<'a> {
    /// Returns the number of characters in the line (excluding trailing newline).
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Hello\nWorld");
    /// let line = buf.get_line(0).unwrap();
    /// assert_eq!(line.len(), 5);
    /// ```
    pub fn len(&self) -> usize {
        let slice = self.rope.line(self.line_idx);
        let len = slice.len_chars();
        if len > 0 {
            if slice.get_char(len - 1) == Some('\n') {
                return len - 1;
            }
        }
        len
    }

    /// Returns the number of characters in the line.
    ///
    /// Alias for [`len()`](Line::len).
    pub fn char_len(&self) -> usize {
        self.len()
    }

    /// Returns the display width of the line in characters.
    ///
    /// This uses Unicode width calculation, so characters like '中' and emoji
    /// will have width 2, while ASCII characters have width 1.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Hello\n");
    /// let line = buf.get_line(0).unwrap();
    /// assert_eq!(line.grapheme_len(), 5);
    /// ```
    pub fn grapheme_len(&self) -> usize {
        self.as_str().width()
    }

    /// Returns the line content as a String.
    ///
    /// The returned string does not include the trailing newline.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Hello\n");
    /// let line = buf.get_line(0).unwrap();
    /// assert_eq!(line.as_str(), "Hello");
    /// ```
    pub fn as_str(&self) -> String {
        let s = self.rope.line(self.line_idx).to_string();
        s.strip_suffix('\n').unwrap_or(&s).to_string()
    }

    /// Gets the character at the given column position.
    ///
    /// # Arguments
    ///
    /// * `col` - Column position (0-indexed)
    ///
    /// Returns `None` if the column is out of bounds.
    pub fn char_at(&self, col: usize) -> Option<char> {
        let slice = self.rope.line(self.line_idx);
        let len = slice.len_chars();
        if len > 0 && slice.get_char(len - 1) == Some('\n') {
            if col < len - 1 {
                slice.get_char(col)
            } else {
                None
            }
        } else {
            slice.get_char(col)
        }
    }

    /// Gets the grapheme cluster at the given column position.
    ///
    /// # Arguments
    ///
    /// * `col` - Column position (0-indexed)
    ///
    /// Returns `None` if the column is out of bounds.
    pub fn grapheme_at(&self, col: usize) -> Option<String> {
        let text = self.as_str();
        let mut graphemes = text.graphemes(true);
        graphemes.nth(col).map(|s| s.to_string())
    }

    /// Returns the character position where this line starts.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Line 1\nLine 2");
    /// let line = buf.get_line(1).unwrap();
    /// assert_eq!(line.to_char(), 7);
    /// ```
    pub fn to_char(&self) -> usize {
        self.rope.line_to_char(self.line_idx)
    }
}

impl<'a> Iterator for GraphemeIndices<'a> {
    type Item = (usize, ropey::RopeSlice<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_byte >= self.rope.len_bytes() {
            return None;
        }

        let start_byte = self.current_byte;
        let (chunk, chunk_byte_idx, _, _) = self.rope.chunk_at_byte(start_byte);
        let chunk_start = chunk_byte_idx;

        match self.cursor.next_boundary(chunk, chunk_start) {
            Ok(Some(end_byte)) => {
                let end_byte = end_byte.min(self.rope.len_bytes());
                let start_char = self.rope.byte_to_char(start_byte);
                let end_char = self.rope.byte_to_char(end_byte);
                let grapheme = self.rope.slice(start_char..end_char);
                self.current_byte = end_byte;
                Some((start_byte, grapheme))
            }
            Ok(None) => None,
            Err(_) => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.rope.len_bytes().saturating_sub(self.current_byte);
        (remaining, Some(remaining))
    }
}

impl<'a> DoubleEndedIterator for GraphemeIndices<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        None
    }
}

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.char_idx >= self.rope.len_chars() {
            return None;
        }
        let byte_idx = self.rope.char_to_byte(self.char_idx);
        self.char_idx += 1;
        self.rope.to_string().as_bytes().get(byte_idx).copied()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.rope.len_chars() - self.char_idx;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for Bytes<'a> {}

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

/// Converts a byte index to a character index.
///
/// # Arguments
///
/// * `byte_idx` - Byte position in the text
/// * `text` - The text to index into
///
/// # Example
///
/// ```
/// use urvim::buffer::to_char_index;
///
/// let text = "aβc";
/// // 'a' = byte 0, 'β' = bytes 1-2, 'c' = byte 3
/// assert_eq!(to_char_index(3, text), 2);
/// ```
pub fn to_char_index(byte_idx: usize, text: &str) -> usize {
    text[..byte_idx].chars().count()
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
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.as_str(), "");
    }

    #[test]
    fn test_from_str() {
        let buf = Buffer::from_str("hello");
        assert!(!buf.is_empty());
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.as_str(), "hello");
    }

    #[test]
    fn test_insert_char() {
        let mut buf = Buffer::from_str("hello");
        buf.insert_char(5, '!');
        assert_eq!(buf.as_str(), "hello!");
    }

    #[test]
    fn test_insert_text() {
        let mut buf = Buffer::from_str("hello");
        buf.insert_text(5, " world");
        assert_eq!(buf.as_str(), "hello world");
    }

    #[test]
    fn test_insert_at_beginning() {
        let mut buf = Buffer::from_str("world");
        buf.insert_text(0, "hello ");
        assert_eq!(buf.as_str(), "hello world");
    }

    #[test]
    fn test_insert_in_middle() {
        let mut buf = Buffer::from_str("hello");
        buf.insert_text(2, "XX");
        assert_eq!(buf.as_str(), "heXXllo");
    }

    #[test]
    fn test_remove() {
        let mut buf = Buffer::from_str("hello world");
        buf.remove(5, 11);
        assert_eq!(buf.as_str(), "hello");
    }

    #[test]
    fn test_remove_from_beginning() {
        let mut buf = Buffer::from_str("hello");
        buf.remove(0, 2);
        assert_eq!(buf.as_str(), "llo");
    }

    #[test]
    fn test_get_char_at() {
        let buf = Buffer::from_str("hello");
        assert_eq!(buf.get_char_at(0), Some('h'));
        assert_eq!(buf.get_char_at(4), Some('o'));
        assert_eq!(buf.get_char_at(5), None);
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
    fn test_get_line() {
        let buf = Buffer::from_str("line1\nline2\nline3");
        let line0 = buf.get_line(0).unwrap();
        assert_eq!(line0.as_str(), "line1");

        let line1 = buf.get_line(1).unwrap();
        assert_eq!(line1.as_str(), "line2");

        let line2 = buf.get_line(2).unwrap();
        assert_eq!(line2.as_str(), "line3");
    }

    #[test]
    fn test_get_line_out_of_bounds() {
        let buf = Buffer::from_str("hello");
        assert!(buf.get_line(1).is_none());
    }

    #[test]
    fn test_line_to_char() {
        let buf = Buffer::from_str("abc\ndef\nghi");
        assert_eq!(buf.line_to_char(0), 0);
        assert_eq!(buf.line_to_char(1), 4);
        assert_eq!(buf.line_to_char(2), 8);
    }

    #[test]
    fn test_char_to_line_char() {
        let buf = Buffer::from_str("abc\ndef\nghi");
        assert_eq!(buf.char_to_line_char(0), (0, 0));
        assert_eq!(buf.char_to_line_char(2), (0, 2));
        assert_eq!(buf.char_to_line_char(4), (1, 0));
        assert_eq!(buf.char_to_line_char(6), (1, 2));
        assert_eq!(buf.char_to_line_char(8), (2, 0));
    }

    #[test]
    fn test_grapheme_indices() {
        let buf = Buffer::from_str("aβc");
        let indices: Vec<_> = buf.grapheme_indices().collect();
        assert_eq!(indices.len(), 3);
        assert_eq!(indices[0].0, 0);
        assert_eq!(indices[0].1.as_str(), Some("a"));
        assert_eq!(indices[1].0, 1);
        assert_eq!(indices[1].1.as_str(), Some("β"));
        // 'β' is 2 bytes (0xCE 0xB2), so 'c' is at byte 3
        assert_eq!(indices[2].0, 3);
        assert_eq!(indices[2].1.as_str(), Some("c"));
    }

    #[test]
    fn test_emoji_grapheme() {
        let buf = Buffer::from_str("a😀c");
        let indices: Vec<_> = buf.grapheme_indices().collect();
        assert_eq!(indices.len(), 3);
        assert_eq!(indices[0].0, 0);
        assert_eq!(indices[0].1.as_str(), Some("a"));
        assert_eq!(indices[1].0, 1);
        assert_eq!(indices[1].1.as_str(), Some("😀"));
        assert_eq!(indices[2].0, 5);
        assert_eq!(indices[2].1.as_str(), Some("c"));
    }

    #[test]
    fn test_combining_char_grapheme() {
        let buf = Buffer::from_str("e\u{0301}");
        let indices: Vec<_> = buf.grapheme_indices().collect();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0].0, 0);
        assert_eq!(indices[0].1.as_str(), Some("e\u{0301}"));
    }

    #[test]
    fn test_get_grapheme_at() {
        let buf = Buffer::from_str("a😀c");
        assert_eq!(buf.get_grapheme_at(0).map(|s| s.as_str()), Some(Some("a")));
        assert_eq!(buf.get_grapheme_at(1).map(|s| s.as_str()), Some(Some("😀")));
        assert_eq!(buf.get_grapheme_at(5).map(|s| s.as_str()), Some(Some("c")));
    }

    #[test]
    fn test_line_len() {
        let buf = Buffer::from_str("hello\nworld");
        let line = buf.get_line(0).unwrap();
        assert_eq!(line.len(), 5);
        assert_eq!(line.char_len(), 5);

        let line2 = buf.get_line(1).unwrap();
        assert_eq!(line2.len(), 5);
    }

    #[test]
    fn test_line_grapheme_len() {
        let buf = Buffer::from_str("a😀c\n");
        let line = buf.get_line(0).unwrap();
        assert_eq!(line.grapheme_len(), 4);
    }

    #[test]
    fn test_line_char_at() {
        let buf = Buffer::from_str("hello");
        let line = buf.get_line(0).unwrap();
        assert_eq!(line.char_at(0), Some('h'));
        assert_eq!(line.char_at(4), Some('o'));
    }

    #[test]
    fn test_line_to_char_method() {
        let buf = Buffer::from_str("line1\nline2");
        let line = buf.get_line(1).unwrap();
        assert_eq!(line.to_char(), 6);
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
        assert_eq!(char_width('\n'), 0);
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
    fn test_bytes_iterator() {
        let buf = Buffer::from_str("abc");
        let bytes: Vec<_> = buf.bytes().collect();
        assert_eq!(bytes, vec![b'a', b'b', b'c']);
    }

    #[test]
    fn test_is_newline() {
        let buf = Buffer::from_str("ab\ncd");
        assert!(!buf.is_newline(0));
        assert!(!buf.is_newline(1));
        assert!(buf.is_newline(2));
        assert!(!buf.is_newline(3));
    }

    #[test]
    fn test_prev_newline() {
        let buf = Buffer::from_str("ab\ncd");
        // byte indices: 0='a', 1='b', 2='\n', 3='c', 4='d'
        // prev_newline searches for newline BEFORE the given position
        assert_eq!(buf.prev_newline(4), Some(2)); // search 0-3, finds at 2
        assert_eq!(buf.prev_newline(3), Some(2)); // search 0-2, finds at 2
        assert_eq!(buf.prev_newline(2), None); // search 0-1, no newline
        assert_eq!(buf.prev_newline(1), None); // search 0, no newline
    }

    #[test]
    fn test_next_newline() {
        let buf = Buffer::from_str("ab\ncd");
        assert_eq!(buf.next_newline(0), Some(2));
        assert_eq!(buf.next_newline(2), Some(2));
        assert_eq!(buf.next_newline(3), None);
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
    fn test_hangul_graphemes() {
        let buf = Buffer::from_str("안녕하세요");
        let indices: Vec<_> = buf.grapheme_indices().collect();
        assert_eq!(indices.len(), 5);
    }

    #[test]
    fn test_multiline_with_empty_lines() {
        let buf = Buffer::from_str("a\n\nb");
        assert_eq!(buf.line_count(), 3);
        assert_eq!(buf.get_line(0).unwrap().as_str(), "a");
        assert_eq!(buf.get_line(1).unwrap().as_str(), "");
        assert_eq!(buf.get_line(2).unwrap().as_str(), "b");
    }

    #[test]
    fn test_remove_all() {
        let mut buf = Buffer::from_str("hello");
        buf.remove(0, 5);
        assert!(buf.is_empty());
        assert_eq!(buf.as_str(), "");
    }

    #[test]
    fn test_insert_into_empty() {
        let mut buf = Buffer::new();
        buf.insert_text(0, "test");
        assert_eq!(buf.as_str(), "test");
    }

    #[test]
    fn test_line_with_tab() {
        let buf = Buffer::from_str("a\tb");
        let line = buf.get_line(0).unwrap();
        assert_eq!(line.char_len(), 3);
    }
}
