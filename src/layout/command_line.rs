//! Command-line overlay state, parsing, and execution.

use super::Layout;
use crate::background::{JobKind, JobToken};
use crate::command;
use crate::lsp::rename_job::LspRenameJob;
use crate::notification::NotificationLevel;
use crate::terminal::{Key, KeyCode};
use crate::ui::inputs::InputWidget;
use crate::ui::{Command, Intent, UiEventResult};
use lsp_types::DiagnosticSeverity;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub struct CommandLineState {
    input: InputWidget,
    history: Vec<String>,
    history_index: Option<usize>,
    history_draft: String,
    cursor: Option<crate::window::Position>,
}

impl CommandLineState {
    pub fn new() -> Self {
        let mut input = InputWidget::new("");
        input.set_prompt(":");
        Self {
            input,
            history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            cursor: None,
        }
    }

    pub fn input(&self) -> &str {
        self.input.text()
    }

    pub fn input_widget_mut(&mut self) -> &mut InputWidget {
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
                self.history_draft = self.input.text().to_string();
                self.history.len() - 1
            }
        };

        self.history_index = Some(next_index);
        self.input.set_text(self.history[next_index].as_str());
    }

    pub fn history_next(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };

        if index + 1 < self.history.len() {
            let next_index = index + 1;
            self.history_index = Some(next_index);
            self.input.set_text(self.history[next_index].as_str());
            return;
        }

        self.history_index = None;
        self.input.set_text(self.history_draft.as_str());
    }

    pub fn reset_input(&mut self) {
        self.input.clear();
        self.history_index = None;
        self.history_draft.clear();
    }

    pub fn cursor(&self) -> Option<crate::window::Position> {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: Option<crate::window::Position>) {
        self.cursor = cursor;
    }
}

impl Default for CommandLineState {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Save { path: Option<String> },
    WriteAll,
    Edit { path: Option<String> },
    PickFile,
    PickGit,
    PickGrep,
    PickColorscheme,
    PickDocumentSymbols,
    PickReferences,
    PickCodeActions,
    LspHover,
    LspDefinition,
    LspReferences,
    LspRename { name: Option<String> },
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseCommandError {
    UnterminatedQuote,
    UnknownCommand(String),
    InvalidArity {
        command: String,
        expected: &'static str,
    },
}

#[allow(dead_code)]
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

impl Layout {
    pub(super) fn open_command_line(&mut self) {
        self.close_all_dialogs();
        self.dialogs.command_line_open = true;
        self.dialogs.command_line.input_widget_mut().set_prompt(":");
        self.dialogs.command_line.reset_input();
    }

    pub(super) fn close_command_line(&mut self) {
        self.dialogs.command_line_open = false;
        self.dialogs.command_line.set_cursor(None);
    }

    pub fn command_line_is_open(&self) -> bool {
        self.dialogs.command_line_open
    }

