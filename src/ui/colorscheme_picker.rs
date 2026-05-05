//! Colorscheme picker source and selection behavior.

use crate::background::JobManager;
use crate::ui::picker::{PickerPreviewEvent, PickerSearchEvent, PickerSource, PickerWidget};
use crate::ui::{Command, Intent};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

static NEXT_COLORSCHEME_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Picker source for browsing and selecting colorschemes.
#[derive(Debug, Clone)]
pub struct ColorschemePickerSource {
    names: Arc<Vec<String>>,
    current_generation: Arc<AtomicU64>,
    jobs: Arc<JobManager>,
}

/// Concrete colorscheme picker widget.
pub type ColorschemePickerWidget = PickerWidget<ColorschemePickerSource>;

impl ColorschemePickerSource {
    /// Creates a colorscheme picker from a sorted list of theme names.
    pub fn new(names: Vec<String>) -> Self {
        Self::with_jobs(names, Arc::new(JobManager::new()))
    }

    /// Creates a colorscheme picker backed by a shared job manager.
    pub fn with_jobs(names: Vec<String>, jobs: Arc<JobManager>) -> Self {
        Self {
            names: Arc::new(names),
            current_generation: Arc::new(AtomicU64::new(
                NEXT_COLORSCHEME_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            jobs,
        }
    }
}

impl PickerSource for ColorschemePickerSource {
    type Item = String;

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
        let _ = sender.send(PickerSearchEvent::PickerSearchStarted {
            generation,
            query: query.to_string(),
        });

        let filtered: Vec<String> = if query.is_empty() {
            self.names.to_vec()
        } else {
            let q = query.to_lowercase();
            self.names
                .iter()
                .filter(|name| name.to_lowercase().contains(&q))
                .cloned()
                .collect()
        };

        if !filtered.is_empty() {
            let _ = sender.send(PickerSearchEvent::PickerChunk {
                generation,
                chunk: filtered,
            });
        }

        let _ = sender.send(PickerSearchEvent::PickerSearchComplete { generation });
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
        crate::globals::update_theme_in_config(item);
        Intent::Command(Command::EnqueueNotification {
            level: crate::notification::NotificationLevel::Info,
            message: format!("colorscheme: {item}"),
        })
    }

    fn cancel_search(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::globals;
    use crate::terminal::Style;
    use crate::ui::Intent;
    use crate::ui::picker::{PickerItem, PickerSearchEvent};

    fn sorted_theme_names() -> Vec<String> {
        vec![
            "Catppuccin".to_string(),
            "Dracula".to_string(),
            "Friday Night".to_string(),
            "Nord".to_string(),
            "Rose Pine".to_string(),
            "Saturday Morning".to_string(),
            "Tokyo Night".to_string(),
        ]
    }

    #[test]
    fn colorscheme_picker_selects_enqueue_notification_intent() {
        let source = ColorschemePickerSource::new(sorted_theme_names());
        let intent = source.select(&"Nord".to_string());
        assert!(matches!(
            intent,
            Intent::Command(Command::EnqueueNotification { .. })
        ));
    }

    #[test]
    fn colorscheme_picker_filters_case_insensitively() {
        let source = ColorschemePickerSource::new(sorted_theme_names());
        let (sender, receiver) = std::sync::mpsc::channel();
        source.start_search("nord", 1, sender);

        let mut results = Vec::new();
        while let Ok(event) = receiver.recv() {
            match event {
                PickerSearchEvent::PickerChunk { chunk, .. } => results.extend(chunk),
                PickerSearchEvent::PickerSearchComplete { .. } => break,
                _ => {}
            }
        }

        assert_eq!(results, vec!["Nord"]);
    }

    #[test]
    fn colorscheme_picker_returns_all_names_when_query_is_empty() {
        let source = ColorschemePickerSource::new(sorted_theme_names());
        let (sender, receiver) = std::sync::mpsc::channel();
        source.start_search("", 1, sender);

        let mut results = Vec::new();
        while let Ok(event) = receiver.recv() {
            match event {
                PickerSearchEvent::PickerChunk { chunk, .. } => results.extend(chunk),
                PickerSearchEvent::PickerSearchComplete { .. } => break,
                _ => {}
            }
        }

        assert_eq!(results, sorted_theme_names());
    }

    #[test]
    fn colorscheme_picker_returns_empty_when_no_match() {
        let source = ColorschemePickerSource::new(sorted_theme_names());
        let (sender, receiver) = std::sync::mpsc::channel();
        source.start_search("zzznonexistent", 1, sender);

        let mut count = 0;
        while let Ok(event) = receiver.recv() {
            match event {
                PickerSearchEvent::PickerChunk { .. } => count += 1,
                PickerSearchEvent::PickerSearchComplete { .. } => break,
                _ => {}
            }
        }

        assert_eq!(count, 0);
    }

    #[test]
    fn colorscheme_item_renders_theme_name() {
        let segments = "Dracula".to_string().render_segments(20, Style::default());
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "Dracula");
    }

    #[test]
    fn colorscheme_select_updates_config() {
        let _cfg_guard = globals::set_test_config(crate::config::Config {
            theme: "Friday Night".to_string(),
            ..crate::config::Config::default()
        });

        let source = ColorschemePickerSource::new(sorted_theme_names());
        source.select(&"Nord".to_string());

        globals::with_config(|config| {
            assert_eq!(config.theme, "Nord");
        });
    }
}
