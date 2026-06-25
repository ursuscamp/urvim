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
pub use registry::{
    CommandRegistry, CommandRegistrySnapshot, RegisteredCommand, RegisteredCommandSnapshot,
    install_configured_commands, install_configured_commands_with_plugins,
    snapshot as command_registry_snapshot,
};

#[cfg(test)]
pub use registry::{TestRegistryGuard, set_test_registry};

use crate::ui::{Command, Intent};
use serde_json::{Map, Value, json};
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

    if root == "plugin" {
        return resolve_plugin_command(&invocation.tokens[1..]);
    }

    catalog::resolve(&invocation).map(|intent| vec![intent])
}

fn resolve_plugin_command(tokens: &[String]) -> Result<Vec<Intent>, CommandError> {
    let plugin = tokens
        .first()
        .ok_or_else(|| CommandError::MissingArgument {
            command: "plugin".to_string(),
            name: "plugin".to_string(),
        })?;
    if plugin == "status" {
        if let Some(extra) = tokens.get(1) {
            return Err(CommandError::UnexpectedArgument {
                command: "plugin status".to_string(),
                value: extra.clone(),
            });
        }
        return Ok(vec![Intent::Command(Command::PluginStatus)]);
    }
    let command = tokens.get(1).ok_or_else(|| CommandError::MissingArgument {
        command: "plugin".to_string(),
        name: "command".to_string(),
    })?;

    if !registry::has_plugin(plugin) {
        return Err(CommandError::UnknownPlugin(plugin.clone()));
    }

    Ok(vec![Intent::Command(Command::PluginRequest {
        plugin: plugin.clone(),
        command: command.clone(),
        args: tokens[2..].to_vec(),
    })])
}

pub struct PluginCommandArgs {
    positionals: Vec<String>,
    named: Map<String, Value>,
}

impl PluginCommandArgs {
    pub fn from_tokens(tokens: &[String]) -> Result<Self, CommandError> {
        let mut positionals = Vec::new();
        let mut named = Map::new();

        for token in tokens {
            if let Some((name, value)) = token.split_once('=') {
                if name.is_empty() {
                    return Err(CommandError::InvalidArgument {
                        command: "plugin".to_string(),
                        name: name.to_string(),
                        value: value.to_string(),
                        expected: "arg=value",
                    });
                }
                if named
                    .insert(name.to_string(), Value::String(value.to_string()))
                    .is_some()
                {
                    return Err(CommandError::DuplicateArgument {
                        command: "plugin".to_string(),
                        name: name.to_string(),
                    });
                }
            } else {
                positionals.push(token.clone());
            }
        }

        Ok(Self { positionals, named })
    }

    pub fn into_params(mut self) -> Value {
        if !self.positionals.is_empty() {
            self.named
                .insert("args".to_string(), json!(self.positionals));
        }
        Value::Object(self.named)
    }
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
            parse("buffer close").expect("buffer close should resolve"),
            Intent::Command(Command::CloseBuffer(None))
        ));
        assert!(matches!(
            parse("buffer unload").expect("buffer unload should resolve"),
            Intent::Command(Command::UnloadBuffer {
                buffer_id: None,
                force: false
            })
        ));
        assert!(matches!(
            parse("buffer unload force=true").expect("forced buffer unload should resolve"),
            Intent::Command(Command::UnloadBuffer {
                buffer_id: None,
                force: true
            })
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

    fn registry_with_plugin_namespace(plugin: &str) -> CommandRegistry {
        let mut registry = CommandRegistry::new();
        registry.register_test_plugin_namespace(plugin);
        registry
    }

    #[test]
    fn resolve_plugin_command_to_request_intent() {
        let registry = registry_with_plugin_namespace("demo-plugin");
        let _guard = set_test_registry(registry);

        let intent = parse("plugin demo-plugin echo text=hello").expect("command should resolve");

        assert!(matches!(
            intent,
            Intent::Command(Command::PluginRequest { plugin, command, args })
                if plugin == "demo-plugin"
                    && command == "echo"
                    && args == vec!["text=hello".to_string()]
        ));
    }

    #[test]
    fn resolve_plugin_command_preserves_positionals() {
        let registry = registry_with_plugin_namespace("demo-plugin");
        let _guard = set_test_registry(registry);

        let intent = parse("plugin demo-plugin echo first second text=hello")
            .expect("command should resolve");

        assert!(matches!(
            intent,
            Intent::Command(Command::PluginRequest { args, .. })
                if args == vec!["first".to_string(), "second".to_string(), "text=hello".to_string()]
        ));
    }

    #[test]
    fn resolve_plugin_status_without_plugins() {
        let _guard = set_test_registry(CommandRegistry::new());

        let intent = parse("plugin status").expect("status command should resolve");

        assert!(matches!(intent, Intent::Command(Command::PluginStatus)));
    }

    #[test]
    fn resolve_plugin_status_rejects_extra_arguments() {
        let _guard = set_test_registry(CommandRegistry::new());

        assert!(matches!(
            parse_many("plugin status extra").expect_err("extra arg should fail"),
            CommandError::UnexpectedArgument { command, value }
                if command == "plugin status" && value == "extra"
        ));
    }

    #[test]
    fn resolve_plugin_command_errors_for_missing_plugin_name() {
        let _guard = set_test_registry(CommandRegistry::new());

        assert!(matches!(
            parse_many("plugin").expect_err("plugin name should be required"),
            CommandError::MissingArgument { command, name }
                if command == "plugin" && name == "plugin"
        ));
    }

    #[test]
    fn resolve_plugin_command_errors_for_missing_command_name() {
        let registry = registry_with_plugin_namespace("demo-plugin");
        let _guard = set_test_registry(registry);

        assert!(matches!(
            parse_many("plugin demo-plugin").expect_err("command name should be required"),
            CommandError::MissingArgument { command, name }
                if command == "plugin" && name == "command"
        ));
    }

    #[test]
    fn resolve_plugin_command_errors_for_unknown_plugin() {
        let _guard = set_test_registry(CommandRegistry::new());

        assert_eq!(
            parse_many("plugin missing wq").expect_err("plugin should be unknown"),
            CommandError::UnknownPlugin("missing".to_string())
        );
    }

    #[test]
    fn resolve_plugin_command_defers_command_to_runtime() {
        let registry = registry_with_plugin_namespace("demo-plugin");
        let _guard = set_test_registry(registry);

        let intent = parse("plugin demo-plugin missing").expect("command should defer");

        assert!(matches!(
            intent,
            Intent::Command(Command::PluginRequest { plugin, command, args })
                if plugin == "demo-plugin" && command == "missing" && args.is_empty()
        ));
    }
}
