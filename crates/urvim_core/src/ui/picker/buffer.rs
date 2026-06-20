//! Visible buffer picker source and selection behavior.

use crate::background::JobManager;
use crate::buffer::{Buffer, BufferId};
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::line::{display_path_relative_to_cwd, push_file_glyph, push_tail_label};
use crate::ui::picker::query::{
    PickerQueryMode, exact_matches, fuzzy_matches, query_prompt_segments,
};
use crate::ui::picker::{
    FormattedLineTemplate, PickerFormattedLine, PickerItem, PickerSearchEvent, PickerSource,
    PickerWidget,
};
use crate::ui::{Command, Intent};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use urvim_terminal::Style;

static NEXT_BUFFER_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A visible buffer displayed by the buffer picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferPickerItem {
    /// Stable buffer identifier.
    pub buffer_id: BufferId,
    path: Option<PathBuf>,
    label: String,
}

/// Picker source for browsing visible buffers.
#[derive(Debug, Clone)]
pub struct BufferPickerSource {
    items: Arc<Vec<BufferPickerItem>>,
    current_generation: Arc<AtomicU64>,
    fuzzy_mode: Arc<AtomicBool>,
    jobs: Arc<JobManager>,
}

/// Visible-buffer picker query mode.
pub type QueryMode = crate::ui::picker::query::PickerQueryMode;

/// Concrete visible-buffer picker widget.
pub type BufferPickerWidget = PickerWidget<BufferPickerSource>;

impl BufferPickerItem {
    /// Creates a picker item from a live buffer.
    pub fn from_buffer(buffer_id: BufferId, buffer: &Buffer) -> Self {
        let path = buffer
            .path()
            .cloned()
            .map(|path| path.as_path().to_path_buf());
        let mut label = if let Some(path) = path.as_ref() {
            display_path_relative_to_cwd(path.as_path())
        } else {
            format!("Untitled #{}", buffer_id.get())
        };
        if buffer.is_modified() {
            label.push_str(" [modified]");
        }

        Self {
            buffer_id,
            path,
            label,
        }
    }

    fn search_text(&self) -> &str {
        &self.label
    }
}

impl BufferPickerSource {
    /// Creates a buffer picker from a list of visible buffers.
    pub fn new(items: Vec<BufferPickerItem>) -> Self {
        Self::with_jobs(items, Arc::new(JobManager::new()))
    }

    /// Creates a buffer picker backed by a shared job manager.
    pub fn with_jobs(items: Vec<BufferPickerItem>, jobs: Arc<JobManager>) -> Self {
        Self {
            items: Arc::new(items),
            current_generation: Arc::new(AtomicU64::new(
                NEXT_BUFFER_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            fuzzy_mode: Arc::new(AtomicBool::new(true)),
            jobs,
        }
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

    /// Returns prompt segments for the buffer picker.
    pub fn query_prompt_segments(mode: QueryMode) -> Vec<PromptSegment> {
        query_prompt_segments(mode)
    }
}

impl PickerSource for BufferPickerSource {
    type Item = BufferPickerItem;

    fn set_generation(&self, generation: u64) {
        self.current_generation.store(generation, Ordering::SeqCst);
    }

    fn job_manager(&self) -> Arc<JobManager> {
        Arc::clone(&self.jobs)
    }

    fn toggle_query_mode(&self) -> Option<PickerQueryMode> {
        Some(BufferPickerSource::toggle_query_mode(self))
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
        let current_generation = self.current_generation.load(Ordering::SeqCst);
        debug_assert_eq!(current_generation, generation);

        sender
            .send(PickerSearchEvent::PickerSearchStarted {
                generation,
                query: query.to_string(),
            })
            .ok();

        let filtered: Vec<BufferPickerItem> = if query.is_empty() {
            self.items.to_vec()
        } else {
            match self.query_mode() {
                QueryMode::Exact => self
                    .items
                    .iter()
                    .filter(|item| exact_matches(query, item.search_text()))
                    .cloned()
                    .collect(),
                QueryMode::Fuzzy => self
                    .items
                    .iter()
                    .filter(|item| fuzzy_matches(query, item.search_text()))
                    .cloned()
                    .collect(),
            }
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

    fn select(&self, item: &Self::Item) -> Intent {
        Intent::Command(Command::FocusBuffer(item.buffer_id))
    }

    fn cancel_search(&self) {}
}

impl PickerItem for BufferPickerItem {
    fn formatted_line(&self, base_style: Style) -> PickerFormattedLine {
        let mut sections = Vec::new();
        let mut values = Vec::new();

        if let Some(path) = self.path.as_ref() {
            push_file_glyph(&mut sections, &mut values, path.as_path(), base_style);
        }
        push_tail_label(&mut sections, &mut values, self.label.clone(), base_style);

        PickerFormattedLine::new(FormattedLineTemplate::new(sections), values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn buffer_picker_selects_focus_buffer_intent() {
        let item = BufferPickerItem {
            buffer_id: BufferId::new(7),
            path: None,
            label: "Untitled #7".to_string(),
        };
        let source = BufferPickerSource::new(vec![item.clone()]);

        assert!(matches!(
            source.select(&item),
            Intent::Command(Command::FocusBuffer(buffer_id)) if buffer_id == BufferId::new(7)
        ));
    }

    #[test]
    fn buffer_picker_filters_visible_items() {
        let source = BufferPickerSource::new(vec![
            BufferPickerItem {
                buffer_id: BufferId::new(1),
                path: None,
                label: "Untitled #1".to_string(),
            },
            BufferPickerItem {
                buffer_id: BufferId::new(2),
                path: Some(PathBuf::from("/tmp/project/src/main.rs")),
                label: "src/main.rs".to_string(),
            },
        ]);

        let (sender, receiver) = std::sync::mpsc::channel();
        source.set_generation(1);
        source.start_search("main", 1, sender);

        let mut results = Vec::new();
        while let Ok(event) = receiver.recv() {
            match event {
                PickerSearchEvent::PickerChunk { chunk, .. } => results.extend(chunk),
                PickerSearchEvent::PickerSearchComplete { .. } => break,
                _ => {}
            }
        }

        assert!(
            results
                .iter()
                .any(|item| item.buffer_id == BufferId::new(2))
        );
    }
}
