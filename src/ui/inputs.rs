//! Reusable input widget.

use crate::screen::Screen;
use crate::terminal::{Key, KeyCode, Style};
use crate::ui::{FocusPolicy, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use crate::window::Position;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

type ChangeCallback = Box<dyn FnMut(&str) + 'static>;
type KeyOverride = Box<dyn FnMut(Key) -> bool + 'static>;

/// A styled prompt segment rendered before the editable text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSegment {
    /// Segment text.
    pub text: String,
    /// Segment style.
    pub style: Style,
}

impl PromptSegment {
    /// Creates a new prompt segment.
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// A rendered line segment for the prompt or input text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineSegment {
    /// Segment text.
    pub text: String,
    /// Segment style.
    pub style: Style,
}

impl LineSegment {
    /// Creates a rendered line segment.
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// Shell-style one-line input state and editing logic.
#[derive(Default)]
pub struct InputWidget {
    text: String,
    cursor: usize,
    prompt: Vec<PromptSegment>,
    right_prompt: Vec<PromptSegment>,
    text_style: Style,
    render_cursor: Option<Position>,
    on_change: Option<ChangeCallback>,
    key_override: Option<KeyOverride>,
}

impl std::fmt::Debug for InputWidget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputWidget")
            .field("text", &self.text)
            .field("cursor", &self.cursor)
            .field("prompt", &self.prompt)
            .field("right_prompt", &self.right_prompt)
            .finish()
    }
}

impl InputWidget {
    /// Creates a new input widget with the provided initial text.
    pub fn new(initial_text: impl Into<String>) -> Self {
        let text = initial_text.into();
        let cursor = text.len();
        Self {
            text,
            cursor,
            prompt: Vec::new(),
            right_prompt: Vec::new(),
            text_style: Style::default(),
            render_cursor: None,
            on_change: None,
            key_override: None,
        }
    }

    /// Returns the current editable text.
    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    /// Returns the cursor byte index.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Returns the display prompt prefix.
    pub fn prompt(&self) -> String {
        self.prompt_text()
    }

    /// Returns the styled prompt segments.
    pub fn prompt_segments(&self) -> &[PromptSegment] {
        self.prompt.as_slice()
    }

    /// Returns the styled right-side prompt segments.
    pub fn right_prompt_segments(&self) -> &[PromptSegment] {
        self.right_prompt.as_slice()
    }

    /// Returns the rendered cursor position, if the widget has been drawn.
    pub fn render_cursor(&self) -> Option<Position> {
        self.render_cursor
    }

    /// Sets the style used for the editable text.
    pub fn set_text_style(&mut self, style: Style) {
        self.text_style = style;
    }

    /// Sets the display prompt prefix.
    pub fn set_prompt(&mut self, prompt: impl Into<String>) {
        let prompt = prompt.into();
        self.prompt = vec![PromptSegment::new(prompt, Style::default())];
    }

    /// Sets the styled prompt segments.
    pub fn set_prompt_segments(&mut self, prompt: Vec<PromptSegment>) {
        self.prompt = prompt;
    }

    /// Sets the display right-side prompt prefix.
    pub fn set_right_prompt(&mut self, prompt: impl Into<String>) {
        let prompt = prompt.into();
        self.right_prompt = vec![PromptSegment::new(prompt, Style::default())];
    }

    /// Sets the styled right-side prompt segments.
    pub fn set_right_prompt_segments(&mut self, prompt: Vec<PromptSegment>) {
        self.right_prompt = prompt;
    }

    /// Replaces the current text and moves the cursor to the end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }

