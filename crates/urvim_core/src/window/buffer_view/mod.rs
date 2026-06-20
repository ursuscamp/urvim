//! Window-local buffer view state and rendering helpers.

mod fold;
mod session;
mod view;
mod wrap;

use crate::buffer::Cursor;
use crate::window::Position;
use std::collections::BTreeSet;
use std::time::Instant;

/// A window-local view of a shared buffer plus scroll and cursor state.
#[derive(Debug, Clone)]
pub struct BufferView {
    buffer: view::BufferBacking,
    scroll_offset: Position,
    wrapped_row_offset: u16,
    cursor: Cursor,
    remembered_visual_col: Option<usize>,
    visual_selection: Option<VisualSelection>,
    yank_flash: Option<YankFlash>,
    folded_lines: BTreeSet<usize>,
    rendered_visual_generation: u64,
}

/// Kind of visual selection currently active in a buffer view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualSelectionKind {
    /// Character-wise selection.
    Character,
    /// Whole-line selection.
    Line,
}

/// Active visual selection metadata stored by a window-local buffer view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualSelection {
    /// Selection anchor cursor.
    pub anchor: Cursor,
    /// Selection granularity.
    pub kind: VisualSelectionKind,
}

/// A transient yank flash selection used for brief normal-mode feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YankFlashSelection {
    /// Characterwise selection range.
    Character(crate::buffer::TextObjectRange),
    /// Linewise selection span.
    Line { start_line: usize, count: usize },
}

/// A transient yank flash with an expiration time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YankFlash {
    /// The flashed region.
    pub selection: YankFlashSelection,
    /// Time when the flash should expire.
    pub expires_at: Instant,
}
