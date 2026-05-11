use super::Layout;
use crate::terminal::KeyCode;
use crate::ui::picker::file::{FilePickerSource, FilePickerWidget};
use crate::ui::{UiEvent, UiEventResult};
use crate::widget::Widget;

impl Layout {
    /// Opens the file picker overlay.
    pub(in crate::layout) fn open_file_picker(&mut self) {
        self.close_all_dialogs();

        match std::env::current_dir() {
            Ok(cwd) => {
                let mut picker =
                    FilePickerWidget::new(FilePickerSource::with_jobs(cwd, self.jobs.clone()));
                picker.set_label("Files");
                self.dialogs.file_picker = Some(picker);
                self.refresh_file_picker_prompt();
            }
            Err(error) => {
                crate::notify_error!("Failed to open file picker: {}", error);
            }
        }
    }

    /// Closes the file picker overlay.
    pub(in crate::layout) fn close_file_picker(&mut self) {
        if let Some(picker) = self.dialogs.file_picker.as_mut() {
            picker.close();
        }
        self.dialogs.file_picker = None;
    }

    /// Returns true when the file picker is open.
    pub(in crate::layout) fn file_picker_is_open(&self) -> bool {
        self.dialogs
            .file_picker
            .as_ref()
            .is_some_and(FilePickerWidget::is_open)
    }

    /// Returns a mutable reference to the file picker when open.
    pub(in crate::layout) fn file_picker_mut(&mut self) -> Option<&mut FilePickerWidget> {
        self.dialogs.file_picker.as_mut()
    }

    /// Routes an event to the file picker overlay.
    pub(in crate::layout) fn handle_file_picker_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(picker) = self.dialogs.file_picker.as_mut() else {
            return UiEventResult::NotHandled;
        };

        if let UiEvent::Key(key) = event {
            if key.code == KeyCode::Tab {
                let mode = picker.source_mut().toggle_query_mode();
                picker.set_query_prompt_segments(FilePickerSource::query_prompt_segments(mode));
                picker.restart_search();
                return UiEventResult::Handled(Vec::new());
            }
        }

        let mut ctx = crate::ui::UiContext;
        let result = picker.handle_ui_event(event, &mut ctx);
        if result.handled() && !picker.is_open() {
            self.close_file_picker();
        }

        result
    }

    fn refresh_file_picker_prompt(&mut self) {
        if let Some(picker) = self.dialogs.file_picker.as_mut() {
            let mode = picker.source_mut().query_mode();
            picker.set_query_prompt_segments(FilePickerSource::query_prompt_segments(mode));
        }
    }
}
