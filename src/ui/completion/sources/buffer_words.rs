//! Buffer-word completion source.

use super::{current_word_range, unique_words_in_buffer};
use crate::buffer::{Buffer, Cursor};
use crate::ui::completion::CompletionCandidate;

const MAX_BUFFER_WORD_COMPLETIONS: usize = 50;

pub fn buffer_words_completion_candidates(
    buffer: &Buffer,
    cursor: Cursor,
    query: &str,
) -> Vec<CompletionCandidate> {
    let range = current_word_range(buffer, cursor);
    let current_word = current_word_text(buffer, range);
    let words = unique_words_in_buffer(buffer);
    if words.is_empty() {
        return Vec::new();
    }

    let query = query.to_lowercase();
    let matches: Vec<_> = if query.is_empty() {
        words
    } else {
        let filtered: Vec<String> = words
            .iter()
            .filter(|word| word.to_lowercase().starts_with(query.as_str()))
            .cloned()
            .collect();
        if filtered.is_empty() { words } else { filtered }
    };
    let current_word = current_word.map(|word| word.to_lowercase());

    matches
        .into_iter()
        .filter(|word| {
            current_word
                .as_deref()
                .is_none_or(|current_word| word.to_lowercase() != current_word)
        })
        .take(MAX_BUFFER_WORD_COMPLETIONS)
        .map(|word| CompletionCandidate::new(word.clone(), word, range, buffer_word_symbol()))
        .collect()
}

fn buffer_word_symbol() -> Option<String> {
    if crate::globals::with_config(|config| config.nerdfont_enabled()).unwrap_or(false) {
        Some(" ".to_string())
    } else {
        None
    }
}

fn current_word_text(buffer: &Buffer, range: crate::buffer::TextObjectRange) -> Option<String> {
    let line = buffer.line_at(range.start.line)?.as_ref();
    line.get(range.start.col..range.end.col)
        .map(|text| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::config::{AdvancedGlyphCapability, Config};
    use crate::globals;
    use std::collections::BTreeSet;

    #[test]
    fn buffer_words_completion_is_case_insensitive_and_deduplicates() {
        let buffer = Buffer::from_str("Alpha beta alpha\nBeta gamma");
        let labels: Vec<_> = buffer_words_completion_candidates(&buffer, Cursor::new(0, 6), "a")
            .into_iter()
            .map(|candidate| candidate.label)
            .collect();

        assert_eq!(labels, vec!["Alpha".to_string()]);
    }

    #[test]
    fn buffer_words_completion_falls_back_when_prefix_has_no_matches() {
        let buffer = Buffer::from_str("alpha beta gamma");
        let labels: Vec<_> = buffer_words_completion_candidates(&buffer, Cursor::new(0, 5), "zzz")
            .into_iter()
            .map(|candidate| candidate.label)
            .collect();

        assert_eq!(labels, vec!["beta", "gamma"]);
    }

    #[test]
    fn buffer_words_completion_returns_no_matches_when_only_match_is_current_word() {
        let buffer = Buffer::from_str("alpha");
        let labels: Vec<_> = buffer_words_completion_candidates(&buffer, Cursor::new(0, 5), "al")
            .into_iter()
            .map(|candidate| candidate.label)
            .collect();

        assert!(labels.is_empty());
    }

    #[test]
    fn buffer_words_completion_uses_symbol_when_nerdfonts_are_enabled() {
        let _guard = globals::set_test_config(Config {
            advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
            ..Config::default()
        });

        let buffer = Buffer::from_str("alpha beta");
        let source = buffer_words_completion_candidates(&buffer, Cursor::new(0, 0), "a");

        assert_eq!(source[0].symbol.as_deref(), Some(" "));
    }

    #[test]
    fn buffer_words_completion_limits_result_count() {
        let buffer = Buffer::from_str(&format!(
            " {}",
            (0..100)
                .map(|index| format!("word{index}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));

        let labels: Vec<_> = buffer_words_completion_candidates(&buffer, Cursor::new(0, 0), "")
            .into_iter()
            .map(|candidate| candidate.label)
            .collect();

        assert_eq!(labels.len(), MAX_BUFFER_WORD_COMPLETIONS);
        assert_eq!(labels.first().map(String::as_str), Some("word0"));
        assert_eq!(labels.last().map(String::as_str), Some("word49"));
    }
}
