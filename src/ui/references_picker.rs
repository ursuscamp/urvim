//! LSP references picker source and selection behavior.

use crate::background::JobManager;
use crate::buffer::Cursor;
use crate::globals;
use crate::lsp::runtime::ReferenceItem;
use crate::syntax::FiletypeGlyph;
use crate::terminal::Style;
use crate::ui::inputs::PromptSegment;
use crate::ui::line_format::{
    EllipsisPlacement, FormattedLineSection, FormattedLineTemplate, LineSectionAlignment,
    LineSectionOverflow,
};
use crate::ui::picker::{
    PickerFormattedLine, PickerItem, PickerPreview, PickerPreviewEvent, PickerSearchEvent,
    PickerSource, PickerWidget, picker_indicator_glyph,
};
use crate::ui::{Command, Intent};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::thread;

const REFERENCES_PREVIEW_CONTEXT_LINES: usize = 100;
static NEXT_REFERENCES_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A reference location displayed by the references picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencesPickerItem {
    path: PathBuf,
    cursor: Cursor,
    line_text: String,
}

/// Picker source for LSP references at the active cursor.
#[derive(Debug, Clone)]
pub struct ReferencesPickerSource {
    items: Arc<Vec<ReferencesPickerItem>>,
    current_generation: Arc<AtomicU64>,
    preview_generation: Arc<AtomicU64>,
    query_fuzzy: Arc<AtomicBool>,
    jobs: Arc<JobManager>,
}

/// References picker query mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// Exact substring search.
    Exact,
    /// Fuzzy subsequence search.
    Fuzzy,
}

/// Concrete LSP references picker widget.
pub type ReferencesPickerWidget = PickerWidget<ReferencesPickerSource>;

impl ReferencesPickerItem {
    /// Creates a picker item from an LSP reference item.
    pub fn new(value: ReferenceItem) -> Self {
        Self {
            path: value.path,
            cursor: value.cursor,
            line_text: value.line_text,
        }
    }
}

impl ReferencesPickerSource {
    /// Creates a references picker source from a resolved reference list.
    pub fn new(items: Vec<ReferenceItem>, jobs: Arc<JobManager>) -> Self {
        Self {
            items: Arc::new(items.into_iter().map(ReferencesPickerItem::new).collect()),
            current_generation: Arc::new(AtomicU64::new(
                NEXT_REFERENCES_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            preview_generation: Arc::new(AtomicU64::new(0)),
            query_fuzzy: Arc::new(AtomicBool::new(false)),
            jobs,
        }
    }

    /// Returns the current query mode.
    pub fn query_mode(&self) -> QueryMode {
        if self.query_fuzzy.load(Ordering::SeqCst) {
            QueryMode::Fuzzy
        } else {
            QueryMode::Exact
        }
    }

    /// Updates the current query mode.
    pub fn set_query_mode(&self, mode: QueryMode) {
        self.query_fuzzy
            .store(matches!(mode, QueryMode::Fuzzy), Ordering::SeqCst);
    }

    /// Toggles between exact and fuzzy query mode.
    pub fn toggle_query_mode(&self) -> QueryMode {
        let next = match self.query_mode() {
            QueryMode::Exact => QueryMode::Fuzzy,
            QueryMode::Fuzzy => QueryMode::Exact,
        };
        self.set_query_mode(next);
        next
    }

    /// Returns prompt segments for the references picker.
    pub fn query_prompt_segments(mode: QueryMode) -> Vec<PromptSegment> {
        vec![
            PromptSegment::new(
                match mode {
                    QueryMode::Exact => "Exact",
                    QueryMode::Fuzzy => "Fuzzy",
                },
                highlight_style(match mode {
                    QueryMode::Exact => "ui.input.prompt.exact",
                    QueryMode::Fuzzy => "ui.input.prompt.fuzzy",
                }),
            ),
            PromptSegment::new(
                format!(" {} ", picker_indicator_glyph()),
                highlight_style("ui.input.prompt.separator"),
            ),
        ]
    }
}

impl PickerSource for ReferencesPickerSource {
    type Item = ReferencesPickerItem;

    fn set_generation(&self, generation: u64) {
        self.current_generation.store(generation, Ordering::SeqCst);
    }

    fn job_manager(&self) -> Arc<JobManager> {
        Arc::clone(&self.jobs)
    }

    fn start_search(
        &self,
        query: &str,
        generation: u64,
        sender: Sender<PickerSearchEvent<Self::Item>>,
    ) {
        let current_generation = self.current_generation.load(Ordering::SeqCst);
        debug_assert_eq!(current_generation, generation);

        let fuzzy = matches!(self.query_mode(), QueryMode::Fuzzy);
        let query = query.to_lowercase();
        let chunk = self
            .items
            .iter()
            .filter(|item| reference_matches(item, query.as_str(), fuzzy))
            .cloned()
            .collect();

        sender
            .send(PickerSearchEvent::PickerChunk { generation, chunk })
            .ok();
        sender
            .send(PickerSearchEvent::PickerSearchComplete { generation })
            .ok();
    }

    fn preview_key(&self, item: &Self::Item) -> Option<String> {
        Some(item.path.to_string_lossy().into_owned())
    }

    fn start_preview(&self, item: Self::Item, generation: u64, sender: Sender<PickerPreviewEvent>) {
        self.preview_generation.store(generation, Ordering::SeqCst);
        let current_generation = self.preview_generation.clone();
        thread::spawn(move || {
            sender.send(PickerPreviewEvent::Started { generation }).ok();
            let result = build_references_preview(&item);
            if current_generation.load(Ordering::SeqCst) != generation {
                return;
            }

            match result {
                Ok(preview) => sender
                    .send(PickerPreviewEvent::Loaded {
                        generation,
                        preview,
                    })
                    .ok(),
                Err(error) => sender
                    .send(PickerPreviewEvent::Failed {
                        generation,
                        message: error.to_string(),
                    })
                    .ok(),
            };
        });
    }

    fn cancel_preview(&self) {
        self.preview_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn select(&self, item: &Self::Item) -> Intent {
        Intent::Command(Command::OpenFileAtCursor(item.path.clone(), item.cursor))
    }
}

impl PickerItem for ReferencesPickerItem {
    fn formatted_line(&self, base_style: Style) -> PickerFormattedLine {
        let label = display_label(self.path.as_path());
        let suffix = format!(":{}:{}", self.cursor.line + 1, self.cursor.col + 1);
        let mut sections = Vec::new();
        let mut values = Vec::new();

        if let Some(glyph) = FiletypeGlyph::from_path(self.path.as_path()) {
            let glyph_width = unicode_width::UnicodeWidthStr::width(glyph.glyph.as_str()) as u16;
            sections.push(FormattedLineSection::fixed(
                glyph_width,
                base_style.accent(glyph.style),
            ));
            values.push(glyph.glyph.to_string());
            sections.push(FormattedLineSection::fixed(1, base_style));
            values.push(" ".to_string());
        }

        sections.push(
            FormattedLineSection::measured(base_style)
                .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::Start)),
        );
        values.push(label);

        let suffix_width = unicode_width::UnicodeWidthStr::width(suffix.as_str()) as u16;
        sections.push(FormattedLineSection::fixed(
            suffix_width,
            base_style.faint().accent(location_style()),
        ));
        values.push(suffix);

        sections.push(
            FormattedLineSection::flex(1, base_style.faint())
                .with_alignment(LineSectionAlignment::Right)
                .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::End)),
        );
        values.push(format!("  {}", self.line_text));

