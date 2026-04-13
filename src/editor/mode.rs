use super::HandleKeyResult;
use crate::terminal::{CursorStyle, Key};

/// Lightweight mode classification used for user-facing labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeKind {
    /// Normal mode.
    Normal,
    /// Insert mode.
    Insert,
    /// Visual mode.
    Visual,
}

impl ModeKind {
    /// Returns the human-readable label for this mode kind.
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
        }
    }
}

/// Behavior shared by all editor modes.
pub trait Mode {
    /// Handles one key event and returns the resulting editor action, if any.
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult;
    /// Returns the terminal cursor style for this mode.
    fn cursor_style(&self) -> CursorStyle;
    /// Returns whether the mode is waiting for additional key input.
    fn is_waiting(&self) -> bool;
    /// Clears any buffered partial key sequence.
    fn clear_buffer(&mut self);
    /// Appends additional committed insert text to the repeat capture, if supported.
    fn append_repeat_text(&mut self, _text: &str) {}
    /// Returns committed insert text for the current mode, if it captured any.
    fn take_repeat_text(&mut self) -> Option<String> {
        None
    }
    /// Returns the editor mode kind used for display purposes.
    fn kind(&self) -> ModeKind;
}
