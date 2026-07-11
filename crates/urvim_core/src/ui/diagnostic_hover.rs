//! Diagnostic hover popup widget.

use crate::lsp::diagnostics::{diagnostic_marker, diagnostic_severity};
use crate::screen::Screen;
use crate::ui::floating_window::{FloatingPlacement, FloatingWindowFrame};
use crate::ui::text_width::{ClipSide, clip_text};
use crate::ui::{FocusPolicy, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use crate::window::Position;
use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};
use unicode_width::UnicodeWidthStr;
use urvim_terminal::{Color, Style};

const MAX_CONTENT_COLS: u16 = 100;
const MAX_CONTENT_ROWS: u16 = 8;

/// Transient popup that shows diagnostics near the cursor.
#[derive(Debug)]
pub struct DiagnosticHoverWidget {
    lines: Vec<DiagnosticLine>,
    anchor: Position,
    open: bool,
}

#[derive(Debug, Clone)]
struct DiagnosticLine {
    text: String,
    style: Style,
}

impl DiagnosticHoverWidget {
    /// Creates a diagnostic popup from diagnostics at the cursor.
    pub fn new(diagnostics: Vec<Diagnostic>, anchor: Position) -> Option<Self> {
        let lines = diagnostics
            .into_iter()
            .filter_map(|diagnostic| Self::format_line(&diagnostic))
            .collect::<Vec<_>>();
        if lines.is_empty() {
            return None;
        }

        Some(Self {
            lines,
            anchor,
            open: true,
        })
    }

    /// Returns true when the popup is active.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Closes the popup.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Handles popup-specific UI input.
    pub fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        if !self.open {
            return UiEventResult::NotHandled;
        }

        match event {
            UiEvent::Key(key) if matches!(key.code, urvim_terminal::KeyCode::Esc) => {
                self.close();
                UiEventResult::Handled(Vec::new())
            }
            UiEvent::Paste(_) | UiEvent::Resize(_, _) | UiEvent::Tick | UiEvent::Key(_) => {
                UiEventResult::NotHandled
            }
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

    fn content_size(&self, bounds: crate::window::Size) -> Option<crate::window::Size> {
        let available_cols = bounds.cols.saturating_sub(2);
        let available_rows = bounds.rows.saturating_sub(2);
        if available_cols == 0 || available_rows == 0 {
            return None;
        }

        let max_width = self
            .lines
            .iter()
            .map(|line| UnicodeWidthStr::width(line.text.as_str()))
            .max()
            .unwrap_or(1)
            .min(usize::from(available_cols.min(MAX_CONTENT_COLS)))
            .max(1) as u16;
        let rows = self
            .lines
            .len()
            .min(usize::from(available_rows.min(MAX_CONTENT_ROWS)))
            .max(1) as u16;
        Some(crate::window::Size::new(rows, max_width))
    }

    fn format_line(diagnostic: &Diagnostic) -> Option<DiagnosticLine> {
        let severity = diagnostic_severity(diagnostic);
        let marker = diagnostic_marker(severity, crate::icon::nerdfont_enabled());
        let mut text = String::new();
        text.push_str(marker);
        text.push(' ');
        if let Some(source) = diagnostic.source.as_ref() {
            if !source.is_empty() {
                text.push_str(source);
                text.push_str(": ");
            }
        }
        if let Some(code) = diagnostic.code.as_ref() {
            let code = match code {
                NumberOrString::String(code) => code.as_str().to_string(),
                NumberOrString::Number(code) => code.to_string(),
            };
            if !code.is_empty() {
                text.push('[');
                text.push_str(code.as_str());
                text.push_str("] ");
            }
        }
        let message = diagnostic.message.replace('\n', " ");
        text.push_str(message.trim());
        if text.trim().is_empty() {
            return None;
        }

        Some(DiagnosticLine {
            text,
            style: Self::style_for(severity),
        })
    }

    fn style_for(severity: DiagnosticSeverity) -> Style {
        let theme_style = crate::globals::with_active_theme(|theme| {
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

        Style::new()
            .fg(Self::fallback_color(severity))
            .bold()
            .accent(theme_style)
    }

    fn fallback_color(severity: DiagnosticSeverity) -> Color {
        match severity {
            DiagnosticSeverity::ERROR => Color::ansi(196),
            DiagnosticSeverity::WARNING => Color::ansi(220),
            DiagnosticSeverity::INFORMATION => Color::ansi(75),
            DiagnosticSeverity::HINT => Color::ansi(81),
            _ => Color::ansi(75),
        }
    }
}

impl Widget for DiagnosticHoverWidget {
    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        if !self.open || rect.size.rows < 3 || rect.size.cols < 3 {
            return;
        }

        let Some(frame) = self.resolve_frame(rect) else {
            return;
        };

        let border_style = crate::globals::with_active_theme(|theme| {
            theme
                .map(|theme| theme.resolve_name_with_default("ui.window.lines.border"))
                .unwrap_or_default()
        });
        let body_style = crate::globals::with_active_theme(|theme| {
            theme
                .map(|theme| theme.resolve_name_with_default("ui.window"))
                .unwrap_or_default()
        });

        frame.render_bordered(screen, border_style, body_style);
        for (row, line) in self
            .lines
            .iter()
            .take(frame.content_size.rows as usize)
            .enumerate()
        {
            let clipped = clip_text(
                line.text.as_str(),
                frame.content_size.cols as usize,
                ClipSide::Start,
            )
            .text;
            screen.write_string(
                frame.content_origin.row + row as u16,
                frame.content_origin.col,
                body_style.overlay(line.style),
                clipped.as_str(),
            );
        }
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Passive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::globals;
    use crate::screen::Screen;
    use crate::ui::UiRect;
    use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

    fn theme() -> Theme {
        let default_style = Style::default();
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.window").expect("tag"),
            Style::new().bg(Color::ansi(14)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines.border").expect("tag"),
            Style::new().fg(Color::ansi(33)),
        );
        Theme::new(
            "diagnostic-hover",
            ThemeKind::Ansi256,
            default_style,
            highlights,
        )
    }

    #[test]
    fn diagnostic_hover_renders() {
        let _theme_guard = globals::set_test_active_theme(theme());
        let diagnostics = vec![Diagnostic {
            range: lsp_types::Range::default(),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("lsp".to_string()),
            message: "problem".to_string(),
            related_information: None,
            tags: None,
            data: None,
        }];
        let mut widget =
            DiagnosticHoverWidget::new(diagnostics, Position::new(1, 1)).expect("widget");
        let mut screen = Screen::new(6, 24);

        widget.render_widget(
            &mut screen,
            UiRect::new(Position::new(0, 0), crate::window::Size::new(6, 24)),
            &UiContext,
        );

        assert!(widget.is_open());
    }
}
