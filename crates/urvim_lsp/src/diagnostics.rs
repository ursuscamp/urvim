//! Pure diagnostic severity and count helpers.
//!
//! UI-specific helpers (styling, markers) remain in `urvim_core` because they
//! depend on icons, terminal styles, and the active theme.

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

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Diagnostic, DiagnosticSeverity, Range};

    #[test]
    fn diagnostic_counts_is_empty_when_none_present() {
        let counts = DiagnosticCounts::default();
        assert!(counts.is_empty());
    }

    #[test]
    fn diagnostic_counts_not_empty_when_error_present() {
        let mut counts = DiagnosticCounts::default();
        counts.add_severity(DiagnosticSeverity::ERROR);
        assert!(!counts.is_empty());
    }

    #[test]
    fn diagnostic_counts_tracks_each_severity() {
        let mut counts = DiagnosticCounts::default();
        counts.add_severity(DiagnosticSeverity::ERROR);
        counts.add_severity(DiagnosticSeverity::ERROR);
        counts.add_severity(DiagnosticSeverity::WARNING);
        counts.add_severity(DiagnosticSeverity::INFORMATION);
        counts.add_severity(DiagnosticSeverity::HINT);
        assert_eq!(counts.error, 2);
        assert_eq!(counts.warning, 1);
        assert_eq!(counts.info, 1);
        assert_eq!(counts.hint, 1);
    }

    #[test]
    fn diagnostic_counts_treats_unknown_as_info() {
        let mut counts = DiagnosticCounts::default();
        // DiagnosticSeverity doesn't export a direct "unknown" value, but the
        // match arm `_ => self.info += 1` handles any future variant.
        let unknown_severity = DiagnosticSeverity::INFORMATION;
        counts.add_severity(unknown_severity);
        assert_eq!(counts.info, 1);
    }

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

    #[test]
    fn highest_diagnostic_severity_returns_none_for_empty() {
        assert_eq!(highest_diagnostic_severity(std::iter::empty()), None);
    }

    #[test]
    fn diagnostic_severity_defaults_to_information() {
        let diagnostic = Diagnostic {
            range: Range::default(),
            severity: None,
            code: None,
            code_description: None,
            source: None,
            message: "test".to_string(),
            related_information: None,
            tags: None,
            data: None,
        };
        assert_eq!(
            diagnostic_severity(&diagnostic),
            DiagnosticSeverity::INFORMATION
        );
    }

    #[test]
    fn diagnostic_severity_rank_orders_correctly() {
        assert_eq!(diagnostic_severity_rank(DiagnosticSeverity::ERROR), 0);
        assert_eq!(diagnostic_severity_rank(DiagnosticSeverity::WARNING), 1);
        assert_eq!(diagnostic_severity_rank(DiagnosticSeverity::INFORMATION), 2);
        assert_eq!(diagnostic_severity_rank(DiagnosticSeverity::HINT), 3);
    }
}
