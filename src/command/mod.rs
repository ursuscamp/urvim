//! User-facing command parsing and resolution.

mod catalog;
mod error;
pub mod highlight;
mod parser;
mod registry;
mod token;

pub use error::CommandError;
pub use highlight::{CommandHighlightKind, CommandHighlightSpan, highlight};
pub use parser::CommandInvocation;
pub use registry::{CommandRegistry, RegisteredCommand, install_configured_commands};

#[cfg(test)]
pub use registry::set_test_registry;

use crate::ui::Intent;
use std::collections::BTreeMap;

const MAX_SCRIPT_EXPANSION_DEPTH: usize = 8;

/// Parses a command line into an executable intent.
pub fn parse(input: &str) -> Result<Intent, CommandError> {
    let mut intents = parse_many(input)?;
    if intents.len() == 1 {
        Ok(intents.remove(0))
    } else {
        Err(CommandError::InvalidArgument {
            command: input.to_string(),
            name: "script".to_string(),
            value: intents.len().to_string(),
            expected: "single command",
        })
    }
}

/// Parses a command line into one or more executable intents.
pub fn parse_many(input: &str) -> Result<Vec<Intent>, CommandError> {
    let invocation = parser::parse(input)?;
    resolve_invocation(invocation, 0)
}

/// Returns true when `name` is a protected canonical command root.
pub fn is_canonical_command_root(name: &str) -> bool {
    registry::is_canonical_root(name)
}

/// Validates that an alias expansion parses into at least one command token.
pub fn validate_alias_expansion(input: &str) -> Result<(), CommandError> {
    parser::parse(input).map(|_| ())
}

/// Parses an alias expansion into command tokens.
pub fn parse_alias_expansion(input: &str) -> Result<Vec<String>, CommandError> {
    parser::parse(input).map(|invocation| invocation.tokens)
}

/// Validates a configured script command template.
pub fn validate_script_command(input: &str) -> Result<(), CommandError> {
    parser::parse(input)?;
    validate_placeholders(input, "script")
}

fn resolve_invocation(
    invocation: CommandInvocation,
    script_depth: usize,
) -> Result<Vec<Intent>, CommandError> {
    let invocation = registry::expand(&invocation)?;
    let Some(root) = invocation.tokens.first() else {
        return Err(CommandError::Empty);
    };

    if let Some(commands) = registry::script(root) {
        if script_depth >= MAX_SCRIPT_EXPANSION_DEPTH {
            return Err(CommandError::ScriptExpansionCycle(root.clone()));
        }
        return expand_script(root, &commands, &invocation.tokens[1..], script_depth + 1);
    }

    catalog::resolve(&invocation).map(|intent| vec![intent])
}

fn expand_script(
    name: &str,
    commands: &[String],
    args: &[String],
    script_depth: usize,
) -> Result<Vec<Intent>, CommandError> {
    let args = ScriptArgs::from_tokens(args)?;
    let mut intents = Vec::new();

    for command in commands {
        let expanded = substitute_script_placeholders(name, command, &args)?;
        let invocation = parser::parse(&expanded)?;
        intents.extend(resolve_invocation(invocation, script_depth)?);
    }

    Ok(intents)
}

struct ScriptArgs {
    positionals: Vec<String>,
    named: BTreeMap<String, String>,
}

impl ScriptArgs {
    fn from_tokens(tokens: &[String]) -> Result<Self, CommandError> {
        let mut positionals = Vec::new();
        let mut named = BTreeMap::new();

        for token in tokens {
            if let Some((name, value)) = token.split_once('=') {
                if name.is_empty() {
                    return Err(CommandError::InvalidArgument {
                        command: "script".to_string(),
                        name: name.to_string(),
                        value: value.to_string(),
                        expected: "arg=value",
                    });
                }
                if named.insert(name.to_string(), value.to_string()).is_some() {
                    return Err(CommandError::DuplicateArgument {
                        command: "script".to_string(),
                        name: name.to_string(),
                    });
                }
            } else {
                positionals.push(token.clone());
            }
        }

        Ok(Self { positionals, named })
    }

    fn get(&self, name: &str) -> Option<&str> {
        if let Ok(index) = name.parse::<usize>() {
            return index
                .checked_sub(1)
                .and_then(|index| self.positionals.get(index))
                .map(String::as_str);
        }

        self.named.get(name).map(String::as_str)
    }
}

fn substitute_script_placeholders(
    script: &str,
    command: &str,
    args: &ScriptArgs,
) -> Result<String, CommandError> {
    let mut expanded = String::new();
    let mut rest = command;

    while let Some(start) = rest.find('{') {
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('}') else {
            return Err(CommandError::InvalidScriptPlaceholder {
                script: script.to_string(),
                placeholder: rest[start..].to_string(),
            });
        };

        let placeholder = &after_start[..end];
        if placeholder.is_empty() || placeholder.contains('{') {
            return Err(CommandError::InvalidScriptPlaceholder {
                script: script.to_string(),
                placeholder: format!("{{{placeholder}}}"),
            });
        }

        let value = args
            .get(placeholder)
            .ok_or_else(|| CommandError::MissingScriptArgument {
                script: script.to_string(),
                name: placeholder.to_string(),
            })?;
        expanded.push_str(&quote_command_token(value));
        rest = &after_start[end + 1..];
    }

    if rest.contains('}') {
        return Err(CommandError::InvalidScriptPlaceholder {
            script: script.to_string(),
            placeholder: "}".to_string(),
        });
    }

    expanded.push_str(rest);
    Ok(expanded)
}

