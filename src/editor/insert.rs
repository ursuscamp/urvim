use super::{Action, HandleKeyResult, Mode, ModeKind, TrieKeymap};
use crate::terminal::{CursorStyle, Key, KeyCode};

/// Insert mode for text input.
pub struct InsertMode {
    keymap: TrieKeymap,
    buffer: Vec<String>,
    waiting: bool,
}

impl InsertMode {
    pub fn new() -> Self {
        let mut keymap = TrieKeymap::new();
        keymap.insert("<Esc>".to_string(), Action::SwitchToNormal);
        keymap.insert("<C-q>".to_string(), Action::Quit);
        keymap.insert("<C-s>".to_string(), Action::SaveBuffer(None));
        keymap.insert("<Left>".to_string(), Action::MoveLeft);
        keymap.insert("<Down>".to_string(), Action::MoveDown);
        keymap.insert("<Up>".to_string(), Action::MoveUp);
        keymap.insert("<Right>".to_string(), Action::MoveRight);
        keymap.insert("<Enter>".to_string(), Action::InsertChar('\n'));
        keymap.insert("<Backspace>".to_string(), Action::DeleteBackward);
        keymap.insert("<Delete>".to_string(), Action::DeleteForward);

        InsertMode {
            keymap,
            buffer: Vec::new(),
            waiting: false,
        }
    }
}

impl Default for InsertMode {
    fn default() -> Self {
        InsertMode::new()
    }
}

impl Mode for InsertMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        if key.code == KeyCode::Esc {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(Action::SwitchToNormal);
        }

        let key_str = key.canonical_string();
        let key_str_ref = std::slice::from_ref(&key_str);
        if let Some(action) = self.keymap.get_action(key_str_ref) {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(action);
        }

        self.buffer.push(key_str);
        if self.keymap.is_prefix(&self.buffer) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        if let KeyCode::Char(c) = key.code
            && !key.modifiers.has_ctrl()
        {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(Action::InsertChar(c));
        }

        self.buffer.clear();
        self.waiting = false;
        HandleKeyResult::InvalidSequence
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBar
    }

    fn is_waiting(&self) -> bool {
        self.waiting
    }

    fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.waiting = false;
    }

    fn kind(&self) -> ModeKind {
        ModeKind::Insert
    }
}
