//! Reusable binary confirmation prompt widget.
//!
//! This module provides a small modal-style widget that asks the user to
//! choose between two caller-defined responses.

use crate::screen::Screen;
use crate::ui::floating_window::{
    FloatingAnchor, FloatingMargins, FloatingPlacement, FloatingWindowFrame,
    FloatingWindowFrameLabel,
};
use crate::ui::{FocusPolicy, Intent, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use urvim_terminal::{KeyCode, Style};

const MAX_PROMPT_CONTENT_WIDTH: usize = 56;

/// Numeric identity of a plugin confirmation instance.
pub type PluginConfirmationId = u64;

/// Response selected from a plugin confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginConfirmationSelection {
    /// The primary response was selected.
    Primary,
    /// The secondary response was selected.
    Secondary,
}

/// Dismissal emitted when a plugin confirmation closes without a response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginConfirmationCancelled {
    /// Plugin that owns the confirmation.
    pub plugin: String,
    /// Confirmation instance that was dismissed.
    pub confirmation_id: PluginConfirmationId,
}

/// One selectable response in a confirmation prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmationResponse {
    label: String,
    key: char,
    intent: Option<Intent>,
}

impl ConfirmationResponse {
    /// Creates a response with a display label, shortcut key, and optional intent.
    pub fn new(label: impl Into<String>, key: char, intent: Option<impl Into<Intent>>) -> Self {
        Self {
            label: label.into(),
            key,
            intent: intent.map(Into::into),
        }
    }

    /// Returns the response label.
    pub fn label(&self) -> &str {
        self.label.as_str()
    }

    /// Returns the response shortcut key.
    pub fn key(&self) -> char {
        self.key
    }
}

/// Reusable binary confirmation prompt state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmationBox {
    title: String,
    query: String,
    primary: ConfirmationResponse,
    secondary: ConfirmationResponse,
    open: bool,
}

impl ConfirmationBox {
    /// Creates a confirmation prompt with two selectable responses.
    pub fn new(
        title: impl Into<String>,
        query: impl Into<String>,
        primary: ConfirmationResponse,
        secondary: ConfirmationResponse,
    ) -> Self {
        Self {
            title: title.into(),
            query: query.into(),
            primary,
            secondary,
            open: true,
        }
    }

    /// Creates a conventional Yes/No prompt that emits an intent for Yes.
    pub fn yes_no(query: impl Into<String>, positive_intent: impl Into<Intent>) -> Self {
        Self::new(
            "Confirm",
            query,
            ConfirmationResponse::new("Yes", 'y', Some(positive_intent)),
            ConfirmationResponse::new("No", 'n', None::<Intent>),
        )
    }

    /// Returns the prompt title.
    pub fn title(&self) -> &str {
        self.title.as_str()
    }

    /// Returns the prompt query text.
    pub fn query(&self) -> &str {
        self.query.as_str()
    }

    /// Returns true when the confirmation prompt is active.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Handles a UI event while the prompt is open.
    pub fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        if !self.open {
            return UiEventResult::NotHandled;
        }

        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Enter => self.select_primary(),
                KeyCode::Esc => self.dismiss(),
                KeyCode::Char(ch)
                    if !key.modifiers.has_ctrl()
                        && !key.modifiers.has_alt()
                        && ch.eq_ignore_ascii_case(&self.primary.key) =>
                {
                    self.select_primary()
                }
                KeyCode::Char(ch)
                    if !key.modifiers.has_ctrl()
                        && !key.modifiers.has_alt()
                        && ch.eq_ignore_ascii_case(&self.secondary.key) =>
                {
                    self.select_secondary()
                }
                _ => UiEventResult::NotHandled,
            },
            UiEvent::Paste(_) => UiEventResult::Handled(Vec::new()),
            UiEvent::Resize(_, _) | UiEvent::Tick => UiEventResult::NotHandled,
        }
    }

    /// Renders the prompt into the provided rectangle.
    pub fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        if !self.open || rect.size.rows < 3 || rect.size.cols < 3 {
            return;
        }

        let border_style = theme_style("ui.window.lines.border");
        let body_style = theme_style("ui.window");
        let prompt_lines = wrap_prompt_text(self.query.as_str(), MAX_PROMPT_CONTENT_WIDTH);
        let response_line = format!(
            "{} / {}",
            format_response(&self.primary),
            format_response(&self.secondary)
        );
        let content_width = prompt_lines
            .iter()
            .map(|line| UnicodeWidthStr::width(line.as_str()))
            .chain(std::iter::once(UnicodeWidthStr::width(
                response_line.as_str(),
            )))
            .max()
            .unwrap_or(0)
            .min(usize::from(rect.size.cols.saturating_sub(2)));
        if content_width == 0 {
            return;
        }

        let content_height = prompt_lines
            .len()
            .saturating_add(1)
            .min(usize::from(rect.size.rows.saturating_sub(2)));
        if content_height < 2 {
            return;
        }

        let frame = FloatingWindowFrame::resolve_placement(
            rect.origin,
            rect.size,
            content_height as u16,
            content_width as u16,
            FloatingPlacement::Anchored {
                anchor: FloatingAnchor::Center,
                margins: FloatingMargins::default(),
            },
        );
        let Some(frame) = frame else {
            return;
        };

        frame.render_bordered_with_label(
            screen,
            border_style,
            body_style,
            Some(FloatingWindowFrameLabel::top_center(self.title.as_str())),
        );
        for (line_idx, line) in prompt_lines
            .iter()
            .take(content_height.saturating_sub(1))
            .enumerate()
        {
            let row = frame.content_origin.row + line_idx as u16;
            write_centered_line(
                screen,
                row,
                frame.content_origin.col,
                frame.content_size.cols,
                body_style,
                line.as_str(),
            );
        }

        let response_row = frame.content_origin.row + content_height as u16 - 1;
        write_centered_line(
            screen,
            response_row,
            frame.content_origin.col,
            frame.content_size.cols,
            body_style,
            response_line.as_str(),
        );
    }

    fn select_primary(&mut self) -> UiEventResult {
        self.select(self.primary.intent.clone())
    }

    fn select_secondary(&mut self) -> UiEventResult {
        self.select(self.secondary.intent.clone())
    }

    fn select(&mut self, intent: Option<Intent>) -> UiEventResult {
        self.open = false;
        UiEventResult::Handled(intent.into_iter().collect())
    }

    fn dismiss(&mut self) -> UiEventResult {
        self.open = false;
        UiEventResult::Handled(Vec::new())
    }
}