fn validate_placeholders(command: &str, script: &str) -> Result<(), CommandError> {
    let empty_args = ScriptArgs {
        positionals: Vec::new(),
        named: BTreeMap::new(),
    };

    match substitute_script_placeholders(script, command, &empty_args) {
        Err(CommandError::MissingScriptArgument { .. }) => Ok(()),
        other => other.map(|_| ()),
    }
}

fn quote_command_token(value: &str) -> String {
    if !value.is_empty()
        && !value
            .chars()
            .any(|ch| matches!(ch, ' ' | '\t' | '"' | '\'' | '='))
    {
        return value.to_string();
    }

    let escaped = value.replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::editor::ModeKind;
    use crate::ui::Command;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn parse_supports_named_and_positional_arguments() {
        assert_eq!(
            parser::parse(r#"buffer write path="notes/today file.txt""#)
                .expect("command should parse")
                .tokens,
            vec!["buffer", "write", "path=notes/today file.txt"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn resolve_write_aliases() {
        assert!(matches!(
            parse("write notes.txt").expect("write should resolve"),
            Intent::Command(Command::SaveBufferAs(_))
        ));
        assert!(matches!(
            parse("buffer write path=notes.txt").expect("buffer write should resolve"),
            Intent::Command(Command::SaveBufferAs(_))
        ));
        assert!(matches!(
            parse("buffer edit").expect("buffer edit should resolve"),
            Intent::Command(Command::OpenUnnamedBuffer)
        ));
        assert!(matches!(
            parse("quit").expect("quit should resolve"),
            Intent::Command(Command::Quit)
        ));
    }

    #[test]
    fn resolve_mode_and_cursor_commands() {
        assert!(matches!(
            parse("mode insert").expect("mode should resolve"),
            Intent::Action(action) if action.to_mode == Some(ModeKind::Insert)
        ));
        assert!(matches!(
            parse("action cursor left count=2").expect("cursor should resolve"),
            Intent::Action(_)
        ));
    }

    #[test]
    fn resolve_edit_operator_and_surround_commands() {
        assert!(matches!(
            parse("action edit delete-line 3").expect("edit action should resolve"),
            Intent::Action(action)
                if matches!(action.kind.as_ref(), Some(crate::editor::ActionKind::Count(3, _)))
        ));
        assert!(matches!(
            parse("action operator delete target=word").expect("operator should resolve"),
            Intent::Action(_)
        ));
        assert!(matches!(
            parse("action tab next").expect("tab navigation should resolve"),
            Intent::Action(_)
        ));
        assert!(matches!(
            parse("action jump backward").expect("jumplist navigation should resolve"),
            Intent::Action(_)
        ));
        assert!(matches!(
            parse("action surround add target=word delimiter=paren")
                .expect("surround should resolve"),
            Intent::Action(_)
        ));
    }

    #[test]
    fn resolve_configured_aliases() {
        let config = Config {
            aliases: BTreeMap::from([(
                "dl".to_string(),
                vec![
                    "action".to_string(),
                    "edit".to_string(),
                    "delete-line".to_string(),
                ],
            )]),
            ..Config::default()
        };
        let mut registry = CommandRegistry::new();
        registry
            .register_configured_commands(&config)
            .expect("configured commands should register");
        let _guard = set_test_registry(registry);

        assert!(matches!(
            parse("dl count=2").expect("configured alias should resolve"),
            Intent::Action(action)
                if matches!(action.kind.as_ref(), Some(crate::editor::ActionKind::Count(2, _)))
        ));
    }

    #[test]
    fn resolve_configured_script_to_multiple_intents() {
        let config = Config {
            scripts: BTreeMap::from([(
                "wq".to_string(),
                vec!["write".to_string(), "quit".to_string()],
            )]),
            ..Config::default()
        };
        let mut registry = CommandRegistry::new();
        registry
            .register_configured_commands(&config)
            .expect("configured commands should register");
        let _guard = set_test_registry(registry);

        let intents = parse_many("wq").expect("script should resolve");

        assert_eq!(intents.len(), 2);
        assert!(matches!(intents[0], Intent::Action(_)));
        assert!(matches!(intents[1], Intent::Command(Command::Quit)));
    }

    #[test]
    fn resolve_script_placeholders_from_positional_and_named_arguments() {
        let config = Config {
            scripts: BTreeMap::from([(
                "save-rust".to_string(),
                vec![
                    "buffer write path={1}".to_string(),
                    "buffer filetype filetype={filetype}".to_string(),
                ],
            )]),
            ..Config::default()
        };
        let mut registry = CommandRegistry::new();
        registry
            .register_configured_commands(&config)
            .expect("configured commands should register");
        let _guard = set_test_registry(registry);

        let intents = parse_many("save-rust \"notes/today file.txt\" filetype=rust")
            .expect("script should resolve");

        assert_eq!(intents.len(), 2);
        assert!(matches!(
            &intents[0],
            Intent::Command(Command::SaveBufferAs(path))
                if path == &PathBuf::from("notes/today file.txt")
        ));
        assert!(matches!(
            &intents[1],
            Intent::Command(Command::SetBufferFiletype(None, filetype)) if filetype == "rust"
        ));
    }

    #[test]
    fn resolve_script_errors_for_missing_placeholder_argument() {
        let config = Config {
            scripts: BTreeMap::from([(
                "save-as".to_string(),
                vec!["buffer write path={path}".to_string()],
            )]),
            ..Config::default()
        };
        let mut registry = CommandRegistry::new();
        registry
            .register_configured_commands(&config)
            .expect("configured commands should register");
        let _guard = set_test_registry(registry);

        assert!(matches!(
            parse_many("save-as").expect_err("script should fail"),
            CommandError::MissingScriptArgument { script, name }
                if script == "save-as" && name == "path"
        ));
    }
}
