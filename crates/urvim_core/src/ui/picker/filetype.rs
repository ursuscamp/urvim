//! Filetype picker source and selection behavior.

use crate::background::JobManager;
use crate::icon::FiletypeIcon;
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::query::{
    PickerQueryMode, exact_matches, fuzzy_matches, query_prompt_segments,
};
use crate::ui::picker::{
    FormattedLineSection, FormattedLineTemplate, LineSectionAlignment, LineSectionOverflow,
    PickerFormattedLine, PickerItem, PickerPreviewEvent, PickerSearchEvent, PickerSource,
    PickerWidget,
};
use crate::ui::{Command, Intent};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use urvim_terminal::Style;

static NEXT_FILETYPE_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A selectable filetype entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiletypePickerItem {
    /// Canonical syntax name.
    pub name: String,
    /// User-facing syntax label.
    pub label: String,
    /// Optional filetype icon shown in the picker.
    pub icon: Option<FiletypeIcon>,
}

impl PickerItem for FiletypePickerItem {
    fn formatted_line(&self, base_style: Style) -> PickerFormattedLine {
        let (icon_style, icon_value) = match self.icon.as_ref() {
            Some(icon) => (base_style.accent(icon.style), icon.glyph.to_string()),
            None => (base_style, "   ".to_string()),
        };

        PickerFormattedLine::new(
            FormattedLineTemplate::new(vec![
                FormattedLineSection::fixed(3, icon_style).with_overflow(LineSectionOverflow::Clip),
                FormattedLineSection::measured(base_style).with_overflow(
                    LineSectionOverflow::Ellipsis(crate::ui::picker::EllipsisPlacement::End),
                ),
                FormattedLineSection::flex(1, base_style.faint())
                    .with_alignment(LineSectionAlignment::Right)
                    .with_overflow(LineSectionOverflow::Ellipsis(
                        crate::ui::picker::EllipsisPlacement::Start,
                    )),
            ]),
            vec![icon_value, self.label.clone(), self.name.clone()],
        )
    }
}

/// Picker source for changing buffer filetypes.
#[derive(Debug, Clone)]
pub struct FiletypePickerSource {
    items: Arc<Vec<FiletypePickerItem>>,
    current_generation: Arc<AtomicU64>,
    fuzzy_mode: Arc<AtomicBool>,
    jobs: Arc<JobManager>,
}

/// Filetype picker query mode.
pub type QueryMode = crate::ui::picker::query::PickerQueryMode;

/// Concrete filetype picker widget.
pub type FiletypePickerWidget = PickerWidget<FiletypePickerSource>;

impl FiletypePickerSource {
    /// Creates a filetype picker from registered built-in syntaxes.
    pub fn new(items: Vec<FiletypePickerItem>) -> Self {
        Self::with_jobs(items, Arc::new(JobManager::new()))
    }

