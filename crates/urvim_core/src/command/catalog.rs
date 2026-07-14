use super::{CommandError, CommandInvocation};
use crate::buffer::BufferId;
use crate::editor::{
    BoundaryMotion, DelimiterFamily, EditorAction, EditorOperation, LinewiseMotion, ModeKind,
    Operator, OperatorTarget, QuoteKind, TextObject,
};
use crate::register::RegisterName;
use crate::ui::{Command, Intent};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Resolves a parsed command line into an executable intent.
pub fn resolve(invocation: &CommandInvocation) -> Result<Intent, CommandError> {
    let Some(first) = invocation.tokens.first().map(String::as_str) else {
        return Err(CommandError::Empty);
    };

    match first {
        "buffer" => resolve_buffer(&invocation.tokens[1..]),
        "action" => resolve_action(&invocation.tokens[1..]),
        "pick" => resolve_pick(&invocation.tokens[1..]),
        "lsp" => resolve_lsp(&invocation.tokens[1..]),
        "pane" => resolve_pane(&invocation.tokens[1..]),
        "window" => resolve_window(&invocation.tokens[1..]),
        "app" => resolve_app(&invocation.tokens[1..]),
        other => Err(unknown_command(other)),
    }
}

fn resolve_buffer(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("buffer", "subcommand"));
    };

    match subcommand {
        "write" => resolve_write(&tokens[1..]),
        "write-all" | "write_all" | "writeall" => resolve_write_all(&tokens[1..]),
        "edit" => resolve_edit(&tokens[1..]),
        "close" => resolve_buffer_close(&tokens[1..]),
        "unload" => resolve_buffer_unload(&tokens[1..]),
        "filetype" | "set-filetype" => {
            let mut args = ArgCursor::from_tokens("buffer filetype", &tokens[1..])?;
            let filetype = args.require_string("filetype")?;
            let buffer_id = args.take_buffer_id()?;
            args.finish()?;
            Ok(Intent::Command(Command::SetBufferFiletype(
                buffer_id, filetype,
            )))
        }
        other => Err(unknown_subcommand("buffer", other)),
    }
}

fn resolve_buffer_close(tokens: &[String]) -> Result<Intent, CommandError> {
    let mut args = ArgCursor::from_tokens("buffer close", tokens)?;
    let buffer_id = args.take_buffer_id()?;
    args.finish()?;
    Ok(Intent::Command(Command::CloseBuffer(buffer_id)))
}

fn resolve_buffer_unload(tokens: &[String]) -> Result<Intent, CommandError> {
    let mut args = ArgCursor::from_tokens("buffer unload", tokens)?;
    let buffer_id = args.take_buffer_id()?;
    let force = args.take_bool("force")?.unwrap_or(false);
    args.finish()?;
    Ok(Intent::Command(Command::UnloadBuffer { buffer_id, force }))
}

fn resolve_write(tokens: &[String]) -> Result<Intent, CommandError> {
    let mut args = ArgCursor::from_tokens("write", tokens)?;
    let path = args.take_string("path")?;
    let buffer_id = args.take_buffer_id()?;
    args.finish()?;

    match path {
        Some(path) => Ok(Intent::Command(Command::SaveBufferAs {
            buffer_id,
            path: PathBuf::from(path),
        })),
        None => Ok(Intent::Command(Command::SaveBuffer(buffer_id))),
    }
}

fn resolve_write_all(tokens: &[String]) -> Result<Intent, CommandError> {
    if !tokens.is_empty() {
        return Err(unexpected_argument("write-all", &tokens[0]));
    }

    Ok(Intent::Command(Command::WriteAll))
}

fn resolve_edit(tokens: &[String]) -> Result<Intent, CommandError> {
    let mut args = ArgCursor::from_tokens("edit", tokens)?;
    let path = args.take_string("path")?;
    args.finish()?;

    match path {
        Some(path) => Ok(Intent::Command(Command::OpenFile(PathBuf::from(path)))),
        None => Ok(Intent::Command(Command::OpenUnnamedBuffer)),
    }
}

fn resolve_action(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("action", "subcommand"));
    };

    match subcommand {
        "cursor" => resolve_cursor(&tokens[1..]),
        "edit" => resolve_edit_action(&tokens[1..]),
        "mode" => resolve_mode(&tokens[1..]),
        "tab" => resolve_tab(&tokens[1..]),
        "jump" => resolve_jump(&tokens[1..]),
        "operator" => resolve_operator(&tokens[1..]),
        "surround" => resolve_surround(&tokens[1..]),
        other => Err(unknown_subcommand("action", other)),
    }
}

