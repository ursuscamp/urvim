use crate::{globals, icon};
use lsp_types::DiagnosticSeverity;
pub use urvim_lsp::diagnostics::{
    DiagnosticCounts, diagnostic_severity, diagnostic_severity_rank, highest_diagnostic_severity,
};
use urvim_terminal::{Color, Style, UnderlineStyle};

/// Returns the display marker for a diagnostic severity.
pub fn diagnostic_marker(severity: DiagnosticSeverity, nerdfont_enabled: bool) -> &'static str {
    icon::diagnostic_marker(severity, nerdfont_enabled)
}

/// Returns the styled severity marker for a diagnostic severity.
pub fn diagnostic_style_for(severity: DiagnosticSeverity, base_style: Style) -> Style {
    let theme_style = globals::with_active_theme(|theme| {
        theme
            .map(|theme| match severity {
                DiagnosticSeverity::ERROR => theme.highlight_style_for_name("ui.diagnostic.error"),
                DiagnosticSeverity::WARNING => {
                    theme.highlight_style_for_name("ui.diagnostic.warning")
                }
                DiagnosticSeverity::INFORMATION => {
                    theme.highlight_style_for_name("ui.diagnostic.info")
                }
                DiagnosticSeverity::HINT => theme.highlight_style_for_name("ui.diagnostic.hint"),
                _ => Style::default(),
            })
            .unwrap_or_default()
    });

    base_style
        .accent(fallback_diagnostic_style(severity))
        .accent(theme_style)
}

/// Returns the diagnostic style with a curly undercurl applied for text ranges.
pub fn diagnostic_undercurl_style_for(severity: DiagnosticSeverity, base_style: Style) -> Style {
    let style = diagnostic_style_for(severity, base_style);
    let underline_color = style
        .foreground()
        .unwrap_or_else(|| fallback_diagnostic_color(severity));

    style
        .underline_style(UnderlineStyle::Curly)
        .set_underline_color(Some(underline_color))
}

fn fallback_diagnostic_style(severity: DiagnosticSeverity) -> Style {
    Style::new().fg(fallback_diagnostic_color(severity)).bold()
}

fn fallback_diagnostic_color(severity: DiagnosticSeverity) -> Color {
    match severity {
        DiagnosticSeverity::ERROR => Color::ansi(196),
        DiagnosticSeverity::WARNING => Color::ansi(220),
        DiagnosticSeverity::INFORMATION => Color::ansi(75),
        DiagnosticSeverity::HINT => Color::ansi(81),
        _ => Color::ansi(75),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Diagnostic, DiagnosticSeverity, Range};

    #[test]
    fn highest_diagnostic_severity_prefers_the_most_urgent_level() {
        let diagnostics = vec![
            Diagnostic {
                range: Range::default(),
                severity: Some(DiagnosticSeverity::HINT),
                code: None,
                code_description: None,
                source: Some("lsp".to_string()),
                message: "hint".to_string(),
                related_information: None,
                tags: None,
                data: None,
            },
            Diagnostic {
                range: Range::default(),
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("lsp".to_string()),
                message: "error".to_string(),
                related_information: None,
                tags: None,
                data: None,
            },
        ];

        assert_eq!(
            highest_diagnostic_severity(diagnostics.iter()),
            Some(DiagnosticSeverity::ERROR)
        );
    }
}