        self.text = text;
        self.cursor = self.text.len();
        self.notify_change();
    }

    /// Clears the current text.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }

    /// Inserts text at the current cursor position.
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        self.text.insert_str(self.cursor, text);
        self.cursor = self.cursor.saturating_add(text.len());
        self.notify_change();
    }

    /// Sets a callback invoked after text changes.
    pub fn set_on_change(&mut self, on_change: impl FnMut(&str) + 'static) {
        self.on_change = Some(Box::new(on_change));
    }

    /// Sets a callback that can intercept any key before built-in handling.
    pub fn set_key_override(&mut self, key_override: impl FnMut(Key) -> bool + 'static) {
        self.key_override = Some(Box::new(key_override));
    }

    /// Removes any registered key override callback.
    pub fn clear_key_override(&mut self) {
        self.key_override = None;
    }

    /// Handles a key press using consumer overrides, built-in shell keys, and normal insertion.
    pub fn handle_key(&mut self, key: Key) -> bool {
        if self
            .key_override
            .as_mut()
            .is_some_and(|override_handler| override_handler(key))
        {
            return true;
        }

        match key.code {
            KeyCode::Backspace => {
                self.delete_prev_grapheme();
                true
            }
            KeyCode::Delete => {
                self.delete_next_grapheme();
                true
            }
            KeyCode::Home => {
                self.cursor = 0;
                true
            }
            KeyCode::Char('a') if key.modifiers.has_ctrl() => {
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.text.len();
                true
            }
            KeyCode::Char('e') if key.modifiers.has_ctrl() => {
                self.cursor = self.text.len();
                true
            }
            KeyCode::Left if key.modifiers.has_alt() || key.modifiers.has_ctrl() => {
                self.move_prev_word();
                true
            }
            KeyCode::Left => {
                self.move_prev_grapheme();
                true
            }
            KeyCode::Right if key.modifiers.has_alt() || key.modifiers.has_ctrl() => {
                self.move_next_word();
                true
            }
            KeyCode::Right => {
                self.move_next_grapheme();
                true
            }
            KeyCode::Char('b') if key.modifiers.has_ctrl() => {
                self.move_prev_grapheme();
                true
            }
            KeyCode::Char('f') if key.modifiers.has_ctrl() => {
                self.move_next_grapheme();
                true
            }
            KeyCode::Char('w') if key.modifiers.has_ctrl() => {
                self.delete_prev_word();
                true
            }
            KeyCode::Char('u') if key.modifiers.has_ctrl() => {
                if self.cursor > 0 {
                    self.text.drain(..self.cursor);
                    self.cursor = 0;
                    self.notify_change();
                }
                true
            }
            KeyCode::Char('b') if key.modifiers.has_alt() => {
                self.move_prev_word();
                true
            }
            KeyCode::Char('f') if key.modifiers.has_alt() => {
                self.move_next_word();
                true
            }
            KeyCode::Tab => {
                self.insert_str("\t");
                true
            }
            KeyCode::Enter | KeyCode::Esc => true,
            KeyCode::Char(ch) if !key.modifiers.has_ctrl() && !key.modifiers.has_alt() => {
                self.insert_char(ch);
                true
            }
            _ => false,
        }
    }

    /// Renders the prompt and visible text as styled segments.
    pub fn render_segments(&self, content_cols: u16, text_style: Style) -> (Vec<LineSegment>, u16) {
        let prompt_width = self
            .prompt
            .iter()
            .map(|segment| UnicodeWidthStr::width(segment.text.as_str()))
            .sum::<usize>() as u16;
        let right_prompt_width = self
            .right_prompt
            .iter()
            .map(|segment| UnicodeWidthStr::width(segment.text.as_str()))
            .sum::<usize>() as u16;
        let visible_text_cols =
            content_cols.saturating_sub(prompt_width.saturating_add(right_prompt_width));
        let (visible_text, cursor_col) =
            self.visible_text_with_cursor(usize::from(visible_text_cols));
        let visible_text_width = UnicodeWidthStr::width(visible_text.as_str()) as u16;

        let mut segments = self
            .prompt
            .iter()
            .cloned()
            .map(|segment| LineSegment::new(segment.text, text_style.accent(segment.style)))
            .collect::<Vec<_>>();
        if !visible_text.is_empty() {
            segments.push(LineSegment::new(visible_text, text_style));
        }

        if right_prompt_width > 0 {
            let gap_width = visible_text_cols.saturating_sub(visible_text_width);
            if gap_width > 0 {
                segments.push(LineSegment::new(
                    " ".repeat(usize::from(gap_width)),
                    text_style,
                ));
            }
        }

        segments.extend(
            self.right_prompt
                .iter()
                .cloned()
                .map(|segment| LineSegment::new(segment.text, text_style.accent(segment.style))),
        );

        let cursor_col = prompt_width.saturating_add(cursor_col);
        let right_edge = content_cols.saturating_sub(right_prompt_width);
        let cursor_col = if right_prompt_width > 0 {
            cursor_col.min(right_edge.saturating_sub(1))
        } else {
            cursor_col.min(content_cols)
        };

        (segments, cursor_col)
    }

    fn prompt_text(&self) -> String {
        self.prompt
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }

    fn notify_change(&mut self) {
        if let Some(on_change) = self.on_change.as_mut() {
            on_change(self.text.as_str());
        }
    }

    fn insert_char(&mut self, ch: char) {
        self.text.insert(self.cursor, ch);
        self.cursor = self.cursor.saturating_add(ch.len_utf8());
        self.notify_change();
    }

    fn delete_prev_grapheme(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let start = prev_grapheme_start(self.text.as_str(), self.cursor);
        if start < self.cursor {
            self.text.drain(start..self.cursor);
            self.cursor = start;
            self.notify_change();
        }
    }

    fn delete_next_grapheme(&mut self) {
        if self.cursor >= self.text.len() {
            return;
        }

        let end = next_grapheme_end(self.text.as_str(), self.cursor);
        if end > self.cursor {
            self.text.drain(self.cursor..end);
            self.notify_change();
        }
    }

    fn move_prev_grapheme(&mut self) {
        self.cursor = prev_grapheme_start(self.text.as_str(), self.cursor);
    }

    fn move_next_grapheme(&mut self) {
        self.cursor = next_grapheme_end(self.text.as_str(), self.cursor);
    }

    fn delete_prev_word(&mut self) {
        let start = prev_word_boundary(self.text.as_str(), self.cursor);
        if start < self.cursor {
            self.text.drain(start..self.cursor);
            self.cursor = start;
            self.notify_change();
        }
    }

    fn move_prev_word(&mut self) {
        self.cursor = prev_word_boundary(self.text.as_str(), self.cursor);
    }

    fn move_next_word(&mut self) {
        self.cursor = next_word_boundary(self.text.as_str(), self.cursor);
    }

    fn visible_text_with_cursor(&self, max_cols: usize) -> (String, u16) {
        if self.text.is_empty() || max_cols == 0 {
            return (String::new(), 0);
        }

        let spans = grapheme_spans(self.text.as_str());
        let total_width = spans.iter().map(|span| span.width).sum::<usize>();
        let cursor_width = spans
            .iter()
            .take_while(|span| span.end <= self.cursor)
            .map(|span| span.width)
            .sum::<usize>();

        if total_width <= max_cols {
            return (self.text.clone(), cursor_width as u16);
        }

        let max_start_col = if self.cursor == self.text.len() && max_cols > 1 {
            total_width.saturating_sub(max_cols.saturating_sub(1))
        } else {
            total_width.saturating_sub(max_cols)
        };
        let mut start_col = if cursor_width <= max_cols / 2 {
            0
        } else if cursor_width.saturating_add(max_cols / 2) >= total_width {
            max_start_col
        } else {
            cursor_width.saturating_sub(max_cols / 2)
        };
        start_col = start_col.min(max_start_col);

        let start_byte = byte_index_at_visual_col(&spans, start_col);
        let end_byte = byte_index_at_visual_col(&spans, start_col.saturating_add(max_cols));
        let visible = self.text[start_byte..end_byte].to_string();
        let cursor_col = cursor_width.saturating_sub(start_col);
        (visible, cursor_col as u16)
    }
}

