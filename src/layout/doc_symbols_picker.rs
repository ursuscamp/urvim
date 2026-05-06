use super::Layout;
use crate::terminal::KeyCode;
use crate::ui::doc_symbols_picker::{DocSymbolsPickerSource, DocSymbolsPickerWidget};
use crate::ui::{UiEvent, UiEventResult};
use crate::widget::Widget;

impl Layout {
    /// Opens the LSP document symbol picker overlay.
    pub(super) fn open_doc_symbols_picker(&mut self) {
        self.close_all_dialogs();

        let buffer_id = self.active_buffer_view().buffer_id();
        if !crate::globals::with_buffer(buffer_id, |buffer| buffer.path().is_some())
            .unwrap_or(false)
        {
            crate::notify_error!("Failed to open document symbol picker: buffer has no path");
            return;
        }

        let mut picker = DocSymbolsPickerWidget::new(DocSymbolsPickerSource::with_jobs(
            buffer_id,
            self.jobs.clone(),
        ));
        picker.set_query_prompt_segments(DocSymbolsPickerSource::query_prompt_segments(
            crate::ui::doc_symbols_picker::QueryMode::Exact,
        ));
        picker.restart_search();
        self.doc_symbols_picker = Some(picker);
    }

    /// Closes the LSP document symbol picker overlay.
    pub(super) fn close_doc_symbols_picker(&mut self) {
        if let Some(picker) = self.doc_symbols_picker.as_mut() {
            picker.close();
        }
        self.doc_symbols_picker = None;
    }

    /// Returns true when the LSP document symbol picker is open.
    pub(super) fn doc_symbols_picker_is_open(&self) -> bool {
        self.doc_symbols_picker
            .as_ref()
            .is_some_and(DocSymbolsPickerWidget::is_open)
    }

    /// Returns a mutable reference to the LSP document symbol picker when open.
    pub(super) fn doc_symbols_picker_mut(&mut self) -> Option<&mut DocSymbolsPickerWidget> {
        self.doc_symbols_picker.as_mut()
    }

    /// Routes an event to the LSP document symbol picker overlay.
    pub(super) fn handle_doc_symbols_picker_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(picker) = self.doc_symbols_picker.as_mut() else {
            return UiEventResult::NotHandled;
        };

        if let UiEvent::Key(key) = event {
            if key.code == KeyCode::Tab {
                let mode = picker.source_mut().toggle_query_mode();
                picker
                    .set_query_prompt_segments(DocSymbolsPickerSource::query_prompt_segments(mode));
                picker.restart_search();
                return UiEventResult::Handled(Vec::new());
            }
        }

        let mut ctx = crate::ui::UiContext;
        let result = picker.handle_ui_event(event, &mut ctx);
        if result.handled() && !picker.is_open() {
            self.close_doc_symbols_picker();
        }

        result
    }
}
