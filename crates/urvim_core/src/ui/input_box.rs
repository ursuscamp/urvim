//! Generic single-line input dialog widget.

use crate::screen::Screen;
use crate::ui::inputs::InputWidget;
use crate::ui::overlay::frame::{
    OverlayAnchor, OverlayFrame, OverlayFrameLabel, OverlayMargins, OverlayPlacement,
};
use crate::ui::{FocusPolicy, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use urvim_terminal::{KeyCode, Style};

const MAX_INPUT_CONTENT_WIDTH: usize = 55;

/// Stable identity for one plugin-owned input box instance.
pub type PluginInputId = u64;

/// Cancellation emitted when a plugin-owned input box closes without submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInputCancelled {
    /// Plugin that owns the input box.
    pub plugin: String,
    /// Input box instance identity.
    pub input_id: PluginInputId,
}

/// Result produced when an input box closes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputBoxOutcome {
    /// The user submitted the exact input text.
    Submitted(String),
    /// The user dismissed the input box.
    Cancelled,
}

/// A centered, framed single-line text input dialog.
#[derive(Debug)]
pub struct InputBox {
    title: String,
    input: InputWidget,
    open: bool,
    outcome: Option<InputBoxOutcome>,
}

impl InputBox {
    /// Creates an open input box with configurable title, prompt, and initial text.
    pub fn new(
        title: impl Into<String>,
        prompt: impl Into<String>,
        initial_text: impl Into<String>,
    ) -> Self {
        let mut input = InputWidget::new(initial_text);
        input.set_prompt(prompt);
        Self {
            title: title.into(),
            input,
            open: true,
            outcome: None,
        }
    }

    /// Returns true while the input box is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns the current input text.
    pub fn text(&self) -> &str {
        self.input.text()
    }

    /// Returns the rendered cursor position, if available.
    pub fn cursor(&self) -> Option<crate::ui::geometry::Position> {
        self.input.render_cursor()
    }

    /// Takes the submit or cancellation outcome after the input box closes.
    pub fn take_outcome(&mut self) -> Option<InputBoxOutcome> {
        self.outcome.take()
    }

    /// Handles an input event.
    pub fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        if !self.open {
            return UiEventResult::NotHandled;
        }

        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Esc => self.close(InputBoxOutcome::Cancelled),
                KeyCode::Enter => {
                    self.close(InputBoxOutcome::Submitted(self.input.text().to_string()))
                }
                _ if self.input.handle_key(*key) => UiEventResult::Handled(Vec::new()),
                _ => UiEventResult::NotHandled,
            },
            UiEvent::Paste(text) => {
                self.input.insert_str(&normalize_single_line(text));
                UiEventResult::Handled(Vec::new())
            }
            UiEvent::Resize(_, _) | UiEvent::Tick => UiEventResult::NotHandled,
        }
    }

    /// Renders the input box into the provided rectangle.
    pub fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        self.input.set_text_style(theme_style("ui.window"));

        if !self.open || rect.size.rows < 3 || rect.size.cols < 3 {
            return;
        }

        let border_style = theme_style("ui.window.lines.border");
        let body_style = theme_style("ui.window");
        let content_width = rect.size.cols.min(MAX_INPUT_CONTENT_WIDTH as u16);
        let frame = OverlayFrame::resolve_placement(
            rect.origin,
            rect.size,
            1,
            content_width.saturating_sub(2),
            OverlayPlacement::Anchored {
                anchor: OverlayAnchor::Center,
                margins: OverlayMargins::default(),
            },
        );
        let Some(frame) = frame else {
            return;
        };

        frame.render_bordered_with_label(
            screen,
            border_style,
            body_style,
            Some(OverlayFrameLabel::top_center(self.title.as_str())),
        );
        self.input.render_widget(
            screen,
            UiRect::new(frame.content_origin, frame.content_size),
            &UiContext,
        );
    }

    fn close(&mut self, outcome: InputBoxOutcome) -> UiEventResult {
        self.open = false;
        self.outcome = Some(outcome);
        UiEventResult::Handled(Vec::new())
    }
}

impl Widget for InputBox {
    fn handle_ui_event(&mut self, event: &UiEvent, ctx: &mut UiContext) -> UiEventResult {
        InputBox::handle_ui_event(self, event, ctx)
    }

    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, ctx: &UiContext) {
        InputBox::render_widget(self, screen, rect, ctx)
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Focusable
    }
}

fn normalize_single_line(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut in_line_break = false;
    for character in text.chars() {
        if matches!(character, '\r' | '\n') {
            if !in_line_break {
                normalized.push(' ');
                in_line_break = true;
            }
        } else {
            normalized.push(character);
            in_line_break = false;
        }
    }
    normalized
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
    use urvim_terminal::{Key, Modifiers};

    fn key(code: KeyCode) -> Key {
        Key {
            code,
            modifiers: Modifiers::default(),
        }
    }

    #[test]
    fn submit_preserves_exact_empty_text() {
        let mut input = InputBox::new("Input", "", "");

        let result = input.handle_ui_event(&UiEvent::Key(key(KeyCode::Enter)), &mut UiContext);

        assert!(result.handled());
        assert_eq!(
            input.take_outcome(),
            Some(InputBoxOutcome::Submitted(String::new()))
        );
    }

    #[test]
    fn escape_cancels() {
        let mut input = InputBox::new("Input", "", "value");

        input.handle_ui_event(&UiEvent::Key(key(KeyCode::Esc)), &mut UiContext);

        assert_eq!(input.take_outcome(), Some(InputBoxOutcome::Cancelled));
    }

    #[test]
    fn paste_normalizes_line_break_runs_to_spaces() {
        let mut input = InputBox::new("Input", "", "start:");

        input.handle_ui_event(
            &UiEvent::Paste("one\r\ntwo\n\nthree".to_string()),
            &mut UiContext,
        );

        assert_eq!(input.text(), "start:one two three");
    }
}
