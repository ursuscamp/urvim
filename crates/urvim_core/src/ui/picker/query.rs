//! Shared query helpers for picker sources.

use crate::ui::inputs::PromptSegment;
use urvim_terminal::Style;

/// Search mode used by picker queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerQueryMode {
    /// Exact substring search.
    Exact,
    /// Fuzzy subsequence search.
    Fuzzy,
}

impl PickerQueryMode {
    /// Returns the opposite query mode.
    pub fn toggled(self) -> Self {
        match self {
            Self::Exact => Self::Fuzzy,
            Self::Fuzzy => Self::Exact,
        }
    }

    /// Returns the picker prompt label for this mode.
    pub fn label(self) -> &'static str {
        match self {
            Self::Exact => "Exact",
            Self::Fuzzy => "Fuzzy",
        }
    }

    fn prompt_style_name(self) -> &'static str {
        match self {
            Self::Exact => "ui.input.prompt.exact",
            Self::Fuzzy => "ui.input.prompt.fuzzy",
        }
    }
}

/// Returns prompt segments for a mode-aware picker query.
pub fn query_prompt_segments(mode: PickerQueryMode) -> Vec<PromptSegment> {
    vec![
        PromptSegment::new(mode.label(), highlight_style(mode.prompt_style_name())),
        PromptSegment::new(
            format!(" {} ", crate::icon::selection_indicator()),
            highlight_style("ui.input.prompt.separator"),
        ),
    ]
}

/// Returns true when `candidate` contains `query`, case-insensitively.
pub fn exact_matches(query: &str, candidate: &str) -> bool {
    candidate
        .to_lowercase()
        .contains(query.to_lowercase().as_str())
}

/// Returns true when `query` is a case-insensitive subsequence of `candidate`.
pub fn fuzzy_matches(query: &str, candidate: &str) -> bool {
    let mut query_chars = query.chars().flat_map(char::to_lowercase);
    let Some(mut needle) = query_chars.next() else {
        return true;
    };

    for hay in candidate.chars().flat_map(char::to_lowercase) {
        if hay == needle {
            match query_chars.next() {
                Some(next) => needle = next,
                None => return true,
            }
        }
    }

    false
}

/// Ranking data for a fuzzy subsequence match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FuzzyMatchScore {
    /// Byte offset of the first matched character.
    pub first_match: usize,
    /// Byte span covered by the full match, smaller is better.
    pub span: usize,
    /// Number of skipped bytes between matched characters, smaller is better.
    pub gaps: usize,
    /// Candidate length used as a final tiebreaker.
    pub candidate_len: usize,
}

/// Returns a fuzzy ranking score when `query` is a case-insensitive subsequence of `candidate`.
pub fn fuzzy_match_score(query: &str, candidate: &str) -> Option<FuzzyMatchScore> {
    let mut query_chars = query.chars().flat_map(char::to_lowercase);
    let Some(mut needle) = query_chars.next() else {
        return Some(FuzzyMatchScore {
            first_match: 0,
            span: 0,
            gaps: 0,
            candidate_len: candidate.len(),
        });
    };

    let mut match_positions = Vec::new();
    for (byte_idx, hay) in candidate
        .char_indices()
        .flat_map(|(idx, ch)| ch.to_lowercase().map(move |lower| (idx, lower)))
    {
        if hay == needle {
            match_positions.push(byte_idx);
            match query_chars.next() {
                Some(next) => needle = next,
                None => {
                    let first_match = *match_positions.first().unwrap_or(&0);
                    let last_match = *match_positions.last().unwrap_or(&0);
                    let gaps = match_positions
                        .windows(2)
                        .map(|window| window[1].saturating_sub(window[0]).saturating_sub(1))
                        .sum();
                    return Some(FuzzyMatchScore {
                        first_match,
                        span: last_match.saturating_sub(first_match),
                        gaps,
                        candidate_len: candidate.len(),
                    });
                }
            }
        }
    }

    None
}

/// Returns the first byte column of a case-insensitive fuzzy match.
pub fn fuzzy_match_column(query: &str, candidate: &str) -> usize {
    let mut query_chars = query.chars().flat_map(char::to_lowercase);
    let Some(mut needle) = query_chars.next() else {
        return 0;
    };

    let mut first_match = None;
    for (byte_idx, hay) in candidate
        .char_indices()
        .flat_map(|(idx, ch)| ch.to_lowercase().map(move |lower| (idx, lower)))
    {
        if hay == needle {
            first_match.get_or_insert(byte_idx);
            match query_chars.next() {
                Some(next) => needle = next,
                None => return first_match.unwrap_or(0),
            }
        }
    }

    first_match.unwrap_or(0)
}

fn highlight_style(name: &str) -> Style {
    crate::globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.highlight_style_for_name(name))
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_toggles_between_exact_and_fuzzy() {
        assert_eq!(PickerQueryMode::Exact.toggled(), PickerQueryMode::Fuzzy);
        assert_eq!(PickerQueryMode::Fuzzy.toggled(), PickerQueryMode::Exact);
    }

    #[test]
    fn prompt_segments_include_mode_label() {
        assert_eq!(
            query_prompt_segments(PickerQueryMode::Exact)[0].text,
            "Exact"
        );
        assert_eq!(
            query_prompt_segments(PickerQueryMode::Fuzzy)[0].text,
            "Fuzzy"
        );
    }

    #[test]
    fn exact_matching_is_case_insensitive() {
        assert!(exact_matches("main", "SRC/Main.rs"));
        assert!(!exact_matches("test", "SRC/Main.rs"));
    }

    #[test]
    fn fuzzy_matching_is_case_insensitive_subsequence() {
        assert!(fuzzy_matches("srM", "src/Main.rs"));
        assert!(!fuzzy_matches("msr", "src/Main.rs"));
    }

    #[test]
    fn fuzzy_match_score_prefers_tighter_matches() {
        let tight = fuzzy_match_score("abc", "abc").expect("tight score");
        let loose = fuzzy_match_score("abc", "a-b-c").expect("loose score");

        assert!(tight < loose);
    }

    #[test]
    fn fuzzy_match_column_returns_first_matched_byte() {
        assert_eq!(fuzzy_match_column("vl", "let value = 1;"), 4);
        assert_eq!(fuzzy_match_column("", "let value = 1;"), 0);
    }
}
