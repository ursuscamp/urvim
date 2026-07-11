use super::Layout;
use crate::ui::UiEventResult;
use crate::ui::picker::filetype::{FiletypePickerSource, FiletypePickerWidget};
use crate::widget::Widget;

impl Layout {
    /// Opens the filetype picker overlay.
    pub fn open_filetype_picker(&mut self) {
        self.close_all_dialogs();

        let mut picker = FiletypePickerWidget::new(FiletypePickerSource::with_jobs(
            FiletypePickerSource::builtin_items(),
            self.jobs.clone(),
        ));
        let mode = picker.source_mut().query_mode();
        picker.set_query_prompt_segments(FiletypePickerSource::query_prompt_segments(mode));
        picker.set_label("Filetypes");
        picker.restart_search();
        self.dialogs.filetype_picker = Some(picker);
    }

    /// Closes the filetype picker overlay.
    pub fn close_filetype_picker(&mut self) {
        if let Some(picker) = self.dialogs.filetype_picker.as_mut() {
            picker.close();
        }
        self.dialogs.filetype_picker = None;
        self.clear_modal_inherited_keys();
    }

    /// Returns true when the filetype picker is open.
    pub fn filetype_picker_is_open(&self) -> bool {
        self.dialogs
            .filetype_picker
            .as_ref()
            .is_some_and(FiletypePickerWidget::is_open)
    }

    /// Returns a mutable reference to the filetype picker when open.
    pub fn filetype_picker_mut(&mut self) -> Option<&mut FiletypePickerWidget> {
        self.dialogs.filetype_picker.as_mut()
    }

    /// Routes an event to the filetype picker overlay.
    pub fn handle_filetype_picker_event(&mut self, event: &crate::ui::UiEvent) -> UiEventResult {
        let result = {
            let Some(picker) = self.dialogs.filetype_picker.as_mut() else {
                return UiEventResult::NotHandled;
            };
            let mut ctx = crate::ui::UiContext;
            picker.handle_ui_event(event, &mut ctx)
        };

        if result.handled() && !self.filetype_picker_is_open() {
            self.close_filetype_picker();
        }

        result
    }
}