fn resolve_cursor(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("action cursor", "subcommand"));
    };

    let intent = match subcommand {
        "left" => action_intent(EditorAction::new(EditorOperation::MoveLeft), tokens, true)?,
        "right" => action_intent(EditorAction::new(EditorOperation::MoveRight), tokens, true)?,
        "up" => action_intent(EditorAction::new(EditorOperation::MoveUp), tokens, true)?,
        "down" => action_intent(EditorAction::new(EditorOperation::MoveDown), tokens, true)?,
        "page-up" => action_intent(EditorAction::new(EditorOperation::MovePageUp), tokens, true)?,
        "page-down" => action_intent(
            EditorAction::new(EditorOperation::MovePageDown),
            tokens,
            true,
        )?,
        "half-page-up" => action_intent(
            EditorAction::new(EditorOperation::MoveHalfPageUp),
            tokens,
            true,
        )?,
        "half-page-down" => action_intent(
            EditorAction::new(EditorOperation::MoveHalfPageDown),
            tokens,
            true,
        )?,
        "line-start" => action_intent(
            EditorAction::new(EditorOperation::MoveToLineStart),
            tokens,
            true,
        )?,
        "line-end" => action_intent(
            EditorAction::new(EditorOperation::MoveToLineEnd),
            tokens,
            true,
        )?,
        "line-content-start" => action_intent(
            EditorAction::new(EditorOperation::MoveToLineContentStart),
            tokens,
            true,
        )?,
        "file-start" => action_intent(
            EditorAction::new(EditorOperation::MoveToFirstLine),
            tokens,
            true,
        )?,
        "file-end" => action_intent(
            EditorAction::new(EditorOperation::MoveToLastLine),
            tokens,
            true,
        )?,
        "screen-top" => action_intent(
            EditorAction::new(EditorOperation::MoveToScreenTop),
            tokens,
            true,
        )?,
        "screen-middle" => action_intent(
            EditorAction::new(EditorOperation::MoveToScreenMiddle),
            tokens,
            true,
        )?,
        "screen-bottom" => action_intent(
            EditorAction::new(EditorOperation::MoveToScreenBottom),
            tokens,
            true,
        )?,
        "paragraph-previous" => action_intent(
            EditorAction::new(EditorOperation::MoveToPreviousParagraph),
            tokens,
            true,
        )?,
        "paragraph-next" => action_intent(
            EditorAction::new(EditorOperation::MoveToNextParagraph),
            tokens,
            true,
        )?,
        "diff-previous" => action_intent(
            EditorAction::new(EditorOperation::MoveToPreviousDiffHunk),
            tokens,
            true,
        )?,
        "diff-next" => action_intent(
            EditorAction::new(EditorOperation::MoveToNextDiffHunk),
            tokens,
            true,
        )?,
        "diff-end-previous" => action_intent(
            EditorAction::new(EditorOperation::MoveToPreviousDiffHunkEnd),
            tokens,
            true,
        )?,
        "diff-end-next" => action_intent(
            EditorAction::new(EditorOperation::MoveToNextDiffHunkEnd),
            tokens,
            true,
        )?,
        "match-bracket" => action_intent(
            EditorAction::new(EditorOperation::MoveToMatchingBracket),
            tokens,
            true,
        )?,
        "find-forward" => {
            let mut args = ArgCursor::from_tokens("action cursor find-forward", &tokens[1..])?;
            let target = args
                .take_char("char")?
                .ok_or(missing_argument("action cursor find-forward", "char"))?;
            let register = args.take_register()?;
            let count = args.take_count(true)?;
            args.finish()?;
            let mut action = EditorAction::find_forward(target);
            if let Some(register) = register {
                action = action.with_register(register);
            }
            if let Some(count) = count {
                action = action
                    .with_count(count)
                    .ok_or_else(|| CommandError::InvalidArgument {
                        command: "action cursor find-forward".to_string(),
                        name: "count".to_string(),
                        value: count.to_string(),
                        expected: "countable action",
                    })?;
            }
            Intent::Editor(action)
        }
        "find-backward" => {
            let mut args = ArgCursor::from_tokens("action cursor find-backward", &tokens[1..])?;
            let target = args
                .take_char("char")?
                .ok_or(missing_argument("action cursor find-backward", "char"))?;
            let register = args.take_register()?;
            let count = args.take_count(true)?;
            args.finish()?;
            let mut action = EditorAction::find_backward(target);
            if let Some(register) = register {
                action = action.with_register(register);
            }
            if let Some(count) = count {
                action = action
                    .with_count(count)
                    .ok_or_else(|| CommandError::InvalidArgument {
                        command: "action cursor find-backward".to_string(),
                        name: "count".to_string(),
                        value: count.to_string(),
                        expected: "countable action",
                    })?;
            }
            Intent::Editor(action)
        }
        "till-forward" => {
            let mut args = ArgCursor::from_tokens("action cursor till-forward", &tokens[1..])?;
            let target = args
                .take_char("char")?
                .ok_or(missing_argument("action cursor till-forward", "char"))?;
            let register = args.take_register()?;
            let count = args.take_count(true)?;
            args.finish()?;
            let mut action = EditorAction::till_forward(target);
            if let Some(register) = register {
                action = action.with_register(register);
            }
            if let Some(count) = count {
                action = action
                    .with_count(count)
                    .ok_or_else(|| CommandError::InvalidArgument {
                        command: "action cursor till-forward".to_string(),
                        name: "count".to_string(),
                        value: count.to_string(),
                        expected: "countable action",
                    })?;
            }
            Intent::Editor(action)
        }
        "till-backward" => {
            let mut args = ArgCursor::from_tokens("action cursor till-backward", &tokens[1..])?;
            let target = args
                .take_char("char")?
                .ok_or(missing_argument("action cursor till-backward", "char"))?;
            let register = args.take_register()?;
            let count = args.take_count(true)?;
            args.finish()?;
            let mut action = EditorAction::till_backward(target);
            if let Some(register) = register {
                action = action.with_register(register);
            }
            if let Some(count) = count {
                action = action
                    .with_count(count)
                    .ok_or_else(|| CommandError::InvalidArgument {
                        command: "action cursor till-backward".to_string(),
                        name: "count".to_string(),
                        value: count.to_string(),
                        expected: "countable action",
                    })?;
            }
            Intent::Editor(action)
        }
        "repeat-find" => action_intent(
            EditorAction::new(EditorOperation::RepeatLastFind),
            tokens,
            true,
        )?,
        "repeat-find-reverse" => action_intent(
            EditorAction::new(EditorOperation::RepeatLastFindReverse),
            tokens,
            true,
        )?,
        other => {
            return Err(unknown_subcommand("action cursor", other));
        }
    };

    Ok(intent)
}

