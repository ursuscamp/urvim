//! LSP rename prompt widget.

use crate::screen::Screen;
use crate::terminal::{KeyCode, Style};
use crate::ui::floating_window::{FloatingAnchor, FloatingWindowFrame, FloatingWindowFrameLabel};
use crate::ui::inputs::InputWidget;
use crate::ui::{Command, FocusPolicy, Intent, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;

const MAX_PROMPT_CONTENT_WIDTH: usize = 55;

/// Dedicated rename prompt used for LSP rename input.
#[derive(Debug)]
pub struct LspRenamePrompt {
    input: InputWidget,
    open: bool,
}

impl LspRenamePrompt {
    /// Creates a new rename prompt.
    pub fn new(initial_text: impl Into<String>) -> Self {
        let mut input = InputWidget::new(initial_text);
        input.set_prompt(" ");
        Self { input, open: true }
    }

    /// Returns true when the prompt is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns the current input text.
    pub fn text(&self) -> &str {
        self.input.text()
    }

    /// Returns the rendered cursor position, if available.
    pub fn cursor(&self) -> Option<crate::window::Position> {
        self.input.render_cursor()
    }

    /// Handles a UI event while the prompt is open.
    pub fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        if !self.open {
            return UiEventResult::NotHandled;
        }

        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Esc => self.cancel(),
                KeyCode::Enter => self.submit(),
                _ if self.input.handle_key(*key) => UiEventResult::Handled(Vec::new()),
                _ => UiEventResult::NotHandled,
            },
            UiEvent::Paste(text) => {
                self.input.insert_str(text);
                UiEventResult::Handled(Vec::new())
            }
            UiEvent::Resize(_, _) | UiEvent::Tick => UiEventResult::NotHandled,
        }
    }

    /// Renders the prompt into the provided rectangle.
    pub fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        self.input.set_text_style(theme_style("ui.window"));

        if !self.open || rect.size.rows < 3 || rect.size.cols < 3 {
            return;
        }

        let border_style = theme_style("ui.window.lines.border");
        let body_style = theme_style("ui.window");
        let content_width = rect.size.cols.min(MAX_PROMPT_CONTENT_WIDTH as u16);
        let content_height = 1;
        let frame = FloatingWindowFrame::resolve(
            rect.origin,
            rect.size,
            content_height,
            content_width.saturating_sub(2),
            FloatingAnchor::Center,
        );
        let Some(frame) = frame else {
            return;
        };

        frame.render_bordered_with_label(
            screen,
            border_style,
            body_style,
            Some(FloatingWindowFrameLabel::top_center("Rename")),
        );
        self.input.render_widget(
            screen,
            UiRect::new(frame.content_origin, frame.content_size),
            &UiContext,
        );
    }

    fn submit(&mut self) -> UiEventResult {
        let text = self.input.text().trim().to_string();
        self.open = false;
        if text.is_empty() {
            return UiEventResult::Handled(Vec::new());
        }

        UiEventResult::Handled(vec![Intent::Command(Command::LspRename(text))])
    }

    fn cancel(&mut self) -> UiEventResult {
        self.open = false;
        UiEventResult::Handled(Vec::new())
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

fn theme_style(name: &str) -> Style {
    crate::globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default(name))
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::{Key, KeyCode, Modifiers};
    use crate::ui::{Intent, UiContext, UiEvent};

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
        prompt.input.insert_str("renamed_symbol");
        let mut ctx = UiContext;

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
