use super::{Action, HandleKeyResult, Mode, ModeKind};
use crate::terminal::{CursorStyle, Key};
use crate::ui::Command;

/// Pane resizing mode for split-layout adjustments.
pub struct ResizingMode;

impl Default for ResizingMode {
    fn default() -> Self {
        Self::new()
    }
}

impl ResizingMode {
    /// Creates a new resizing mode.
    pub fn new() -> Self {
        Self
    }

    fn command_for_key(&self, key: &Key) -> HandleKeyResult {
        let intent = match key.canonical_string().as_str() {
            "h" => Command::ResizePaneLeft(1),
            "H" => Command::ResizePaneLeft(5),
            "l" => Command::ResizePaneRight(1),
            "L" => Command::ResizePaneRight(5),
            "j" => Command::ResizePaneDown(1),
            "J" => Command::ResizePaneDown(5),
            "k" => Command::ResizePaneUp(1),
            "K" => Command::ResizePaneUp(5),
            "=" => Command::EqualizeSplits,
            "<Esc>" => {
                return HandleKeyResult::complete(
                    Action::mode_transition(ModeKind::Normal).with_from_mode(ModeKind::Resizing),
                );
            }
            _ => return HandleKeyResult::InvalidSequence,
        };

        HandleKeyResult::complete(intent)
    }
}

impl Mode for ResizingMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        self.command_for_key(key)
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyUnderline
    }

    fn is_waiting(&self) -> bool {
        false
    }

    fn clear_buffer(&mut self) {}

    fn kind(&self) -> ModeKind {
        ModeKind::Resizing
    }
}