impl InputWidget {
    /// Renders the prompt and visible text while reserving a cell for a block cursor at end-of-line.
    pub fn render_widget_with_cursor_padding(
        &mut self,
        screen: &mut Screen,
        rect: UiRect,
        _ctx: &UiContext,
    ) {
        self.render_cursor = None;
        if rect.size.rows == 0 || rect.size.cols == 0 {
            return;
        }

        let (segments, cursor_col) = self.render_segments(rect.size.cols, self.text_style);
        let mut col = rect.origin.col;
        for segment in segments {
            if col >= rect.origin.col.saturating_add(rect.size.cols) {
                break;
            }

            screen.write_string(rect.origin.row, col, segment.style, segment.text.as_str());
            col = col.saturating_add(UnicodeWidthStr::width(segment.text.as_str()) as u16);
        }

        self.render_cursor = Some(Position::new(
            rect.origin.row,
            rect.origin.col.saturating_add(cursor_col).min(
                rect.origin
                    .col
                    .saturating_add(rect.size.cols.saturating_sub(1)),
            ),
        ));
    }
}

impl Widget for InputWidget {
    fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        match event {
            UiEvent::Key(key) if self.handle_key(*key) => UiEventResult::Handled(Vec::new()),
            UiEvent::Paste(text) => {
                self.insert_str(text);
                UiEventResult::Handled(Vec::new())
            }
            _ => UiEventResult::NotHandled,
        }
    }

    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, ctx: &UiContext) {
        self.render_widget_with_cursor_padding(screen, rect, ctx);
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Focusable
    }
}