fn resolve_edit_action(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("action edit", "subcommand"));
    };

    let intent = match subcommand {
        "delete-forward" => action_intent(
            EditorAction::new(EditorOperation::DeleteForward),
            tokens,
            true,
        )?,
        "delete-backward" => action_intent(
            EditorAction::new(EditorOperation::DeleteBackward),
            tokens,
            true,
        )?,
        "delete-selection" => action_intent(
            EditorAction::new(EditorOperation::DeleteSelection),
            tokens,
            true,
        )?,
        "delete-line" => {
            action_intent(EditorAction::new(EditorOperation::DeleteLine), tokens, true)?
        }
        "yank-line" => action_intent(EditorAction::new(EditorOperation::YankLine), tokens, true)?,
        "yank-selection" => action_intent(
            EditorAction::new(EditorOperation::YankSelection),
            tokens,
            true,
        )?,
        "change-line" => action_intent(
            EditorAction::new(EditorOperation::ChangeLine).with_to_mode(ModeKind::Insert),
            tokens,
            true,
        )?,
        "change-selection" => action_intent(
            EditorAction::new(EditorOperation::ChangeSelection).with_to_mode(ModeKind::Insert),
            tokens,
            true,
        )?,
        "change-to-line-end" => action_intent(
            EditorAction::new(EditorOperation::ChangeToLineEnd).with_to_mode(ModeKind::Insert),
            tokens,
            true,
        )?,
        "paste-after" => action_intent(EditorAction::paste_after(), tokens, true)?,
        "paste-before" => action_intent(EditorAction::paste_before(), tokens, true)?,
        "join-space" => action_intent(
            EditorAction::new(EditorOperation::JoinWithSpace),
            tokens,
            true,
        )?,
        "join-no-space" => action_intent(
            EditorAction::new(EditorOperation::JoinWithoutSpace),
            tokens,
            true,
        )?,
        "indent-decrease" => action_intent(
            EditorAction::new(EditorOperation::IndentDecrease),
            tokens,
            true,
        )?,
        "indent-increase" => action_intent(
            EditorAction::new(EditorOperation::IndentIncrease),
            tokens,
            true,
        )?,
        "toggle-line-comment" => action_intent(EditorAction::toggle_line_comment(), tokens, true)?,
        "undo" => action_intent(EditorAction::new(EditorOperation::Undo), tokens, false)?,
        "redo" => action_intent(EditorAction::new(EditorOperation::Redo), tokens, false)?,
        "repeat-last-change" => action_intent(
            EditorAction::new(EditorOperation::RepeatLastChange),
            tokens,
            false,
        )?,
        "append-after-cursor" => action_intent(
            EditorAction::new(EditorOperation::AppendAfterCursor).with_to_mode(ModeKind::Insert),
            tokens,
            true,
        )?,
        "append-to-line-end" => action_intent(
            EditorAction::new(EditorOperation::AppendToLineEnd).with_to_mode(ModeKind::Insert),
            tokens,
            true,
        )?,
        "insert-at-line-start" => action_intent(
            EditorAction::new(EditorOperation::InsertAtLineStart).with_to_mode(ModeKind::Insert),
            tokens,
            true,
        )?,
        "open-line-below" => action_intent(
            EditorAction::new(EditorOperation::OpenLineBelow).with_to_mode(ModeKind::Insert),
            tokens,
            true,
        )?,
        "open-line-above" => action_intent(
            EditorAction::new(EditorOperation::OpenLineAbove).with_to_mode(ModeKind::Insert),
            tokens,
            true,
        )?,
        other => return Err(unknown_subcommand("action edit", other)),
    };

    Ok(intent)
}

