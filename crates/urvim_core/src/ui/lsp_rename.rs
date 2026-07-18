//! LSP rename prompt widget.

use crate::screen::Screen;
use crate::ui::input_box::{InputBox, InputBoxOutcome};
use crate::ui::{Command, FocusPolicy, Intent, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;

/// Dedicated rename prompt used for LSP rename input.
#[derive(Debug)]
pub struct LspRenamePrompt {
    input: InputBox,
}

impl LspRenamePrompt {
    /// Creates a new rename prompt.
    pub fn new(initial_text: impl Into<String>) -> Self {
        Self {
            input: InputBox::new("Rename", " ", initial_text),
        }
    }

    /// Returns true when the prompt is open.
    pub fn is_open(&self) -> bool {
        self.input.is_open()
    }

    /// Returns the current input text.
    pub fn text(&self) -> &str {
        self.input.text()
    }

    /// Returns the rendered cursor position, if available.
    pub fn cursor(&self) -> Option<crate::ui::geometry::Position> {
        self.input.cursor()
    }

    /// Handles a UI event while the prompt is open.
    pub fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        let result = self.input.handle_ui_event(event, _ctx);
        match self.input.take_outcome() {
            Some(InputBoxOutcome::Submitted(text)) => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    UiEventResult::Handled(Vec::new())
                } else {
                    UiEventResult::Handled(vec![Intent::Command(Command::LspRename(text))])
                }
            }
            Some(InputBoxOutcome::Cancelled) | None => result,
        }
    }

    /// Renders the prompt into the provided rectangle.
    pub fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        self.input.render_widget(screen, rect, _ctx);
    }
}

impl Widget for LspRenamePrompt {
    fn handle_ui_event(&mut self, event: &UiEvent, ctx: &mut UiContext) -> UiEventResult {
        LspRenamePrompt::handle_ui_event(self, event, ctx)
    }

    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, ctx: &UiContext) {
        LspRenamePrompt::render_widget(self, screen, rect, ctx)
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Focusable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Intent, UiContext, UiEvent};
    use urvim_terminal::{Key, KeyCode, Modifiers};

    fn key(code: KeyCode) -> Key {
        Key {
            code,
            modifiers: Modifiers::default(),
        }
    }

    #[test]
    fn prompt_prefills_placeholder_text() {
        let prompt = LspRenamePrompt::new("existing_name");
        assert_eq!(prompt.text(), "existing_name");
    }

    #[test]
    fn enter_submits_rename_command() {
        let mut prompt = LspRenamePrompt::new("");
        let mut ctx = UiContext;

        prompt.handle_ui_event(&UiEvent::Paste("renamed_symbol".to_string()), &mut ctx);

        let result = prompt.handle_ui_event(&UiEvent::Key(key(KeyCode::Enter)), &mut ctx);
        let intents = result.into_intents();

        assert_eq!(intents.len(), 1);
        assert!(matches!(
            &intents[0],
            Intent::Command(Command::LspRename(name)) if name == "renamed_symbol"
        ));
    }

    #[test]
    fn esc_cancels_prompt() {
        let mut prompt = LspRenamePrompt::new("existing_name");
        let mut ctx = UiContext;

        let result = prompt.handle_ui_event(&UiEvent::Key(key(KeyCode::Esc)), &mut ctx);

        assert!(result.into_intents().is_empty());
        assert!(!prompt.is_open());
    }
}
