//! Window rendering module.
//!
//! This module provides the Window type, which owns a buffer view and is
//! responsible for rendering its buffer content to the screen. A window has a
//! screen position (origin) and size, and renders the shared buffer content
//! starting from its origin.

mod commands;
mod geometry;
mod gutter;
mod motions;
mod render;
mod view;
mod widget;

use crate::action::ActionResult;
use crate::buffer::{Boundary, Buffer, BufferId, Cursor};
use crate::editor::{
    Action, InsertMode, LinewiseMotion, Mode, ModeKind, NormalMode, Operator, ResizingMode,
    VisualLineMode, VisualMode,
};
use crate::globals;
use crate::screen::Screen;
use crate::terminal::Color;
use crate::terminal::CursorStyle;
use crate::terminal::Key;
use crate::terminal::Style;
use crate::widget::Widget;
use std::fmt;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Position {
    pub row: u16,
    pub col: u16,
}

impl Position {}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}

impl Size {}

/// Gutter renders line numbers on the left side of the editor window.
///
/// The gutter displays line numbers for the visible buffer content,
/// with a distinct background color to separate it from the content.
#[derive(Debug, Clone)]
pub struct Gutter {
    /// First visible buffer line (scroll offset)
    start_line: usize,
    /// Number of visible rows in the window
    visible_rows: u16,
    /// Total number of lines in the buffer (for width calculation)
    total_buffer_lines: usize,
    /// Last rendered buffer line number (for wrapping detection)
    last_buffer_line: Option<usize>,
    /// Resolved style for the gutter.
    style: Style,
}

#[derive(Debug, Clone)]
pub struct RenderChunk {
    pub text: String,
    pub style: Style,
}

#[derive(Debug, Clone)]
pub struct LineData {
    pub buffer_line: usize,
    pub byte_offset: usize,
    pub width_offset: usize,
    pub chunks: Vec<RenderChunk>,
}

#[derive(Debug, Clone)]
pub struct RenderData {
    line_data: Vec<LineData>,
    visible_rows: u16,
}

#[derive(Debug, Clone)]
/// A window-local view of a shared buffer plus scroll and cursor state.
pub struct BufferView {
    buffer_id: BufferId,
    scroll_offset: Position,
    cursor: Cursor,
    remembered_visual_col: Option<usize>,
    visual_selection: Option<VisualSelection>,
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

pub struct Window {
    buffer_view: BufferView,
    render_data: RenderData,
    size: Size,
    pending_repeat_suffix: Option<String>,
    mode: Box<dyn Mode>,
}

impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Window")
            .field("buffer_view", &self.buffer_view)
            .field("render_data", &self.render_data)
            .field("size", &self.size)
            .field("pending_repeat_suffix", &self.pending_repeat_suffix)
            .field("mode_kind", &self.mode.kind())
            .finish()
    }
}

impl Window {
    /// Creates a new window backed by a buffer that will be registered in the
    /// global buffer pool.
    pub fn new(buffer: Buffer) -> Self {
        let buffer_view = BufferView::new(buffer);
        Self {
            buffer_view,
            render_data: RenderData::new(0),
            size: Size::default(),
            pending_repeat_suffix: None,
            mode: Box::new(NormalMode::new()),
        }
    }

    /// Creates a window from an existing buffer ID in the global buffer pool.
    pub fn from_buffer_id(buffer_id: BufferId) -> Self {
        Self {
            buffer_view: BufferView::from_buffer_id(buffer_id),
            render_data: RenderData::new(0),
            size: Size::default(),
            pending_repeat_suffix: None,
            mode: Box::new(NormalMode::new()),
        }
    }

    pub fn buffer_view(&self) -> &BufferView {
        &self.buffer_view
    }

    pub fn buffer_view_mut(&mut self) -> &mut BufferView {
        &mut self.buffer_view
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn render_data(&self) -> &RenderData {
        &self.render_data
    }

    /// Returns the current mode kind owned by this window.
    pub fn mode_kind(&self) -> ModeKind {
        self.mode.kind()
    }

    /// Returns the current mode label owned by this window.
    pub fn mode_label(&self) -> &'static str {
        self.mode_kind().label()
    }

