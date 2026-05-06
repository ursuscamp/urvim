use super::Layout;
use crate::background::JobEvent;
use crate::background::JobPayload;
use crate::terminal::KeyCode;
use crate::ui::file_picker::{FilePickerSource, FilePickerWidget};
use crate::ui::picker::PickerSearchEvent;
use crate::ui::{UiEvent, UiEventResult};
use crate::widget::Widget;

impl Layout {
    /// Opens the file picker overlay.
    pub(super) fn open_file_picker(&mut self) {
        self.command_line_open = false;
        self.confirmation_box = None;
        self.close_all_pickers();

        match std::env::current_dir() {
            Ok(cwd) => {
                let picker =
                    FilePickerWidget::new(FilePickerSource::with_jobs(cwd, self.jobs.clone()));
                self.file_picker = Some(picker);
                self.refresh_file_picker_prompt();
            }
            Err(error) => {
                crate::notify_error!("Failed to open file picker: {}", error);
            }
        }
    }

    /// Closes the file picker overlay.
    pub(super) fn close_file_picker(&mut self) {
        if let Some(picker) = self.file_picker.as_mut() {
            picker.close();
        }
        self.file_picker = None;
    }

    /// Returns true when the file picker is open.
    pub(super) fn file_picker_is_open(&self) -> bool {
        self.file_picker
            .as_ref()
            .is_some_and(FilePickerWidget::is_open)
    }

    /// Returns a mutable reference to the file picker when open.
    pub(super) fn file_picker_mut(&mut self) -> Option<&mut FilePickerWidget> {
        self.file_picker.as_mut()
    }

    /// Routes an event to the file picker overlay.
    pub(super) fn handle_file_picker_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(picker) = self.file_picker.as_mut() else {
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

    /// Dispatches a picker-related job event.
    pub fn dispatch_job_event(&mut self, event: JobEvent) {
        match event {
            JobEvent::Started { .. } => {}
            JobEvent::Chunk {
                kind,
                token,
                payload: JobPayload::PreviewSyntax(result),
            } if kind == crate::background::JobKind::PickerPreviewSyntax => {
                if let Some(picker) = self.file_picker.as_mut() {
                    picker.handle_preview_syntax_refresh_chunk(token.generation(), result);
                } else if let Some(picker) = self.grep_picker.as_mut() {
                    picker.handle_preview_syntax_refresh_chunk(token.generation(), result);
                }
            }
            JobEvent::Chunk {
                kind,
                token,
                payload: JobPayload::FileSearchChunk(chunk),
            } if kind == crate::background::JobKind::FilePickerSearch => {
                if let Some(picker) = self.file_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerChunk {
                        generation: token.generation(),
                        chunk,
                    });
                }
            }
            JobEvent::Completed {
                kind,
                token,
                payload: Some(JobPayload::PreviewSyntax(result)),
            } if kind == crate::background::JobKind::PickerPreviewSyntax => {
                if let Some(picker) = self.file_picker.as_mut() {
                    picker.handle_preview_syntax_refresh(token.generation(), result);
                } else if let Some(picker) = self.grep_picker.as_mut() {
                    picker.handle_preview_syntax_refresh(token.generation(), result);
                }
            }
            JobEvent::Failed { kind, token, .. }
                if kind == crate::background::JobKind::PickerPreviewSyntax =>
            {
                if let Some(picker) = self.file_picker.as_mut() {
                    picker.handle_preview_syntax_refresh_failed(token.generation());
                } else if let Some(picker) = self.grep_picker.as_mut() {
                    picker.handle_preview_syntax_refresh_failed(token.generation());
                }
            }
            JobEvent::Completed { kind, token, .. }
                if kind == crate::background::JobKind::FilePickerSearch =>
            {
                if let Some(picker) = self.file_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            }
            JobEvent::Failed { kind, token, .. }
                if kind == crate::background::JobKind::FilePickerSearch =>
            {
                if let Some(picker) = self.file_picker.as_mut() {
                    picker.handle_search_event(PickerSearchEvent::PickerSearchComplete {
                        generation: token.generation(),
                    });
                }
            }
            other => self.dispatch_grep_picker_job_event(other),
        }
    }

    fn refresh_file_picker_prompt(&mut self) {
        if let Some(picker) = self.file_picker.as_mut() {
            let mode = picker.source_mut().query_mode();
            picker.set_query_prompt_segments(FilePickerSource::query_prompt_segments(mode));
        }
    }
}