#[derive(Debug, Clone, Copy)]
struct GraphemeSpan {
    start: usize,
    end: usize,
    width: usize,
}

fn grapheme_spans(text: &str) -> Vec<GraphemeSpan> {
    text.grapheme_indices(true)
        .map(|(start, grapheme)| GraphemeSpan {
            start,
            end: start + grapheme.len(),
            width: UnicodeWidthStr::width(grapheme),
        })
        .collect()
}

fn byte_index_at_visual_col(spans: &[GraphemeSpan], target_col: usize) -> usize {
    let mut visual_col = 0usize;

    for span in spans {
        if target_col < visual_col.saturating_add(span.width) {
            return span.start;
        }
        visual_col = visual_col.saturating_add(span.width);
    }

    spans.last().map(|span| span.end).unwrap_or(0)
}

fn prev_grapheme_start(text: &str, cursor: usize) -> usize {
    text[..cursor]
        .grapheme_indices(true)
        .next_back()
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

fn next_grapheme_end(text: &str, cursor: usize) -> usize {
    if cursor >= text.len() {
        return text.len();
    }

    for (offset, _grapheme) in text[cursor..].grapheme_indices(true) {
        if offset == 0 {
            continue;
        }
        return cursor + offset;
    }

    text.len()
}

fn is_whitespace(grapheme: &str) -> bool {
    grapheme.chars().all(char::is_whitespace)
}

fn prev_word_boundary(text: &str, cursor: usize) -> usize {
    let mut pos = cursor;

    while pos > 0 {
        let start = prev_grapheme_start(text, pos);
        let grapheme = &text[start..pos];
        if !is_whitespace(grapheme) {
            break;
        }
        pos = start;
    }

    while pos > 0 {
        let start = prev_grapheme_start(text, pos);
        let grapheme = &text[start..pos];
        if is_whitespace(grapheme) {
            break;
        }
        pos = start;
    }

    pos
}

fn next_word_boundary(text: &str, cursor: usize) -> usize {
    let mut pos = cursor;

    while pos < text.len() {
        let next = next_grapheme_end(text, pos);
        let grapheme = &text[pos..next];
        if !is_whitespace(grapheme) {
            break;
        }
        pos = next;
    }

    while pos < text.len() {
        let next = next_grapheme_end(text, pos);
        let grapheme = &text[pos..next];
        if is_whitespace(grapheme) {
            break;
        }
        pos = next;
    }

    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Screen;
    use crate::terminal::{KeyCode, Modifiers, Style};
    use crate::ui::{UiContext, UiRect};
    use crate::widget::Widget;
    use crate::window::Position;

    #[test]
    fn inserts_and_moves_like_a_shell_line() {
        let mut input = InputWidget::new("");
        input.set_prompt(":");
        assert!(input.handle_key(KeyCode::Char('a').key()));
        assert!(input.handle_key(KeyCode::Char('b').key()));
        assert!(input.handle_key(KeyCode::Char('c').key()));
        assert_eq!(input.text(), "abc");

        assert!(input.handle_key(KeyCode::Left.key()));
        assert!(input.handle_key(KeyCode::Backspace.key()));
        assert_eq!(input.text(), "ac");
    }

    #[test]
    fn supports_word_editing_and_prompt_rendering() {
        let mut input = InputWidget::new("one two");
        input.set_prompt(">");
        input.handle_key(KeyCode::End.key());
        input.handle_key(KeyCode::Char('w').with_modifiers(Modifiers::CTRL));
        assert_eq!(input.text(), "one ");

        let mut screen = Screen::new(1, 8);
        let ctx = UiContext;
        input.render_widget(
            &mut screen,
            UiRect::new(Position::new(0, 0), crate::window::Size::new(1, 8)),
            &ctx,
        );

        assert_eq!(input.render_cursor(), Some(Position::new(0, 5)));
        let (segments, cursor_col) = input.render_segments(8, Style::default());
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, ">");
        assert_eq!(segments[1].text, "one ");
        assert_eq!(cursor_col, 5);
    }

    #[test]
    fn supports_styled_prompt_segments() {
        let mut input = InputWidget::new("abc");
        input.set_prompt_segments(vec![
            PromptSegment::new("Exact", Style::new().bold()),
            PromptSegment::new(" > ", Style::new().faint()),
        ]);

        let base = Style::new().fg(crate::terminal::Color::ansi(14));
        let (segments, cursor_col) = input.render_segments(16, base);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].text, "Exact");
        assert_eq!(segments[1].text, " > ");
        assert_eq!(segments[2].text, "abc");
        assert_eq!(segments[0].style, base.accent(Style::new().bold()));
        assert_eq!(segments[1].style, base.accent(Style::new().faint()));
        assert_eq!(segments[2].style, base);
        assert_eq!(cursor_col, 11);
    }

    #[test]
    fn supports_right_prompt_rendering() {
        let mut input = InputWidget::new("abc");
        input.set_prompt(">");
        input.set_right_prompt_segments(vec![PromptSegment::new("2/3", Style::new().bold())]);

        let (segments, cursor_col) = input.render_segments(12, Style::default());

        assert_eq!(segments.len(), 4);
        assert_eq!(segments[0].text, ">");
        assert_eq!(segments[1].text, "abc");
        assert_eq!(segments[2].text, "     ");
        assert_eq!(segments[3].text, "2/3");
        assert_eq!(cursor_col, 4);
    }

    #[test]
    fn keeps_the_cursor_after_scrolled_end_of_line() {
        let mut input = InputWidget::new("abcdefghij");
        input.set_prompt(">");
        input.handle_key(KeyCode::End.key());

        let mut screen = Screen::new(1, 8);
        let ctx = UiContext;
        input.render_widget(
            &mut screen,
            UiRect::new(Position::new(0, 0), crate::window::Size::new(1, 8)),
            &ctx,
        );

        let cursor = input.render_cursor().expect("cursor should render");
        assert_eq!(cursor, Position::new(0, 7));
        assert_eq!(screen.get_cell_mut(0, 7).unwrap().text, " ");
    }

    #[test]
    fn override_callback_can_consume_keys() {
        let mut input = InputWidget::new("");
        input.set_key_override(|key| matches!(key.code, KeyCode::Enter));

        assert!(input.handle_key(KeyCode::Enter.key()));
        assert_eq!(input.text(), "");
    }
}
