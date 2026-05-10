//! LSP code action picker source and selection behavior.

use crate::background::JobManager;
use crate::buffer::BufferId;
use crate::globals;
use crate::lsp::runtime::CodeActionApplication;
use crate::terminal::Style;
use crate::ui::inputs::PromptSegment;
use crate::ui::picker::{
    PickerItem, PickerRenderSegment, PickerSearchEvent, PickerSource, PickerWidget,
    picker_indicator_glyph, visible_tail_text,
};
use crate::ui::{Command, Intent};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

static NEXT_CODE_ACTIONS_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// A single code action displayed by the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionsPickerItem {
    number: usize,
    title: String,
    kind: Option<String>,
    search_text: String,
    application: CodeActionApplication,
}

/// Picker source for LSP code actions.
#[derive(Debug, Clone)]
pub struct CodeActionsPickerSource {
    buffer_id: BufferId,
    actions: Arc<Vec<CodeActionsPickerItem>>,
    current_generation: Arc<AtomicU64>,
    jobs: Arc<JobManager>,
}

/// Concrete code actions picker widget.
pub type CodeActionsPickerWidget = PickerWidget<CodeActionsPickerSource>;

impl CodeActionsPickerSource {
    /// Creates a picker source from the actions returned by LSP.
    pub fn new(
        buffer_id: BufferId,
        actions: Vec<CodeActionApplication>,
        jobs: Arc<JobManager>,
    ) -> Self {
        let actions = actions
            .into_iter()
            .enumerate()
            .map(|(index, application)| {
                let number = index + 1;
                let kind = application.kind.clone();
                let search_text =
                    build_search_text(number, application.title.as_str(), kind.as_deref());
                CodeActionsPickerItem {
                    number,
                    title: application.title.clone(),
                    kind,
                    search_text,
                    application,
                }
            })
            .collect();

        Self {
            buffer_id,
            actions: Arc::new(actions),
            current_generation: Arc::new(AtomicU64::new(
                NEXT_CODE_ACTIONS_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            jobs,
        }
    }

    /// Returns prompt segments for the code actions picker.
    pub fn query_prompt_segments() -> Vec<PromptSegment> {
        vec![
            PromptSegment::new("Exact", highlight_style("ui.input.prompt.exact")),
            PromptSegment::new(
                format!(" {} ", picker_indicator_glyph()),
                highlight_style("ui.input.prompt.separator"),
            ),
        ]
    }
}

impl PickerSource for CodeActionsPickerSource {
    type Item = CodeActionsPickerItem;

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

        let _ = sender.send(PickerSearchEvent::PickerSearchStarted {
            generation,
            query: query.to_string(),
        });

        let query = query.trim().to_lowercase();
        let chunk = if query.is_empty() {
            self.actions.as_ref().clone()
        } else {
            self.actions
                .iter()
                .filter(|item| item.search_text.contains(query.as_str()))
                .cloned()
                .collect::<Vec<_>>()
        };

        let _ = sender.send(PickerSearchEvent::PickerChunk { generation, chunk });
        let _ = sender.send(PickerSearchEvent::PickerSearchComplete { generation });
    }

    fn select(&self, item: &Self::Item) -> Intent {
        Intent::Command(Command::LspApplyCodeAction {
            buffer_id: self.buffer_id,
            action: item.application.clone(),
        })
    }
}

impl PickerItem for CodeActionsPickerItem {
    fn render_segments(
        &self,
        available_cols: usize,
        base_style: Style,
    ) -> Vec<PickerRenderSegment> {
        let number_style = base_style.accent(theme_style("ui.picker.accent"));
        let tag_style = base_style.accent(theme_style("ui.picker.location"));
        let title_style = base_style;

        let number = format!("{}.", self.number);
        let number_cols = unicode_width::UnicodeWidthStr::width(number.as_str());
        let mut segments = Vec::new();
        let mut remaining_cols = available_cols;

        if remaining_cols > 0 {
            let (visible_number, _) = visible_tail_text(number.as_str(), remaining_cols, true);
            if !visible_number.is_empty() {
                remaining_cols = remaining_cols.saturating_sub(number_cols.min(remaining_cols));
                segments.push(PickerRenderSegment::new(visible_number, number_style));
            }
        }

        if remaining_cols > 0 {
            let separator = " ";
            let separator_cols = unicode_width::UnicodeWidthStr::width(separator);
            if remaining_cols > separator_cols {
                segments.push(PickerRenderSegment::new(separator, title_style));
                remaining_cols = remaining_cols.saturating_sub(separator_cols);
            }
        }

        let kind_suffix = self
            .kind
            .as_ref()
            .map(|kind| format!(" [{}]", kind))
            .unwrap_or_default();
        let kind_cols = unicode_width::UnicodeWidthStr::width(kind_suffix.as_str());
        let title_cols = remaining_cols.saturating_sub(kind_cols);
        let (visible_title, _) = visible_tail_text(self.title.as_str(), title_cols, true);
        if !visible_title.is_empty() {
            segments.push(PickerRenderSegment::new(visible_title, title_style));
        }

        if !kind_suffix.is_empty() && remaining_cols > kind_cols {
            segments.push(PickerRenderSegment::new(kind_suffix, tag_style));
        }

        segments
    }
}

fn theme_style(name: &str) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default(name))
            .unwrap_or_default()
    })
}

