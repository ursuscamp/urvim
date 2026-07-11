//! LSP hover popup widget.

use crate::buffer::{Buffer, TextRef};
use crate::config::WrapMode;
use crate::path::AbsolutePath;
use crate::screen::Screen;
use crate::ui::floating_window::{FloatingPlacement, FloatingWindowFrame};
use crate::ui::{FocusPolicy, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use crate::window::renderer::{self, BufferRenderState, WindowRenderTheme};
use crate::window::{BufferView, Gutter, Position, RenderData, Size};
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;
use urvim_terminal::{KeyCode, Style};

const MAX_CONTENT_COLS: u16 = 100;
const PREFERRED_CONTENT_COLS: u16 = 80;
const MAX_CONTENT_ROWS: u16 = 16;
const PREFERRED_CONTENT_ROWS: u16 = 8;

/// Transient hover popup rendered near the cursor.
#[derive(Debug)]
pub struct HoverWidget {
    buffer_view: BufferView,
    render_data: RenderData,
    anchor: Position,
    open: bool,
    last_viewport_rows: u16,
}

impl HoverWidget {
    /// Creates a hover popup from LSP hover text and a cursor anchor.
    pub fn new(text: String, anchor: Position) -> Option<Self> {
        if text.trim().is_empty() {
            return None;
        }

        let buffer = Buffer::from_str_with_path(text.as_str(), hover_path());
        Some(Self {
            buffer_view: BufferView::from_owned_buffer(buffer),
            render_data: RenderData::new(0),
            anchor,
            open: true,
            last_viewport_rows: 0,
        })
    }

    /// Returns true when the hover popup is active.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Closes the hover popup.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Scrolls the hover up by one rendered page.
    pub fn page_up(&mut self) {
        self.page_by_pages(true);
    }

    /// Scrolls the hover down by one rendered page.
    pub fn page_down(&mut self) {
        self.page_by_pages(false);
    }

    /// Handles hover-specific UI input.
    pub fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        if !self.open {
            return UiEventResult::NotHandled;
        }

        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::PageUp => {
                    self.page_up();
                    UiEventResult::Handled(Vec::new())
                }
                KeyCode::PageDown => {
                    self.page_down();
                    UiEventResult::Handled(Vec::new())
                }
                KeyCode::Esc => {
                    self.close();
                    UiEventResult::Handled(Vec::new())
                }
                _ => UiEventResult::NotHandled,
            },
            UiEvent::Paste(_) => UiEventResult::NotHandled,
            UiEvent::Resize(_, _) | UiEvent::Tick => UiEventResult::NotHandled,
        }
    }

    fn resolve_frame(&self, rect: UiRect) -> Option<FloatingWindowFrame> {
        let content_size = self.content_size(rect.size)?;
        FloatingWindowFrame::resolve_placement(
            rect.origin,
            rect.size,
            content_size.rows,
            content_size.cols,
            FloatingPlacement::NearCursor {
                cursor: self.anchor,
            },
        )
    }

    fn content_size(&self, bounds: Size) -> Option<Size> {
        let available_cols = bounds.cols.saturating_sub(2);
        let available_rows = bounds.rows.saturating_sub(2);
        if available_cols == 0 || available_rows == 0 {
            return None;
        }

        let mut max_width = 0usize;
        let line_count = self.buffer_view.with_buffer(|buffer| {
            let line_count = buffer.line_count();
            for line_idx in 0..line_count {
                if let Some(line) = buffer.line_at(line_idx) {
                    let width = line.chunks().map(UnicodeWidthStr::width).sum::<usize>();
                    max_width = max_width.max(width);
                }
            }
            line_count
        })?;

        let gutter_width =
            Gutter::new(0, available_rows.min(MAX_CONTENT_ROWS), line_count).calculate_width();
        let content_cols = max_width
            .saturating_add(usize::from(gutter_width))
            .max(usize::from(PREFERRED_CONTENT_COLS))
            .min(usize::from(available_cols.min(MAX_CONTENT_COLS)))
            .max(1) as u16;
        let content_rows = line_count
            .max(usize::from(PREFERRED_CONTENT_ROWS))
            .min(usize::from(available_rows.min(MAX_CONTENT_ROWS)))
            .max(1) as u16;
        Some(Size::new(content_rows, content_cols))
    }
}

impl Widget for HoverWidget {
    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        if !self.open || rect.size.rows < 3 || rect.size.cols < 3 {
            return;
        }

        let Some(frame) = self.resolve_frame(rect) else {
            return;
        };

        let border_style = theme_style("ui.window.lines.border");
        let body_style = theme_style("ui.window");
        frame.render_bordered(screen, border_style, body_style);