fn format_response(response: &ConfirmationResponse) -> String {
    let matching_char = response
        .label
        .char_indices()
        .find(|(_, ch)| ch.eq_ignore_ascii_case(&response.key));
    let Some((index, ch)) = matching_char else {
        return format!("[{}] {}", response.key, response.label);
    };
    let after = index + ch.len_utf8();
    format!(
        "{}[{ch}]{}",
        &response.label[..index],
        &response.label[after..]
    )
}

impl Widget for ConfirmationBox {
    fn handle_ui_event(&mut self, event: &UiEvent, ctx: &mut UiContext) -> UiEventResult {
        ConfirmationBox::handle_ui_event(self, event, ctx)
    }

    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, ctx: &UiContext) {
        ConfirmationBox::render_widget(self, screen, rect, ctx)
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Passive
    }
}

fn theme_style(name: &str) -> Style {
    crate::globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default(name))
            .unwrap_or_default()
    })
}

fn write_centered_line(
    screen: &mut Screen,
    row: u16,
    content_origin_col: u16,
    content_width: u16,
    style: Style,
    text: &str,
) {
    let text_width = UnicodeWidthStr::width(text) as u16;
    let left_pad = content_width.saturating_sub(text_width) / 2;
    screen.write_string(row, content_origin_col + left_pad, style, text);
}

fn wrap_prompt_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return Vec::new();
    }

    let mut result = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            result.push(String::new());
            continue;
        }

        let graphemes = paragraph
            .grapheme_indices(true)
            .map(|(start_byte, grapheme)| GraphemeSlice {
                start_byte,
                width: UnicodeWidthStr::width(grapheme),
                is_whitespace: grapheme.chars().all(char::is_whitespace),
            })
            .collect::<Vec<_>>();

        if graphemes.is_empty() {
            result.push(String::new());
            continue;
        }

        let mut start = 0usize;
        while start < graphemes.len() {
            let mut width = 0usize;
            let mut end = start;
            let mut last_soft_break = None;

            while end < graphemes.len() {
                let grapheme = graphemes[end];
                let next_width = width + grapheme.width;
                if next_width > max_width {
                    if end == start {
                        end += 1;
                    }
                    break;
                }

                width = next_width;
                end += 1;

                if end < graphemes.len()
                    && graphemes[end - 1].is_whitespace != graphemes[end].is_whitespace
                {
                    last_soft_break = Some(end);
                }
            }

            let segment_end = if end < graphemes.len() {
                last_soft_break
                    .filter(|break_idx| *break_idx > start)
                    .unwrap_or(end)
            } else {
                graphemes.len()
            };

            let start_byte = graphemes[start].start_byte;
            let end_byte = if segment_end < graphemes.len() {
                graphemes[segment_end].start_byte
            } else {
                paragraph.len()
            };
            result.push(paragraph[start_byte..end_byte].to_string());
            start = segment_end;
        }
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}

#[derive(Debug, Clone, Copy)]
struct GraphemeSlice {
    start_byte: usize,
    width: usize,
    is_whitespace: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Screen;
    use crate::ui::{Intent, UiContext, UiEvent};
    use crate::window::{Position, Size};
    use urvim_terminal::{Key, KeyCode, Modifiers};

