use super::{Action, ActionKind, HandleKeyResult, Mode, ModeKind, TrieKeymap};
use crate::buffer::{Buffer, Cursor};
use crate::editor::validate_key_string;
use crate::editor::pairs;
use crate::globals;
use crate::terminal::{CursorStyle, Key, KeyCode};

/// Insert mode for text input.
pub struct InsertMode {
    keymap: TrieKeymap,
    buffer: Vec<String>,
    waiting: bool,
    repeat_capture: Buffer,
    repeat_cursor: Cursor,
    auto_close_pairs: bool,
}

impl InsertMode {
    /// Creates a new insert mode with an empty repeat capture buffer.
    pub fn new() -> Self {
        let mut keymap = TrieKeymap::new();
        keymap.insert_str("<Esc>", Action::mode_transition(ModeKind::Normal));
        keymap.insert_str("<C-q>", Action::new(ActionKind::Quit));
        keymap.insert_str("<C-s>", Action::save_buffer(None));
        keymap.insert_str("<Left>", Action::new(ActionKind::MoveLeft));
        keymap.insert_str("<Down>", Action::new(ActionKind::MoveDown));
        keymap.insert_str("<Up>", Action::new(ActionKind::MoveUp));
        keymap.insert_str("<Right>", Action::new(ActionKind::MoveRight));
        keymap.insert_str("<Enter>", Action::insert_char('\n'));
        keymap.insert_str("<Backspace>", Action::new(ActionKind::DeleteBackward));
        keymap.insert_str("<Delete>", Action::new(ActionKind::DeleteForward));
        globals::with_config(|config| {
            if let Some(insert_escape) = config.and_then(|config| config.insert_escape.as_deref()) {
                let parsed = validate_key_string(insert_escape)
                    .expect("invalid canonical insert escape binding in resolved config");
                keymap.insert_sequence(parsed, Action::mode_transition(ModeKind::Normal));
            }
        });
        let auto_close_pairs = globals::with_config(|config| {
            config.map(|config| config.auto_close_pairs).unwrap_or(true)
        });

        InsertMode {
            keymap,
            buffer: Vec::new(),
            waiting: false,
            repeat_capture: Buffer::new(),
            repeat_cursor: Cursor::new(0, 0),
            auto_close_pairs,
        }
    }

    fn record_action(&mut self, action: &Action) {
        match action.kind.as_ref() {
            Some(ActionKind::InsertChar(ch)) => self.record_insert_char(*ch),
            Some(ActionKind::InsertText(text)) => self.record_insert_text(text),
            Some(ActionKind::DeleteBackward) => self.record_delete_backward(),
            Some(ActionKind::DeleteForward) => self.record_delete_forward(),
            Some(ActionKind::MoveLeft) => self.record_move_left(),
            Some(ActionKind::MoveRight) => self.record_move_right(),
            Some(ActionKind::MoveUp) => self.record_move_up(),
            Some(ActionKind::MoveDown) => self.record_move_down(),
            _ => {}
        }
    }

    fn record_insert_char(&mut self, ch: char) {
        let cursor = self.repeat_cursor;
        if self.auto_close_pairs {
            if let Some(closer) = pairs::closer_for(ch) {
                if closer == ch && self.repeat_capture.char_at_cursor(cursor) == Some(ch) {
                    if let Some(new_cursor) = self.repeat_capture.next_cursor(cursor) {
                        self.repeat_cursor = new_cursor;
                    }
                    return;
                }
                self.record_insert_pair(ch, closer);
                return;
            }

            if pairs::is_supported_closer(ch)
                && self.repeat_capture.char_at_cursor(cursor) == Some(ch)
            {
                if let Some(new_cursor) = self.repeat_capture.next_cursor(cursor) {
                    self.repeat_cursor = new_cursor;
                }
                return;
            }
        }

        self.repeat_capture.insert_char(cursor, ch);
        self.repeat_cursor = match ch {
            '\n' => Cursor::new(cursor.line + 1, 0),
            _ => Cursor::new(cursor.line, cursor.col + ch.len_utf8()),
        };
    }

    fn record_insert_text(&mut self, text: &str) {
        if self.auto_close_pairs
            && let Some((opening, closing)) = pair_text(text)
        {
            self.record_insert_pair(opening, closing);
            return;
        }

        for ch in text.chars() {
            self.record_insert_char(ch);
        }
    }

    fn record_insert_pair(&mut self, opening: char, closing: char) {
        let cursor = self.repeat_cursor;
        self.repeat_capture.insert_char(cursor, opening);
        let between = Cursor::new(cursor.line, cursor.col + opening.len_utf8());
        self.repeat_capture.insert_char(between, closing);
        self.repeat_cursor = between;
    }

    fn record_delete_backward(&mut self) {
        if self.auto_close_pairs
            && let Some(opening) = self.repeat_capture.char_before_cursor(self.repeat_cursor)
            && let Some(closing) = self.repeat_capture.char_at_cursor(self.repeat_cursor)
            && pairs::closer_for(opening) == Some(closing)
            && let Some(start) = self.repeat_capture.prev_cursor_line(self.repeat_cursor)
            && let Some(end) = self.repeat_capture.next_cursor(self.repeat_cursor)
        {
            self.repeat_capture.remove(start, end);
            self.repeat_cursor = start;
            return;
        }

        if let Some(new_cursor) = self.repeat_capture.delete_char_before_cursor(self.repeat_cursor) {
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

    fn insert_action_for_char(&self, ch: char) -> Action {
        Action::insert_char(ch)
    }

    fn insert_text_for_char(&self, ch: char) -> String {
        if self.auto_close_pairs
            && let Some(closer) = pairs::closer_for(ch)
        {
            return format!("{ch}{closer}");
        }

        ch.to_string()
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
            return HandleKeyResult::Complete(Action::mode_transition(ModeKind::Normal).with_from_mode(ModeKind::Insert));
        }

        let key_str = key.canonical_string();
        let prior_buffer = self.buffer.clone();
        self.buffer.push(key_str);
        if let Some(action) = self.keymap.get_action(&self.buffer) {
            self.buffer.clear();
            self.waiting = false;
            self.record_action(&action);
            return HandleKeyResult::Complete(action.with_from_mode(ModeKind::Insert));
        }

        if self.keymap.is_prefix(&self.buffer) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        if !prior_buffer.is_empty()
            && let Some(text) = buffered_text(&prior_buffer)
            && let KeyCode::Char(c) = key.code
            && !key.modifiers.has_ctrl()
        {
            self.buffer.clear();
            self.waiting = false;
            let mut inserted = text;
            inserted.push_str(&self.insert_text_for_char(c));
            let action = Action::insert_text(inserted).with_from_mode(ModeKind::Insert);
            self.record_action(&action);
            return HandleKeyResult::Complete(action);
        }

        if let KeyCode::Char(c) = key.code
            && !key.modifiers.has_ctrl()
        {
            self.buffer.clear();
            self.waiting = false;
            let action = self
                .insert_action_for_char(c)
                .with_from_mode(ModeKind::Insert);
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

fn buffered_text(buffer: &[String]) -> Option<String> {
    let mut text = String::new();
    for token in buffer {
        if token.starts_with('<') || token.is_empty() {
            return None;
        }
        text.push_str(token);
    }

    Some(text)
}

fn pair_text(text: &str) -> Option<(char, char)> {
    let mut chars = text.chars();
    let opening = chars.next()?;
    let closing = chars.next()?;
    if chars.next().is_some() {
        return None;
    }

    let expected_closing = pairs::closer_for(opening)?;
    if expected_closing == closing {
        Some((opening, closing))
    } else {
        None
    }
}
