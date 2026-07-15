//! Plugin-owned picker source and selection behavior.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;

use urvim_terminal::Style;

use crate::background::JobManager;
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::query::{
    PickerQueryMode, exact_matches, fuzzy_match_score, query_prompt_segments,
};
use crate::ui::picker::{
    EllipsisPlacement, FormattedLineSection, FormattedLineTemplate, LineSectionAlignment,
    LineSectionOverflow, PickerFormattedLine, PickerItem, PickerSearchEvent, PickerSource,
    PickerWidget,
};
use crate::ui::{Command, Intent};

/// Numeric identity of a plugin picker instance.
pub type PluginPickerId = u64;

/// Numeric identity of an item in a plugin picker.
pub type PluginPickerItemId = u64;

/// Cancellation emitted when an unresolved plugin picker is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPickerCancelled {
    /// Plugin that owns the picker.
    pub plugin: String,
    /// Picker instance that was cancelled.
    pub picker_id: PluginPickerId,
}

/// A displayable item in a plugin picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPickerItem {
    /// Opaque item identity used to recover the script value on selection.
    pub id: PluginPickerItemId,
    /// Stable key used to preserve highlighting across updates.
    pub key: String,
    /// Primary row label.
    pub label: String,
    /// Optional secondary detail shown on the right.
    pub detail: Option<String>,
}

impl PickerItem for PluginPickerItem {
    fn formatted_line(&self, base_style: Style) -> PickerFormattedLine {
        PickerFormattedLine::new(
            FormattedLineTemplate::new(vec![
                FormattedLineSection::measured(base_style)
                    .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::End)),
                FormattedLineSection::flex(1, base_style.faint())
                    .with_alignment(LineSectionAlignment::Right)
                    .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::Start)),
            ]),
            vec![self.label.clone(), self.detail.clone().unwrap_or_default()],
        )
    }
}

/// Picker source backed by items supplied from a plugin script.
#[derive(Debug)]
pub struct PluginPickerSource {
    plugin: String,
    picker_id: PluginPickerId,
    items: Vec<PluginPickerItem>,
    current_generation: AtomicU64,
    fuzzy_mode: AtomicBool,
    resolved: AtomicBool,
    cancellation_sender: Sender<PluginPickerCancelled>,
    jobs: Arc<JobManager>,
}

/// Concrete plugin picker widget.
pub type PluginPickerWidget = PickerWidget<PluginPickerSource>;

impl PluginPickerSource {
    /// Creates a plugin picker source.
    pub fn new(
        plugin: impl Into<String>,
        picker_id: PluginPickerId,
        items: Vec<PluginPickerItem>,
        cancellation_sender: Sender<PluginPickerCancelled>,
        jobs: Arc<JobManager>,
    ) -> Self {
        Self {
            plugin: plugin.into(),
            picker_id,
            items,
            current_generation: AtomicU64::new(0),
            fuzzy_mode: AtomicBool::new(true),
            resolved: AtomicBool::new(false),
            cancellation_sender,
            jobs,
        }
    }

    /// Returns the picker identity.
    pub fn picker_id(&self) -> PluginPickerId {
        self.picker_id
    }

    /// Returns the plugin that owns the picker.
    pub fn plugin(&self) -> &str {
        self.plugin.as_str()
    }

    /// Returns the current query mode.
    pub fn query_mode(&self) -> PickerQueryMode {
        if self.fuzzy_mode.load(Ordering::SeqCst) {
            PickerQueryMode::Fuzzy
        } else {
            PickerQueryMode::Exact
        }
    }

    /// Replaces all picker items.
    pub fn set_items(&mut self, items: Vec<PluginPickerItem>) {
        self.items = items;
    }

    /// Appends picker items.
    pub fn append_items(&mut self, items: Vec<PluginPickerItem>) {
        self.items.extend(items);
    }

    /// Returns prompt segments for a plugin picker.
    pub fn query_prompt_segments(mode: PickerQueryMode) -> Vec<PromptSegment> {
        query_prompt_segments(mode)
    }

    fn toggle_mode(&self) -> PickerQueryMode {
        let mode = self.query_mode().toggled();
        self.fuzzy_mode
            .store(matches!(mode, PickerQueryMode::Fuzzy), Ordering::SeqCst);
        mode
    }
}

