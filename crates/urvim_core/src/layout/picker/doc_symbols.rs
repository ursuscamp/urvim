use super::Layout;
use crate::ui::picker::doc_symbols::{
    DocSymbolsPickerScope, DocSymbolsPickerSource, DocSymbolsPickerWidget,
};
use crate::ui::{UiEvent, UiEventResult};
use crate::widget::Widget;

impl Layout {
    /// Opens the LSP document symbol picker overlay.
    pub(in crate::layout) fn open_doc_symbols_picker(&mut self) {
        let buffer_id = self.active_buffer_view().buffer_id();
        if !crate::globals::with_buffer(buffer_id, |buffer| buffer.path().is_some())
            .unwrap_or(false)
        {
            crate::notify_error!("Failed to open document symbol picker: buffer has no path");
            return;
        }

        self.open_symbols_picker(DocSymbolsPickerScope::Document(buffer_id));
    }

    /// Opens the LSP workspace symbol picker overlay.
    pub(in crate::layout) fn open_workspace_symbols_picker(&mut self) {
        self.open_symbols_picker(DocSymbolsPickerScope::Workspace);
    }

    fn open_symbols_picker(&mut self, scope: DocSymbolsPickerScope) {
        self.close_all_dialogs();
        let mut picker = DocSymbolsPickerWidget::new(match scope {
            DocSymbolsPickerScope::Document(buffer_id) => {
                DocSymbolsPickerSource::with_document_jobs(buffer_id, self.jobs.clone())
            }
            DocSymbolsPickerScope::Workspace => {
                DocSymbolsPickerSource::with_workspace_jobs(self.jobs.clone())
            }
        });
        picker.set_query_prompt_segments(DocSymbolsPickerSource::query_prompt_segments(
            crate::ui::picker::doc_symbols::QueryMode::Fuzzy,
        ));
        picker.set_label(match scope {
            DocSymbolsPickerScope::Document(_) => "Document Symbols",
            DocSymbolsPickerScope::Workspace => "Workspace Symbols",
        });
        picker.restart_search();

        match scope {
            DocSymbolsPickerScope::Document(_) => self.dialogs.doc_symbols_picker = Some(picker),
            DocSymbolsPickerScope::Workspace => {
                self.dialogs.workspace_symbols_picker = Some(picker)
            }
        }
    }

    /// Closes the LSP document symbol picker overlay.
    pub(in crate::layout) fn close_doc_symbols_picker(&mut self) {
        if let Some(picker) = self.dialogs.doc_symbols_picker.as_mut() {
            picker.close();
        }
        self.dialogs.doc_symbols_picker = None;
        self.clear_modal_inherited_keys();
    }

    /// Closes the LSP workspace symbol picker overlay.
    pub(in crate::layout) fn close_workspace_symbols_picker(&mut self) {
        if let Some(picker) = self.dialogs.workspace_symbols_picker.as_mut() {
            picker.close();
        }
        self.dialogs.workspace_symbols_picker = None;
        self.clear_modal_inherited_keys();
    }

    /// Returns true when the LSP document symbol picker is open.
    pub(in crate::layout) fn doc_symbols_picker_is_open(&self) -> bool {
        self.dialogs
            .doc_symbols_picker
            .as_ref()
            .is_some_and(DocSymbolsPickerWidget::is_open)
    }

    /// Returns a mutable reference to the LSP document symbol picker when open.
    pub(in crate::layout) fn doc_symbols_picker_mut(
        &mut self,
    ) -> Option<&mut DocSymbolsPickerWidget> {
        self.dialogs.doc_symbols_picker.as_mut()
    }

    /// Returns true when the LSP workspace symbol picker is open.
    pub(in crate::layout) fn workspace_symbols_picker_is_open(&self) -> bool {
        self.dialogs
            .workspace_symbols_picker
            .as_ref()
            .is_some_and(DocSymbolsPickerWidget::is_open)
    }

    /// Returns a mutable reference to the LSP workspace symbol picker when open.
    pub(in crate::layout) fn workspace_symbols_picker_mut(
        &mut self,
    ) -> Option<&mut DocSymbolsPickerWidget> {
        self.dialogs.workspace_symbols_picker.as_mut()
    }

    /// Routes an event to the LSP document symbol picker overlay.
    pub(in crate::layout) fn handle_doc_symbols_picker_event(
        &mut self,
        event: &UiEvent,
    ) -> UiEventResult {
        let Some(picker) = self.dialogs.doc_symbols_picker.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = picker.handle_ui_event(event, &mut ctx);
        if result.handled() && !picker.is_open() {
            self.close_doc_symbols_picker();
        }

        result
    }

    /// Routes an event to the LSP workspace symbol picker overlay.
    pub(in crate::layout) fn handle_workspace_symbols_picker_event(
        &mut self,
        event: &UiEvent,
    ) -> UiEventResult {
        let Some(picker) = self.dialogs.workspace_symbols_picker.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = picker.handle_ui_event(event, &mut ctx);
        if result.handled() && !picker.is_open() {
            self.close_workspace_symbols_picker();
        }

        result
    }
}
