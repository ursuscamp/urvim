use super::Layout;
use crate::ui::code_actions_picker::{CodeActionsPickerSource, CodeActionsPickerWidget};
use crate::ui::{UiEvent, UiEventResult};
use crate::widget::Widget;

impl Layout {
    /// Opens the LSP code actions picker overlay.
    pub(super) fn open_lsp_code_actions_picker(&mut self) {
        self.close_all_dialogs();

        let buffer_id = self.active_buffer_view().buffer_id();
        let cursor = self.active_buffer_view().cursor();
        let Some(result) = crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.code_actions_buffer(buffer_id, cursor)
        }) else {
            crate::notify_error!("Failed to open code actions picker: LSP runtime is busy");
            return;
        };

        let result = match result {
            Ok(result) => result,
            Err(error) => {
                crate::notify_error!("Failed to open code actions picker: {}", error);
                return;
            }
        };

        let Some(actions) = result else {
            crate::notify_error!("No code actions available at the current cursor position");
            return;
        };

        let mut picker = CodeActionsPickerWidget::new(CodeActionsPickerSource::new(
            buffer_id,
            actions,
            self.jobs.clone(),
        ));
        picker.set_query_prompt_segments(CodeActionsPickerSource::query_prompt_segments());
        picker.restart_search();
        self.code_actions_picker = Some(picker);
    }

    /// Closes the LSP code actions picker overlay.
    pub(super) fn close_code_actions_picker(&mut self) {
        if let Some(picker) = self.code_actions_picker.as_mut() {
            picker.close();
        }
        self.code_actions_picker = None;
    }

    /// Returns true when the LSP code actions picker is open.
    pub(super) fn code_actions_picker_is_open(&self) -> bool {
        self.code_actions_picker
            .as_ref()
            .is_some_and(CodeActionsPickerWidget::is_open)
    }

    /// Returns a mutable reference to the LSP code actions picker when open.
    pub(super) fn code_actions_picker_mut(&mut self) -> Option<&mut CodeActionsPickerWidget> {
        self.code_actions_picker.as_mut()
    }

    /// Routes an event to the LSP code actions picker overlay.
    pub(super) fn handle_code_actions_picker_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(picker) = self.code_actions_picker.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = picker.handle_ui_event(event, &mut ctx);
        if result.handled() && !picker.is_open() {
            self.close_code_actions_picker();
        }

        result
    }
}
