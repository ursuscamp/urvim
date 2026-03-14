//! Editor module for vim-style modal editing.
//!
//! This module provides the Mode trait and implementations for Normal and Insert modes,
//! along with the Action enum that represents actions triggered by keypresses.

use crate::buffer::Boundary;
use crate::terminal::{CursorStyle, Key, KeyCode, Modifiers};

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
}

/// Trait for mode-specific key handling.
pub trait Mode {
    /// Process a key event and return the corresponding action.
    fn handle_key(&self, key: &Key) -> Action;

    /// Get the cursor style for this mode.
    fn cursor_style(&self) -> CursorStyle;
}

/// Normal mode for vim-style navigation and commands.
pub struct NormalMode;

impl NormalMode {
    pub fn new() -> Self {
        NormalMode
    }
}

impl Default for NormalMode {
    fn default() -> Self {
        NormalMode
    }
}

impl Mode for NormalMode {
    fn handle_key(&self, key: &Key) -> Action {
        match (key.code, key.modifiers) {
            // Movement keys (h, j, k, l)
            (KeyCode::Char('h'), _) if !key.modifiers.has_ctrl() => Action::MoveLeft,
            (KeyCode::Char('j'), _) if !key.modifiers.has_ctrl() => Action::MoveDown,
            (KeyCode::Char('k'), _) if !key.modifiers.has_ctrl() => Action::MoveUp,
            (KeyCode::Char('l'), _) if !key.modifiers.has_ctrl() => Action::MoveRight,

            // Word motions
            (KeyCode::Char('w'), _) if !key.modifiers.has_ctrl() => {
                Action::ForwardTo(Boundary::Word)
            }
            (KeyCode::Char('b'), _) if !key.modifiers.has_ctrl() => Action::BackTo(Boundary::Word),
            (KeyCode::Char('e'), _) if !key.modifiers.has_ctrl() => {
                Action::ForwardTo(Boundary::WordEnd)
            }

            // BigWord motions
            (KeyCode::Char('W'), _) => Action::ForwardTo(Boundary::BigWord),
            (KeyCode::Char('B'), _) => Action::BackTo(Boundary::BigWord),
            (KeyCode::Char('E'), _) => Action::ForwardTo(Boundary::BigWordEnd),

            // Mode switching
            (KeyCode::Char('i'), _) if !key.modifiers.has_ctrl() => Action::SwitchToInsert,

            // Quit (Ctrl-q)
            (KeyCode::Char('q'), _) => Action::Quit,

            // Arrow keys for convenience
            (KeyCode::Left, _) => Action::MoveLeft,
            (KeyCode::Down, _) => Action::MoveDown,
            (KeyCode::Up, _) => Action::MoveUp,
            (KeyCode::Right, _) => Action::MoveRight,

            // Ignore other keys in normal mode
            _ => Action::None,
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBlock
    }
}

/// Insert mode for text input.
pub struct InsertMode;

impl InsertMode {
    pub fn new() -> Self {
        InsertMode
    }
}

impl Default for InsertMode {
    fn default() -> Self {
        InsertMode
    }
}

impl Mode for InsertMode {
    fn handle_key(&self, key: &Key) -> Action {
        match (key.code, key.modifiers) {
            // Mode switching
            (KeyCode::Esc, _) => Action::SwitchToNormal,

            // Quit (Ctrl-q)
            (KeyCode::Char('q'), Modifiers::CTRL) => Action::Quit,

            // Character insertion
            (KeyCode::Char(c), _) if !key.modifiers.has_ctrl() => Action::InsertChar(c),

            // Enter key inserts newline
            (KeyCode::Enter, _) => Action::InsertChar('\n'),

            // Arrow keys for cursor movement while in insert mode
            (KeyCode::Left, _) => Action::MoveLeft,
            (KeyCode::Down, _) => Action::MoveDown,
            (KeyCode::Up, _) => Action::MoveUp,
            (KeyCode::Right, _) => Action::MoveRight,

            // Ignore other keys
            _ => Action::None,
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBar
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::Key;

    fn key(c: char) -> Key {
        Key::new(KeyCode::Char(c))
    }

    #[test]
    fn test_normal_mode_move_left() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('h')), Action::MoveLeft);
    }

    #[test]
    fn test_normal_mode_move_down() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('j')), Action::MoveDown);
    }

    #[test]
    fn test_normal_mode_move_up() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('k')), Action::MoveUp);
    }

    #[test]
    fn test_normal_mode_move_right() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('l')), Action::MoveRight);
    }

    #[test]
    fn test_normal_mode_switch_to_insert() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('i')), Action::SwitchToInsert);
    }

    #[test]
    fn test_normal_mode_cursor_style() {
        let mode = NormalMode::new();
        assert_eq!(mode.cursor_style(), CursorStyle::SteadyBlock);
    }

    #[test]
    fn test_insert_mode_insert_char() {
        let mode = InsertMode::new();
        assert_eq!(mode.handle_key(&key('a')), Action::InsertChar('a'));
    }

    #[test]
    fn test_insert_mode_escape_switches_to_normal() {
        let mode = InsertMode::new();
        assert_eq!(
            mode.handle_key(&Key::new(KeyCode::Esc)),
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
        let mode = InsertMode::new();
        assert_eq!(
            mode.handle_key(&Key::new(KeyCode::Enter)),
            Action::InsertChar('\n')
        );
    }

    #[test]
    fn test_normal_mode_ignore_other_keys() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('x')), Action::None);
        assert_eq!(mode.handle_key(&key('a')), Action::None);
    }

    #[test]
    fn test_normal_mode_word_forward() {
        use crate::buffer::Boundary;
        let mode = NormalMode::new();
        assert_eq!(
            mode.handle_key(&key('w')),
            Action::ForwardTo(Boundary::Word)
        );
    }

    #[test]
    fn test_normal_mode_word_backward() {
        use crate::buffer::Boundary;
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('b')), Action::BackTo(Boundary::Word));
    }

    #[test]
    fn test_normal_mode_word_end_forward() {
        use crate::buffer::Boundary;
        let mode = NormalMode::new();
        assert_eq!(
            mode.handle_key(&key('e')),
            Action::ForwardTo(Boundary::WordEnd)
        );
    }

    #[test]
    fn test_normal_mode_bigword_forward() {
        use crate::buffer::Boundary;
        let mode = NormalMode::new();
        assert_eq!(
            mode.handle_key(&Key::new(KeyCode::Char('W'))),
            Action::ForwardTo(Boundary::BigWord)
        );
    }

    #[test]
    fn test_normal_mode_bigword_backward() {
        use crate::buffer::Boundary;
        let mode = NormalMode::new();
        assert_eq!(
            mode.handle_key(&Key::new(KeyCode::Char('B'))),
            Action::BackTo(Boundary::BigWord)
        );
    }

    #[test]
    fn test_normal_mode_bigword_end_forward() {
        use crate::buffer::Boundary;
        let mode = NormalMode::new();
        assert_eq!(
            mode.handle_key(&Key::new(KeyCode::Char('E'))),
            Action::ForwardTo(Boundary::BigWordEnd)
        );
    }
}
