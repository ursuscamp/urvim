//! Text buffer module backed by isolated text storage.
//!
//! This module provides the `Buffer` type, a text buffer implementation built on
//! top of a text storage abstraction. The buffer supports
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
//! use urvim_core::buffer::{Buffer, Cursor};
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
mod diff;
mod edit;
mod indent;
mod io;
mod marker;
mod operator_target;
mod pool;
mod quote_text_object;
mod search;
mod surround;
pub(crate) mod syntax;
mod tab;
mod text;
mod text_object;
mod undo;
mod unicode;

pub use diff::{
    DiffCache, DiffHunk, DiffInput, DiffMarkerKind, DiffProvider, DiffRefreshJob,
    DiffRefreshResult, DiffSnapshot, merge_hunks, parse_unified_diff_hunk,
};
pub use indent::IndentDirection;
pub use marker::{
    DeleteShape, Gravity, InsertShape, Marker, MarkerId, MarkerKind, MarkerPayload, MarkerShape,
    MarkerStore, PointMarker, RangeMarker,
};
pub use pool::{BufferId, BufferPool};
pub use syntax::{
    BufferCache, BufferCacheRefreshResult, IndentScope, IndentScopeId, IndentScopeRefreshJob,
    IndentScopeRefreshResult, SyntaxFoldRegion, SyntaxRefreshJob, SyntaxRefreshResult, SyntaxSpan,
};
pub use text::{
    Cursor, PieceTable, PieceTableRef, TextChange, TextEncoding, TextObjectRange, TextPosition,
    TextRange, TextRef, TextSnapshot, TextStorage,
};

pub use unicode::{
    char_width, configured_tab_width, display_char_width, display_grapheme_width, display_width_at,
    expand_tabs, grapheme_width, str_width, to_byte_index,
};

use crate::path::AbsolutePath;
use smol_str::SmolStr;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

/// Structural line effect produced by a buffer text mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferEditEffect {
    /// First line affected by the edit in the pre-edit coordinate space.
    pub start_line: usize,
    /// Number of old logical lines replaced by the edit.
    pub old_line_count: usize,
    /// Number of new logical lines produced by the edit.
    pub new_line_count: usize,
    /// Net change in line count caused by the edit.
    pub line_delta: isize,
}

impl BufferEditEffect {
    /// Creates a new structural line edit effect.
    pub fn new(start_line: usize, old_line_count: usize, new_line_count: usize) -> Self {
        Self {
            start_line,
            old_line_count,
            new_line_count,
            line_delta: new_line_count as isize - old_line_count as isize,
        }
    }

    /// Creates an effect for inserting text at a cursor.
    pub fn insert(start_line: usize, text: &str) -> Self {
        Self::new(start_line, 1, text.split('\n').count())
    }

    /// Creates an effect for deleting a cursor range.
    pub fn delete(start: Cursor, end: Cursor) -> Self {
        Self::new(start.line, end.line.saturating_sub(start.line) + 1, 1)
    }

    /// Creates an effect for replacing a cursor range with text.
    pub fn replace(start: Cursor, end: Cursor, text: &str) -> Self {
        Self::new(
            start.line,
            end.line.saturating_sub(start.line) + 1,
            text.split('\n').count(),
        )
    }

    /// Creates an effect from a start line and net line delta.
    pub fn from_line_delta(start_line: usize, line_delta: isize) -> Self {
        if line_delta >= 0 {
            Self::new(start_line, 1, 1 + line_delta.unsigned_abs())
        } else {
            Self::new(start_line, 1 + line_delta.unsigned_abs(), 1)
        }
    }
}

