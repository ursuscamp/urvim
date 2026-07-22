use super::visual_common::VisualModeState;
use super::{HandleKeyResult, KeyGuideSnapshot, Mode, ModeKind};
use urvim_terminal::{CursorStyle, Key};

/// Visual mode for character-wise selection.
pub struct VisualMode(VisualModeState);

impl Default for VisualMode {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualMode {
    /// Creates a new visual mode with motion bindings and selection actions.
    pub fn new() -> Self {
        Self(VisualModeState::new(
            ModeKind::Visual,
            "v",
            "V",
            ModeKind::VisualLine,
        ))
    }
}

impl Mode for VisualMode {
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

    fn key_guide(&self) -> Option<KeyGuideSnapshot> {
        self.0.key_guide()
    }

    fn kind(&self) -> ModeKind {
        self.0.kind()
    }
}
