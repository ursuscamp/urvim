//! Colorscheme picker source and selection behavior.

use crate::background::JobManager;
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::query::{
    PickerQueryMode, exact_matches, fuzzy_matches, query_prompt_segments,
};
use crate::ui::picker::{PickerPreviewEvent, PickerSearchEvent, PickerSource, PickerWidget};
use crate::ui::{Command, Intent};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;

static NEXT_COLORSCHEME_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Picker source for browsing and selecting colorschemes.
#[derive(Debug, Clone)]
pub struct ColorschemePickerSource {
    names: Arc<Vec<String>>,
    current_generation: Arc<AtomicU64>,
    fuzzy_mode: Arc<AtomicBool>,
    jobs: Arc<JobManager>,
}

/// Colorscheme picker query mode.
pub type QueryMode = crate::ui::picker::query::PickerQueryMode;

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

    /// Returns prompt segments for the colorscheme picker.
    pub fn query_prompt_segments(mode: QueryMode) -> Vec<PromptSegment> {
        query_prompt_segments(mode)
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

    fn toggle_query_mode(&self) -> Option<PickerQueryMode> {
        Some(ColorschemePickerSource::toggle_query_mode(self))
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

        let filtered: Vec<String> = if query.is_empty() {
            self.names.to_vec()
        } else {
            match self.query_mode() {
                QueryMode::Exact => self
                    .names
                    .iter()
                    .filter(|name| exact_matches(query, name.as_str()))
                    .cloned()
                    .collect(),
                QueryMode::Fuzzy => self
                    .names
                    .iter()
                    .filter(|name| fuzzy_matches(query, name.as_str()))
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
        if let Err(error) =
            crate::globals::activate_theme(item, crate::event::ThemeChangeSource::Picker)
        {
            tracing::warn!(theme = item, %error, "failed to activate selected colorscheme");
        }
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
    use crate::ui::Intent;
    use crate::ui::picker::{PickerItem, PickerSearchEvent};
    use urvim_terminal::Style;

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
        let registry = urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load");
        let _registry_guard = globals::set_test_theme_registry(registry.clone());
        let _theme_guard = globals::set_test_active_theme(
            registry
                .get("Friday Night")
                .expect("default theme should exist")
                .clone(),
        );
        let _config_guard = globals::set_test_config(crate::config::Config {
            theme: "Friday Night".to_string(),
            ..Default::default()
        });
        globals::clear_editor_events_for_tests();
        let source = ColorschemePickerSource::new(sorted_theme_names());
        let intent = source.select(&"Nord".to_string());
        assert!(matches!(
            intent,
            Intent::Command(Command::EnqueueNotification { .. })
        ));
        assert!(matches!(
            globals::take_editor_event(),
            Some(crate::event::EditorEvent::ThemeChanged {
                previous_theme,
                theme,
                source: crate::event::ThemeChangeSource::Picker,
            }) if previous_theme == "Friday Night" && theme == "Nord"
        ));
        assert!(globals::take_editor_event().is_none());
        assert_eq!(
            globals::with_config(|config| config.theme.clone()).as_deref(),
            Some("Nord")
        );
        assert_eq!(
            globals::with_active_theme(|theme| theme.map(|theme| theme.name().to_string())),
            Some("Nord".to_string())
        );
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
    fn colorscheme_picker_supports_exact_and_fuzzy_modes() {
        let source = ColorschemePickerSource::new(sorted_theme_names());

        let (sender, receiver) = std::sync::mpsc::channel();
        source.set_query_mode(QueryMode::Exact);
        source.start_search("night", 1, sender);

        let mut exact_results = Vec::new();
        while let Ok(event) = receiver.recv() {
            match event {
                PickerSearchEvent::PickerChunk { chunk, .. } => exact_results.extend(chunk),
                PickerSearchEvent::PickerSearchComplete { .. } => break,
                _ => {}
            }
        }

        let (sender, receiver) = std::sync::mpsc::channel();
        source.set_query_mode(QueryMode::Fuzzy);
        source.start_search("tkn", 2, sender);

        let mut fuzzy_results = Vec::new();
        while let Ok(event) = receiver.recv() {
            match event {
                PickerSearchEvent::PickerChunk { chunk, .. } => fuzzy_results.extend(chunk),
                PickerSearchEvent::PickerSearchComplete { .. } => break,
                _ => {}
            }
        }

        assert_eq!(exact_results, vec!["Friday Night", "Tokyo Night"]);
        assert_eq!(fuzzy_results, vec!["Tokyo Night"]);
    }

    #[test]
    fn colorscheme_picker_prompt_segments_include_mode_label() {
        assert_eq!(
            ColorschemePickerSource::query_prompt_segments(QueryMode::Exact)[0].text,
            "Exact"
        );
        assert_eq!(
            ColorschemePickerSource::query_prompt_segments(QueryMode::Fuzzy)[0].text,
            "Fuzzy"
        );
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
        let registry = urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load");
        let _registry_guard = globals::set_test_theme_registry(registry.clone());
        let _theme_guard = globals::set_test_active_theme(
            registry
                .get("Friday Night")
                .expect("default theme should exist")
                .clone(),
        );
        let _cfg_guard = globals::set_test_config(crate::config::Config {
            theme: "Friday Night".to_string(),
            ..crate::config::Config::default()
        });
        globals::clear_editor_events_for_tests();

        let source = ColorschemePickerSource::new(sorted_theme_names());
        source.select(&"Nord".to_string());

        assert_eq!(
            globals::with_config(|config| config.theme.clone()).as_deref(),
            Some("Nord")
        );
        assert_eq!(
            globals::with_active_theme(|theme| theme.map(|theme| theme.name().to_string())),
            Some("Nord".to_string())
        );
        assert!(matches!(
            globals::take_editor_event(),
            Some(crate::event::EditorEvent::ThemeChanged {
                previous_theme,
                theme,
                source: crate::event::ThemeChangeSource::Picker,
            }) if previous_theme == "Friday Night" && theme == "Nord"
        ));
        assert!(globals::take_editor_event().is_none());
    }
}
