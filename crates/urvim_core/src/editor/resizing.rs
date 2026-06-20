use super::{Action, HandleKeyResult, Mode, ModeKind, TrieKeymap};
use crate::globals;
use crate::ui::Command;
use urvim_terminal::{CursorStyle, Key};

/// Pane resizing mode for split-layout adjustments.
pub struct ResizingMode {
    keymap: TrieKeymap,
    buffer: Vec<String>,
    waiting: bool,
}

impl Default for ResizingMode {
    fn default() -> Self {
        Self::new()
    }
}

impl ResizingMode {
    /// Creates a new resizing mode.
    pub fn new() -> Self {
        let mut keymap = TrieKeymap::new();
        keymap.insert_str("h", Command::ResizePaneLeft(1));
        keymap.insert_str("H", Command::ResizePaneLeft(5));
        keymap.insert_str("l", Command::ResizePaneRight(1));
        keymap.insert_str("L", Command::ResizePaneRight(5));
        keymap.insert_str("j", Command::ResizePaneDown(1));
        keymap.insert_str("J", Command::ResizePaneDown(5));
        keymap.insert_str("k", Command::ResizePaneUp(1));
        keymap.insert_str("K", Command::ResizePaneUp(5));
        keymap.insert_str("=", Command::EqualizeSplits);
        keymap.insert_str("<Esc>", Action::mode_transition(ModeKind::Normal));
        globals::with_opt_config(|config| {
            if let Some(config) = config {
                keymap.insert_configured(&config.keymaps.resizing);
            }
        });

        Self {
            keymap,
            buffer: Vec::new(),
            waiting: false,
        }
    }

    fn reset(&mut self) {
        self.buffer.clear();
        self.waiting = false;
    }
}

impl Mode for ResizingMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        self.buffer.push(key.canonical_string());
        if let Some(intent) = self.keymap.get_action(&self.buffer) {
            self.reset();
            if let Some(action) = intent.as_action().cloned() {
                return HandleKeyResult::complete(action.with_from_mode(ModeKind::Resizing));
            }
            return HandleKeyResult::complete(intent);
        }

        if self.keymap.is_prefix(&self.buffer) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyUnderline
    }

    fn is_waiting(&self) -> bool {
        self.waiting
    }

    fn clear_buffer(&mut self) {
        self.reset();
    }

    fn kind(&self) -> ModeKind {
        ModeKind::Resizing
    }
}
