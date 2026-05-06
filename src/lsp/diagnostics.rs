use crate::globals;
use crate::terminal::{Color, Style, UnderlineStyle};
use lsp_types::{Diagnostic, DiagnosticSeverity};

/// Compact diagnostic counts grouped by severity.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticCounts {
    /// Error diagnostics.
    pub error: usize,
    /// Warning diagnostics.
    pub warning: usize,
    /// Information diagnostics.
    pub info: usize,
    /// Hint diagnostics.
    pub hint: usize,
}

impl DiagnosticCounts {
    /// Returns true when no diagnostics are present.
    pub fn is_empty(self) -> bool {
        self.error == 0 && self.warning == 0 && self.info == 0 && self.hint == 0
    }

    /// Adds one diagnostic severity to the counts.
    pub fn add_severity(&mut self, severity: DiagnosticSeverity) {
        match severity {
            DiagnosticSeverity::ERROR => self.error += 1,
            DiagnosticSeverity::WARNING => self.warning += 1,
            DiagnosticSeverity::INFORMATION => self.info += 1,
            DiagnosticSeverity::HINT => self.hint += 1,
            _ => self.info += 1,
        }
    }
}

/// Returns the display marker for a diagnostic severity.
pub fn diagnostic_marker(severity: DiagnosticSeverity, nerdfont_enabled: bool) -> &'static str {
    if nerdfont_enabled {
        match severity {
            DiagnosticSeverity::ERROR => "",
            DiagnosticSeverity::WARNING => "",
            DiagnosticSeverity::INFORMATION => "",
            DiagnosticSeverity::HINT => "",
            _ => "",
        }
    } else {
        match severity {
            DiagnosticSeverity::ERROR => "E",
            DiagnosticSeverity::WARNING => "W",
            DiagnosticSeverity::INFORMATION => "I",
            DiagnosticSeverity::HINT => "H",
            _ => "I",
        }
    }
}

/// Returns a numeric rank where lower values represent higher severity.
pub fn diagnostic_severity_rank(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::ERROR => 0,
        DiagnosticSeverity::WARNING => 1,
        DiagnosticSeverity::INFORMATION => 2,
        DiagnosticSeverity::HINT => 3,
        _ => 2,
    }
}

/// Returns the severity attached to a diagnostic, defaulting to information.
pub fn diagnostic_severity(diagnostic: &Diagnostic) -> DiagnosticSeverity {
    diagnostic
        .severity
        .unwrap_or(DiagnosticSeverity::INFORMATION)
}

/// Returns the highest severity among the provided diagnostics.
pub fn highest_diagnostic_severity<'a>(
    diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
) -> Option<DiagnosticSeverity> {
    diagnostics
        .into_iter()
        .map(diagnostic_severity)
        .min_by_key(|severity| diagnostic_severity_rank(*severity))
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
