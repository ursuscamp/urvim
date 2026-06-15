use super::{CommandError, CommandInvocation};
use crate::globals;
use std::collections::BTreeMap;
use std::sync::LazyLock;

const MAX_ALIAS_EXPANSION_DEPTH: usize = 8;

/// Registered user-facing command entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisteredCommand {
    /// Expands the command root into another command prefix.
    Alias {
        /// Token-level command prefix used to replace the alias root.
        expands_to: Vec<String>,
    },
}

/// Returns the canonical built-in command roots.
pub fn canonical_roots() -> &'static [&'static str] {
    &["buffer", "action", "pick", "lsp", "pane", "app"]
}

/// Returns true if `name` is a canonical command root.
pub fn is_canonical_root(name: &str) -> bool {
    canonical_roots().contains(&name)
}

/// Returns true if `name` is a registered command root.
pub fn is_registered_root(name: &str) -> bool {
    is_canonical_root(name) || config_alias_exists(name) || BUILTIN_REGISTRY.contains_key(name)
}

/// Returns true if `prefix` can still become a registered command root.
pub fn is_registered_root_prefix(prefix: &str) -> bool {
    canonical_roots()
        .iter()
        .any(|command| command.starts_with(prefix))
        || config_alias_prefix_exists(prefix)
        || BUILTIN_REGISTRY
            .keys()
            .any(|command| command.starts_with(prefix))
}

/// Expands registered aliases in the command root position.
pub fn expand(invocation: &CommandInvocation) -> Result<CommandInvocation, CommandError> {
    let mut tokens = invocation.tokens.clone();

    for _ in 0..MAX_ALIAS_EXPANSION_DEPTH {
        let Some(root) = tokens.first() else {
            return Ok(CommandInvocation { tokens });
        };

        let Some(expands_to) = alias_expansion(root) else {
            return Ok(CommandInvocation { tokens });
        };

        let mut expanded = expands_to;
        expanded.extend(tokens.into_iter().skip(1));
        tokens = expanded;
    }

    Err(CommandError::AliasExpansionCycle(
        tokens.first().cloned().unwrap_or_default(),
    ))
}

fn alias_expansion(name: &str) -> Option<Vec<String>> {
    globals::with_config(|config| config.aliases.get(name).cloned())
        .flatten()
        .or_else(|| match BUILTIN_REGISTRY.get(name) {
            Some(RegisteredCommand::Alias { expands_to }) => Some(expands_to.clone()),
            None => None,
        })
}

fn config_alias_exists(name: &str) -> bool {
    globals::with_config(|config| config.aliases.contains_key(name)).unwrap_or(false)
}

fn config_alias_prefix_exists(prefix: &str) -> bool {
    globals::with_config(|config| config.aliases.keys().any(|name| name.starts_with(prefix)))
        .unwrap_or(false)
}

static BUILTIN_REGISTRY: LazyLock<BTreeMap<String, RegisteredCommand>> = LazyLock::new(|| {
    [
        alias("write", &["buffer", "write"]),
        alias("write-all", &["buffer", "write-all"]),
        alias("edit", &["buffer", "edit"]),
        alias("quit", &["app", "quit"]),
        alias("try-quit", &["app", "try-quit"]),
        alias("command-line", &["app", "command-line"]),
        alias("completion", &["app", "completion"]),
        alias("mode", &["action", "mode"]),
        alias("cursor", &["action", "cursor"]),
        alias("operator", &["action", "operator"]),
        alias("surround", &["action", "surround"]),
    ]
    .into_iter()
    .collect()
});

fn alias(name: &'static str, expands_to: &[&str]) -> (String, RegisteredCommand) {
    (
        name.to_string(),
        RegisteredCommand::Alias {
            expands_to: expands_to.iter().map(|token| token.to_string()).collect(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_builtin_alias_and_preserves_arguments() {
        let invocation = CommandInvocation {
            tokens: vec![
                "cursor".to_string(),
                "left".to_string(),
                "count=2".to_string(),
            ],
        };

        assert_eq!(
            expand(&invocation).expect("alias should expand").tokens,
            ["action", "cursor", "left", "count=2"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn leaves_canonical_commands_unchanged() {
        let invocation = CommandInvocation {
            tokens: vec![
                "action".to_string(),
                "cursor".to_string(),
                "left".to_string(),
            ],
        };

        assert_eq!(
            expand(&invocation)
                .expect("canonical command should be accepted")
                .tokens,
            invocation.tokens
        );
    }
}