fn resolve_mode(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("action mode", "mode"));
    };

    let mode = parse_mode_kind("action mode", "mode", subcommand)?;

    if tokens.len() > 1 {
        return Err(unexpected_argument("action mode", &tokens[1]));
    }

    Ok(Intent::Editor(EditorAction::mode_transition(mode)))
}

fn resolve_operator(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("action operator", "subcommand"));
    };

    let operator = match subcommand {
        "delete" => Operator::Delete,
        "change" => Operator::Change,
        "yank" => Operator::Yank,
        "lowercase" => Operator::Lowercase,
        "uppercase" => Operator::Uppercase,
        "toggle-case" => Operator::ToggleCase,
        other => {
            return Err(unknown_subcommand("action operator", other));
        }
    };

    let mut args = ArgCursor::from_tokens("action operator", &tokens[1..])?;
    let target = args
        .take_target("target")?
        .ok_or(missing_argument("action operator", "target"))?;
    let count = args.take_count(true)?;
    let register = args.take_register()?;
    args.finish()?;

    let mut action = EditorAction::operation(operator, target);
    if operator == Operator::Change {
        action = action.with_to_mode(ModeKind::Insert);
    }
    if let Some(register) = register {
        action = action.with_register(register);
    }
    if let Some(count) = count {
        action = action
            .with_count(count)
            .ok_or_else(|| CommandError::InvalidArgument {
                command: "action operator".to_string(),
                name: "count".to_string(),
                value: count.to_string(),
                expected: "countable action",
            })?;
    }

    Ok(Intent::Editor(action))
}

fn resolve_surround(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("action surround", "subcommand"));
    };

    let mut args = ArgCursor::from_tokens("action surround", &tokens[1..])?;
    let intent = match subcommand {
        "add" => {
            let target = args
                .take_text_object("target")?
                .ok_or(missing_argument("action surround add", "target"))?;
            let delimiter = args
                .take_delimiter_family("delimiter")?
                .ok_or(missing_argument("action surround add", "delimiter"))?;
            args.finish()?;
            Intent::Editor(EditorAction::new(EditorOperation::SurroundAdd {
                target,
                delimiter,
            }))
        }
        "delete" => {
            let target = args
                .take_delimiter_family("target")?
                .ok_or(missing_argument("action surround delete", "target"))?;
            args.finish()?;
            Intent::Editor(EditorAction::new(EditorOperation::SurroundDelete {
                target,
            }))
        }
        "replace" => {
            let target = args
                .take_delimiter_family("target")?
                .ok_or(missing_argument("action surround replace", "target"))?;
            let replacement = args
                .take_delimiter_family("replacement")?
                .ok_or(missing_argument("action surround replace", "replacement"))?;
            args.finish()?;
            Intent::Editor(EditorAction::new(EditorOperation::SurroundReplace {
                target,
                replacement,
            }))
        }
        other => {
            return Err(unknown_subcommand("action surround", other));
        }
    };

    Ok(intent)
}

fn resolve_tab(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("action tab", "subcommand"));
    };

    let mut args = ArgCursor::from_tokens("action tab", &tokens[1..])?;
    let count = args.take_count(true)?.unwrap_or(1);
    args.finish()?;

    let intent = match subcommand {
        "previous" => Intent::Command(Command::PreviousTab(count)),
        "next" => Intent::Command(Command::NextTab(count)),
        other => {
            return Err(unknown_subcommand("action tab", other));
        }
    };

    Ok(intent)
}

fn resolve_jump(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("action jump", "subcommand"));
    };

    let intent = match subcommand {
        "backward" => action_intent(EditorAction::jump_backward(), tokens, true)?,
        "forward" => action_intent(EditorAction::jump_forward(), tokens, true)?,
        other => {
            return Err(unknown_subcommand("action jump", other));
        }
    };

    Ok(intent)
}

