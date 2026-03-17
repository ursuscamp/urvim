//! Editor module for vim-style modal editing.
//!
//! This module provides the Mode trait and implementations for Normal and Insert modes,
//! along with the Action enum that represents actions triggered by keypresses.

use crate::buffer::Boundary;
use crate::terminal::{CursorStyle, Key, KeyCode};

/// Actions that the main event loop processes.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Move cursor left
    MoveLeft,
    /// Move cursor down
    MoveDown,
    /// Move cursor up
    MoveUp,
    /// Move cursor right
    MoveRight,
    /// Insert a character at cursor position
    InsertChar(char),
    /// Switch to Normal mode
    SwitchToNormal,
    /// Switch to Insert mode
    SwitchToInsert,
    /// Quit the editor
    Quit,
    /// No action (ignored key)
    None,
    /// Move forward to boundary
    ForwardTo(Boundary),
    /// Move backward to boundary
    BackTo(Boundary),
    /// Move cursor to end of current line
    MoveToLineEnd,
    /// Move cursor to absolute start of line (column 0)
    MoveToLineStart,
    /// Move cursor to first non-whitespace of line
    MoveToLineContentStart,
    /// Delete character before cursor (backspace)
    DeleteBackward,
    /// Delete character at cursor (delete key)
    DeleteForward,
}

impl Action {
    /// Returns true if this action is a horizontal movement that should reset
    /// the remembered visual column to the current position.
    pub fn resets_remembered_column(&self) -> bool {
        matches!(
            self,
            Action::MoveLeft
                | Action::MoveRight
                | Action::ForwardTo(_)
                | Action::BackTo(_)
                | Action::MoveToLineEnd
                | Action::MoveToLineStart
                | Action::MoveToLineContentStart
                | Action::InsertChar(_)
                | Action::DeleteBackward
                | Action::DeleteForward
        )
    }

    /// Returns true if this action is a vertical movement that should use
    /// and update the remembered visual column.
    pub fn uses_remembered_column(&self) -> bool {
        matches!(self, Action::MoveUp | Action::MoveDown)
    }
}

/// Result of processing a key in a mode.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleKeyResult {
    /// A complete action is ready to execute.
    Complete(Action),
    /// Waiting for more keys to complete a sequence.
    WaitForMore,
    /// The key sequence was invalid or incomplete with no possible match.
    InvalidSequence,
}

/// Trait for mapping normalized key sequences to actions.
pub trait Keymap {
    /// Get the action for a key sequence, if one exists.
    fn get_action(&self, keys: &[String]) -> Option<Action>;

    /// Check if the given key sequence could be a prefix of a longer binding.
    fn is_prefix(&self, keys: &[String]) -> bool;
}

/// A simple single-key keymap implementation using HashMap.
pub struct SimpleKeymap {
    bindings: std::collections::HashMap<String, Action>,
}

impl SimpleKeymap {
    /// Creates a new empty keymap.
    pub fn new() -> Self {
        Self {
            bindings: std::collections::HashMap::new(),
        }
    }

    /// Inserts a key-action binding.
    pub fn insert(&mut self, key: String, action: Action) {
        self.bindings.insert(key, action);
    }
}

impl Keymap for SimpleKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action> {
        // For now, only support single-key lookups
        if keys.len() == 1 {
            self.bindings.get(&keys[0]).cloned()
        } else {
            None
        }
    }

    fn is_prefix(&self, _keys: &[String]) -> bool {
        // For single-key maps, no sequence is a prefix of another
        // This will change when multi-key bindings are added
        false
    }
}

impl Default for SimpleKeymap {
    fn default() -> Self {
        Self::new()
    }
}
pub trait Mode {
    /// Process a key event and return the corresponding result.
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult;

    /// Get the cursor style for this mode.
    fn cursor_style(&self) -> CursorStyle;

    /// Whether the mode is waiting for more keys to complete a sequence.
    fn is_waiting(&self) -> bool;

    /// Clear the pending key buffer.
    fn clear_buffer(&mut self);
}

/// Normal mode for vim-style navigation and commands.
pub struct NormalMode {
    keymap: SimpleKeymap,
    buffer: Vec<String>,
    waiting: bool,
}

