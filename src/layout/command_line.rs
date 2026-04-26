//! Command-line overlay state, parsing, and execution.

use super::Layout;
use crate::notification::NotificationLevel;
use crate::terminal::{Key, KeyCode};
use crate::ui::{Command, Intent, UiEventResult};
use std::io;
use std::path::Path;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLineState {
    input: String,
    history: Vec<String>,
    history_index: Option<usize>,
    history_draft: String,
}

impl CommandLineState {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
        }
    }

    pub fn input(&self) -> &str {
        self.input.as_str()
    }

    pub fn input_mut(&mut self) -> &mut String {
        &mut self.input
    }

    pub fn push_history(&mut self, command: String) {
        if command.trim().is_empty() {
            self.history_index = None;
            self.history_draft.clear();
            return;
        }

        self.history.push(command);
        self.history_index = None;
        self.history_draft.clear();
    }

    pub fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let next_index = match self.history_index {
            Some(index) => index.saturating_sub(1),
            None => {
                self.history_draft = self.input.clone();
                self.history.len() - 1
            }
        };

        self.history_index = Some(next_index);
        self.input = self.history[next_index].clone();
    }

    pub fn history_next(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };

        if index + 1 < self.history.len() {
            let next_index = index + 1;
            self.history_index = Some(next_index);
            self.input = self.history[next_index].clone();
            return;
        }

        self.history_index = None;
        self.input = self.history_draft.clone();
    }

    pub fn reset_input(&mut self) {
        self.input.clear();
        self.history_index = None;
        self.history_draft.clear();
    }
}

impl Default for CommandLineState {
    fn default() -> Self {
        Self::new()
    }
}

