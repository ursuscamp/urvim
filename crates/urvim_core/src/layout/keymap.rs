//! Keymap inheritance for modal layout dialogs.

use super::Layout;
use crate::editor::HandleKeyResult;
use crate::ui::{KeymapInheritance, UiEventResult};
use urvim_terminal::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ModalKeySequence {
    None,
    Inherited,
}

impl Layout {
    pub(super) fn modal_dialog_is_open(&self) -> bool {
        self.overlay_modal_is_open() || self.picker_is_open()
    }

    pub(super) fn overlay_modal_is_open(&self) -> bool {
        self.confirmation_box_is_open() || self.lsp_rename_prompt_is_open()
    }

    pub(super) fn picker_is_open(&self) -> bool {
        self.buffer_picker_is_open()
            || self.colorscheme_picker_is_open()
            || self.code_actions_picker_is_open()
            || self.workspace_symbols_picker_is_open()
            || self.references_picker_is_open()
            || self.doc_symbols_picker_is_open()
            || self.grep_picker_is_open()
            || self.git_picker_is_open()
            || self.file_picker_is_open()
            || self.filetype_picker_is_open()
            || self.plugin_picker_is_open()
    }

    pub(super) fn clear_modal_inherited_keys(&mut self) {
        self.modal_inherited_keymap.clear_pending();
        self.modal_key_sequence = ModalKeySequence::None;
    }

    pub(super) fn route_modal_inherited_key(&mut self, key: &Key) -> UiEventResult {
        let result = self.modal_inherited_keymap.handle_key(key, |inheritance| {
            inheritance == KeymapInheritance::Application
        });
        match result {
            HandleKeyResult::Complete(intent) => {
                self.modal_key_sequence = ModalKeySequence::None;
                UiEventResult::Handled(vec![intent])
            }
            HandleKeyResult::WaitForMore => {
                self.modal_key_sequence = ModalKeySequence::Inherited;
                UiEventResult::Handled(Vec::new())
            }
            HandleKeyResult::InvalidSequence => {
                self.modal_key_sequence = ModalKeySequence::None;
                UiEventResult::Handled(Vec::new())
            }
        }
    }
}
