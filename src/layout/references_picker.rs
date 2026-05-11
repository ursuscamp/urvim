use super::Layout;
use crate::terminal::KeyCode;
use crate::ui::references_picker::{ReferencesPickerSource, ReferencesPickerWidget};
use crate::ui::{UiEvent, UiEventResult};
use crate::widget::Widget;

impl Layout {
    /// Opens the LSP references picker overlay.
    pub(super) fn open_lsp_references_picker(&mut self) {
        self.close_all_dialogs();

        let buffer_id = self.active_buffer_view().buffer_id();
        let cursor = self.active_buffer_view().cursor();
        let Some(result) = crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.references_buffer(buffer_id, cursor)
        }) else {
            crate::notify_error!("Failed to open references picker: LSP runtime is busy");
            return;
        };

        let result = match result {
            Ok(result) => result,
            Err(error) => {
                crate::notify_error!("Failed to open references picker: {}", error);
                return;
            }
        };

        let Some(references) = result else {
            crate::notify_error!("No references found at the current cursor position");
            return;
        };

        let mut picker =
            ReferencesPickerWidget::new(ReferencesPickerSource::new(references, self.jobs.clone()));
        picker.set_label("References");
        picker.set_query_prompt_segments(ReferencesPickerSource::query_prompt_segments(
            crate::ui::references_picker::QueryMode::Exact,
        ));
        picker.restart_search();
        self.references_picker = Some(picker);
    }

    /// Closes the LSP references picker overlay.
    pub(super) fn close_references_picker(&mut self) {
        if let Some(picker) = self.references_picker.as_mut() {
            picker.close();
        }
        self.references_picker = None;
    }

    /// Returns true when the LSP references picker is open.
    pub(super) fn references_picker_is_open(&self) -> bool {
        self.references_picker
            .as_ref()
            .is_some_and(ReferencesPickerWidget::is_open)
    }

    /// Returns a mutable reference to the LSP references picker when open.
    pub(super) fn references_picker_mut(&mut self) -> Option<&mut ReferencesPickerWidget> {
        self.references_picker.as_mut()
    }

    /// Routes an event to the LSP references picker overlay.
    pub(super) fn handle_references_picker_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(picker) = self.references_picker.as_mut() else {
            return UiEventResult::NotHandled;
        };

        if let UiEvent::Key(key) = event {
            if key.code == KeyCode::Tab {
                let mode = picker.source_mut().toggle_query_mode();
                picker
                    .set_query_prompt_segments(ReferencesPickerSource::query_prompt_segments(mode));
                picker.restart_search();
                return UiEventResult::Handled(Vec::new());
            }
        }

        let mut ctx = crate::ui::UiContext;
        let result = picker.handle_ui_event(event, &mut ctx);
        if result.handled() && !picker.is_open() {
            self.close_references_picker();
        }

        result
    }
}