/// A single snapshot of buffer state (text, cursor, and syntax cache).
///
/// This is used for undo/redo functionality to store the buffer state
/// at a particular point in time.
#[derive(Debug, Clone)]
struct Snapshot {
    /// The text content at this point in time.
    lines: PieceTable,
    /// The cursor position at this point in time.
    cursor: Cursor,
    /// The buffer cache state at this point in time.
    buffer_cache: BufferCache,
    /// The marker state at this point in time.
    markers: MarkersStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiskState {
    modified: Option<SystemTime>,
    len: u64,
}

impl DiskState {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Generations {
    syntax: u64,
    syntax_background: Option<u64>,
    indent_background: Option<u64>,
    diff: u64,
    diff_background: Option<u64>,
    visual: u64,
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
    history: Vec<Snapshot>,
    /// Current position in history.
    /// - position == 0: no snapshots yet (or at oldest)
    /// - position > 0: "active snapshot" is at position - 1
    /// - position == history.len(): at "current" state
    position: usize,
}

/// A text buffer backed by isolated text storage.
///
/// Buffer provides efficient text editing with proper Unicode support.
/// Each line is stored as an Arc<str> without trailing newline characters.
/// Newlines exist implicitly between lines.
///
/// # Example
///
/// ```
/// use urvim_core::buffer::{Buffer, Cursor};
///
/// let mut buf = Buffer::from_str("Hello, World!");
/// buf.insert_text(Cursor::new(0, 7), "Beautiful ");
/// assert_eq!(buf.as_str(), "Hello, Beautiful World!");
/// ```
#[derive(Debug)]
pub struct Buffer {
    lines: PieceTable,
    saved_lines: PieceTable,
    saved_disk_state: Option<DiskState>,
    path: Option<AbsolutePath>,
    generations: Generations,
    diff_tracked: Option<bool>,
    undo_state: UndoState,
    buffer_cache: BufferCache,
    diff_cache: DiffCache,
    markers: MarkersStore,
}

/// Shared marker store used by `Buffer`.
pub type MarkersStore = MarkerStore<MarkerPayload>;

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self {
            lines: self.lines.clone(),
            saved_lines: self.saved_lines.clone(),
            saved_disk_state: self.saved_disk_state,
            path: self.path.clone(),
            generations: self.generations,
            diff_tracked: self.diff_tracked,
            undo_state: self.undo_state.clone(),
            buffer_cache: self.buffer_cache.clone(),
            diff_cache: self.diff_cache.clone(),
            markers: self.markers.clone(),
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
    /// use urvim_core::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Hello");
    /// assert_eq!(buf.len(), 5);
    /// ```
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Returns true if the buffer is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim_core::buffer::Buffer;
    ///
    /// let buf = Buffer::new();
    /// assert!(buf.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Returns the resolved path for this buffer, if it has one.
    pub fn path(&self) -> Option<&AbsolutePath> {
        self.path.as_ref()
    }

    /// Sets the resolved path for this buffer and refreshes syntax detection.
    pub fn set_path(&mut self, path: AbsolutePath) {
        self.path = Some(path);
        self.diff_cache.clear();
        self.generations.diff_background = None;
        self.diff_tracked = None;
        self.refresh_syntax();
    }

    /// Returns the buffer file name, if a path has been resolved.
    pub fn file_name(&self) -> Option<&std::ffi::OsStr> {
        self.path.as_ref().and_then(|p| p.file_name())
    }

    /// Returns the buffer lines as owned strings without trailing newlines.
    pub fn line_texts(&self) -> Vec<String> {
        self.lines.iter().map(|line| line.to_text()).collect()
    }

    /// Returns the cached diff hunks for this buffer.
    pub fn diff_hunks(&self) -> &[DiffHunk] {
        self.diff_cache.hunks()
    }

    /// Returns true when the diff cache is stale or still waiting on a refresh.
    pub fn diff_cache_stale(&self) -> bool {
        if self.path.is_none() || self.diff_tracked == Some(false) {
            return false;
        }

        !self.diff_cache.is_current_for(self.generations.diff)
            || self.generations.diff_background == Some(self.generations.diff)
    }

    /// Returns the reserved gutter width for diff markers.
    pub fn diff_sign_width(&self) -> u16 {
        if self.path.is_none() || self.diff_tracked == Some(false) {
            return 0;
        }

        if self.diff_cache.is_empty() && self.diff_cache.is_current_for(self.generations.diff) {
            0
        } else {
            1
        }
    }

    /// Returns the diff markers for visible rows starting at the given line.
    pub fn diff_markers_for_visible_rows(
        &self,
        start_line: usize,
        visible_rows: usize,
    ) -> Vec<Option<DiffMarkerKind>> {
        if self.path.is_none() || self.diff_tracked == Some(false) {
            return vec![None; visible_rows];
        }

        self.diff_cache
            .markers_for_visible_rows(start_line, visible_rows)
    }

    /// Returns the next diff hunk cursor after the current cursor.
    pub fn next_diff_hunk_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        let line = self
            .diff_cache
            .next_hunk_start_line_including_current(cursor.line)?;
        Some(Cursor::new(line, 0))
    }

    /// Returns the previous diff hunk cursor before the current cursor.
    pub fn previous_diff_hunk_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        let line = self
            .diff_cache
            .previous_hunk_start_line_including_current(cursor.line)?;
        Some(Cursor::new(line, 0))
    }

