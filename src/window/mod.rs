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
mod wrap;

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
use std::time::Instant;
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
    /// Resolved style for the gutter.
    style: Style,
}

/// Per-render gutter state used to resolve relative numbering and active-row styling.
#[derive(Debug, Clone, Copy)]
pub struct GutterRenderState {
    /// The current cursor line in the buffer.
    pub cursor_line: usize,
    /// Whether relative line numbering is enabled.
    pub relative_number: bool,
    /// The visible screen row containing the cursor, if any.
    pub active_screen_row: Option<usize>,
    /// The overlay style to apply to the active gutter row, if any.
    pub active_line_style: Option<Style>,
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
    pub end_byte: usize,
    pub width_offset: usize,
    pub show_gutter_line_number: bool,
    /// Extra base style applied before this line's chunks are rendered.
    pub base_style: Style,
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
    wrapped_row_offset: u16,
    cursor: Cursor,
    remembered_visual_col: Option<usize>,
    visual_selection: Option<VisualSelection>,
    yank_flash: Option<YankFlash>,
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

pub struct Window {
    buffer_view: BufferView,
    render_data: RenderData,
    size: Size,
    wrap_enabled: bool,
    pending_repeat_suffix: Option<String>,
    mode: Box<dyn Mode>,
}

impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Window")
            .field("buffer_view", &self.buffer_view)
            .field("render_data", &self.render_data)
            .field("size", &self.size)
            .field("wrap_enabled", &self.wrap_enabled)
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
            wrap_enabled: false,
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
            wrap_enabled: false,
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

    /// Returns whether visual wrapping is enabled for this window.
    pub fn wrap_enabled(&self) -> bool {
        self.wrap_enabled
    }

    /// Enables or disables visual wrapping for this window.
    pub fn set_wrap_enabled(&mut self, enabled: bool) {
        self.wrap_enabled = enabled;
    }

    /// Toggles visual wrapping for this window.
    pub fn toggle_wrap(&mut self) {
        self.wrap_enabled = !self.wrap_enabled;
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
        self.buffer_view.prune_yank_flash(std::time::Instant::now());
        self.size = size;
        let (gutter_style, default_style) = globals::with_active_theme(|theme| {
            theme
                .map(|theme| {
                    (
                        theme.resolve_name_with_default("ui.window.gutter"),
                        theme.default_style(),
                    )
                })
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
        let wrap_mode = globals::with_config(|config| config.wrap_mode).unwrap_or_default();
        let relative_number =
            globals::with_config(|config| config.relative_number).unwrap_or(false);
        let active_line_enabled =
            globals::with_config(|config| config.active_line).unwrap_or(false);
        let is_normal_mode = self.mode.kind() == ModeKind::Normal;

        // Resolve scrolling before building the gutter so line numbers and
        // visible content are derived from the same viewport.
        self.buffer_view.scroll_to_cursor_with_wrap(
            size,
            gutter_width,
            self.wrap_enabled,
            wrap_mode,
        );
        let start_line = self.buffer_view.scroll_offset().row as usize;

        // Create gutter with the finalized viewport state.
        let mut gutter = Gutter::new_with_style(start_line, size.rows, total_lines, gutter_style);

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

        self.render_data = self.buffer_view.build_render_data_with_options(
            content_size,
            default_style,
            self.wrap_enabled,
            wrap_mode,
        );
        let active_cursor_row = if active_line_enabled {
            self.render_data
                .cursor_screen_position(self.buffer_view.cursor())
                .map(|position| position.row as usize)
        } else {
            None
        };
        let active_gutter_style = if active_line_enabled {
            globals::with_active_theme(|theme| {
                theme.map(|theme| theme.highlight_style_for_name("ui.window.gutter.active_line"))
            })
        } else {
            None
        };
        gutter.render_for_render_data(
            screen,
            origin,
            &self.render_data,
            GutterRenderState {
                cursor_line: self.buffer_view.cursor().line,
                relative_number,
                active_screen_row: active_cursor_row,
                active_line_style: active_gutter_style,
            },
        );
        if active_line_enabled
            && is_normal_mode
            && let Some(cursor_row) = active_cursor_row
            && let Some(active_line_style) = globals::with_active_theme(|theme| {
                theme.map(|theme| theme.resolve_name_with_default("ui.window.active_line"))
            })
        {
            self.render_data
                .set_line_base_style(cursor_row, active_line_style);
        }

        self.render_data
            .render_with_base_style(screen, content_origin, default_style);
        self.render_indent_guides(screen, content_origin, content_size);
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.buffer_view.set_cursor(cursor);
    }

    /// Sets the cursor from stored state after syncing it to the current buffer.
    pub fn set_cursor_synced(&mut self, cursor: Cursor) {
        self.buffer_view.set_cursor_synced(cursor);
    }

    fn render_indent_guides(
        &self,
        screen: &mut Screen,
        content_origin: Position,
        content_size: Size,
    ) {
        let indent_guides_enabled =
            globals::with_config(|config| config.indent_guides).unwrap_or(true);
        if !indent_guides_enabled {
            return;
        }

        let Some((guide_column, start_exclusive, end_exclusive)) =
            self.buffer_view.active_indent_guide()
        else {
            return;
        };

        let unicode_indent =
            globals::with_config(|config| config.unicode_indent_enabled()).unwrap_or(false);
        let glyph = Self::indent_guide_glyph(unicode_indent);
        let guide_style = globals::with_active_theme(|theme| {
            theme
                .map(|theme| theme.resolve_name_with_default("ui.window.lines.indent"))
                .unwrap_or_default()
        });
        self.overlay_indent_guide(
            screen,
            content_origin,
            content_size,
            guide_column,
            start_exclusive,
            end_exclusive,
            glyph,
            guide_style,
        );
    }

    fn overlay_indent_guide(
        &self,
        screen: &mut Screen,
        content_origin: Position,
        content_size: Size,
        guide_column: usize,
        start_exclusive: usize,
        end_exclusive: usize,
        glyph: &str,
        guide_style: Style,
    ) {
        if content_size.rows == 0 || content_size.cols == 0 {
            return;
        }

        for (screen_row, line_data) in self.render_data.line_data.iter().enumerate() {
            let buffer_line = line_data.buffer_line;
            if buffer_line <= start_exclusive || buffer_line >= end_exclusive {
                continue;
            }

            if guide_column < line_data.width_offset {
                continue;
            }

            let relative_col = guide_column - line_data.width_offset;
            if relative_col >= content_size.cols as usize {
                continue;
            }

            let row = content_origin.row + screen_row as u16;
            let col = content_origin.col + relative_col as u16;
            if let Some(cell) = screen.get_cell_mut(row, col) {
                if !cell.text.chars().all(char::is_whitespace) {
                    continue;
                }
                cell.text.clear();
                cell.text.push_str(glyph);
                cell.style = cell.style.accent(guide_style);
            }
        }
    }

    fn indent_guide_glyph(unicode_indent: bool) -> &'static str {
        if unicode_indent { "│" } else { "|" }
    }
}

#[cfg(test)]
mod tests;