fn build_search_text(number: usize, title: &str, kind: Option<&str>) -> String {
    let mut text = format!("{} {}", number, title).to_lowercase();
    if let Some(kind) = kind
        && !kind.is_empty()
    {
        text.push(' ');
        text.push_str(kind.to_lowercase().as_str());
    }
    text
}

fn highlight_style(name: &str) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.highlight_style_for_name(name))
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::JobManager;
    use crate::lsp::runtime::CodeActionApplication;
    use std::sync::Arc;
    use std::sync::mpsc::channel;

    #[test]
    fn picker_source_filters_by_number() {
        let actions = vec![
            CodeActionApplication {
                title: "First action".to_string(),
                kind: Some("quickfix".to_string()),
                edit: None,
                command: None,
                command_arguments_json: None,
            },
            CodeActionApplication {
                title: "Second action".to_string(),
                kind: None,
                edit: None,
                command: None,
                command_arguments_json: None,
            },
        ];
        let source = CodeActionsPickerSource::new(
            crate::buffer::BufferId::new(7),
            actions,
            Arc::new(JobManager::new()),
        );
        let (sender, receiver) = channel();

        source.start_search("1", 1, sender);

        let mut seen = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            seen.push(event);
        }

        assert!(matches!(
            seen.as_slice(),
            [
                PickerSearchEvent::PickerSearchStarted { .. },
                PickerSearchEvent::PickerChunk { chunk, .. },
                PickerSearchEvent::PickerSearchComplete { .. }
            ] if chunk.len() == 1 && chunk[0].title == "First action"
        ));
    }

    #[test]
    fn picker_query_prompt_matches_other_pickers() {
        let segments = CodeActionsPickerSource::query_prompt_segments();

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "Exact");
        assert_eq!(segments[1].text, format!(" {} ", picker_indicator_glyph()));
    }

    #[test]
    fn picker_item_renders_number_and_tag_separately() {
        let item = CodeActionsPickerItem {
            number: 12,
            title: "Rename symbol".to_string(),
            kind: Some("refactor.rename".to_string()),
            search_text: "12 rename symbol refactor.rename".to_string(),
            application: CodeActionApplication {
                title: "Rename symbol".to_string(),
                kind: Some("refactor.rename".to_string()),
                edit: None,
                command: None,
                command_arguments_json: None,
            },
        };

        let segments = item.render_segments(80, Style::default());

        assert!(segments.len() >= 3);
        assert_eq!(segments[0].text, "12.");
        assert_eq!(segments[1].text, " ");
        assert_eq!(segments[2].text, "Rename symbol");
    }
}
