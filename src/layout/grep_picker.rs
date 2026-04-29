use super::Layout;
use crate::job::{JobEvent, JobPayload};
use crate::ui::grep_picker::{GREP_PICKER_SEARCH_JOB_KIND, GrepPickerSource, GrepPickerWidget};
use crate::ui::picker::PickerSearchEvent;
use crate::ui::{UiEvent, UiEventResult};
use crate::widget::Widget;

impl Layout {
    /// Opens the live grep picker overlay.
    pub(super) fn open_grep_picker(&mut self) {
        self.command_line_open = false;
        self.confirmation_box = None;
        self.close_file_picker();
        self.close_grep_picker();

        match std::env::current_dir() {
            Ok(cwd) => {
                let picker = GrepPickerWidget::new(GrepPickerSource::new(cwd));
                self.grep_picker = Some(picker);
            }
            Err(error) => {
                crate::notify_error!("Failed to open live grep picker: {}", error);
            }
        }
    }

    /// Closes the live grep picker overlay.
    pub(super) fn close_grep_picker(&mut self) {
        if let Some(picker) = self.grep_picker.as_mut() {
            picker.close();
        }
        self.grep_picker = None;
    }

    /// Returns true when the live grep picker is open.
    pub(super) fn grep_picker_is_open(&self) -> bool {
        self.grep_picker
            .as_ref()
            .is_some_and(GrepPickerWidget::is_open)
    }

    /// Returns a mutable reference to the live grep picker when open.
    pub(super) fn grep_picker_mut(&mut self) -> Option<&mut GrepPickerWidget> {
        self.grep_picker.as_mut()
    }

    /// Routes an event to the live grep picker overlay.
    pub(super) fn handle_grep_picker_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(picker) = self.grep_picker.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = picker.handle_ui_event(event, &mut ctx);
        if result.handled() && !picker.is_open() {
            self.close_grep_picker();
        }

        result
    }

    /// Dispatches a live grep picker job event.
    pub fn dispatch_grep_picker_job_event(&mut self, event: JobEvent) {
        match event {
            JobEvent::Started { .. } => {}
            JobEvent::Chunk {
                kind,
                token,
                payload: JobPayload::GrepPickerChunk(chunk),
            } if kind.as_str() == GREP_PICKER_SEARCH_JOB_KIND => {
                if let Some(picker) = self.grep_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerChunk {
                        generation: token.generation(),
                        chunk,
                    });
                }
            }
            JobEvent::Completed { kind, token, .. }
                if kind.as_str() == GREP_PICKER_SEARCH_JOB_KIND =>
            {
                if let Some(picker) = self.grep_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            }
            JobEvent::Failed { kind, token, .. }
                if kind.as_str() == GREP_PICKER_SEARCH_JOB_KIND =>
            {
                if let Some(picker) = self.grep_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            }
            _ => {}
        }
    }
}