        PickerFormattedLine::new(FormattedLineTemplate::new(sections), values)
    }
}

fn reference_matches(item: &ReferencesPickerItem, query: &str, fuzzy: bool) -> bool {
    if query.is_empty() {
        return true;
    }

    let candidate =
        format!("{} {}", display_label(item.path.as_path()), item.line_text).to_lowercase();
    if fuzzy {
        fuzzy_matches(query, candidate.as_str())
    } else {
        candidate.contains(query)
    }
}

fn fuzzy_matches(query: &str, candidate: &str) -> bool {
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

fn build_references_preview(item: &ReferencesPickerItem) -> std::io::Result<PickerPreview> {
    let start_line = item
        .cursor
        .line
        .saturating_sub(REFERENCES_PREVIEW_CONTEXT_LINES);
    let _ = std::fs::metadata(item.path.as_path())?;

    Ok(PickerPreview::new(
        item.path.to_string_lossy(),
        start_line + 1,
        Some(item.cursor.line + 1),
    ))
}

fn display_label(path: &Path) -> String {
    let Ok(cwd) = std::env::current_dir() else {
        return path.to_string_lossy().into_owned();
    };

    path.strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn highlight_style(name: &str) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.highlight_style_for_name(name))
            .unwrap_or_default()
    })
}

fn location_style() -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default("ui.picker.location"))
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::Style;

    fn test_item() -> ReferencesPickerItem {
        ReferencesPickerItem {
            path: PathBuf::from("/tmp/example.rs"),
            cursor: Cursor::new(2, 4),
            line_text: "let value = example();".to_string(),
        }
    }

    #[test]
    fn references_picker_selects_open_file_at_cursor_intent() {
        let source = ReferencesPickerSource::new(
            vec![ReferenceItem {
                path: PathBuf::from("/tmp/example.rs"),
                cursor: Cursor::new(2, 4),
                line_text: "let value = example();".to_string(),
            }],
            Arc::new(JobManager::new()),
        );

        assert!(matches!(
            source.select(&test_item()),
            Intent::Command(Command::OpenFileAtCursor(_, Cursor { line: 2, col: 4 }))
        ));
    }

    #[test]
    fn references_picker_item_renders_location_and_line_text() {
        let rendered = test_item().render_segments(80, Style::default());
        let text = rendered
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<String>();

        assert!(text.contains("example.rs"));
        assert!(text.contains(":3:5"));
        assert!(text.contains("let value"));
    }

    #[test]
    fn references_picker_filters_by_path_or_line_text() {
        let item = test_item();

        assert!(reference_matches(&item, "example", false));
        assert!(reference_matches(&item, "value", false));
        assert!(!reference_matches(&item, "missing", false));
    }

    #[test]
    fn references_picker_supports_fuzzy_query_mode() {
        let source = ReferencesPickerSource::new(Vec::new(), Arc::new(JobManager::new()));
        assert_eq!(source.query_mode(), QueryMode::Exact);
        assert_eq!(source.toggle_query_mode(), QueryMode::Fuzzy);
        assert_eq!(source.query_mode(), QueryMode::Fuzzy);

        assert!(reference_matches(&test_item(), "lvex", true));
        assert!(!reference_matches(&test_item(), "zzz", true));
    }

    #[test]
    fn references_picker_query_prompt_segments_follow_mode() {
        let exact = ReferencesPickerSource::query_prompt_segments(QueryMode::Exact);
        let fuzzy = ReferencesPickerSource::query_prompt_segments(QueryMode::Fuzzy);

        assert_eq!(exact[0].text, "Exact");
        assert_eq!(fuzzy[0].text, "Fuzzy");
    }
}