fn resolve_pick(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("pick", "subcommand"));
    };

    let command = match subcommand {
        "file" => Command::OpenFilePicker,
        "buffer" => Command::OpenBufferPicker,
        "git" => Command::OpenGitPicker,
        "grep" => Command::OpenGrepPicker,
        "colorscheme" => Command::OpenColorschemePicker,
        "filetype" => Command::OpenFiletypePicker,
        "doc-symbols" | "document-symbols" => Command::OpenDocumentSymbolsPicker,
        "workspace-symbols" => Command::OpenWorkspaceSymbolsPicker,
        "references" => Command::LspReferences,
        "code-actions" => Command::LspCodeActions,
        other => return Err(unknown_subcommand("pick", other)),
    };

    if tokens.len() > 1 {
        return Err(unexpected_argument("pick", &tokens[1]));
    }

    Ok(Intent::Command(command))
}

fn resolve_lsp(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("lsp", "subcommand"));
    };

    match subcommand {
        "hover" => Ok(Intent::Command(Command::LspHover)),
        "definition" => Ok(Intent::Command(Command::LspDefinition)),
        "references" => Ok(Intent::Command(Command::LspReferences)),
        "rename" => {
            let mut args = ArgCursor::from_tokens("lsp rename", &tokens[1..])?;
            let name = args.take_string("name")?;
            args.finish()?;
            Ok(match name {
                Some(name) => Intent::Command(Command::LspRename(name)),
                None => Intent::Command(Command::LspRenamePrompt),
            })
        }
        "code-actions" => Ok(Intent::Command(Command::LspCodeActions)),
        "diagnostic" => resolve_lsp_diagnostic(&tokens[1..]),
        other => Err(unknown_subcommand("lsp", other)),
    }
}

fn resolve_lsp_diagnostic(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("lsp diagnostic", "direction"));
    };

    let command = match subcommand {
        "previous" => Command::LspPreviousDiagnostic,
        "next" => Command::LspNextDiagnostic,
        "error-previous" | "previous-error" => Command::LspPreviousErrorDiagnostic,
        "error-next" | "next-error" => Command::LspNextErrorDiagnostic,
        other => return Err(unknown_subcommand("lsp diagnostic", other)),
    };

    if tokens.len() > 1 {
        return Err(unexpected_argument("lsp diagnostic", &tokens[1]));
    }

    Ok(Intent::Command(command))
}

fn resolve_pane(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("pane", "subcommand"));
    };

    match subcommand {
        "split-vertical" => Ok(Intent::Command(Command::SplitVertical)),
        "split-horizontal" => Ok(Intent::Command(Command::SplitHorizontal)),
        "focus-left" => Ok(Intent::Command(Command::FocusPaneLeft)),
        "focus-right" => Ok(Intent::Command(Command::FocusPaneRight)),
        "focus-up" => Ok(Intent::Command(Command::FocusPaneUp)),
        "focus-down" => Ok(Intent::Command(Command::FocusPaneDown)),
        "close" => Ok(Intent::Command(Command::ClosePane)),
        "equalize" => Ok(Intent::Command(Command::EqualizeSplits)),
        "wrap-toggle" => Ok(Intent::Command(Command::ToggleWrap)),
        "resize-left" => resolve_pane_resize(Command::ResizePaneLeft, &tokens[1..]),
        "resize-right" => resolve_pane_resize(Command::ResizePaneRight, &tokens[1..]),
        "resize-up" => resolve_pane_resize(Command::ResizePaneUp, &tokens[1..]),
        "resize-down" => resolve_pane_resize(Command::ResizePaneDown, &tokens[1..]),
        other => Err(unknown_subcommand("pane", other)),
    }
}

fn resolve_window(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("window", "subcommand"));
    };

    let command = match subcommand {
        "focus-next" => Command::FocusNextWindow,
        "focus-previous" => Command::FocusPreviousWindow,
        other => return Err(unknown_subcommand("window", other)),
    };

    if tokens.len() > 1 {
        return Err(unexpected_argument("window", &tokens[1]));
    }

    Ok(Intent::Command(command))
}

fn resolve_pane_resize(
    constructor: fn(usize) -> Command,
    tokens: &[String],
) -> Result<Intent, CommandError> {
    let mut args = ArgCursor::from_tokens("pane resize", tokens)?;
    let count = args.take_count(true)?.unwrap_or(1);
    args.finish()?;
    Ok(Intent::Command(constructor(count)))
}

fn resolve_app(tokens: &[String]) -> Result<Intent, CommandError> {
    let Some(subcommand) = tokens.first().map(String::as_str) else {
        return Err(missing_argument("app", "subcommand"));
    };

    let command = match subcommand {
        "command-line" => Command::OpenCommandLine,
        "completion" => Command::OpenCompletion,
        "quit" => Command::Quit,
        "try-quit" => Command::TryQuit,
        other => return Err(unknown_subcommand("app", other)),
    };

    if tokens.len() > 1 {
        return Err(unexpected_argument("app", &tokens[1]));
    }

    Ok(Intent::Command(command))
}

