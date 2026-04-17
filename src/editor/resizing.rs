use super::{Action, ActionKind, HandleKeyResult, Mode, ModeKind};
use crate::terminal::{CursorStyle, Key};

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

    fn resize_action(kind: ActionKind, larger_step: bool) -> Action {
        let action = Action::new(kind);
        if larger_step {
            Action::count(5, Box::new(action))
        } else {
            action
        }
    }

    fn action_for_key(&self, key: &Key) -> HandleKeyResult {
        let action = match key.canonical_string().as_str() {
            "h" => Self::resize_action(ActionKind::ResizePaneLeft, false),
            "H" => Self::resize_action(ActionKind::ResizePaneLeft, true),
            "l" => Self::resize_action(ActionKind::ResizePaneRight, false),
            "L" => Self::resize_action(ActionKind::ResizePaneRight, true),
            "j" => Self::resize_action(ActionKind::ResizePaneDown, false),
            "J" => Self::resize_action(ActionKind::ResizePaneDown, true),
            "k" => Self::resize_action(ActionKind::ResizePaneUp, false),
            "K" => Self::resize_action(ActionKind::ResizePaneUp, true),
            "=" => Action::new(ActionKind::EqualizeSplits),
            "<Esc>" => Action::mode_transition(ModeKind::Normal),
            _ => return HandleKeyResult::InvalidSequence,
        };

        HandleKeyResult::Complete(action.with_from_mode(ModeKind::Resizing))
    }
}

impl Mode for ResizingMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        self.action_for_key(key)
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