    /// Creates a filetype picker backed by a shared job manager.
    pub fn with_jobs(items: Vec<FiletypePickerItem>, jobs: Arc<JobManager>) -> Self {
        Self {
            items: Arc::new(items),
            current_generation: Arc::new(AtomicU64::new(
                NEXT_FILETYPE_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            fuzzy_mode: Arc::new(AtomicBool::new(true)),
            jobs,
        }
    }

    /// Returns all built-in filetype picker items sorted by display label.
    pub fn builtin_items() -> Vec<FiletypePickerItem> {
        let mut items = urvim_syntax::builtin_syntax_registry()
            .ok()
            .map(|registry| {
                registry
                    .names()
                    .into_iter()
                    .map(|name| FiletypePickerItem {
                        icon: registry.metadata(&name).and_then(|metadata| {
                            FiletypeIcon::from_metadata(
                                Some(&metadata),
                                crate::icon::nerdfont_enabled(),
                            )
                        }),
                        label: registry
                            .display_name(&name)
                            .map(|label| label.to_string())
                            .unwrap_or_else(|| name.clone()),
                        name,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        items.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then(left.name.cmp(&right.name))
        });
        items
    }

    /// Returns the current search mode.
    pub fn query_mode(&self) -> QueryMode {
        if self.fuzzy_mode.load(Ordering::SeqCst) {
            QueryMode::Fuzzy
        } else {
            QueryMode::Exact
        }
    }

    /// Updates the current search mode.
    pub fn set_query_mode(&self, mode: QueryMode) {
        self.fuzzy_mode
            .store(matches!(mode, QueryMode::Fuzzy), Ordering::SeqCst);
    }

    /// Toggles between exact and fuzzy query mode.
    pub fn toggle_query_mode(&self) -> QueryMode {
        let next = self.query_mode().toggled();
        self.set_query_mode(next);
        next
    }

    /// Returns prompt segments for the filetype picker.
    pub fn query_prompt_segments(mode: QueryMode) -> Vec<PromptSegment> {
        query_prompt_segments(mode)
    }
}

impl PickerSource for FiletypePickerSource {
    type Item = FiletypePickerItem;

    fn set_generation(&self, generation: u64) {
        self.current_generation.store(generation, Ordering::SeqCst);
    }

    fn job_manager(&self) -> Arc<JobManager> {
        Arc::clone(&self.jobs)
    }

    fn toggle_query_mode(&self) -> Option<PickerQueryMode> {
        Some(FiletypePickerSource::toggle_query_mode(self))
    }

    fn query_prompt_segments_for_mode(&self, mode: PickerQueryMode) -> Option<Vec<PromptSegment>> {
        Some(Self::query_prompt_segments(mode))
    }

    fn start_search(
        &self,
        query: &str,
        generation: u64,
        sender: Sender<PickerSearchEvent<Self::Item>>,
    ) {
        sender
            .send(PickerSearchEvent::PickerSearchStarted {
                generation,
                query: query.to_string(),
            })
            .ok();

        let filtered: Vec<FiletypePickerItem> = if query.is_empty() {
            self.items.to_vec()
        } else {
            self.items
                .iter()
                .filter(|item| {
                    let haystack = format!("{} {}", item.label, item.name);
                    match self.query_mode() {
                        QueryMode::Exact => exact_matches(query, haystack.as_str()),
                        QueryMode::Fuzzy => fuzzy_matches(query, haystack.as_str()),
                    }
                })
                .cloned()
                .collect()
        };

        if !filtered.is_empty() {
            sender
                .send(PickerSearchEvent::PickerChunk {
                    generation,
                    chunk: filtered,
                })
                .ok();
        }

        sender
            .send(PickerSearchEvent::PickerSearchComplete { generation })
            .ok();
    }

    fn preview_key(&self, _item: &Self::Item) -> Option<String> {
        None
    }

    fn start_preview(
        &self,
        _item: Self::Item,
        _generation: u64,
        _sender: Sender<PickerPreviewEvent>,
    ) {
    }

    fn select(&self, item: &Self::Item) -> Intent {
        Intent::Command(Command::SetBufferFiletype(None, item.name.clone()))
    }

    fn cancel_search(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::picker::PickerSource;

    fn items() -> Vec<FiletypePickerItem> {
        vec![
            FiletypePickerItem {
                name: "rust".to_string(),
                label: "Rust".to_string(),
                icon: None,
            },
            FiletypePickerItem {
                name: "javascript".to_string(),
                label: "JavaScript".to_string(),
                icon: None,
            },
        ]
    }

    #[test]
    fn filetype_picker_selects_set_buffer_filetype_intent() {
        let source = FiletypePickerSource::new(items());
        let intent = source.select(&FiletypePickerItem {
            name: "rust".to_string(),
            label: "Rust".to_string(),
            icon: None,
        });

        assert!(matches!(
            intent,
            Intent::Command(Command::SetBufferFiletype(None, filetype)) if filetype == "rust"
        ));
    }

    #[test]
    fn filetype_picker_filters_by_label_and_name() {
        let source = FiletypePickerSource::new(items());
        let (sender, receiver) = std::sync::mpsc::channel();
        source.start_search("js", 1, sender);

        let mut results = Vec::new();
        while let Ok(event) = receiver.recv() {
            match event {
                PickerSearchEvent::PickerChunk { chunk, .. } => results.extend(chunk),
                PickerSearchEvent::PickerSearchComplete { .. } => break,
                _ => {}
            }
        }

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "javascript");
    }

    #[test]
    fn filetype_picker_renders_icon_and_muted_name() {
        let item = FiletypePickerItem {
            name: "rust".to_string(),
            label: "Rust".to_string(),
            icon: Some(FiletypeIcon {
                glyph: "".into(),
                style: Style::new().fg(urvim_terminal::Color::ansi(160)),
            }),
        };

        let base_style = Style::new().bg(urvim_terminal::Color::ansi(30));
        let segments = item.formatted_line(base_style).render_segments(80);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].text, "  ");
        assert_eq!(
            segments[0].style,
            base_style.accent(Style::new().fg(urvim_terminal::Color::ansi(160)))
        );
        assert_eq!(segments[1].text, "Rust");
        assert_eq!(segments[1].style, base_style);
        assert!(segments[2].text.ends_with("rust"));
        assert!(segments[2].text.len() > "rust".len());
        assert_eq!(segments[2].style, base_style.faint());
    }

    #[test]
    fn filetype_picker_renders_blank_icon_column_when_missing() {
        let item = FiletypePickerItem {
            name: "plaintext".to_string(),
            label: "Plain Text".to_string(),
            icon: None,
        };

        let segments = item.formatted_line(Style::new()).render_segments(80);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].text, "   ");
        assert_eq!(segments[1].text, "Plain Text");
        assert!(segments[2].text.ends_with("plaintext"));
        assert!(segments[2].text.len() > "plaintext".len());
        assert_eq!(segments[2].style, Style::new().faint());
    }
}