fn action_intent(
    mut action: EditorAction,
    tokens: &[String],
    allow_positional_count: bool,
) -> Result<Intent, CommandError> {
    let mut args = ArgCursor::from_tokens("action", &tokens[1..])?;
    let register = args.take_register()?;
    let count = args.take_count(allow_positional_count)?;
    args.finish()?;

    if let Some(register) = register {
        action = action.with_register(register);
    }
    if let Some(count) = count {
        action = action
            .with_count(count)
            .ok_or_else(|| CommandError::InvalidArgument {
                command: "action".to_string(),
                name: "count".to_string(),
                value: count.to_string(),
                expected: "countable action",
            })?;
    }

    Ok(Intent::Editor(action))
}

struct ArgCursor {
    command: String,
    positionals: Vec<String>,
    named: BTreeMap<String, String>,
    next_positional: usize,
}

impl ArgCursor {
    fn from_tokens(command: &str, tokens: &[String]) -> Result<Self, CommandError> {
        let mut positionals = Vec::new();
        let mut named = BTreeMap::new();

        for token in tokens {
            if let Some((name, value)) = token.split_once('=') {
                if name.is_empty() {
                    return Err(CommandError::InvalidArgument {
                        command: command.to_string(),
                        name: name.to_string(),
                        value: value.to_string(),
                        expected: "arg=value",
                    });
                }
                if named.insert(name.to_string(), value.to_string()).is_some() {
                    return Err(CommandError::DuplicateArgument {
                        command: command.to_string(),
                        name: name.to_string(),
                    });
                }
            } else {
                positionals.push(token.clone());
            }
        }

        Ok(Self {
            command: command.to_string(),
            positionals,
            named,
            next_positional: 0,
        })
    }

    fn take_positional(&mut self) -> Option<String> {
        let value = self.positionals.get(self.next_positional).cloned();
        if value.is_some() {
            self.next_positional += 1;
        }
        value
    }

    fn take_named(&mut self, name: &str) -> Option<String> {
        self.named.remove(name)
    }

    fn take_string(&mut self, name: &str) -> Result<Option<String>, CommandError> {
        Ok(self.take_named(name).or_else(|| self.take_positional()))
    }

    fn require_string(&mut self, name: &str) -> Result<String, CommandError> {
        self.take_string(name)?
            .ok_or_else(|| missing_argument(&self.command, name))
    }

    fn take_count(&mut self, allow_positional: bool) -> Result<Option<usize>, CommandError> {
        if let Some(value) = self.take_named("count") {
            return Ok(Some(parse_usize(&self.command, "count", &value)?));
        }

        if allow_positional && let Some(value) = self.take_positional() {
            return Ok(Some(parse_usize(&self.command, "count", &value)?));
        }

        Ok(None)
    }

    fn take_bool(&mut self, name: &str) -> Result<Option<bool>, CommandError> {
        self.take_named(name)
            .map(|value| parse_bool(&self.command, name, &value))
            .transpose()
    }

    fn take_buffer_id(&mut self) -> Result<Option<BufferId>, CommandError> {
        self.take_named("buffer")
            .map(|value| {
                value.parse::<usize>().map(BufferId::new).map_err(|_| {
                    CommandError::InvalidArgument {
                        command: self.command.clone(),
                        name: "buffer".to_string(),
                        value,
                        expected: "non-negative integer",
                    }
                })
            })
            .transpose()
    }

    fn take_register(&mut self) -> Result<Option<RegisterName>, CommandError> {
        if let Some(value) = self.take_named("register") {
            return Ok(Some(parse_register(&self.command, &value)?));
        }

        Ok(None)
    }

    fn take_char(&mut self, name: &str) -> Result<Option<char>, CommandError> {
        if let Some(value) = self.take_named(name) {
            return Ok(Some(parse_char(&self.command, name, &value)?));
        }

        Ok(self
            .take_positional()
            .map(|value| parse_char(&self.command, name, &value))
            .transpose()?)
    }

    fn take_delimiter_family(
        &mut self,
        name: &str,
    ) -> Result<Option<DelimiterFamily>, CommandError> {
        if let Some(value) = self.take_named(name) {
            return Ok(Some(parse_delimiter_family(&self.command, name, &value)?));
        }

        Ok(self
            .take_positional()
            .map(|value| parse_delimiter_family(&self.command, name, &value))
            .transpose()?)
    }