impl Drop for PluginPickerSource {
    fn drop(&mut self) {
        if !self.resolved.swap(true, Ordering::SeqCst) {
            self.cancellation_sender
                .send(PluginPickerCancelled {
                    plugin: self.plugin.clone(),
                    picker_id: self.picker_id,
                })
                .ok();
        }
    }
}

impl PickerSource for PluginPickerSource {
    type Item = PluginPickerItem;

    fn set_generation(&self, generation: u64) {
        self.current_generation.store(generation, Ordering::SeqCst);
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

        let mut results = if query.is_empty() {
            self.items.clone()
        } else {
            match self.query_mode() {
                PickerQueryMode::Exact => self
                    .items
                    .iter()
                    .filter(|item| exact_matches(query, &item_search_text(item)))
                    .cloned()
                    .collect(),
                PickerQueryMode::Fuzzy => {
                    let mut matches = self
                        .items
                        .iter()
                        .filter_map(|item| {
                            fuzzy_match_score(query, &item_search_text(item))
                                .map(|score| (score, item.clone()))
                        })
                        .collect::<Vec<_>>();
                    matches.sort_by_key(|(score, _)| *score);
                    matches.into_iter().map(|(_, item)| item).collect()
                }
            }
        };

        sender
            .send(PickerSearchEvent::PickerResults {
                generation,
                results: std::mem::take(&mut results),
            })
            .ok();
        sender
            .send(PickerSearchEvent::PickerSearchComplete { generation })
            .ok();
    }

    fn job_manager(&self) -> Arc<JobManager> {
        Arc::clone(&self.jobs)
    }

    fn toggle_query_mode(&self) -> Option<PickerQueryMode> {
        Some(self.toggle_mode())
    }

    fn query_prompt_segments_for_mode(&self, mode: PickerQueryMode) -> Option<Vec<PromptSegment>> {
        Some(Self::query_prompt_segments(mode))
    }

    fn result_key(&self, item: &Self::Item) -> Option<String> {
        Some(item.key.clone())
    }

    fn select(&self, item: &Self::Item) -> Intent {
        self.resolved.store(true, Ordering::SeqCst);
        Intent::Command(Command::PluginPickerSelect {
            plugin: self.plugin.clone(),
            picker_id: self.picker_id,
            item_id: item.id,
        })
    }
}

fn item_search_text(item: &PluginPickerItem) -> String {
    match item.detail.as_deref() {
        Some(detail) => format!("{} {detail}", item.label),
        None => item.label.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64, key: &str, label: &str, detail: Option<&str>) -> PluginPickerItem {
        PluginPickerItem {
            id,
            key: key.to_string(),
            label: label.to_string(),
            detail: detail.map(str::to_string),
        }
    }

    fn source(
        items: Vec<PluginPickerItem>,
    ) -> (
        PluginPickerSource,
        std::sync::mpsc::Receiver<PluginPickerCancelled>,
    ) {
        let (sender, receiver) = std::sync::mpsc::channel();
        (
            PluginPickerSource::new(
                "demo",
                7,
                items,
                sender,
                crate::background::shared_test_manager(),
            ),
            receiver,
        )
    }

    #[test]
    fn plugin_picker_filters_labels_and_details() {
        let (source, _receiver) = source(vec![
            item(1, "main", "main", Some("origin/main")),
            item(2, "feature", "feature", Some("local")),
        ]);
        let (sender, receiver) = std::sync::mpsc::channel();

        source.start_search("orgmain", 3, sender);

        let results = receiver
            .into_iter()
            .find_map(|event| match event {
                PickerSearchEvent::PickerResults { results, .. } => Some(results),
                _ => None,
            })
            .expect("picker results");
        assert_eq!(results, vec![item(1, "main", "main", Some("origin/main"))]);
    }

    #[test]
    fn plugin_picker_selection_suppresses_cancellation() {
        let (source, receiver) = source(vec![item(9, "main", "main", None)]);

        assert_eq!(
            source.select(&item(9, "main", "main", None)),
            Intent::Command(Command::PluginPickerSelect {
                plugin: "demo".to_string(),
                picker_id: 7,
                item_id: 9,
            })
        );
        drop(source);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn dropping_unresolved_plugin_picker_cancels_once() {
        let (source, receiver) = source(Vec::new());

        drop(source);

        assert_eq!(
            receiver.try_recv().expect("cancellation"),
            PluginPickerCancelled {
                plugin: "demo".to_string(),
                picker_id: 7,
            }
        );
        assert!(receiver.try_recv().is_err());
    }
}
