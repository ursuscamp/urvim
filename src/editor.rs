//! Editor module for vim-style modal editing.
//!
//! This module provides the Mode trait and implementations for Normal and Insert modes,
//! along with the KeyAction enum that represents actions triggered by keypresses.

use crate::terminal::{CursorStyle, Key, KeyCode, Modifiers};

/// Actions that the main event loop processes.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
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
}

/// Trait for mode-specific key handling.
pub trait Mode {
    /// Process a key event and return the corresponding action.
    fn handle_key(&self, key: &Key) -> KeyAction;

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
    fn handle_key(&self, key: &Key) -> KeyAction {
        match (key.code, key.modifiers) {
            // Movement keys (h, j, k, l)
            (KeyCode::Char('h'), _) if !key.modifiers.has_ctrl() => KeyAction::MoveLeft,
            (KeyCode::Char('j'), _) if !key.modifiers.has_ctrl() => KeyAction::MoveDown,
            (KeyCode::Char('k'), _) if !key.modifiers.has_ctrl() => KeyAction::MoveUp,
            (KeyCode::Char('l'), _) if !key.modifiers.has_ctrl() => KeyAction::MoveRight,

            // Mode switching
            (KeyCode::Char('i'), _) if !key.modifiers.has_ctrl() => KeyAction::SwitchToInsert,

            // Quit (Ctrl-q)
            (KeyCode::Char('q'), _) => KeyAction::Quit,

            // Arrow keys for convenience
            (KeyCode::Left, _) => KeyAction::MoveLeft,
            (KeyCode::Down, _) => KeyAction::MoveDown,
            (KeyCode::Up, _) => KeyAction::MoveUp,
            (KeyCode::Right, _) => KeyAction::MoveRight,

            // Ignore other keys in normal mode
            _ => KeyAction::None,
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
    fn handle_key(&self, key: &Key) -> KeyAction {
        match (key.code, key.modifiers) {
            // Mode switching
            (KeyCode::Esc, _) => KeyAction::SwitchToNormal,

            // Quit (Ctrl-q)
            (KeyCode::Char('q'), Modifiers::CTRL) => KeyAction::Quit,

            // Character insertion
            (KeyCode::Char(c), _) if !key.modifiers.has_ctrl() => KeyAction::InsertChar(c),

            // Enter key inserts newline
            (KeyCode::Enter, _) => KeyAction::InsertChar('\n'),

            // Arrow keys for cursor movement while in insert mode
            (KeyCode::Left, _) => KeyAction::MoveLeft,
            (KeyCode::Down, _) => KeyAction::MoveDown,
            (KeyCode::Up, _) => KeyAction::MoveUp,
            (KeyCode::Right, _) => KeyAction::MoveRight,

            // Ignore other keys
            _ => KeyAction::None,
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
        assert_eq!(mode.handle_key(&key('h')), KeyAction::MoveLeft);
    }

    #[test]
    fn test_normal_mode_move_down() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('j')), KeyAction::MoveDown);
    }

    #[test]
    fn test_normal_mode_move_up() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('k')), KeyAction::MoveUp);
    }

    #[test]
    fn test_normal_mode_move_right() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('l')), KeyAction::MoveRight);
    }

    #[test]
    fn test_normal_mode_switch_to_insert() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('i')), KeyAction::SwitchToInsert);
    }

    #[test]
    fn test_normal_mode_cursor_style() {
        let mode = NormalMode::new();
        assert_eq!(mode.cursor_style(), CursorStyle::SteadyBlock);
    }

    #[test]
    fn test_insert_mode_insert_char() {
        let mode = InsertMode::new();
        assert_eq!(mode.handle_key(&key('a')), KeyAction::InsertChar('a'));
    }

    #[test]
    fn test_insert_mode_escape_switches_to_normal() {
        let mode = InsertMode::new();
        assert_eq!(
            mode.handle_key(&Key::new(KeyCode::Esc)),
            KeyAction::SwitchToNormal
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
            KeyAction::InsertChar('\n')
        );
    }

    #[test]
    fn test_normal_mode_ignore_other_keys() {
        let mode = NormalMode::new();
        assert_eq!(mode.handle_key(&key('x')), KeyAction::None);
        assert_eq!(mode.handle_key(&key('a')), KeyAction::None);
    }

    #[test]
    fn test_insert_mode_ignore_other_keys() {
        let mode = InsertMode::new();
        // Ctrl+a should be ignored
        assert_eq!(
            mode.handle_key(&Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL)),
            KeyAction::None
        );
    }
}