        self.last_viewport_rows = frame.content_size.rows;
        let wrap_mode = WrapMode::Soft;
        let gutter_style = theme_style("ui.window.gutter");
        let active_gutter_style = theme_style("ui.window.gutter.active_line");
        let active_line_style = theme_style("ui.window.active_line");
        let diff_added_gutter_style = theme_style("ui.window.gutter.diff.added");
        let diff_deleted_gutter_style = theme_style("ui.window.gutter.diff.deleted");
        let diff_modified_gutter_style = theme_style("ui.window.gutter.diff.modified");
        let mut render_state = BufferRenderState {
            cursor: self.buffer_view.cursor(),
            scroll_offset: self.buffer_view.scroll_offset(),
            wrapped_row_offset: self.buffer_view.wrapped_row_offset(),
            size: frame.content_size,
            wrap_enabled: true,
            wrap_mode,
            relative_number: false,
            scroll_to_cursor: false,
            active_line_enabled: false,
            is_normal_mode: false,
            syntax_warmup: true,
        };

        renderer::render_buffer_view(
            screen,
            frame.content_origin,
            &mut self.buffer_view,
            &mut self.render_data,
            WindowRenderTheme {
                gutter_style: gutter_style,
                default_style: body_style,
                active_gutter_style: Some(active_gutter_style),
                active_line_style: Some(active_line_style),
                diff_added_gutter_style,
                diff_deleted_gutter_style,
                diff_modified_gutter_style,
            },
            &mut render_state,
        );
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Passive
    }
}

fn theme_style(name: &str) -> Style {
    crate::globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default(name))
            .unwrap_or_default()
    })
}

fn hover_path() -> AbsolutePath {
    AbsolutePath::new(PathBuf::from("/tmp/urvim-lsp-hover.md"))
        .expect("hover path should be absolute")
}

impl HoverWidget {
    fn page_by_pages(&mut self, upwards: bool) {
        let viewport_rows = self.last_viewport_rows as usize;
        if viewport_rows == 0 {
            return;
        }

        let line_count = self.buffer_view.line_count();
        if line_count == 0 {
            self.buffer_view.set_scroll_offset(Position::new(0, 0));
            return;
        }

        let current_row = self.buffer_view.scroll_offset().row as usize;
        let max_top_row = line_count.saturating_sub(viewport_rows);
        let next_row = if upwards {
            current_row.saturating_sub(viewport_rows)
        } else {
            current_row.saturating_add(viewport_rows).min(max_top_row)
        };

        self.buffer_view.set_scroll_offset(Position::new(
            next_row as u16,
            self.buffer_view.scroll_offset().col,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::globals;
    use crate::screen::Screen;
    use crate::ui::UiRect;
    use crate::window::Position;
    use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

    fn theme() -> Theme {
        let default_style = Style::default();
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.window").expect("tag"),
            Style::new().bg(urvim_terminal::Color::ansi(14)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines.border").expect("tag"),
            Style::new().fg(urvim_terminal::Color::ansi(33)),
        );
        Theme::new("hover", ThemeKind::Ansi256, default_style, highlights)
    }

    #[test]
    fn hover_widget_uses_markdown_syntax() {
        let widget = HoverWidget::new(
            "```rust\nfn main() {}\n```".to_string(),
            Position::new(1, 1),
        )
        .expect("hover widget");

        let syntax_name = widget
            .buffer_view
            .with_buffer(|buffer| buffer.syntax_name().to_string())
            .expect("buffer");
        assert_eq!(syntax_name, "markdown");
    }

    #[test]
    fn hover_widget_resolves_frame_near_cursor() {
        let widget =
            HoverWidget::new("hover text".to_string(), Position::new(4, 10)).expect("hover widget");
        let frame = widget
            .resolve_frame(UiRect::new(Position::new(0, 0), Size::new(20, 40)))
            .expect("frame");

        assert!(frame.origin.row >= 4);
        assert!(frame.origin.col <= 10);
    }

    #[test]
    fn hover_widget_renders_border_and_text() {
        let _guard = globals::set_test_config(Config::default());
        let _theme_guard = globals::set_test_active_theme(theme());

        let mut widget =
            HoverWidget::new("hover text".to_string(), Position::new(1, 1)).expect("hover widget");
        let mut screen = Screen::new(8, 24);
        widget.render_widget(
            &mut screen,
            UiRect::new(Position::new(0, 0), Size::new(8, 24)),
            &UiContext,
        );

        assert!(widget.is_open());
    }

    #[test]
    fn hover_widget_pages_by_last_rendered_viewport_height() {
        let _guard = globals::set_test_config(Config::default());
        let _theme_guard = globals::set_test_active_theme(theme());

        let mut widget = HoverWidget::new(
            "one\ntwo\nthree\nfour\nfive\n".to_string(),
            Position::new(1, 1),
        )
        .expect("hover widget");
        let mut screen = Screen::new(6, 24);
        widget.render_widget(
            &mut screen,
            UiRect::new(Position::new(0, 0), Size::new(6, 24)),
            &UiContext,
        );

        widget.page_down();
        assert_eq!(widget.buffer_view.scroll_offset().row, 1);

        widget.page_up();
        assert_eq!(widget.buffer_view.scroll_offset().row, 0);
    }
}
