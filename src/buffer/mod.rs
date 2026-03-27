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

mod boundary;
mod bracket_text_object;
mod cursor;
mod edit;
mod io;
mod operator_target;
mod pool;
mod search;
mod text_object;
mod undo;
mod unicode;

pub use pool::{BufferId, BufferPool};
pub use unicode::{char_width, grapheme_width, str_width, to_byte_index};

use crate::path::AbsolutePath;
use imbl::Vector;
use std::fs::File;
use std::io::{Read, Write};
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

/// Represents a selected text region for a text object.
/// The range is inclusive at start and exclusive at end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextObjectRange {
    /// Start cursor position (inclusive)
    pub start: Cursor,
    /// End cursor position (exclusive - cursor would be here after selection)
    pub end: Cursor,
}

/// A single snapshot of buffer state (text + cursor).
///
/// This is used for undo/redo functionality to store the buffer state
/// at a particular point in time.
#[derive(Debug, Clone)]
struct Snapshot {
    /// The text content at this point in time.
    lines: Vector<Arc<str>>,
    /// The cursor position at this point in time.
    cursor: Cursor,
}

/// Stores undo/redo history for a buffer.
///
/// The history is a list of snapshots, with a position pointer indicating
/// the "active" snapshot (the one we'd restore if we undo).
///
/// Invariants:
/// - `0 <= position <= history.len()`
/// - position == 0 means no snapshots yet (or at oldest)
/// - position > 0 means "active snapshot" is at position - 1
/// - position == history.len() means at "current" state (no redo available)
#[derive(Debug, Clone)]
struct UndoState {
    /// History of snapshots, oldest first.
    history: Vector<Snapshot>,
    /// Current position in history.
    /// - position == 0: no snapshots yet (or at oldest)
    /// - position > 0: "active snapshot" is at position - 1
    /// - position == history.len(): at "current" state
    position: usize,
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
    undo_state: UndoState,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
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
}

#[cfg(test)]
mod tests;