    fn take_text_object(&mut self, name: &str) -> Result<Option<TextObject>, CommandError> {
        if let Some(value) = self.take_named(name) {
            return Ok(Some(parse_text_object(&self.command, name, &value)?));
        }

        Ok(self
            .take_positional()
            .map(|value| parse_text_object(&self.command, name, &value))
            .transpose()?)
    }

    fn take_target(&mut self, name: &str) -> Result<Option<OperatorTarget>, CommandError> {
        if let Some(value) = self.take_named(name) {
            return Ok(Some(parse_operator_target(&self.command, name, &value)?));
        }

        Ok(self
            .take_positional()
            .map(|value| parse_operator_target(&self.command, name, &value))
            .transpose()?)
    }

    fn finish(self) -> Result<(), CommandError> {
        if self.next_positional < self.positionals.len() {
            return Err(unexpected_argument(
                &self.command,
                &self.positionals[self.next_positional],
            ));
        }

        if let Some((name, value)) = self.named.into_iter().next() {
            return Err(unexpected_argument(
                &self.command,
                &format!("{name}={value}"),
            ));
        }

        Ok(())
    }
}

fn parse_usize(command: &str, name: &str, value: &str) -> Result<usize, CommandError> {
    value.parse().map_err(|_| CommandError::InvalidArgument {
        command: command.to_string(),
        name: name.to_string(),
        value: value.to_string(),
        expected: "positive integer",
    })
}

fn parse_bool(command: &str, name: &str, value: &str) -> Result<bool, CommandError> {
    match value {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(CommandError::InvalidArgument {
            command: command.to_string(),
            name: name.to_string(),
            value: value.to_string(),
            expected: "true or false",
        }),
    }
}

fn parse_char(command: &str, name: &str, value: &str) -> Result<char, CommandError> {
    let mut chars = value.chars();
    let Some(ch) = chars.next() else {
        return Err(CommandError::InvalidArgument {
            command: command.to_string(),
            name: name.to_string(),
            value: value.to_string(),
            expected: "single character",
        });
    };

    if chars.next().is_some() {
        return Err(CommandError::InvalidArgument {
            command: command.to_string(),
            name: name.to_string(),
            value: value.to_string(),
            expected: "single character",
        });
    }

    Ok(ch)
}

fn parse_register(command: &str, value: &str) -> Result<RegisterName, CommandError> {
    let ch = parse_char(command, "register", value)?;
    Ok(RegisterName::new(ch))
}

fn parse_mode_kind(command: &str, name: &str, value: &str) -> Result<ModeKind, CommandError> {
    match value {
        "normal" => Ok(ModeKind::Normal),
        "insert" => Ok(ModeKind::Insert),
        "replace" => Ok(ModeKind::Replace),
        "visual" => Ok(ModeKind::Visual),
        "visual-line" => Ok(ModeKind::VisualLine),
        "resizing" => Ok(ModeKind::Resizing),
        _ => Err(CommandError::InvalidArgument {
            command: command.to_string(),
            name: name.to_string(),
            value: value.to_string(),
            expected: "normal|insert|replace|visual|visual-line|resizing",
        }),
    }
}

fn parse_delimiter_family(
    command: &str,
    name: &str,
    value: &str,
) -> Result<DelimiterFamily, CommandError> {
    match value {
        "(" | ")" | "paren" => Ok(DelimiterFamily::Paren),
        "[" | "]" | "square" => Ok(DelimiterFamily::Square),
        "{" | "}" | "curly" => Ok(DelimiterFamily::Curly),
        "<" | ">" | "angle" | "<LessThan>" | "<GreaterThan>" => Ok(DelimiterFamily::Angle),
        "\"" | "double-quote" => Ok(DelimiterFamily::DoubleQuote),
        "'" | "single-quote" => Ok(DelimiterFamily::SingleQuote),
        "`" | "backtick" => Ok(DelimiterFamily::Backtick),
        _ => Err(CommandError::InvalidArgument {
            command: command.to_string(),
            name: name.to_string(),
            value: value.to_string(),
            expected: "delimiter family",
        }),
    }
}

