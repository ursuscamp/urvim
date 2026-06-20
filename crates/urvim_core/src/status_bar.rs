//! Status bar rendering module.
//!
//! This module provides the root layout footer that summarizes the active
//! editor state for the user.

use crate::globals;
use crate::icon::FiletypeIcon;
use crate::lsp::diagnostics::{DiagnosticCounts, diagnostic_marker};
use crate::screen::Screen;
use crate::window::{Position, Size};
use lsp_types::DiagnosticSeverity;
use unicode_width::UnicodeWidthStr;
use urvim_syntax::builtin_syntax_registry;
use urvim_terminal::{Color, Style};

/// Derived state used to render the footer status bar.
pub struct StatusBarContext<'a> {
    /// Human-readable mode label.
    pub mode_label: &'a str,
    /// Whether the active buffer is modified.
    pub modified: bool,
    /// Canonical syntax name used to resolve glyph metadata.
    pub syntax_name: &'a str,
    /// Human-readable syntax label.
    pub syntax_label: &'a str,
    /// Active buffer display name.
    pub buffer_name: &'a str,
    /// Zero-based cursor line in the active buffer.
    pub cursor_line: usize,
    /// Zero-based cursor column in bytes.
    pub cursor_byte_col: usize,
    /// Total number of lines in the active buffer.
    pub line_count: usize,
    /// Diagnostics currently attached to the active buffer by severity.
    pub diagnostic_counts: DiagnosticCounts,
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
        let buffer_name = if context.modified {
            format!("{}*", context.buffer_name)
        } else {
            context.buffer_name.to_string()
        };

        let mut text = format!(
            "{} | {} | {} | {}:{} | {}%",
            context.mode_label,
            context.syntax_label,
            buffer_name,
            line_number,
            context.cursor_byte_col,
            percent
        );

        let diagnostics = self.diagnostic_segment_text(context, false);
        if !diagnostics.is_empty() {
            text.push_str(" | ");
            text.push_str(&diagnostics);
        }

        text
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

        let (style, modified_style) = globals::with_active_theme(|theme| {
            theme
                .map(|theme| {
                    let status_bar = theme.resolve_name_with_default("ui.status_bar");
                    let modified_marker =
                        theme.resolve_name_with_default("ui.status_bar.modified_marker");
                    (status_bar, status_bar.accent(modified_marker))
                })
                .unwrap_or_else(|| (Default::default(), Default::default()))
        });
        let syntax_metadata = self.syntax_metadata(context.syntax_name);
        let nerdfont_enabled = crate::icon::nerdfont_enabled();

        let width = size.cols as usize;
        screen.write_string(origin.row, origin.col, style, &" ".repeat(width));
        let mut current_col = origin.col;

        current_col =
            self.write_segment(screen, origin.row, current_col, style, context.mode_label);
        current_col = self.write_segment(screen, origin.row, current_col, style, " | ");
        current_col = self.write_syntax_segment(
            screen,
            Position::new(origin.row, current_col),
            style,
            syntax_metadata.as_ref(),
            nerdfont_enabled,
            context.syntax_label,
        );
        current_col = self.write_segment(screen, origin.row, current_col, style, " | ");

        current_col =
            self.write_segment(screen, origin.row, current_col, style, context.buffer_name);

        if context.modified {
            screen.write_string(origin.row, current_col, modified_style, "*");
            current_col += 1;
        }
        current_col = self.write_segment(screen, origin.row, current_col, style, " | ");

        let line_number = context
            .cursor_line
            .min(context.line_count.saturating_sub(1))
            + 1;
        current_col = self.write_segment(
            screen,
            origin.row,
            current_col,
            style,
            &format!("{}:{}", line_number, context.cursor_byte_col),
        );
        current_col = self.write_segment(screen, origin.row, current_col, style, " | ");
        let percent = self.progress_percent(context.cursor_line, context.line_count);
        current_col = self.write_segment(
            screen,
            origin.row,
            current_col,
            style,
            &format!("{percent}%"),
        );

        let segments = self
            .diagnostic_items(context, nerdfont_enabled)
            .into_iter()
            .filter(|(_, count, _)| *count > 0)
            .collect::<Vec<_>>();
        if !segments.is_empty() {
            current_col = self.write_segment(screen, origin.row, current_col, style, " | ");
            self.write_diagnostic_items(
                screen,
                origin.row,
                current_col,
                style,
                nerdfont_enabled,
                &segments,
            );
        }
    }

    fn diagnostic_segment_text(
        &self,
        context: &StatusBarContext<'_>,
        nerdfont_enabled: bool,
    ) -> String {
        let mut segments = Vec::new();
        for (label, count, _) in self.diagnostic_items(context, nerdfont_enabled) {
            if count == 0 {
                continue;
            }
            segments.push(format!("{label}{count}"));
        }

        segments.join(" ")
    }

    fn diagnostic_items<'a>(
        &self,
        context: &'a StatusBarContext<'_>,
        nerdfont_enabled: bool,
    ) -> Vec<(&'a str, usize, DiagnosticSeverity)> {
        let items = if nerdfont_enabled {
            vec![
                (
                    diagnostic_marker(DiagnosticSeverity::ERROR, true),
                    context.diagnostic_counts.error,
                    DiagnosticSeverity::ERROR,
                ),
                (
                    diagnostic_marker(DiagnosticSeverity::WARNING, true),
                    context.diagnostic_counts.warning,
                    DiagnosticSeverity::WARNING,
                ),
                (
                    diagnostic_marker(DiagnosticSeverity::INFORMATION, true),
                    context.diagnostic_counts.info,
                    DiagnosticSeverity::INFORMATION,
                ),
                (
                    diagnostic_marker(DiagnosticSeverity::HINT, true),
                    context.diagnostic_counts.hint,
                    DiagnosticSeverity::HINT,
                ),
            ]
        } else {
            vec![
                (
                    diagnostic_marker(DiagnosticSeverity::ERROR, false),
                    context.diagnostic_counts.error,
                    DiagnosticSeverity::ERROR,
                ),
                (
                    diagnostic_marker(DiagnosticSeverity::WARNING, false),
                    context.diagnostic_counts.warning,
                    DiagnosticSeverity::WARNING,
                ),
                (
                    diagnostic_marker(DiagnosticSeverity::INFORMATION, false),
                    context.diagnostic_counts.info,
                    DiagnosticSeverity::INFORMATION,
                ),
                (
                    diagnostic_marker(DiagnosticSeverity::HINT, false),
                    context.diagnostic_counts.hint,
                    DiagnosticSeverity::HINT,
                ),
            ]
        };

        items
    }

    fn write_diagnostic_items(
        &self,
        screen: &mut Screen,
        row: u16,
        start_col: u16,
        base_style: Style,
        nerdfont_enabled: bool,
        segments: &[(&str, usize, DiagnosticSeverity)],
    ) -> u16 {
        let mut current_col = start_col;
        for (idx, (label, count, severity)) in segments.iter().enumerate() {
            if idx > 0 {
                current_col = self.write_segment(screen, row, current_col, base_style, " ");
            }
            let severity_style = self.diagnostic_style_for(*severity, base_style);
            current_col = self.write_segment(screen, row, current_col, severity_style, label);
            if nerdfont_enabled {
                current_col = self.write_segment(screen, row, current_col, base_style, " ");
            }
            current_col =
                self.write_segment(screen, row, current_col, severity_style, &count.to_string());
        }
        current_col
    }

    fn diagnostic_style_for(&self, severity: DiagnosticSeverity, base_style: Style) -> Style {
        let theme_style = globals::with_active_theme(|theme| {
            theme
                .map(|theme| match severity {
                    DiagnosticSeverity::ERROR => {
                        theme.highlight_style_for_name("ui.diagnostic.error")
                    }
                    DiagnosticSeverity::WARNING => {
                        theme.highlight_style_for_name("ui.diagnostic.warning")
                    }
                    DiagnosticSeverity::INFORMATION => {
                        theme.highlight_style_for_name("ui.diagnostic.info")
                    }
                    DiagnosticSeverity::HINT => {
                        theme.highlight_style_for_name("ui.diagnostic.hint")
                    }
                    _ => Style::default(),
                })
                .unwrap_or_default()
        });

        base_style
            .accent(Self::fallback_diagnostic_style(severity))
            .accent(theme_style)
    }

    fn fallback_diagnostic_style(severity: DiagnosticSeverity) -> Style {
        let color = match severity {
            DiagnosticSeverity::ERROR => Color::ansi(196),
            DiagnosticSeverity::WARNING => Color::ansi(220),
            DiagnosticSeverity::INFORMATION => Color::ansi(75),
            DiagnosticSeverity::HINT => Color::ansi(81),
            _ => Color::ansi(75),
        };

        Style::new().fg(color).bold()
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

    fn syntax_metadata(&self, syntax_name: &str) -> Option<urvim_syntax::SyntaxMetadata> {
        builtin_syntax_registry()
            .ok()
            .and_then(|registry| registry.metadata(syntax_name))
    }

    fn write_segment(
        &self,
        screen: &mut Screen,
        row: u16,
        col: u16,
        style: Style,
        text: &str,
    ) -> u16 {
        screen.write_string(row, col, style, text);
        col + UnicodeWidthStr::width(text) as u16
    }

    fn write_syntax_segment(
        &self,
        screen: &mut Screen,
        origin: Position,
        style: Style,
        metadata: Option<&urvim_syntax::SyntaxMetadata>,
        nerdfont_enabled: bool,
        syntax_label: &str,
    ) -> u16 {
        if let Some(glyph) = FiletypeIcon::from_metadata(metadata, nerdfont_enabled) {
            screen.write_string(
                origin.row,
                origin.col,
                style.accent(glyph.style),
                glyph.glyph.as_str(),
            );
            let mut next_col = origin.col + UnicodeWidthStr::width(glyph.glyph.as_str()) as u16;
            screen.write_string(origin.row, next_col, style, " ");
            next_col += 1;
            screen.write_string(origin.row, next_col, style, syntax_label);
            return next_col + UnicodeWidthStr::width(syntax_label) as u16;
        }

        self.write_segment(screen, origin.row, origin.col, style, syntax_label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AdvancedGlyphCapability, Config};
    use crate::globals;
    use std::collections::BTreeSet;
    use urvim_terminal::Color;
    use urvim_terminal::Style;
    use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

    fn context<'a>(
        parts: (
            &'a str,
            bool,
            &'a str,
            &'a str,
            &'a str,
            usize,
            usize,
            usize,
        ),
    ) -> StatusBarContext<'a> {
        let (
            mode_label,
            modified,
            syntax_name,
            syntax_label,
            buffer_name,
            cursor_line,
            cursor_byte_col,
            line_count,
        ) = parts;
        StatusBarContext {
            mode_label,
            modified,
            syntax_name,
            syntax_label,
            buffer_name,
            cursor_line,
            cursor_byte_col,
            line_count,
            diagnostic_counts: DiagnosticCounts::default(),
        }
    }

    fn themed_status_bar() -> Theme {
        let default_style = Style::new().fg(Color::ansi(10)).bg(Color::ansi(20));
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.status_bar").expect("valid tag"),
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
        );
        highlights.insert(
            Tag::parse("ui.status_bar.modified_marker").expect("valid tag"),
            Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)).bold(),
        );
        highlights.insert(
            Tag::parse("ui.selection").expect("valid tag"),
            Style::new().reverse(),
        );
        highlights.insert(
            Tag::parse("ui.window.active_line").expect("valid tag"),
            Style::new().bg(Color::ansi(21)),
        );
        highlights.insert(
            Tag::parse("ui.tab.active").expect("valid tag"),
            Style::new().fg(Color::ansi(4)),
        );
        highlights.insert(
            Tag::parse("ui.tab.inactive").expect("valid tag"),
            Style::new().fg(Color::ansi(5)),
        );
        highlights.insert(
            Tag::parse("ui.tab.scroll_indicator").expect("valid tag"),
            Style::new().fg(Color::ansi(6)),
        );
        highlights.insert(
            Tag::parse("ui.window.gutter").expect("valid tag"),
            Style::new().fg(Color::ansi(7)),
        );
        highlights.insert(
            Tag::parse("ui.window").expect("valid tag"),
            Style::new().fg(Color::ansi(8)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines").expect("valid tag"),
            Style::new().fg(Color::ansi(9)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines.resize").expect("valid tag"),
            Style::new().fg(Color::ansi(10)),
        );

        Theme::new("demo", ThemeKind::Ansi256, default_style, highlights)
    }

    #[test]
    fn test_text_formats_footer_fields() {
        let status_bar = StatusBar::new();
        let text = status_bar.text(&context((
            "NORMAL",
            false,
            "rust",
            "Rust",
            "notes.txt",
            2,
            7,
            10,
        )));

        assert_eq!(text, "NORMAL | Rust | notes.txt | 3:7 | 22%");
    }

    #[test]
    fn test_text_formats_modified_footer_fields() {
        let status_bar = StatusBar::new();
        let text = status_bar.text(&context((
            "NORMAL",
            true,
            "rust",
            "Rust",
            "notes.txt",
            2,
            7,
            10,
        )));

        assert_eq!(text, "NORMAL | Rust | notes.txt* | 3:7 | 22%");
    }

    #[test]
    fn test_text_reports_hundred_percent_on_last_line() {
        let status_bar = StatusBar::new();
        let text = status_bar.text(&context((
            "INSERT",
            false,
            "python",
            "Python",
            "notes.txt",
            4,
            0,
            5,
        )));

        assert!(text.ends_with("100%"));
    }

    #[test]
    fn test_text_reports_hundred_percent_for_single_line() {
        let status_bar = StatusBar::new();
        let text = status_bar.text(&context((
            "NORMAL",
            false,
            "plaintext",
            "Plain Text",
            "Untitled",
            0,
            0,
            1,
        )));

        assert!(text.ends_with("100%"));
    }

    #[test]
    fn test_text_appends_diagnostics_counts() {
        let status_bar = StatusBar::new();
        let mut ctx = context(("NORMAL", false, "rust", "Rust", "notes.txt", 0, 0, 10));
        ctx.diagnostic_counts = DiagnosticCounts {
            error: 1,
            warning: 2,
            info: 3,
            hint: 4,
        };

        assert!(status_bar.text(&ctx).ends_with("| E1 W2 I3 H4"));
    }

    #[test]
    fn test_render_uses_diagnostic_glyphs_when_enabled() {
        let status_bar = StatusBar::new();
        let mut screen = Screen::new(1, 48);
        let _config_guard = globals::set_test_config(Config {
            theme: "demo".to_string(),
            syntax: true,
            auto_close_pairs: true,
            advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
            ..Default::default()
        });

        let mut ctx = context(("NORMAL", false, "rust", "Rust", "notes.txt", 0, 0, 10));
        ctx.diagnostic_counts = DiagnosticCounts {
            error: 1,
            warning: 2,
            ..DiagnosticCounts::default()
        };

        status_bar.render(&mut screen, Position::new(0, 0), Size::new(1, 48), &ctx);

        let mut text = String::new();
        for col in 0..48 {
            text.push_str(&screen.get_cell_mut(0, col).unwrap().text);
        }
        let error_col = text
            .find("")
            .map(|byte_idx| text[..byte_idx].chars().count())
            .expect("error glyph should render");
        assert!(text.contains(" 1  2"));
        assert_eq!(
            screen.get_cell_mut(0, error_col as u16).unwrap().style,
            status_bar.diagnostic_style_for(DiagnosticSeverity::ERROR, Style::default())
        );
    }

    #[test]
    fn test_render_uses_diagnostic_abbreviations_without_nerdfonts() {
        let status_bar = StatusBar::new();
        let mut screen = Screen::new(1, 48);
        let mut ctx = context(("NORMAL", false, "rust", "Rust", "notes.txt", 0, 0, 10));
        ctx.diagnostic_counts = DiagnosticCounts {
            info: 1,
            hint: 1,
            ..DiagnosticCounts::default()
        };

        status_bar.render(&mut screen, Position::new(0, 0), Size::new(1, 48), &ctx);

        let mut text = String::new();
        for col in 0..48 {
            text.push_str(&screen.get_cell_mut(0, col).unwrap().text);
        }
        let info_col = text
            .find("I1")
            .map(|byte_idx| text[..byte_idx].chars().count())
            .expect("info abbreviation should render");
        assert!(text.contains("I1 H1"));
        assert_eq!(
            screen.get_cell_mut(0, info_col as u16).unwrap().style,
            status_bar.diagnostic_style_for(DiagnosticSeverity::INFORMATION, Style::default())
        );
    }

    #[test]
    fn test_render_truncates_to_available_width() {
        let status_bar = StatusBar::new();
        let mut screen = Screen::new(1, 8);

        status_bar.render(
            &mut screen,
            Position::new(0, 0),
            Size::new(1, 8),
            &context(("NORMAL", false, "rust", "Rust", "notes.txt", 0, 0, 10)),
        );

        let cell = screen.get_cell_mut(0, 0).unwrap();
        assert_eq!(cell.text, "N");
    }

    #[test]
    fn test_render_uses_theme_status_bar_style() {
        let status_bar = StatusBar::new();
        let theme = themed_status_bar();
        let expected_style = theme.resolve_name_with_default("ui.status_bar");
        let _theme_guard = globals::set_test_active_theme(theme);

        let mut screen = Screen::new(1, 12);
        status_bar.render(
            &mut screen,
            Position::new(0, 0),
            Size::new(1, 12),
            &context(("NORMAL", false, "rust", "Rust", "notes.txt", 0, 0, 10)),
        );

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().style, expected_style);
    }

    #[test]
    fn test_render_uses_theme_modified_marker_style() {
        let status_bar = StatusBar::new();
        let theme = themed_status_bar();
        let expected_style = theme.resolve_name_with_default("ui.status_bar");
        let expected_marker_style =
            expected_style.accent(theme.highlight_style_for_name("ui.status_bar.modified_marker"));
        let _theme_guard = globals::set_test_active_theme(theme);

        let mut screen = Screen::new(1, 32);
        status_bar.render(
            &mut screen,
            Position::new(0, 0),
            Size::new(1, 32),
            &context(("NORMAL", true, "rust", "Rust", "a", 0, 0, 10)),
        );

        assert_eq!(screen.get_cell_mut(0, 17).unwrap().text, "*");
        assert_eq!(
            screen.get_cell_mut(0, 17).unwrap().style,
            expected_marker_style
        );
    }

    #[test]
    fn test_render_uses_glyph_when_enabled() {
        let status_bar = StatusBar::new();
        let mut screen = Screen::new(1, 32);
        let _config_guard = globals::set_test_config(Config {
            theme: "demo".to_string(),
            syntax: true,
            auto_close_pairs: true,
            advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
            ..Default::default()
        });

        status_bar.render(
            &mut screen,
            Position::new(0, 0),
            Size::new(1, 32),
            &context(("NORMAL", false, "rust", "Rust", "notes.txt", 0, 0, 10)),
        );

        assert_eq!(screen.get_cell_mut(0, 9).unwrap().text, "");
    }
}