    /// Returns the terminal cursor style for the current mode owned by this window.
    pub fn cursor_style(&self) -> CursorStyle {
        self.mode.cursor_style()
    }

    /// Handles one key event through the window-owned mode.
    pub fn handle_key(&mut self, key: &Key) -> crate::editor::HandleKeyResult {
        self.mode.handle_key(key)
    }

    /// Appends committed insert text to the current mode's repeat capture, if supported.
    pub fn append_repeat_text(&mut self, text: &str) {
        self.mode.append_repeat_text(text);
    }

    /// Switches this window to a different mode.
    pub fn switch_mode(&mut self, to_mode: ModeKind) -> Option<String> {
        let repeat_text = if to_mode == ModeKind::Normal {
            self.mode.take_repeat_text()
        } else {
            None
        };

        if self.mode.kind().is_visual() && to_mode != self.mode.kind() {
            self.buffer_view.clear_visual_selection();
        }

        self.mode = match to_mode {
            ModeKind::Normal => Box::new(NormalMode::new()),
            ModeKind::Insert => Box::new(InsertMode::new()),
            ModeKind::Resizing => Box::new(ResizingMode::new()),
            ModeKind::Visual => {
                self.buffer_view
                    .begin_visual_selection(crate::window::VisualSelectionKind::Character);
                Box::new(VisualMode::new())
            }
            ModeKind::VisualLine => {
                self.buffer_view
                    .begin_visual_selection(crate::window::VisualSelectionKind::Line);
                Box::new(VisualLineMode::new())
            }
        };

        repeat_text
    }

    /// Returns and clears the repeat-text suffix produced by the last handled action.
    pub fn take_pending_repeat_suffix(&mut self) -> Option<String> {
        self.pending_repeat_suffix.take()
    }

    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.size = size;
        let (gutter_style, default_style) = globals::with_active_theme(|theme| {
            theme
                .map(|theme| (theme.ui.gutter, theme.default_style()))
                .unwrap_or_else(|| {
                    (
                        Style::new().bg(Color::ansi(236)).fg(Color::ansi(245)),
                        Style::default(),
                    )
                })
        });
        let total_lines = self.buffer_view.line_count();
        let gutter_width =
            Gutter::new_with_style(0, size.rows, total_lines, gutter_style).calculate_width();

        // Resolve scrolling before building the gutter so line numbers and
        // visible content are derived from the same viewport.
        self.buffer_view.scroll_to_cursor(size, gutter_width);
        let start_line = self.buffer_view.scroll_offset().row as usize;

        // Create gutter with the finalized viewport state.
        let mut gutter = Gutter::new_with_style(start_line, size.rows, total_lines, gutter_style);

        // Render gutter at origin position
        gutter.render(screen, origin);

        // Render buffer content offset by gutter width
        let content_origin = Position::new(origin.row, origin.col + gutter_width);
        let content_size = Size::new(size.rows, size.cols.saturating_sub(gutter_width));
        screen.fill_region(
            content_origin.row,
            content_origin.col,
            content_size.rows,
            content_size.cols,
            default_style,
        );

        self.render_data = self
            .buffer_view
            .build_render_data_with_style(content_size, default_style);
        self.render_data.render(screen, content_origin);

        let active_line_enabled =
            globals::with_config(|config| config.active_line).unwrap_or(false);
        let is_normal_mode = self.mode.kind() == ModeKind::Normal;
        if active_line_enabled
            && is_normal_mode
            && let Some(cursor_position) = self
                .render_data
                .cursor_screen_position(self.buffer_view.cursor())
            && let Some(active_line_style) =
                globals::with_active_theme(|theme| theme.map(|theme| theme.ui.active_line))
        {
            screen.overlay_region(
                content_origin.row + cursor_position.row,
                content_origin.col,
                1,
                content_size.cols,
                active_line_style,
            );
        }
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.buffer_view.set_cursor(cursor);
    }

    /// Sets the cursor from stored state after syncing it to the current buffer.
    pub fn set_cursor_synced(&mut self, cursor: Cursor) {
        self.buffer_view.set_cursor_synced(cursor);
    }
}

#[cfg(test)]
mod tests;