    /// Returns the next diff hunk end cursor after the current cursor.
    pub fn next_diff_hunk_end_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        let line = self
            .diff_cache
            .next_hunk_end_line_including_current(cursor.line)?;
        Some(Cursor::new(line, 0))
    }

    /// Returns the previous diff hunk end cursor before the current cursor.
    pub fn previous_diff_hunk_end_cursor(&self, cursor: Cursor) -> Option<Cursor> {
        let line = self
            .diff_cache
            .previous_hunk_end_line_including_current(cursor.line)?;
        Some(Cursor::new(line, 0))
    }

    /// Returns the resolved canonical syntax name for this buffer.
    pub fn syntax_name(&self) -> &str {
        self.buffer_cache.syntax_name()
    }

    /// Returns the user-facing syntax label for this buffer.
    pub fn syntax_label(&self) -> String {
        urvim_syntax::builtin_syntax_registry()
            .ok()
            .and_then(|registry| registry.display_name(self.syntax_name()))
            .unwrap_or_else(|| self.syntax_name().to_owned().into())
            .to_string()
    }

    /// Sets the resolved canonical syntax name for this buffer.
    pub fn set_syntax_name(&mut self, syntax_name: impl Into<smol_str::SmolStr>) {
        let syntax_name = syntax_name.into();
        if self.syntax_name() != syntax_name {
            self.buffer_cache.set_syntax_name(syntax_name);
            self.buffer_cache.invalidate_from(0, 0);
            self.generations.syntax = self.generations.syntax.wrapping_add(1);
            self.generations.syntax_background = None;
            self.generations.indent_background = None;
        }
    }

    /// Converts a protocol text position into a buffer cursor.
    pub fn cursor_for_position(
        &self,
        position: TextPosition,
        encoding: TextEncoding,
    ) -> Option<Cursor> {
        self.lines.cursor_for_position(position, encoding)
    }

    /// Returns true when the current buffer contents differ from the last saved baseline.
    pub fn is_modified(&self) -> bool {
        self.lines != self.saved_lines
    }

    fn disk_state_for_path(path: &Path) -> Option<DiskState> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(DiskState::from_metadata(&metadata))
    }

    fn current_disk_state(&self) -> Option<DiskState> {
        self.path
            .as_ref()
            .and_then(|path| Self::disk_state_for_path(path.as_path()))
    }

    fn refresh_saved_disk_state(&mut self) {
        self.saved_disk_state = self.current_disk_state();
    }

    /// Returns the generation for rendered visual decorations.
    pub fn visual_generation(&self) -> u64 {
        self.generations.visual
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
    /// use urvim_core::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Line 1\nLine 2\n");
    /// let line = buf.line_at(0);
    /// assert!(line.is_some());
    /// ```
    pub fn line_at(&self, line_idx: usize) -> Option<PieceTableRef<'_>> {
        self.lines.line(line_idx)
    }

    #[cfg(test)]
    fn test_line_str(&self, line_idx: usize) -> Option<String> {
        self.line_at(line_idx).map(|line| line.to_text())
    }

    /// Returns the number of lines in the buffer.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim_core::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Line 1\nLine 2\nLine 3");
    /// assert_eq!(buf.line_count(), 3);
    /// ```
    pub fn line_count(&self) -> usize {
        self.lines.line_count()
    }

    /// Returns the buffer contents as a String.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim_core::buffer::Buffer;
    ///
    /// let buf = Buffer::from_str("Hello");
    /// assert_eq!(buf.as_str(), "Hello");
    /// ```
    pub fn as_str(&self) -> String {
        self.lines.text().to_text()
    }

    /// Iterates buffer lines.
    pub fn lines(&self) -> impl Iterator<Item = PieceTableRef<'_>> + '_ {
        self.lines.lines()
    }

    /// Returns a snapshot of the buffer text.
    pub fn text_snapshot(&self) -> PieceTable {
        self.lines.clone()
    }

    /// Replaces the full buffer contents and refreshes syntax state once.
    pub fn replace_text(&mut self, text: &str) {
        self.lines.replace_text(text);
        let syntax_name = self.buffer_cache.syntax_name().to_owned();
        self.buffer_cache = BufferCache::new(syntax_name);
        self.generations.syntax = self.generations.syntax.wrapping_add(1);
        self.generations.syntax_background = None;
        self.generations.indent_background = None;
        self.clear_markers();
    }

    fn bump_visual_generation(&mut self) {
        self.generations.visual = self.generations.visual.wrapping_add(1);
    }

    fn refresh_syntax(&mut self) {
        let current_syntax = self.syntax_name();
        if current_syntax != urvim_syntax::fallback_syntax_name() {
            return;
        }

        let first_line = self.lines.line(0);
        let new_syntax_name = urvim_syntax::resolve_builtin_syntax(
            self.path.as_ref().map(|path| path.as_path()),
            first_line.as_ref().and_then(|line| line.contiguous_text()),
        )
        .unwrap_or_else(|| smol_str::SmolStr::new(urvim_syntax::fallback_syntax_name()));

        if new_syntax_name == urvim_syntax::fallback_syntax_name() {
            return;
        }

        self.buffer_cache.set_syntax_name(new_syntax_name);
        self.buffer_cache.invalidate_from(0, 0);
        self.generations.syntax = self.generations.syntax.wrapping_add(1);
        self.generations.syntax_background = None;
        self.generations.indent_background = None;
    }

    /// Records the current text as the last saved baseline and refreshes syntax detection.
    pub fn mark_saved(&mut self) {
        self.saved_lines = self.lines.clone();
        self.refresh_syntax();
        self.refresh_saved_disk_state();
    }

    /// Reloads the buffer contents from its resolved path.
    pub fn reload_from_disk(&mut self) -> std::io::Result<()> {
        let path = self.path.clone().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "buffer has no path")
        })?;
        let mut file = File::open(path.as_path())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let cursor = self.current_cursor();
        self.replace_text(&contents);
        self.push_snapshot(cursor);
        self.mark_saved();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
