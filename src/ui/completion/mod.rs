//! Insert-mode completion framework and popup widget.

mod job;
mod render;
mod sources;
mod widget;

pub use job::CompletionJob;
pub use widget::CompletionWidget;

use crate::buffer::{Buffer, Cursor, TextRef};

/// An insert-mode completion source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CompletionSourceKind {
    /// Results from the attached language server.
    Lsp,
    /// Paths under the current working directory.
    Paths,
    /// Unique words from the current buffer.
    BufferWords,
}

impl CompletionSourceKind {
    /// Returns the canonical config name for this source.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lsp => "lsp",
            Self::Paths => "paths",
            Self::BufferWords => "buffer_words",
        }
    }

    /// Parses a canonical config name into a source kind.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "lsp" => Some(Self::Lsp),
            "paths" => Some(Self::Paths),
            "buffer_words" => Some(Self::BufferWords),
            _ => None,
        }
    }
}

/// The insertion style for a completion candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionInsertFormat {
    /// Insert the text literally.
    PlainText,
    /// Insert the text as a snippet.
    Snippet,
}

/// A text edit attached to a completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionTextEdit {
    /// Buffer range replaced by the edit.
    pub range: crate::buffer::TextObjectRange,
    /// Replacement text for the range.
    pub text: String,
}

/// A single completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    /// Text shown in the popup.
    pub label: String,
    /// Text inserted when accepted.
    pub replacement: String,
    /// Buffer range replaced when accepted.
    pub range: crate::buffer::TextObjectRange,
    /// Optional symbol shown for the item when advanced glyphs are available.
    pub symbol: Option<String>,
    /// Optional LSP completion kind.
    pub kind: Option<lsp_types::CompletionItemKind>,
    /// Optional insert format for the replacement text.
    pub insert_format: Option<CompletionInsertFormat>,
    /// Optional detail shown in the popup row.
    pub detail: Option<String>,
    /// Optional secondary text rendered after the label.
    pub label_detail: Option<String>,
    /// Optional tertiary text rendered after the label detail.
    pub label_description: Option<String>,
    /// Additional edits applied when the completion is accepted.
    pub additional_text_edits: Vec<CompletionTextEdit>,
    /// Opaque serialized LSP completion item for resolve requests.
    pub lsp_completion_item: Option<serde_json::Value>,
    /// Whether the item is deprecated.
    pub deprecated: bool,
    /// Whether the item should start highlighted.
    pub preselect: bool,
}

impl CompletionCandidate {
    /// Creates a completion candidate with default LSP metadata.
    pub fn new(
        label: impl Into<String>,
        replacement: impl Into<String>,
        range: crate::buffer::TextObjectRange,
        symbol: Option<String>,
    ) -> Self {
        Self {
            label: label.into(),
            replacement: replacement.into(),
            range,
            symbol,
            kind: None,
            insert_format: None,
            detail: None,
            label_detail: None,
            label_description: None,
            additional_text_edits: Vec::new(),
            lsp_completion_item: None,
            deprecated: false,
            preselect: false,
        }
    }
}

/// Returns the configured completion sources in priority order.
pub fn completion_source_kinds(configured: &[String]) -> Vec<CompletionSourceKind> {
    configured
        .iter()
        .filter_map(|name| CompletionSourceKind::from_name(name.as_str()))
        .collect()
}

/// Returns the configured completion source names in priority order.
pub fn completion_source_names(configured: &[String]) -> Vec<String> {
    configured.to_vec()
}

/// Returns true when the current cursor context should auto-trigger completion.
pub fn should_autocomplete(buffer: &Buffer, cursor: Cursor) -> bool {
    let Some(line) = buffer.line_at(cursor.line) else {
        return false;
    };

    let cursor_col = cursor.col.min(line.len());

    let mut path_start = cursor_col;
    while path_start > 0 {
        let Some((prev_start, prev)) = line.previous_char(path_start) else {
            break;
        };
        if !is_path_char(prev) {
            break;
        }
        path_start = prev_start;
    }

    if path_start < cursor_col {
        if is_path_completion_prefix(&line, path_start, cursor_col) {
            return true;
        }
    }

    if line
        .previous_char(cursor_col)
        .is_some_and(|(_, ch)| ch == '.')
    {
        return true;
    }

    let mut word_start = cursor_col;
    while word_start > 0 {
        let Some((prev_start, prev)) = line.previous_char(word_start) else {
            break;
        };
        if !is_word_char(prev) {
            break;
        }
        word_start = prev_start;
    }

    word_start < cursor_col && cursor_col - word_start >= 2
}

fn configured_completion_sources() -> Vec<CompletionSourceKind> {
    crate::globals::with_config(|config| completion_source_kinds(&config.completion_sources))
        .unwrap_or_else(|| {
            vec![
                CompletionSourceKind::Lsp,
                CompletionSourceKind::Paths,
                CompletionSourceKind::BufferWords,
            ]
        })
}

fn is_path_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '~')
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn is_path_completion_prefix(line: &impl TextRef, start: usize, end: usize) -> bool {
    line.range_starts_with(start, end, "./") == Some(true)
        || line.range_starts_with(start, end, "../") == Some(true)
        || line.range_starts_with(start, end, "~/") == Some(true)
        || line.range_starts_with(start, end, "/") == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_source_names_preserve_configured_order() {
        assert_eq!(
            completion_source_names(&["buffer_words".to_string(), "lsp".to_string()]),
            vec!["buffer_words".to_string(), "lsp".to_string()]
        );
    }

    #[test]
    fn completion_source_kinds_maps_supported_names() {
        assert_eq!(
            completion_source_kinds(&[
                "lsp".to_string(),
                "paths".to_string(),
                "buffer_words".to_string()
            ]),
            vec![
                CompletionSourceKind::Lsp,
                CompletionSourceKind::Paths,
                CompletionSourceKind::BufferWords
            ]
        );
    }

    #[test]
    fn should_autocomplete_requires_a_real_prefix() {
        let buffer = crate::buffer::Buffer::from_str("x");
        assert!(!should_autocomplete(
            &buffer,
            crate::buffer::Cursor::new(0, 1)
        ));
    }

    #[test]
    fn should_autocomplete_accepts_word_and_path_prefixes() {
        let words = crate::buffer::Buffer::from_str("alpha");
        let paths = crate::buffer::Buffer::from_str("./src/");
        let methods = crate::buffer::Buffer::from_str("foo.");
        let whitespace = crate::buffer::Buffer::from_str("   ");

        assert!(should_autocomplete(
            &words,
            crate::buffer::Cursor::new(0, 5)
        ));
        assert!(should_autocomplete(
            &paths,
            crate::buffer::Cursor::new(0, 6)
        ));
        assert!(should_autocomplete(
            &methods,
            crate::buffer::Cursor::new(0, 4)
        ));
        assert!(!should_autocomplete(
            &whitespace,
            crate::buffer::Cursor::new(0, 3)
        ));
    }
}