impl NormalMode {
    pub fn new() -> Self {
        let mut keymap = SimpleKeymap::new();

        // Movement keys (h, j, k, l)
        keymap.insert("h".to_string(), Action::MoveLeft);
        keymap.insert("j".to_string(), Action::MoveDown);
        keymap.insert("k".to_string(), Action::MoveUp);
        keymap.insert("l".to_string(), Action::MoveRight);

        // Word motions
        keymap.insert("w".to_string(), Action::ForwardTo(Boundary::Word));
        keymap.insert("b".to_string(), Action::BackTo(Boundary::Word));
        keymap.insert("e".to_string(), Action::ForwardTo(Boundary::WordEnd));

        // BigWord motions
        keymap.insert("W".to_string(), Action::ForwardTo(Boundary::BigWord));
        keymap.insert("B".to_string(), Action::BackTo(Boundary::BigWord));
        keymap.insert("E".to_string(), Action::ForwardTo(Boundary::BigWordEnd));

        // Line end navigation
        keymap.insert("$".to_string(), Action::MoveToLineEnd);

        // Line start navigation
        keymap.insert("0".to_string(), Action::MoveToLineStart);
        keymap.insert("^".to_string(), Action::MoveToLineContentStart);

        // Mode switching
        keymap.insert("i".to_string(), Action::SwitchToInsert);

        // Delete operations
        keymap.insert("x".to_string(), Action::DeleteForward);
        keymap.insert("X".to_string(), Action::DeleteBackward);

        // Quit (Ctrl-q)
        keymap.insert("<C-q>".to_string(), Action::Quit);

        // Arrow keys for convenience
        keymap.insert("<Left>".to_string(), Action::MoveLeft);
        keymap.insert("<Down>".to_string(), Action::MoveDown);
        keymap.insert("<Up>".to_string(), Action::MoveUp);
        keymap.insert("<Right>".to_string(), Action::MoveRight);

        NormalMode {
            keymap,
            buffer: Vec::new(),
            waiting: false,
        }
    }
}

impl Default for NormalMode {
    fn default() -> Self {
        NormalMode::new()
    }
}

impl Mode for NormalMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        // Escape always clears buffer and returns to idle
        if key.code == KeyCode::Esc {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::InvalidSequence;
        }

        // Convert key to canonical string
        let key_str = key.canonical_string();

        // Add to buffer
        self.buffer.push(key_str);

        // Check for exact match
        if let Some(action) = self.keymap.get_action(&self.buffer) {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(action);
        }

        // Check if we could be waiting for more keys
        if self.keymap.is_prefix(&self.buffer) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        // No match - clear buffer and return invalid
        self.buffer.clear();
        self.waiting = false;
        HandleKeyResult::InvalidSequence
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBlock
    }

    fn is_waiting(&self) -> bool {
        self.waiting
    }

    fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.waiting = false;
    }
}

/// Insert mode for text input.
pub struct InsertMode {
    keymap: SimpleKeymap,
    buffer: Vec<String>,
    waiting: bool,
}