    fn key(code: KeyCode) -> Key {
        Key {
            code,
            modifiers: Modifiers::default(),
        }
    }

    #[test]
    fn query_and_open_state_are_exposed() {
        let prompt = ConfirmationBox::yes_no("Quit?", Intent::Command(crate::ui::Command::Quit));
        assert_eq!(prompt.query(), "Quit?");
        assert!(prompt.is_open());
    }

    #[test]
    fn enter_returns_positive_intent() {
        let mut prompt =
            ConfirmationBox::yes_no("Quit?", Intent::Command(crate::ui::Command::Quit));
        let mut ctx = UiContext;
        let result = prompt.handle_ui_event(&UiEvent::Key(key(KeyCode::Enter)), &mut ctx);
        assert!(matches!(result, UiEventResult::Handled(_)));
        assert_eq!(
            result.into_intents(),
            vec![Intent::Command(crate::ui::Command::Quit)]
        );
        assert!(!prompt.is_open());
    }

    #[test]
    fn y_and_n_keys_confirm_and_cancel() {
        let mut yes_prompt =
            ConfirmationBox::yes_no("Quit?", Intent::Command(crate::ui::Command::Quit));
        let mut ctx = UiContext;
        let result = yes_prompt.handle_ui_event(&UiEvent::Key(key(KeyCode::Char('y'))), &mut ctx);
        assert_eq!(
            result.into_intents(),
            vec![Intent::Command(crate::ui::Command::Quit)]
        );

        let mut no_prompt =
            ConfirmationBox::yes_no("Quit?", Intent::Command(crate::ui::Command::Quit));
        let result = no_prompt.handle_ui_event(&UiEvent::Key(key(KeyCode::Char('N'))), &mut ctx);
        assert!(result.into_intents().is_empty());
        assert!(!no_prompt.is_open());
    }

    #[test]
    fn escapes_and_other_inputs_are_handled_without_intents() {
        let mut prompt =
            ConfirmationBox::yes_no("Quit?", Intent::Command(crate::ui::Command::Quit));
        let mut ctx = UiContext;
        let result = prompt.handle_ui_event(&UiEvent::Key(key(KeyCode::Esc)), &mut ctx);
        assert!(result.into_intents().is_empty());

        let mut prompt =
            ConfirmationBox::yes_no("Quit?", Intent::Command(crate::ui::Command::Quit));
        let result = prompt.handle_ui_event(&UiEvent::Paste("ignored".to_string()), &mut ctx);
        assert!(result.into_intents().is_empty());

        let mut prompt =
            ConfirmationBox::yes_no("Quit?", Intent::Command(crate::ui::Command::Quit));
        assert_eq!(
            prompt.handle_ui_event(&UiEvent::Key(key(KeyCode::F7)), &mut ctx),
            UiEventResult::NotHandled
        );
    }

    #[test]
    fn render_centers_query_and_separates_responses_on_the_next_line() {
        let mut prompt =
            ConfirmationBox::yes_no("Quit?", Intent::Command(crate::ui::Command::Quit));
        let mut screen = Screen::new(8, 40);
        let ctx = UiContext;
        prompt.render_widget(
            &mut screen,
            UiRect::new(Position::new(0, 0), Size::new(8, 40)),
            &ctx,
        );

        assert_eq!(screen.get_cell_mut(3, 17).unwrap().text, "Q");
        assert_eq!(screen.get_cell_mut(4, 14).unwrap().text, "[");
    }

    #[test]
    fn custom_responses_emit_distinct_intents_and_render_shortcuts() {
        let mut prompt = ConfirmationBox::new(
            "Delete",
            "Delete this file?",
            ConfirmationResponse::new(
                "Delete",
                'd',
                Some(Intent::Command(crate::ui::Command::Quit)),
            ),
            ConfirmationResponse::new(
                "Keep",
                'k',
                Some(Intent::Command(crate::ui::Command::OpenCommandLine)),
            ),
        );
        let mut ctx = UiContext;
        let result = prompt.handle_ui_event(&UiEvent::Key(key(KeyCode::Char('K'))), &mut ctx);
        assert_eq!(
            result.into_intents(),
            vec![Intent::Command(crate::ui::Command::OpenCommandLine)]
        );

        let mut prompt = ConfirmationBox::new(
            "Delete",
            "Delete this file?",
            ConfirmationResponse::new(
                "Delete",
                'd',
                Some(Intent::Command(crate::ui::Command::Quit)),
            ),
            ConfirmationResponse::new("Keep", 'k', None::<Intent>),
        );
        let mut screen = Screen::new(8, 40);
        prompt.render_widget(
            &mut screen,
            UiRect::new(Position::new(0, 0), Size::new(8, 40)),
            &ctx,
        );
        let rendered = (0..40)
            .map(|col| screen.get_cell_mut(4, col).unwrap().text.clone())
            .collect::<String>();
        assert!(rendered.contains("[D]elete / [K]eep"));
    }
}
