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
pub mod renderer;
mod session;
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
use crate::lsp::diagnostics::{diagnostic_severity, diagnostic_severity_rank};
use crate::screen::Screen;
use crate::terminal::Color;
use crate::terminal::CursorStyle;
use crate::terminal::Key;
use crate::terminal::Style;
use lsp_types::DiagnosticSeverity;
use std::fmt;
use std::time::Instant;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Position {
    pub row: u16,
    pub col: u16,
}

impl Position {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    /// Width reserved for diagnostic signs.
    diagnostic_sign_width: u16,
    /// Resolved style for the gutter.
    style: Style,
}

/// Per-render gutter state used to resolve relative numbering and active-row styling.
#[derive(Debug, Clone)]
pub struct GutterRenderState {
    /// The current cursor line in the buffer.
    pub cursor_line: usize,
    /// Whether relative line numbering is enabled.
    pub relative_number: bool,
    /// The visible screen row containing the cursor, if any.
    pub active_screen_row: Option<usize>,
    /// The overlay style to apply to the active gutter row, if any.
    pub active_line_style: Option<Style>,
    /// Diagnostic severity for each visible screen row.
    pub diagnostic_severities: Vec<Option<DiagnosticSeverity>>,
    /// Width reserved for the diagnostic sign column.
    pub diagnostic_sign_width: u16,
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
    buffer: view::BufferBacking,
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

    /// Creates a window backed by an owned buffer that stays outside the global pool.
    pub fn from_owned_buffer(buffer: Buffer) -> Self {
        Self {
            buffer_view: BufferView::from_owned_buffer(buffer),
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
        crate::session::mark_dirty();
    }

    pub fn render_data(&self) -> &RenderData {
        &self.render_data
    }

    /// Rebuilds the cached render snapshot for the current viewport.
    pub fn refresh_render_data(&mut self) {
        let wrap_mode = globals::with_config(|config| config.wrap_mode).unwrap_or_default();
        self.render_data = self.buffer_view.build_render_data_with_options(
            self.size,
            Style::default(),
            self.wrap_enabled,
            wrap_mode,
            false,
        );
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
        let (gutter_style, default_style, active_gutter_style, active_line_style) =
            globals::with_active_theme(|theme| {
                theme
                    .map(|theme| {
                        (
                            theme.resolve_name_with_default("ui.window.gutter"),
                            theme.default_style(),
                            theme.highlight_style_for_name("ui.window.gutter.active_line"),
                            theme.resolve_name_with_default("ui.window.active_line"),
                        )
                    })
                    .unwrap_or_else(|| {
                        (
                            Style::new().bg(Color::ansi(236)).fg(Color::ansi(245)),
                            Style::default(),
                            Style::default(),
                            Style::default(),
                        )
                    })
            });
        let wrap_mode = globals::with_config(|config| config.wrap_mode).unwrap_or_default();
        let relative_number =
            globals::with_config(|config| config.relative_number).unwrap_or(false);
        let active_line_enabled =
            globals::with_config(|config| config.active_line).unwrap_or(false);
        let is_normal_mode = self.mode.kind() == ModeKind::Normal;

        let total_lines = self.buffer_view.line_count();
        let diagnostic_sign_width =
            diagnostic_sign_width_for_buffer(self.buffer_view.buffer_id_opt());
        let gutter_width = Gutter::new_with_style(0, size.rows, total_lines, gutter_style)
            .with_diagnostic_sign_width(diagnostic_sign_width)
            .calculate_width();
        let mut render_state = renderer::BufferRenderState {
            cursor: self.buffer_view.cursor(),
            scroll_offset: self.buffer_view.scroll_offset(),
            wrapped_row_offset: self.buffer_view.wrapped_row_offset(),
            size,
            wrap_enabled: self.wrap_enabled,
            wrap_mode,
            relative_number,
            scroll_to_cursor: true,
            active_line_enabled,
            is_normal_mode,
            syntax_warmup: true,
        };

        renderer::render_buffer_view(
            screen,
            origin,
            &mut self.buffer_view,
            &mut self.render_data,
            renderer::WindowRenderTheme {
                gutter_style,
                default_style,
                active_gutter_style: if active_line_enabled {
                    Some(active_gutter_style)
                } else {
                    None
                },
                active_line_style: if active_line_enabled && is_normal_mode {
                    Some(active_line_style)
                } else {
                    None
                },
            },
            &mut render_state,
        );
        self.render_indent_guides(
            screen,
            Position::new(origin.row, origin.col + gutter_width),
            Size::new(size.rows, size.cols.saturating_sub(gutter_width)),
        );
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.buffer_view.set_cursor(cursor);
    }

    /// Sets the cursor from stored state after syncing it to the current buffer.
    pub fn set_cursor_synced(&mut self, cursor: Cursor) {
        self.buffer_view.set_cursor_synced(cursor);
    }

    /// Sets the cursor, scrolls it into view, and refreshes render state.
    pub fn reveal_cursor(&mut self, cursor: Cursor) {
        self.set_cursor_synced(cursor);
        let wrap_mode = globals::with_config(|config| config.wrap_mode).unwrap_or_default();
        let size = self.size;
        let wrap_enabled = self.wrap_enabled;
        let gutter_width = Gutter::new(0, size.rows, self.buffer_view.line_count())
            .with_diagnostic_sign_width(diagnostic_sign_width_for_buffer(
                self.buffer_view.buffer_id_opt(),
            ))
            .calculate_width();

        self.buffer_view
            .scroll_to_cursor_with_wrap(size, gutter_width, wrap_enabled, wrap_mode);
        self.refresh_render_data();
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

/// Returns the reserved gutter width used for LSP diagnostics in a buffer.
pub fn diagnostic_sign_width_for_buffer(buffer_id: Option<BufferId>) -> u16 {
    let Some(buffer_id) = buffer_id else {
        return 0;
    };

    let has_lsp =
        globals::with_lsp_runtime_mut(|runtime| runtime.buffer_has_lsp(buffer_id)).unwrap_or(false);
    if !has_lsp {
        return 0;
    }

    if globals::with_config(|config| config.nerdfont_enabled()).unwrap_or(false) {
        2
    } else {
        1
    }
}

fn visible_diagnostic_severities(
    buffer_id: Option<BufferId>,
    start_line: usize,
    visible_rows: usize,
) -> Vec<Option<DiagnosticSeverity>> {
    let mut severities = vec![None; visible_rows];
    let Some(buffer_id) = buffer_id else {
        return severities;
    };

    let diagnostics =
        globals::with_diagnostics_store(|store| store.diagnostics_for_buffer(buffer_id))
            .unwrap_or_default();

    for diagnostic in diagnostics {
        let line = diagnostic.range.start.line as usize;
        if line < start_line || line >= start_line.saturating_add(visible_rows) {
            continue;
        }

        let row = line - start_line;
        let severity = diagnostic_severity(&diagnostic);
        let replace = match severities[row] {
            Some(current) => diagnostic_severity_rank(severity) < diagnostic_severity_rank(current),
            None => true,
        };
        if replace {
            severities[row] = Some(severity);
        }
    }

    severities
}

#[cfg(test)]
mod tests;
