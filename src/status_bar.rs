//! Status bar rendering module.
//!
//! This module provides the root layout footer that summarizes the active
//! editor state for the user.

use crate::globals;
use crate::screen::Screen;
use crate::window::{Position, Size};

/// Derived state used to render the footer status bar.
pub struct StatusBarContext<'a> {
    /// Human-readable mode label.
    pub mode_label: &'a str,
    /// Active buffer display name.
    pub buffer_name: &'a str,
    /// Zero-based cursor line in the active buffer.
    pub cursor_line: usize,
    /// Zero-based cursor column in bytes.
    pub cursor_byte_col: usize,
    /// Total number of lines in the active buffer.
    pub line_count: usize,
}

/// Root footer renderer for editor metadata.
#[derive(Debug, Default)]
pub struct StatusBar;

impl StatusBar {
    /// Creates a new status bar renderer.
    pub fn new() -> Self {
        Self
    }

    /// Returns the formatted footer text for the provided context.
    pub fn text(&self, context: &StatusBarContext<'_>) -> String {
        let line_number = context
            .cursor_line
            .min(context.line_count.saturating_sub(1))
            + 1;
        let percent = self.progress_percent(context.cursor_line, context.line_count);

        format!(
            "{} | {} | {}:{}b | {}%",
            context.mode_label, context.buffer_name, line_number, context.cursor_byte_col, percent
        )
    }

    /// Renders the status bar into a single footer row.
    pub fn render(
        &self,
        screen: &mut Screen,
        origin: Position,
        size: Size,
        context: &StatusBarContext<'_>,
    ) {
        if size.rows == 0 || size.cols == 0 {
            return;
        }

        let style = globals::with_active_theme(|theme| {
            theme.map(|theme| theme.ui.status_bar).unwrap_or_default()
        });

        let width = size.cols as usize;
        screen.write_string(origin.row, origin.col, style, &" ".repeat(width));
        screen.write_string(origin.row, origin.col, style, &self.text(context));
    }

    fn progress_percent(&self, cursor_line: usize, line_count: usize) -> usize {
        if line_count <= 1 {
            return 100;
        }

        let last_line = line_count.saturating_sub(1);
        if cursor_line >= last_line {
            return 100;
        }

        cursor_line.saturating_mul(100) / last_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::globals;
    use crate::terminal::Color;
    use crate::terminal::Style;
    use crate::theme::{SyntaxStyles, Theme, ThemeKind, UiStyles};

    fn context<'a>(
        mode_label: &'a str,
        buffer_name: &'a str,
        cursor_line: usize,
        cursor_byte_col: usize,
        line_count: usize,
    ) -> StatusBarContext<'a> {
        StatusBarContext {
            mode_label,
            buffer_name,
            cursor_line,
            cursor_byte_col,
            line_count,
        }
    }

    fn themed_status_bar() -> Theme {
        let default_style = Style::new().fg(Color::ansi(10)).bg(Color::ansi(20));
        let ui_styles = UiStyles::new(
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
            Style::new().fg(Color::ansi(3)),
            Style::new().fg(Color::ansi(4)),
            Style::new().fg(Color::ansi(5)),
            Style::new().fg(Color::ansi(6)),
            Style::new().fg(Color::ansi(7)),
        );
        let syntax_styles = SyntaxStyles::new(
            Style::new(),
            Style::new(),
            Style::new(),
            Style::new(),
            Style::new(),
            Style::new(),
            Style::new(),
            Style::new(),
            Style::new(),
            Style::new(),
        );

        Theme::new(
            "demo",
            ThemeKind::Ansi256,
            default_style,
            ui_styles,
            syntax_styles,
        )
    }

    #[test]
    fn test_text_formats_footer_fields() {
        let status_bar = StatusBar::new();
        let text = status_bar.text(&context("NORMAL", "notes.txt", 2, 7, 10));

        assert_eq!(text, "NORMAL | notes.txt | 3:7b | 22%");
    }

    #[test]
    fn test_text_reports_hundred_percent_on_last_line() {
        let status_bar = StatusBar::new();
        let text = status_bar.text(&context("INSERT", "notes.txt", 4, 0, 5));

        assert!(text.ends_with("100%"));
    }

    #[test]
    fn test_text_reports_hundred_percent_for_single_line() {
        let status_bar = StatusBar::new();
        let text = status_bar.text(&context("NORMAL", "Untitled", 0, 0, 1));

        assert!(text.ends_with("100%"));
    }

    #[test]
    fn test_render_truncates_to_available_width() {
        let status_bar = StatusBar::new();
        let mut screen = Screen::new(1, 8);

        status_bar.render(
            &mut screen,
            Position::new(0, 0),
            Size::new(1, 8),
            &context("NORMAL", "notes.txt", 0, 0, 10),
        );

        let cell = screen.get_cell_mut(0, 0).unwrap();
        assert_eq!(cell.text, "N");
    }

    #[test]
    fn test_render_uses_theme_status_bar_style() {
        let status_bar = StatusBar::new();
        let theme = themed_status_bar();
        let expected_style = theme.ui.status_bar;
        let _theme_guard = globals::set_test_active_theme(theme);

        let mut screen = Screen::new(1, 12);
        status_bar.render(
            &mut screen,
            Position::new(0, 0),
            Size::new(1, 12),
            &context("NORMAL", "notes.txt", 0, 0, 10),
        );

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().style, expected_style);
    }
}