impl InsertMode {
    pub fn new() -> Self {
        let mut keymap = SimpleKeymap::new();

        // Mode switching
        keymap.insert("<Esc>".to_string(), Action::SwitchToNormal);

        // Quit (Ctrl-q)
        keymap.insert("<C-q>".to_string(), Action::Quit);

        // Arrow keys for cursor movement while in insert mode
        keymap.insert("<Left>".to_string(), Action::MoveLeft);
        keymap.insert("<Down>".to_string(), Action::MoveDown);
        keymap.insert("<Up>".to_string(), Action::MoveUp);
        keymap.insert("<Right>".to_string(), Action::MoveRight);

        // Enter inserts newline
        keymap.insert("<Enter>".to_string(), Action::InsertChar('\n'));

        // Delete operations
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
        // Escape always clears buffer and switches to normal
        if key.code == KeyCode::Esc {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(Action::SwitchToNormal);
        }

        // Convert key to canonical string
        let key_str = key.canonical_string();

        // Check for special key bindings first
        if let Some(action) = self.keymap.get_action(&[key_str.clone()]) {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(action);
        }

        // Check if it could be a prefix of a multi-key sequence
        self.buffer.push(key_str);
        if self.keymap.is_prefix(&self.buffer) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        // For printable characters without Ctrl, insert them
        if let KeyCode::Char(c) = key.code {
            if !key.modifiers.has_ctrl() {
                self.buffer.clear();
                self.waiting = false;
                return HandleKeyResult::Complete(Action::InsertChar(c));
            }
        }

        // No match - clear buffer
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::Key;

    fn key(c: char) -> Key {
        Key::new(KeyCode::Char(c))
    }

    fn handle_and_unwrap(mode: &mut impl Mode, k: &Key) -> Action {
        match mode.handle_key(k) {
            HandleKeyResult::Complete(action) => action,
            HandleKeyResult::WaitForMore => Action::None,
            HandleKeyResult::InvalidSequence => Action::None,
        }
    }

    #[test]
    fn test_normal_mode_move_left() {
        let mut mode = NormalMode::new();
        assert_eq!(handle_and_unwrap(&mut mode, &key('h')), Action::MoveLeft);
    }

    #[test]
    fn test_normal_mode_move_down() {
        let mut mode = NormalMode::new();
        assert_eq!(handle_and_unwrap(&mut mode, &key('j')), Action::MoveDown);
    }

    #[test]
    fn test_normal_mode_move_up() {
        let mut mode = NormalMode::new();
        assert_eq!(handle_and_unwrap(&mut mode, &key('k')), Action::MoveUp);
    }

    #[test]
    fn test_normal_mode_move_right() {
        let mut mode = NormalMode::new();
        assert_eq!(handle_and_unwrap(&mut mode, &key('l')), Action::MoveRight);
    }

    #[test]
    fn test_normal_mode_switch_to_insert() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('i')),
            Action::SwitchToInsert
        );
    }

    #[test]
    fn test_normal_mode_cursor_style() {
        let mode = NormalMode::new();
        assert_eq!(mode.cursor_style(), CursorStyle::SteadyBlock);
    }

    #[test]
    fn test_insert_mode_insert_char() {
        let mut mode = InsertMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('a')),
            Action::InsertChar('a')
        );
    }

    #[test]
    fn test_insert_mode_escape_switches_to_normal() {
        let mut mode = InsertMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Esc)),
            Action::SwitchToNormal
        );
    }

    #[test]
    fn test_insert_mode_cursor_style() {
        let mode = InsertMode::new();
        assert_eq!(mode.cursor_style(), CursorStyle::SteadyBar);
    }

    #[test]
    fn test_insert_mode_enter_inserts_newline() {
        let mut mode = InsertMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Enter)),
            Action::InsertChar('\n')
        );
    }

    #[test]
    fn test_insert_mode_delete_key() {
        use crate::terminal::Key;
        let mut mode = InsertMode::new();
        // Test Delete key
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Delete)),
            Action::DeleteForward
        );
    }

    #[test]
    fn test_insert_mode_backspace_key() {
        use crate::terminal::Key;
        let mut mode = InsertMode::new();
        // Test Backspace key
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Backspace)),
            Action::DeleteBackward
        );
    }

    #[test]
    fn test_insert_mode_delete_key_simulation() {
        // Simulate what happens when Delete is pressed in insert mode
        // by creating a buffer with text and checking delete behavior
        use crate::terminal::Key;
        let mut mode = InsertMode::new();

        // Verify Delete key triggers DeleteForward
        let action = handle_and_unwrap(&mut mode, &Key::new(KeyCode::Delete));
        assert_eq!(
            action,
            Action::DeleteForward,
            "Delete key should trigger DeleteForward"
        );

        // Verify Backspace triggers DeleteBackward
        let mut mode2 = InsertMode::new();
        let action2 = handle_and_unwrap(&mut mode2, &Key::new(KeyCode::Backspace));
        assert_eq!(
            action2,
            Action::DeleteBackward,
            "Backspace should trigger DeleteBackward"
        );
    }

    #[test]
    fn test_normal_mode_x_key() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('x')),
            Action::DeleteForward
        );
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('X'))),
            Action::DeleteBackward
        );
    }

    #[test]
    fn test_normal_mode_ignore_other_keys() {
        let mut mode = NormalMode::new();
        // 'x' and 'X' are now bound to DeleteForward/DeleteBackward
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('x')),
            Action::DeleteForward
        );
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('X'))),
            Action::DeleteBackward
        );
        // Other keys still return None
        assert_eq!(handle_and_unwrap(&mut mode, &key('a')), Action::None);
    }

    #[test]
    fn test_normal_mode_word_forward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('w')),
            Action::ForwardTo(Boundary::Word)
        );
    }

    #[test]
    fn test_normal_mode_word_backward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('b')),
            Action::BackTo(Boundary::Word)
        );
    }

    #[test]
    fn test_normal_mode_word_end_forward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('e')),
            Action::ForwardTo(Boundary::WordEnd)
        );
    }

    #[test]
    fn test_normal_mode_bigword_forward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('W'))),
            Action::ForwardTo(Boundary::BigWord)
        );
    }

    #[test]
    fn test_normal_mode_bigword_backward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('B'))),
            Action::BackTo(Boundary::BigWord)
        );
    }

    #[test]
    fn test_normal_mode_bigword_end_forward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('E'))),
            Action::ForwardTo(Boundary::BigWordEnd)
        );
    }

    #[test]
    fn test_normal_mode_move_to_line_end() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('$'))),
            Action::MoveToLineEnd
        );
    }

    #[test]
    fn test_normal_mode_move_to_line_start() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('0'))),
            Action::MoveToLineStart
        );
    }

    #[test]
    fn test_normal_mode_move_to_line_content_start() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('^'))),
            Action::MoveToLineContentStart
        );
    }
}
