use super::{Action, HandleKeyResult, Mode, ModeKind, TrieKeymap};
use crate::buffer::{Buffer, Cursor};
use crate::terminal::{CursorStyle, Key, KeyCode};

/// Insert mode for text input.
pub struct InsertMode {
    keymap: TrieKeymap,
    buffer: Vec<String>,
    waiting: bool,
    repeat_capture: Buffer,
    repeat_cursor: Cursor,
}

impl InsertMode {
    /// Creates a new insert mode with an empty repeat capture buffer.
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
            repeat_capture: Buffer::new(),
            repeat_cursor: Cursor::new(0, 0),
        }
    }

    fn record_action(&mut self, action: &Action) {
        match action {
            Action::InsertChar(ch) => self.record_insert_char(*ch),
            Action::DeleteBackward => self.record_delete_backward(),
            Action::DeleteForward => self.record_delete_forward(),
            Action::MoveLeft => self.record_move_left(),
            Action::MoveRight => self.record_move_right(),
            Action::MoveUp => self.record_move_up(),
            Action::MoveDown => self.record_move_down(),
            _ => {}
        }
    }

    fn record_insert_char(&mut self, ch: char) {
        let cursor = self.repeat_cursor;
        self.repeat_capture.insert_char(cursor, ch);
        self.repeat_cursor = match ch {
            '\n' => Cursor::new(cursor.line + 1, 0),
            _ => Cursor::new(cursor.line, cursor.col + ch.len_utf8()),
        };
    }

    fn record_delete_backward(&mut self) {
        if let Some(new_cursor) = self
            .repeat_capture
            .delete_char_before_cursor(self.repeat_cursor)
        {
            self.repeat_cursor = new_cursor;
        }
    }

    fn record_delete_forward(&mut self) {
        if let Some(new_cursor) = self
            .repeat_capture
            .delete_char_at_cursor(self.repeat_cursor)
        {
            self.repeat_cursor = new_cursor;
        }
    }

    fn record_move_left(&mut self) {
        if let Some(new_cursor) = self.repeat_capture.prev_cursor(self.repeat_cursor) {
            self.repeat_cursor = new_cursor;
        }
    }

    fn record_move_right(&mut self) {
        if let Some(new_cursor) = self.repeat_capture.next_cursor(self.repeat_cursor) {
            self.repeat_cursor = new_cursor;
        }
    }

    fn record_move_up(&mut self) {
        let visual_col = self.repeat_capture.visual_col_at(self.repeat_cursor);
        if let Some(new_cursor) = self
            .repeat_capture
            .cursor_up(self.repeat_cursor, visual_col)
        {
            self.repeat_cursor = new_cursor;
        }
    }

    fn record_move_down(&mut self) {
        let visual_col = self.repeat_capture.visual_col_at(self.repeat_cursor);
        if let Some(new_cursor) = self
            .repeat_capture
            .cursor_down(self.repeat_cursor, visual_col)
        {
            self.repeat_cursor = new_cursor;
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
            self.record_action(&action);
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
            let action = Action::InsertChar(c);
            self.record_action(&action);
            return HandleKeyResult::Complete(action);
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

    fn take_repeat_text(&mut self) -> Option<String> {
        let text = self.repeat_capture.as_str();
        if text.is_empty() {
            self.repeat_capture = Buffer::new();
            self.repeat_cursor = Cursor::new(0, 0);
            return None;
        }

        self.repeat_capture = Buffer::new();
        self.repeat_cursor = Cursor::new(0, 0);
        Some(text)
    }

    fn kind(&self) -> ModeKind {
        ModeKind::Insert
    }
}
