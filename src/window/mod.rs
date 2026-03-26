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
mod widget_impl;

use crate::action::{ActionResult, ActionResult::NotHandled};
use crate::buffer::{Boundary, Buffer, BufferId, Cursor};
use crate::editor::{Action, LinewiseMotion, Operator, OperatorTarget};
use crate::globals;
use crate::screen::Screen;
use crate::terminal::Color;
use crate::terminal::Style;
use crate::widget::Widget;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

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
    /// Background color for gutter (ANSI 236 = dark gray)
    background_color: Color,
    /// Foreground color for line numbers (ANSI 245 = light gray)
    foreground_color: Color,
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
}

#[derive(Debug)]
pub struct Window {
    buffer_view: BufferView,
    render_data: RenderData,
    size: Size,
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
        }
    }

    /// Creates a window from an existing buffer ID in the global buffer pool.
    pub fn from_buffer_id(buffer_id: BufferId) -> Self {
        Self {
            buffer_view: BufferView::from_buffer_id(buffer_id),
            render_data: RenderData::new(0),
            size: Size::default(),
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

    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.size = size;
        // Get buffer info for gutter
        let buffer = self.buffer_view.buffer();
        let total_lines = buffer.line_count();
        let start_line = self.buffer_view.scroll_offset().row as usize;

        // Create gutter with needed info (no buffer reference)
        let mut gutter = Gutter::new(start_line, size.rows, total_lines);
        let gutter_width = gutter.calculate_width();

        // Scroll to make cursor visible before rendering
        self.buffer_view.scroll_to_cursor(size, gutter_width);

        // Render gutter at origin position
        gutter.render(screen, origin);

        // Render buffer content offset by gutter width
        let content_origin = Position::new(origin.row, origin.col + gutter_width);
        let content_size = Size::new(size.rows, size.cols.saturating_sub(gutter_width));

        self.render_data = self.buffer_view.build_render_data(content_size);
        self.render_data.render(screen, content_origin);
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.buffer_view.set_cursor(cursor);
    }
}

#[cfg(test)]
mod tests;