    pub(super) fn handle_command_line_key(&mut self, key: &Key) -> UiEventResult {
        if !self.dialogs.command_line_open {
            return UiEventResult::NotHandled;
        }

        match key.code {
            KeyCode::Esc => {
                self.close_command_line();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Enter => {
                let command = self.dialogs.command_line.input().trim().to_string();
                self.dialogs.command_line.push_history(command.clone());
                self.close_command_line();

                if command.is_empty() {
                    return UiEventResult::Handled(Vec::new());
                }

                let intent = match self.execute_command_line(command.as_str()) {
                    Ok(intent) => Some(intent),
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
                self.dialogs
                    .command_line
                    .input_widget_mut()
                    .handle_key(key.clone());
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Up => {
                self.dialogs.command_line.history_previous();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Down => {
                self.dialogs.command_line.history_next();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Char('p') if key.modifiers.has_ctrl() => {
                self.dialogs.command_line.history_previous();
                UiEventResult::Handled(Vec::new())
            }
            KeyCode::Char('n') if key.modifiers.has_ctrl() => {
                self.dialogs.command_line.history_next();
                UiEventResult::Handled(Vec::new())
            }
            _ => {
                let _ = self
                    .dialogs
                    .command_line
                    .input_widget_mut()
                    .handle_key(*key);
                UiEventResult::Handled(Vec::new())
            }
        }
    }

    pub(super) fn handle_command_line_paste(&mut self, text: &str) -> UiEventResult {
        if !self.dialogs.command_line_open {
            return UiEventResult::NotHandled;
        }

        self.dialogs
            .command_line
            .input_widget_mut()
            .insert_str(text);
        UiEventResult::Handled(Vec::new())
    }

    fn execute_command_line(&mut self, input: &str) -> Result<Intent, String> {
        command::parse(input).map_err(|error| error.to_string())
    }

    #[allow(dead_code)]
    fn execute_save_current(&mut self) -> Result<(), String> {
        let buffer_id = self.active_buffer_view().buffer_id();
        if crate::globals::with_buffer_pool(|pool| {
            pool.buffer_needs_overwrite_confirmation(buffer_id)
        }) {
            self.prompt_overwrite_buffer(buffer_id);
            return Ok(());
        }

        match crate::globals::with_buffer_pool(|pool| pool.save_buffer(buffer_id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                Err("Cannot write unnamed buffer; provide a path with :write <path>".to_string())
            }
            Err(error) => Err(format!("Failed to write buffer: {error}")),
        }
    }

    pub(super) fn execute_save_as(&mut self, path: &Path) -> Result<(), String> {
        if path.exists() {
            return Err(format!(
                "Cannot write: path already exists: {}",
                path.display()
            ));
        }

        let buffer_id = self.active_buffer_view().buffer_id();
        crate::globals::with_buffer_pool(|pool| pool.save_buffer_to_path(buffer_id, path))
            .map_err(|error| format!("Failed to write buffer to {}: {error}", path.display()))
    }

    pub(super) fn execute_write_all(&mut self) -> Result<(), String> {
        let buffer_ids = crate::globals::with_buffer_pool(|pool| pool.modified_buffer_ids());
        let mut saved_buffer_ids = Vec::new();
        let mut unnamed_buffer_ids = Vec::new();

        if let Some(buffer_id) = buffer_ids.iter().copied().find(|buffer_id| {
            crate::globals::with_buffer_pool(|pool| {
                pool.buffer_needs_overwrite_confirmation(*buffer_id)
            })
        }) {
            self.prompt_overwrite_buffer(buffer_id);
            return Ok(());
        }

        for buffer_id in buffer_ids {
            let has_path = crate::globals::with_buffer(buffer_id, |buffer| buffer.path().is_some())
                .unwrap_or(false);
            if !has_path {
                unnamed_buffer_ids.push(buffer_id);
                continue;
            }

            crate::globals::with_buffer_pool(|pool| pool.save_buffer(buffer_id))
                .map_err(|error| format!("Failed to write buffer {:?}: {error}", buffer_id))?;
            saved_buffer_ids.push(buffer_id);
        }

        crate::globals::with_lsp_runtime_mut(|runtime| {
            for buffer_id in &saved_buffer_ids {
                runtime.did_save_buffer(*buffer_id);
            }
        });

        if !unnamed_buffer_ids.is_empty() {
            return Err(format!(
                "Cannot write {} unnamed buffer{}",
                unnamed_buffer_ids.len(),
                if unnamed_buffer_ids.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        }

        Ok(())
    }

    #[allow(dead_code)]
    fn execute_edit_path(&mut self, path: &str) -> Result<(), String> {
        let buffer_id = crate::globals::with_buffer_pool(|pool| pool.open_buffer(path))
            .map_err(|error| format!("Failed to open {path}: {error}"))?;
        self.active_window_group_mut()
            .activate_or_open_buffer(buffer_id);
        Ok(())
    }

    pub(super) fn execute_lsp_hover(&mut self) -> Result<(), String> {
        let buffer_id = self.active_buffer_view().buffer_id();
        let cursor = self.active_buffer_view().cursor();
        let result =
            crate::globals::with_lsp_runtime_mut(|runtime| runtime.hover_buffer(buffer_id, cursor))
                .ok_or_else(|| "LSP runtime is not available".to_string())??;

        if let Some(text) = result {
            if let Some(anchor) = self.editor_cursor_position() {
                self.open_lsp_hover(text, anchor);
            }
        }

        Ok(())
    }

    pub(super) fn execute_lsp_definition(&mut self) -> Result<(), String> {
        let buffer_id = self.active_buffer_view().buffer_id();
        let cursor = self.active_buffer_view().cursor();
        let result = crate::globals::with_lsp_runtime_mut(|runtime| {
            runtime.definition_buffer(buffer_id, cursor)
        })
        .ok_or_else(|| "LSP runtime is not available".to_string())??;

        let Some((target_buffer_id, target_cursor)) = result else {
            return Ok(());
        };

        self.active_window_group_mut().record_cursor_position();
        self.active_window_group_mut()
            .activate_or_open_buffer(target_buffer_id);
        self.active_buffer_view_mut()
            .set_cursor_synced(target_cursor);
        Ok(())
    }

    pub(super) fn execute_lsp_previous_diagnostic(&mut self) -> Result<(), String> {
        self.execute_lsp_diagnostic_navigation(false, None)
    }

    pub(super) fn execute_lsp_next_diagnostic(&mut self) -> Result<(), String> {
        self.execute_lsp_diagnostic_navigation(true, None)
    }

    pub(super) fn execute_lsp_previous_error_diagnostic(&mut self) -> Result<(), String> {
        self.execute_lsp_diagnostic_navigation(false, Some(DiagnosticSeverity::ERROR))
    }

    pub(super) fn execute_lsp_next_error_diagnostic(&mut self) -> Result<(), String> {
        self.execute_lsp_diagnostic_navigation(true, Some(DiagnosticSeverity::ERROR))
    }

    pub(super) fn execute_lsp_rename(&mut self, new_name: String) -> Result<(), String> {
        let new_name = new_name.trim().to_string();
        if new_name.is_empty() {
            return Err("Rename requires a new symbol name".to_string());
        }

        let buffer_id = self.active_buffer_view().buffer_id();
        let cursor = self.active_buffer_view().cursor();
        let Some(supports_rename) = crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.buffer_supports_rename(buffer_id)
        }) else {
            return Err("LSP runtime is busy".to_string());
        };

        if !supports_rename {
            return Err("attached server does not support rename".to_string());
        }

        let job = LspRenameJob::new(buffer_id, cursor, new_name);
        let token = JobToken::new(LspRenameJob::next_generation());
        self.jobs
            .submit_latest_only(JobKind::LspRename(buffer_id), token, job)
            .map_err(|error| error.to_string())?;

        Ok(())
    }

    fn execute_lsp_diagnostic_navigation(
        &mut self,
        forward: bool,
        severity: Option<DiagnosticSeverity>,
    ) -> Result<(), String> {
        let buffer_id = self.active_buffer_view().buffer_id();
        let cursor = self.active_buffer_view().cursor();
        let target = crate::globals::with_diagnostics_store(|store| {
            if forward {
                store.next_diagnostic_cursor(buffer_id, cursor, severity)
            } else {
                store.previous_diagnostic_cursor(buffer_id, cursor, severity)
            }
        })
        .flatten();

        let Some(target_cursor) = target else {
            return Ok(());
        };

        {
            let active_window = self.active_window_group_mut().active_window_mut();
            active_window.reveal_cursor(target_cursor);
        }

        let diagnostics = crate::globals::with_diagnostics_store(|store| {
            store.diagnostics_at_buffer_cursor(buffer_id, target_cursor)
        })
        .unwrap_or_default();

        if let Some(anchor) = self.editor_cursor_position() {
            self.open_diagnostic_hover(diagnostics, anchor);
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub(super) fn command_line_input(&self) -> Option<&str> {
        if self.dialogs.command_line_open {
            return Some(self.dialogs.command_line.input());
        }

        None
    }

    pub(super) fn command_line_input_widget_mut(&mut self) -> Option<&mut InputWidget> {
        if self.dialogs.command_line_open {
            return Some(self.dialogs.command_line.input_widget_mut());
        }

        None
    }

    pub(super) fn command_line_should_capture_events(&self) -> bool {
        self.dialogs.command_line_open
    }
}

#[allow(dead_code)]
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
        "pick" => match args {
            [target] if target == "file" => Ok(ParsedCommand::PickFile),
            [target] if target == "git" => Ok(ParsedCommand::PickGit),
            [target] if target == "grep" => Ok(ParsedCommand::PickGrep),
            [target] if target == "colorscheme" => Ok(ParsedCommand::PickColorscheme),
            [target] if target == "doc-symbols" => Ok(ParsedCommand::PickDocumentSymbols),
            [target] if target == "references" => Ok(ParsedCommand::PickReferences),
            [target] if target == "code-actions" => Ok(ParsedCommand::PickCodeActions),
            [target, ..] => Err(ParseCommandError::UnknownCommand(format!("pick {target}"))),
            [] => Err(ParseCommandError::InvalidArity {
                command: "pick".to_string(),
                expected: "pick <file|git|grep|colorscheme|doc-symbols|references|code-actions>",
            }),
        },
        "lsp" => match args {
            [subcommand] if subcommand == "hover" => Ok(ParsedCommand::LspHover),
            [subcommand] if subcommand == "definition" => Ok(ParsedCommand::LspDefinition),
            [subcommand] if subcommand == "references" => Ok(ParsedCommand::LspReferences),
            [subcommand] if subcommand == "rename" => Ok(ParsedCommand::LspRename { name: None }),
            [subcommand, name] if subcommand == "rename" => Ok(ParsedCommand::LspRename {
                name: Some(name.clone()),
            }),
            [subcommand] if subcommand == "code-actions" => Ok(ParsedCommand::PickCodeActions),
            [subcommand, ..] => Err(ParseCommandError::UnknownCommand(format!(
                "lsp {subcommand}"
            ))),
            [] => Err(ParseCommandError::InvalidArity {
                command: "lsp".to_string(),
                expected: "lsp <hover|definition|references|rename|code-actions> [name]",
            }),
        },
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
        "write-all" => match args {
            [] => Ok(ParsedCommand::WriteAll),
            _ => Err(ParseCommandError::InvalidArity {
                command: "write-all".to_string(),
                expected: "write-all",
            }),
        },
        other => Err(ParseCommandError::UnknownCommand(other.to_string())),
    }
}

#[allow(dead_code)]
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
        assert_eq!(
            parse_command_line("pick file").expect("file picker should parse"),
            ParsedCommand::PickFile
        );
        assert_eq!(
            parse_command_line("pick git").expect("git picker should parse"),
            ParsedCommand::PickGit
        );
        assert_eq!(
            parse_command_line("pick grep").expect("grep picker should parse"),
            ParsedCommand::PickGrep
        );
        assert_eq!(
            parse_command_line("pick colorscheme").expect("colorscheme picker should parse"),
            ParsedCommand::PickColorscheme
        );
        assert_eq!(
            parse_command_line("pick doc-symbols").expect("doc symbols picker should parse"),
            ParsedCommand::PickDocumentSymbols
        );
        assert_eq!(
            parse_command_line("pick references").expect("references picker should parse"),
            ParsedCommand::PickReferences
        );
        assert_eq!(
            parse_command_line("pick code-actions").expect("code actions picker should parse"),
            ParsedCommand::PickCodeActions
        );
        assert_eq!(
            parse_command_line("write-all").expect("write-all should parse"),
            ParsedCommand::WriteAll
        );
    }

    #[test]
    fn parse_lsp_commands() {
        assert_eq!(
            parse_command_line("lsp hover").expect("hover should parse"),
            ParsedCommand::LspHover
        );
        assert_eq!(
            parse_command_line("lsp definition").expect("definition should parse"),
            ParsedCommand::LspDefinition
        );
        assert_eq!(
            parse_command_line("lsp references").expect("references should parse"),
            ParsedCommand::LspReferences
        );
        assert_eq!(
            parse_command_line("lsp rename").expect("rename prompt should parse"),
            ParsedCommand::LspRename { name: None }
        );
        assert_eq!(
            parse_command_line("lsp rename next_name").expect("rename should parse"),
            ParsedCommand::LspRename {
                name: Some("next_name".to_string())
            }
        );
        assert_eq!(
            parse_command_line("lsp code-actions").expect("code actions should parse"),
            ParsedCommand::PickCodeActions
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
        layout
            .dialogs
            .command_line
            .input_widget_mut()
            .set_text("unknown");

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
        layout
            .dialogs
            .command_line
            .push_history("write".to_string());
        layout
            .dialogs
            .command_line
            .push_history("edit one.txt".to_string());

        let ctrl_p = KeyCode::Char('p').with_modifiers(Modifiers::CTRL);
        let ctrl_n = KeyCode::Char('n').with_modifiers(Modifiers::CTRL);

        layout.handle_command_line_key(&ctrl_p);
        assert_eq!(layout.command_line_input(), Some("edit one.txt"));

        layout.handle_command_line_key(&ctrl_p);
        assert_eq!(layout.command_line_input(), Some("write"));

        layout.handle_command_line_key(&ctrl_n);
        assert_eq!(layout.command_line_input(), Some("edit one.txt"));
    }
}
