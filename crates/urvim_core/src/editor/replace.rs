use super::{
    EditorAction, EditorOperation, HandleKeyResult, KeyGuideSnapshot, Mode, ModeKind, TrieKeymap,
};
use crate::ui::Intent;
use urvim_terminal::{CursorStyle, Key, KeyCode};

/// Replace mode for overwriting text character by character.
pub struct ReplaceMode {
    keymap: TrieKeymap,
    buffer: Vec<String>,
    waiting: bool,
}

impl ReplaceMode {
    /// Creates a new replace mode.
    pub fn new() -> Self {
        let mut keymap = TrieKeymap::<Intent>::new();
        keymap.insert_str("<F1>", crate::ui::Command::OpenFilePicker);
        keymap.insert_str("<F2>", crate::ui::Command::OpenGrepPicker);
        keymap.insert_str("<F3>", crate::ui::Command::OpenBufferPicker);
        keymap.insert_str("<F4>", crate::ui::Command::OpenGitPicker);
        keymap.insert_str("<F5>", crate::ui::Command::OpenColorschemePicker);
        keymap.insert_str("<F6>", crate::ui::Command::OpenFiletypePicker);
        keymap.insert_str("<Esc>", EditorAction::mode_transition(ModeKind::Normal));
        keymap.insert_str("<C-q>", crate::ui::Command::TryQuit);
        keymap.insert_str("<C-s>", crate::ui::Command::SaveBuffer(None));
        keymap.insert_str("<Left>", EditorAction::new(EditorOperation::MoveLeft));
        keymap.insert_str("<Down>", EditorAction::new(EditorOperation::MoveDown));
        keymap.insert_str("<Up>", EditorAction::new(EditorOperation::MoveUp));
        keymap.insert_str("<Right>", EditorAction::new(EditorOperation::MoveRight));
        keymap.insert_str("<PageUp>", EditorAction::new(EditorOperation::MovePageUp));
        keymap.insert_str(
            "<PageDown>",
            EditorAction::new(EditorOperation::MovePageDown),
        );
        keymap.insert_str("<C-u>", EditorAction::new(EditorOperation::MoveHalfPageUp));
        keymap.insert_str(
            "<C-d>",
            EditorAction::new(EditorOperation::MoveHalfPageDown),
        );
        keymap.insert_str("<Enter>", EditorAction::insert_newline());
        keymap.insert_str(
            "<Delete>",
            EditorAction::new(EditorOperation::DeleteForward),
        );

        ReplaceMode {
            keymap,
            buffer: Vec::new(),
            waiting: false,
        }
    }

    fn replace_action_for_char(&mut self, ch: char) -> EditorAction {
        EditorAction::new(EditorOperation::ReplaceChar(ch)).with_from_mode(ModeKind::Replace)
    }

    fn replace_backspace_action(&mut self) -> HandleKeyResult {
        HandleKeyResult::complete(
            EditorAction::new(EditorOperation::ReplaceBackspaceLast)
                .with_from_mode(ModeKind::Replace),
        )
    }
}

impl Default for ReplaceMode {
    fn default() -> Self {
        ReplaceMode::new()
    }
}

impl Mode for ReplaceMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        if key.code == KeyCode::Backspace {
            self.buffer.clear();
            self.waiting = false;
            return self.replace_backspace_action();
        }

        self.buffer.push(key.canonical_string());
        if let Some(intent) = self.keymap.get_action(&self.buffer) {
            self.buffer.clear();
            self.waiting = false;
            if let Some(action) = intent.as_editor_action().cloned() {
                return HandleKeyResult::complete(action.with_from_mode(ModeKind::Replace));
            }
            return HandleKeyResult::complete(intent);
        }

        if self.keymap.is_prefix(&self.buffer) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        if let KeyCode::Char(c) = key.code
            && !key.modifiers.has_ctrl()
        {
            self.buffer.clear();
            self.waiting = false;
            let action = self.replace_action_for_char(c);
            return HandleKeyResult::complete(action);
        }

        self.buffer.clear();
        self.waiting = false;
        HandleKeyResult::InvalidSequence
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyUnderline
    }

    fn is_waiting(&self) -> bool {
        self.waiting
    }

    fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.waiting = false;
    }

    fn key_guide(&self) -> Option<KeyGuideSnapshot> {
        self.waiting.then(|| KeyGuideSnapshot {
            prefix: self.buffer.clone(),
            entries: self.keymap.continuations(&self.buffer),
        })
    }

    fn kind(&self) -> ModeKind {
        ModeKind::Replace
    }
}
