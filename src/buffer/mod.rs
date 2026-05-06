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
mod comment;
mod cursor;
mod edit;
mod indent;
mod io;
mod operator_target;
mod pool;
mod quote_text_object;
mod search;
mod surround;
mod syntax;
mod tab;
mod text_object;
mod undo;
mod unicode;

pub use indent::IndentDirection;
pub use pool::{BufferId, BufferPool};
pub use syntax::{
    BufferCache, BufferCacheRefreshJob, BufferCacheRefreshResult, IndentScope, IndentScopeId,
    SyntaxCatchUpResult, SyntaxSpan,
};

pub use unicode::{
    char_width, configured_tab_width, display_char_width, display_grapheme_width, display_width_at,
    expand_tabs, grapheme_width, str_width, to_byte_index,
};

use crate::path::AbsolutePath;
use imbl::Vector;
use smol_str::SmolStr;
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

/// A single snapshot of buffer state (text, cursor, and syntax cache).
///
/// This is used for undo/redo functionality to store the buffer state
/// at a particular point in time.
#[derive(Debug, Clone)]
struct Snapshot {
    /// The text content at this point in time.
    lines: Vector<Arc<str>>,
    /// The cursor position at this point in time.
    cursor: Cursor,
    /// The buffer cache state at this point in time.
    buffer_cache: BufferCache,
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
#[derive(Debug)]
pub struct Buffer {
    lines: Vector<Arc<str>>,
    saved_lines: Vector<Arc<str>>,
    path: Option<AbsolutePath>,
    syntax_generation: u64,
    syntax_background_generation: Option<u64>,
    undo_state: UndoState,
    buffer_cache: BufferCache,
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self {
            lines: self.lines.clone(),
            saved_lines: self.saved_lines.clone(),
            path: self.path.clone(),
            syntax_generation: self.syntax_generation,
            syntax_background_generation: self.syntax_background_generation,
            undo_state: self.undo_state.clone(),
            buffer_cache: self.buffer_cache.clone(),
        }
    }
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

    /// Returns the resolved path for this buffer, if it has one.
    pub fn path(&self) -> Option<&AbsolutePath> {
        self.path.as_ref()
    }

    /// Sets the resolved path for this buffer and refreshes syntax detection.
    pub fn set_path(&mut self, path: AbsolutePath) {
        self.path = Some(path);
        self.refresh_syntax();
    }

    /// Returns the buffer file name, if a path has been resolved.
    pub fn file_name(&self) -> Option<&std::ffi::OsStr> {
        self.path.as_ref().and_then(|p| p.file_name())
    }

    /// Returns the resolved canonical syntax name for this buffer.
    pub fn syntax_name(&self) -> &str {
        self.buffer_cache.syntax_name()
    }

    /// Returns the user-facing syntax label for this buffer.
    pub fn syntax_label(&self) -> String {
        crate::syntax::builtin_syntax_registry()
            .ok()
            .and_then(|registry| registry.display_name(self.syntax_name()))
            .unwrap_or_else(|| self.syntax_name().to_owned().into())
            .to_string()
    }

    /// Returns true when the current buffer contents differ from the last saved baseline.
    pub fn is_modified(&self) -> bool {
        self.lines != self.saved_lines
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

    /// Returns a snapshot of the buffer lines.
    pub fn line_texts(&self) -> Vector<Arc<str>> {
        self.lines.clone()
    }

    /// Replaces the full buffer contents and refreshes syntax state once.
    pub fn replace_text(&mut self, text: &str) {
        self.lines = if text.is_empty() {
            Vector::unit(Arc::from(""))
        } else {
            text.lines().map(Arc::from).collect::<Vector<_>>()
        };
        let syntax_name = self.buffer_cache.syntax_name().to_owned();
        self.buffer_cache = BufferCache::new(syntax_name);
        self.syntax_generation = self.syntax_generation.wrapping_add(1);
        self.syntax_background_generation = None;
    }

    fn refresh_syntax(&mut self) {
        let new_syntax_name = crate::syntax::resolve_builtin_syntax(
            self.path.as_ref().map(|path| path.as_path()),
            self.lines.get(0).map(|line| line.as_ref()),
        )
        .unwrap_or_else(|| smol_str::SmolStr::new(crate::syntax::fallback_syntax_name()));

        if self.syntax_name() != new_syntax_name {
            self.buffer_cache.set_syntax_name(new_syntax_name);
            self.buffer_cache.invalidate_from(0, 0);
            self.syntax_generation = self.syntax_generation.wrapping_add(1);
            self.syntax_background_generation = None;
        }
    }

    /// Records the current text as the last saved baseline and refreshes syntax detection.
    pub fn mark_saved(&mut self) {
        self.saved_lines = self.lines.clone();
        self.refresh_syntax();
    }
}

#[cfg(test)]
mod tests;
