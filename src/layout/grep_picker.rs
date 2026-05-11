use super::Layout;
use crate::background::JobEvent;
use crate::background::JobPayload;
use crate::terminal::KeyCode;
use crate::ui::grep_picker::{GrepPickerSource, GrepPickerWidget};
use crate::ui::picker::PickerSearchEvent;
use crate::ui::{UiEvent, UiEventResult};
use crate::widget::Widget;

impl Layout {
    /// Opens the live grep picker overlay.
    pub(super) fn open_grep_picker(&mut self) {
        self.close_all_dialogs();

        match std::env::current_dir() {
            Ok(cwd) => {
                let mut picker =
                    GrepPickerWidget::new(GrepPickerSource::with_jobs(cwd, self.jobs.clone()));
                picker.set_label("Grep");
                self.grep_picker = Some(picker);
                self.refresh_grep_picker_prompt();
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

        if let UiEvent::Key(key) = event {
            if key.code == KeyCode::Tab {
                let mode = picker.source_mut().toggle_query_mode();
                picker.set_query_prompt_segments(GrepPickerSource::query_prompt_segments(mode));
                picker.restart_search();
                return UiEventResult::Handled(Vec::new());
            }
        }

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
                payload: JobPayload::GrepSearchChunk(chunk),
            } if kind == crate::background::JobKind::GrepPickerSearch => {
                if let Some(picker) = self.grep_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerChunk {
                        generation: token.generation(),
                        chunk,
                    });
                }
            }
            JobEvent::Completed { kind, token, .. }
                if kind == crate::background::JobKind::GrepPickerSearch =>
            {
                if let Some(picker) = self.grep_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            }
            JobEvent::Failed { kind, token, .. }
                if kind == crate::background::JobKind::GrepPickerSearch =>
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

    fn refresh_grep_picker_prompt(&mut self) {
        if let Some(picker) = self.grep_picker.as_mut() {
            let mode = picker.source_mut().query_mode();
            picker.set_query_prompt_segments(GrepPickerSource::query_prompt_segments(mode));
        }
    }
}