fn parse_text_object(command: &str, name: &str, value: &str) -> Result<TextObject, CommandError> {
    match value {
        "word" => Ok(TextObject::AroundWord),
        "inner-word" => Ok(TextObject::InnerWord),
        "big-word" => Ok(TextObject::AroundBigWord),
        "inner-big-word" => Ok(TextObject::InnerBigWord),
        "paren" => Ok(TextObject::AroundBracket(crate::editor::BracketKind::Paren)),
        "inner-paren" => Ok(TextObject::InnerBracket(crate::editor::BracketKind::Paren)),
        "square" => Ok(TextObject::AroundBracket(
            crate::editor::BracketKind::Square,
        )),
        "inner-square" => Ok(TextObject::InnerBracket(crate::editor::BracketKind::Square)),
        "curly" => Ok(TextObject::AroundBracket(crate::editor::BracketKind::Curly)),
        "inner-curly" => Ok(TextObject::InnerBracket(crate::editor::BracketKind::Curly)),
        "angle" => Ok(TextObject::AroundBracket(crate::editor::BracketKind::Angle)),
        "inner-angle" => Ok(TextObject::InnerBracket(crate::editor::BracketKind::Angle)),
        "double-quote" => Ok(TextObject::AroundQuote(QuoteKind::Double)),
        "inner-double-quote" => Ok(TextObject::InnerQuote(QuoteKind::Double)),
        "single-quote" => Ok(TextObject::AroundQuote(QuoteKind::Single)),
        "inner-single-quote" => Ok(TextObject::InnerQuote(QuoteKind::Single)),
        "backtick" => Ok(TextObject::AroundQuote(QuoteKind::Backtick)),
        "inner-backtick" => Ok(TextObject::InnerQuote(QuoteKind::Backtick)),
        _ => Err(CommandError::InvalidArgument {
            command: command.to_string(),
            name: name.to_string(),
            value: value.to_string(),
            expected: "text object",
        }),
    }
}

fn parse_operator_target(
    command: &str,
    name: &str,
    value: &str,
) -> Result<OperatorTarget, CommandError> {
    match value {
        "selection" => Ok(OperatorTarget::Selection),
        "word" | "word-forward" => Ok(OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward)),
        "word-end" => Ok(OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd)),
        "word-backward" => Ok(OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward)),
        "big-word" | "big-word-forward" => Ok(OperatorTarget::BoundaryMotion(
            BoundaryMotion::BigWordForward,
        )),
        "big-word-end" => Ok(OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordEnd)),
        "big-word-backward" => Ok(OperatorTarget::BoundaryMotion(
            BoundaryMotion::BigWordBackward,
        )),
        "line-start" => Ok(OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart)),
        "line-end" => Ok(OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd)),
        "line-content-start" => Ok(OperatorTarget::BoundaryMotion(
            BoundaryMotion::LineContentStart,
        )),
        "first-line" => Ok(OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine)),
        "last-line" => Ok(OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine)),
        "inner-word" => Ok(OperatorTarget::TextObject(TextObject::InnerWord)),
        "around-word" => Ok(OperatorTarget::TextObject(TextObject::AroundWord)),
        "inner-big-word" => Ok(OperatorTarget::TextObject(TextObject::InnerBigWord)),
        "around-big-word" => Ok(OperatorTarget::TextObject(TextObject::AroundBigWord)),
        "inner-paren" => Ok(OperatorTarget::TextObject(TextObject::InnerBracket(
            crate::editor::BracketKind::Paren,
        ))),
        "around-paren" => Ok(OperatorTarget::TextObject(TextObject::AroundBracket(
            crate::editor::BracketKind::Paren,
        ))),
        "inner-square" => Ok(OperatorTarget::TextObject(TextObject::InnerBracket(
            crate::editor::BracketKind::Square,
        ))),
        "around-square" => Ok(OperatorTarget::TextObject(TextObject::AroundBracket(
            crate::editor::BracketKind::Square,
        ))),
        "inner-curly" => Ok(OperatorTarget::TextObject(TextObject::InnerBracket(
            crate::editor::BracketKind::Curly,
        ))),
        "around-curly" => Ok(OperatorTarget::TextObject(TextObject::AroundBracket(
            crate::editor::BracketKind::Curly,
        ))),
        "inner-angle" => Ok(OperatorTarget::TextObject(TextObject::InnerBracket(
            crate::editor::BracketKind::Angle,
        ))),
        "around-angle" => Ok(OperatorTarget::TextObject(TextObject::AroundBracket(
            crate::editor::BracketKind::Angle,
        ))),
        _ => Err(CommandError::InvalidArgument {
            command: command.to_string(),
            name: name.to_string(),
            value: value.to_string(),
            expected: "operator target",
        }),
    }
}

fn missing_argument(command: &str, name: &str) -> CommandError {
    CommandError::MissingArgument {
        command: command.to_string(),
        name: name.to_string(),
    }
}

fn unknown_subcommand(command: &str, subcommand: &str) -> CommandError {
    CommandError::UnknownSubcommand {
        command: command.to_string(),
        subcommand: subcommand.to_string(),
    }
}

fn unknown_command(command: &str) -> CommandError {
    CommandError::UnknownCommand(command.to_string())
}

fn unexpected_argument(command: &str, value: &str) -> CommandError {
    CommandError::UnexpectedArgument {
        command: command.to_string(),
        value: value.to_string(),
    }
}
