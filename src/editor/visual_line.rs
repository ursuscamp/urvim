use super::visual_common::VisualModeState;
use super::{HandleKeyResult, Mode, ModeKind};
use crate::terminal::{CursorStyle, Key};

/// Linewise visual mode for whole-line selection.
pub struct VisualLineMode(VisualModeState);

impl Default for VisualLineMode {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualLineMode {
    /// Creates a new linewise visual mode with motion bindings and selection actions.
    pub fn new() -> Self {
        Self(VisualModeState::new(
            ModeKind::VisualLine,
            "V",
            "v",
            ModeKind::Visual,
        ))
    }
}

impl Mode for VisualLineMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        self.0.handle_key(key)
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBlock
    }

    fn is_waiting(&self) -> bool {
        self.0.is_waiting()
    }

    fn clear_buffer(&mut self) {
        self.0.clear_buffer();
    }

    fn kind(&self) -> ModeKind {
        self.0.kind()
    }
}
