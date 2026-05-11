use super::Layout;
use crate::ui::lsp_rename::LspRenamePrompt;
use crate::ui::{UiEvent, UiEventResult};

impl Layout {
    /// Opens the dedicated LSP rename prompt.
    pub(super) fn open_lsp_rename_prompt(&mut self) {
        self.close_all_dialogs();
        let buffer_id = self.active_buffer_view().buffer_id();
        let cursor = self.active_buffer_view().cursor();
        let placeholder = crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.rename_placeholder(buffer_id, cursor)
        })
        .flatten();

        self.dialogs.lsp_rename_prompt =
            Some(LspRenamePrompt::new(placeholder.unwrap_or_default()));
    }

    /// Closes the dedicated LSP rename prompt.
    pub(super) fn close_lsp_rename_prompt(&mut self) {
        self.dialogs.lsp_rename_prompt = None;
    }

    /// Returns true when the LSP rename prompt is open.
    pub(super) fn lsp_rename_prompt_is_open(&self) -> bool {
        self.dialogs
            .lsp_rename_prompt
            .as_ref()
            .is_some_and(LspRenamePrompt::is_open)
    }

    /// Returns a mutable reference to the LSP rename prompt when it is open.
    pub(super) fn lsp_rename_prompt_mut(&mut self) -> Option<&mut LspRenamePrompt> {
        self.dialogs.lsp_rename_prompt.as_mut()
    }

    /// Routes a UI event to the dedicated LSP rename prompt.
    pub(super) fn handle_lsp_rename_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(prompt) = self.dialogs.lsp_rename_prompt.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = prompt.handle_ui_event(event, &mut ctx);
        if result.handled() && !prompt.is_open() {
            self.close_lsp_rename_prompt();
        }

        result
    }
}
