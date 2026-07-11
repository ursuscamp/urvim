use super::{EditorAction, EditorOperation, HandleKeyResult, Mode, ModeKind, TrieKeymap};
use crate::buffer::{Buffer, Cursor, IndentDirection};
use crate::config::{DEFAULT_TAB_WIDTH, TabBehavior, TabInsertion};
use crate::editor::pairs;
use crate::globals;
use crate::ui::{Command, Intent};
use urvim_terminal::{CursorStyle, Key, KeyCode};

/// Insert mode for text input.
pub struct InsertMode {
    keymap: TrieKeymap,
    buffer: Vec<String>,
    waiting: bool,
    repeat_capture: Buffer,
    repeat_cursor: Cursor,
    auto_close_pairs: bool,
    tab_insertion: TabInsertion,
    tab_behavior: TabBehavior,
    tab_width: usize,
}

impl InsertMode {
    /// Creates a new insert mode with an empty repeat capture buffer.
    pub fn new() -> Self {
        let mut keymap = TrieKeymap::<Intent>::new();
        keymap.insert_str("<F1>", Command::OpenFilePicker);
        keymap.insert_str("<F2>", Command::OpenGrepPicker);
        keymap.insert_str("<F3>", Command::OpenBufferPicker);
        keymap.insert_str("<F4>", Command::OpenGitPicker);
        keymap.insert_str("<F5>", Command::OpenColorschemePicker);
        keymap.insert_str("<F6>", Command::OpenFiletypePicker);
        keymap.insert_str("<C-Backspace>", Command::OpenCompletion);
        keymap.insert_str("<Esc>", EditorAction::mode_transition(ModeKind::Normal));
        keymap.insert_str("<C-q>", Command::TryQuit);
        keymap.insert_str("<C-s>", Command::SaveBuffer(None));
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
            "<S-Tab>",
            EditorAction::new(EditorOperation::IndentDecrease),
        );
        keymap.insert_str(
            "<Backspace>",
            EditorAction::new(EditorOperation::DeleteBackward),
        );
        keymap.insert_str(
            "<Delete>",
            EditorAction::new(EditorOperation::DeleteForward),
        );
        globals::with_opt_config(|config| {
            if let Some(config) = config {
                keymap.insert_configured(&config.keymaps.insert);
            }
        });
        keymap.insert_intents(&globals::plugin_keymap_intents_for_mode(ModeKind::Insert));
        let auto_close_pairs =
            globals::with_config(|config| config.auto_close_pairs).unwrap_or(true);
        let tab_insertion = globals::with_config(|config| config.tab_insertion).unwrap_or_default();
        let tab_behavior = globals::with_config(|config| config.tab_behavior).unwrap_or_default();
        let tab_width =
            globals::with_config(|config| config.tab_width).unwrap_or(DEFAULT_TAB_WIDTH);

        InsertMode {
            keymap,
            buffer: Vec::new(),
            waiting: false,
            repeat_capture: Buffer::new(),
            repeat_cursor: Cursor::new(0, 0),
            auto_close_pairs,
            tab_insertion,
            tab_behavior,
            tab_width,
        }
    }

    fn record_action(&mut self, action: &EditorAction) {
        match action.kind.as_ref() {
            Some(EditorOperation::InsertChar(ch)) => self.record_insert_char(*ch),
            Some(EditorOperation::InsertText(text)) => self.record_insert_text(text),
            Some(EditorOperation::InsertNewline) => self.record_insert_char('\n'),
            Some(EditorOperation::DeleteBackward) => self.record_delete_backward(),
            Some(EditorOperation::DeleteForward) => self.record_delete_forward(),
            Some(EditorOperation::MoveLeft) => self.record_move_left(),
            Some(EditorOperation::MoveRight) => self.record_move_right(),
            Some(EditorOperation::MoveUp) => self.record_move_up(),
            Some(EditorOperation::MoveDown) => self.record_move_down(),
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
        let prefix_len = self
            .repeat_capture
            .line_leading_whitespace_prefix(self.repeat_cursor.line)
            .map(|prefix| prefix.len())
            .unwrap_or(0);
        if prefix_len > 0
            && self.repeat_cursor.col <= prefix_len
            && let Some(delta) = self
                .repeat_capture
                .shift_line_indentation(self.repeat_cursor.line, IndentDirection::Decrease)
        {
            self.repeat_cursor = Cursor::new(
                self.repeat_cursor.line,
                self.repeat_cursor.col.saturating_sub(delta),
            );
            return;
        }

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

    fn insert_action_for_char(&self, ch: char) -> EditorAction {
        EditorAction::insert_char(ch)
    }

    fn insert_text_for_char(&self, ch: char) -> String {
        if ch == '\t' {
            return self.insert_tab_text();
        }
        if self.auto_close_pairs
            && let Some(closer) = pairs::closer_for(ch)
        {
            return format!("{ch}{closer}");
        }

        ch.to_string()
    }

    fn insert_tab_text(&self) -> String {
        let resolved = match self.tab_behavior {
            TabBehavior::Simple => self.tab_insertion,
            TabBehavior::Smart => globals::with_active_buffer_id(|buffer_id| {
                buffer_id
                    .and_then(|buffer_id| {
                        globals::with_buffer(buffer_id, |buffer| buffer.inferred_tab_insertion())
                    })
                    .flatten()
                    .unwrap_or(self.tab_insertion)
            }),
        };

        match resolved {
            TabInsertion::Tabs => "\t".to_string(),
            TabInsertion::Spaces => " ".repeat(self.tab_width.max(1)),
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
            return HandleKeyResult::complete(
                EditorAction::mode_transition(ModeKind::Normal).with_from_mode(ModeKind::Insert),
            );
        }

        if key.code == KeyCode::Enter {
            self.buffer.clear();
            self.waiting = false;
            let action = EditorAction::insert_newline().with_from_mode(ModeKind::Insert);
            self.record_action(&action);
            return HandleKeyResult::complete(action);
        }

        if key.code == KeyCode::Tab && !key.modifiers.has_shift() {
            self.buffer.clear();
            self.waiting = false;
            let action =
                EditorAction::insert_text(self.insert_tab_text()).with_from_mode(ModeKind::Insert);
            self.record_action(&action);
            return HandleKeyResult::complete(action);
        }

        let key_str = key.canonical_string();
        let prior_buffer = self.buffer.clone();
        self.buffer.push(key_str);
        if let Some(intent) = self.keymap.get_action(&self.buffer) {
            self.buffer.clear();
            self.waiting = false;
            if let Some(action) = intent.as_editor_action().cloned() {
                self.record_action(&action);
                return HandleKeyResult::complete(action.with_from_mode(ModeKind::Insert));
            }

            return HandleKeyResult::complete(intent);
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
            let action = EditorAction::insert_text(inserted).with_from_mode(ModeKind::Insert);
            self.record_action(&action);
            return HandleKeyResult::complete(action);
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
            return HandleKeyResult::complete(action);
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

    fn append_repeat_text(&mut self, text: &str) {
        if !text.is_empty() {
            self.record_insert_text(text);
        }
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