const COMMAND_LINE_PROMPT: &str = ":";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Save { path: Option<String> },
    Edit { path: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseCommandError {
    UnterminatedQuote,
    UnknownCommand(String),
    InvalidArity {
        command: String,
        expected: &'static str,
    },
}

impl ParseCommandError {
    pub fn message(&self) -> String {
        match self {
            ParseCommandError::UnterminatedQuote => {
                "Invalid command: unterminated quoted argument".to_string()
            }
            ParseCommandError::UnknownCommand(command) => format!("Unknown command: {command}"),
            ParseCommandError::InvalidArity { command, expected } => {
                format!("Invalid usage for {command}: expected {expected}")
            }
        }
    }
}

pub(super) fn command_line_render_text(input: &str, content_cols: u16) -> (String, u16) {
    let prompt_width = UnicodeWidthStr::width(COMMAND_LINE_PROMPT) as u16;
    let visible_input_cols = content_cols.saturating_sub(prompt_width);
    let (visible_input, visible_input_width) =
        visible_command_line_input(input, visible_input_cols);
    let rendered_text = format!("{COMMAND_LINE_PROMPT}{visible_input}");
    let rendered_width = prompt_width.saturating_add(visible_input_width);
    (rendered_text, rendered_width)
}

fn visible_command_line_input(input: &str, max_cols: u16) -> (String, u16) {
    if input.is_empty() || max_cols == 0 {
        return (String::new(), 0);
    }

    let mut start_byte = input.len();
    let mut visible_cols = 0u16;

    for (byte_idx, grapheme) in input.grapheme_indices(true).rev() {
        let width = UnicodeWidthStr::width(grapheme) as u16;
        if visible_cols > 0 && visible_cols.saturating_add(width) > max_cols {
            break;
        }

        start_byte = byte_idx;
        visible_cols = visible_cols.saturating_add(width);

        if visible_cols >= max_cols {
            break;
        }
    }

    (input[start_byte..].to_string(), visible_cols)
}

impl Layout {
    pub(super) fn open_command_line(&mut self) {
        self.command_line_open = true;
        self.command_line.reset_input();
    }

    pub(super) fn close_command_line(&mut self) {
        self.command_line_open = false;
        self.command_line_cursor = None;
    }

    pub fn command_line_is_open(&self) -> bool {
        self.command_line_open
    }

    pub(super) fn handle_command_line_key(&mut self, key: &Key) -> UiEventResult {
        if !self.command_line_open {
            return UiEventResult::NotHandled;
        }

        match key.code {
            KeyCode::Esc => {
                self.close_command_line();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Enter => {
                let command = self.command_line.input().trim().to_string();
                self.command_line.push_history(command.clone());
                self.close_command_line();

                if command.is_empty() {
                    return UiEventResult::Handled(Vec::new());
                }

                let intent = match self.execute_command_line(command.as_str()) {
                    Ok(()) => None,
                    Err(message) => Some(Intent::Command(Command::EnqueueNotification {
                        level: NotificationLevel::Error,
                        message,
                    })),
                };

                if let Some(intent) = intent {
                    UiEventResult::Handled(vec![intent])
                } else {
                    UiEventResult::Handled(Vec::new())
                }
            }
            KeyCode::Backspace => {
                self.command_line.input_mut().pop();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Up => {
                self.command_line.history_previous();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Down => {
                self.command_line.history_next();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Char('p') if key.modifiers.has_ctrl() => {
                self.command_line.history_previous();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Char('n') if key.modifiers.has_ctrl() => {
                self.command_line.history_next();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Char(ch) if !key.modifiers.has_ctrl() && !key.modifiers.has_alt() => {
                self.command_line.input_mut().push(ch);
                UiEventResult::Handled(Vec::new())
            }
            _ => UiEventResult::Handled(Vec::new()),
        }
    }

    pub(super) fn handle_command_line_paste(&mut self, text: &str) -> UiEventResult {
        if !self.command_line_open {
            return UiEventResult::NotHandled;
        }

        self.command_line.input_mut().push_str(text);
        UiEventResult::Handled(Vec::new())
    }

    fn execute_command_line(&mut self, input: &str) -> Result<(), String> {
        let parsed = parse_command_line(input).map_err(|error| error.message())?;
        self.execute_parsed_command(parsed)
    }

    fn execute_parsed_command(&mut self, command: ParsedCommand) -> Result<(), String> {
        match command {
            ParsedCommand::Save { path: None } => self.execute_save_current(),
            ParsedCommand::Save { path: Some(path) } => self.execute_save_as(path.as_str()),
            ParsedCommand::Edit { path: None } => {
                self.active_window_group_mut().open_unnamed_buffer_tab();
                Ok(())
            }
            ParsedCommand::Edit { path: Some(path) } => self.execute_edit_path(path.as_str()),
        }
    }

    fn execute_save_current(&mut self) -> Result<(), String> {
        let buffer_id = self.active_buffer_view().buffer_id();
        match crate::globals::with_buffer_pool(|pool| pool.save_buffer(buffer_id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                Err("Cannot write unnamed buffer; provide a path with :write <path>".to_string())
            }
            Err(error) => Err(format!("Failed to write buffer: {error}")),
        }
    }

    fn execute_save_as(&mut self, path: &str) -> Result<(), String> {
        if Path::new(path).exists() {
            return Err(format!("Cannot write: path already exists: {path}"));
        }

        let buffer_id = self.active_buffer_view().buffer_id();
        crate::globals::with_buffer_pool(|pool| pool.save_buffer_to_path(buffer_id, path))
            .map_err(|error| format!("Failed to write buffer to {path}: {error}"))
    }

    fn execute_edit_path(&mut self, path: &str) -> Result<(), String> {
        let buffer_id = crate::globals::with_buffer_pool(|pool| pool.open_buffer(path))
            .map_err(|error| format!("Failed to open {path}: {error}"))?;
        self.active_window_group_mut()
            .activate_or_open_buffer(buffer_id);
        Ok(())
    }

    pub(super) fn command_line_input(&self) -> Option<&str> {
        if self.command_line_open {
            return Some(self.command_line.input());
        }

        None
    }

    pub(super) fn command_line_should_capture_events(&self) -> bool {
        self.command_line_open
    }

    pub(super) fn set_command_line_cursor(&mut self, cursor: Option<crate::window::Position>) {
        self.command_line_cursor = cursor;
    }
}

pub fn parse_command_line(input: &str) -> Result<ParsedCommand, ParseCommandError> {
    let tokens = tokenize_command_line(input)?;
    if tokens.is_empty() {
        return Err(ParseCommandError::InvalidArity {
            command: "".to_string(),
            expected: "a command",
        });
    }

    let command = tokens[0].as_str();
    let args = &tokens[1..];
    match command {
        "write" => match args {
            [] => Ok(ParsedCommand::Save { path: None }),
            [path] => Ok(ParsedCommand::Save {
                path: Some(path.clone()),
            }),
            _ => Err(ParseCommandError::InvalidArity {
                command: "write".to_string(),
                expected: "write [path]",
            }),
        },
        "edit" => match args {
            [] => Ok(ParsedCommand::Edit { path: None }),
            [path] => Ok(ParsedCommand::Edit {
                path: Some(path.clone()),
            }),
            _ => Err(ParseCommandError::InvalidArity {
                command: "edit".to_string(),
                expected: "edit [path]",
            }),
        },
        other => Err(ParseCommandError::UnknownCommand(other.to_string())),
    }
}

fn tokenize_command_line(input: &str) -> Result<Vec<String>, ParseCommandError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut in_quote: Option<char> = None;

    while let Some(ch) = chars.next() {
        if let Some(quote) = in_quote {
            match ch {
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                value if value == quote => {
                    in_quote = None;
                }
                _ => current.push(ch),
            }
            continue;
        }

        match ch {
            '"' | '\'' => in_quote = Some(ch),
            ' ' | '\t' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if in_quote.is_some() {
        return Err(ParseCommandError::UnterminatedQuote);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::layout::Layout;
    use crate::terminal::{KeyCode, Modifiers};
    use crate::window_group::WindowGroup;

    #[test]
    fn parse_write_and_edit_commands() {
        assert_eq!(
            parse_command_line("write").expect("write command should parse"),
            ParsedCommand::Save { path: None }
        );
        assert_eq!(
            parse_command_line("write path.txt").expect("write path command should parse"),
            ParsedCommand::Save {
                path: Some("path.txt".to_string())
            }
        );
        assert_eq!(
            parse_command_line("edit").expect("edit command should parse"),
            ParsedCommand::Edit { path: None }
        );
        assert_eq!(
            parse_command_line("edit src/main.rs").expect("edit path command should parse"),
            ParsedCommand::Edit {
                path: Some("src/main.rs".to_string())
            }
        );
    }

    #[test]
    fn parse_supports_quoted_arguments() {
        assert_eq!(
            parse_command_line("edit \"notes/today file.txt\"")
                .expect("quoted path command should parse"),
            ParsedCommand::Edit {
                path: Some("notes/today file.txt".to_string())
            }
        );
    }

    #[test]
    fn parse_rejects_unknown_command() {
        let error = parse_command_line("save").expect_err("unknown command should fail");
        assert!(matches!(error, ParseCommandError::UnknownCommand(_)));
    }

    #[test]
    fn parse_rejects_invalid_arity() {
        let error = parse_command_line("write one two").expect_err("arity should fail");
        assert!(matches!(error, ParseCommandError::InvalidArity { .. }));
    }

    #[test]
    fn parse_rejects_unterminated_quote() {
        let error = parse_command_line("edit \"foo").expect_err("unterminated quote should fail");
        assert_eq!(error, ParseCommandError::UnterminatedQuote);
    }

    #[test]
    fn command_line_history_navigation_round_trip() {
        let mut state = CommandLineState::new();
        state.push_history("write".to_string());
        state.push_history("edit one.txt".to_string());
        state.push_history("edit two.txt".to_string());

        state.history_previous();
        assert_eq!(state.input(), "edit two.txt");
        state.history_previous();
        assert_eq!(state.input(), "edit one.txt");
        state.history_next();
        assert_eq!(state.input(), "edit two.txt");
        state.history_next();
        assert_eq!(state.input(), "");
    }

    #[test]
    fn action_kind_contains_open_command_line_variant() {
        let command = crate::ui::Command::OpenCommandLine;
        assert!(matches!(command, crate::ui::Command::OpenCommandLine));
    }

    #[test]
    fn enter_on_unknown_command_emits_error_and_closes_overlay() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
        layout.open_command_line();
        layout.command_line.input_mut().push_str("unknown");

        let result = layout.handle_command_line_key(&KeyCode::Enter.key());
        let intents = result.into_intents();

        assert!(!layout.command_line_is_open());
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            &intents[0],
            Intent::Command(Command::EnqueueNotification { level, message })
                if *level == NotificationLevel::Error && message.contains("Unknown command")
        ));
    }

    #[test]
    fn ctrl_p_and_ctrl_n_navigate_history() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
        layout.open_command_line();
        layout.command_line.push_history("write".to_string());
        layout.command_line.push_history("edit one.txt".to_string());

        let ctrl_p = KeyCode::Char('p').with_modifiers(Modifiers::CTRL);
        let ctrl_n = KeyCode::Char('n').with_modifiers(Modifiers::CTRL);

        layout.handle_command_line_key(&ctrl_p);
        assert_eq!(layout.command_line_input(), Some("edit one.txt"));

        layout.handle_command_line_key(&ctrl_p);
        assert_eq!(layout.command_line_input(), Some("write"));

        layout.handle_command_line_key(&ctrl_n);
        assert_eq!(layout.command_line_input(), Some("edit one.txt"));
    }

    #[test]
    fn command_line_render_text_keeps_a_fixed_visible_width() {
        let (rendered, rendered_width) = command_line_render_text("abcdefghijklmnopqrstuvwxyz", 10);

        assert_eq!(rendered, ":rstuvwxyz");
        assert_eq!(rendered_width, 10);
    }
}
