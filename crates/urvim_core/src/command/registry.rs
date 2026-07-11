use super::{CommandError, CommandInvocation};
use crate::config::Config;
use std::collections::{BTreeMap, BTreeSet};
use urvim_plugin::PluginRegistry;

#[cfg(test)]
use std::cell::RefCell;
#[cfg(not(test))]
use std::sync::{LazyLock, RwLock};

const MAX_ALIAS_EXPANSION_DEPTH: usize = 8;

#[cfg(test)]
thread_local! {
    static TEST_REGISTRY: RefCell<CommandRegistry> = RefCell::new(CommandRegistry::default());
}

#[cfg(not(test))]
static REGISTRY: LazyLock<RwLock<CommandRegistry>> =
    LazyLock::new(|| RwLock::new(CommandRegistry::default()));

/// Registered user-facing command entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisteredCommand {
    /// Expands the command root into another command prefix.
    Alias {
        /// Token-level command prefix used to replace the alias root.
        expands_to: Vec<String>,
    },
    /// Expands the command root into ordered command lines.
    Script {
        /// Ordered command-line templates to execute.
        commands: Vec<String>,
    },
}

/// Runtime registry for user-facing command roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRegistry {
    commands: BTreeMap<String, RegisteredCommand>,
    plugins: BTreeSet<String>,
}

impl CommandRegistry {
    /// Creates a registry populated with built-in command entries.
    pub fn new() -> Self {
        Self {
            commands: builtin_commands(),
            plugins: BTreeSet::new(),
        }
    }

    /// Registers a command alias.
    pub fn register_alias(
        &mut self,
        name: impl Into<String>,
        expands_to: Vec<String>,
    ) -> Result<(), CommandError> {
        self.register(name, RegisteredCommand::Alias { expands_to })
    }

    /// Registers a command script.
    pub fn register_script(
        &mut self,
        name: impl Into<String>,
        commands: Vec<String>,
    ) -> Result<(), CommandError> {
        self.register(name, RegisteredCommand::Script { commands })
    }

    /// Registers all configured aliases and scripts.
    pub fn register_configured_commands(&mut self, config: &Config) -> Result<(), CommandError> {
        for (name, expands_to) in &config.aliases {
            self.register_alias(name.clone(), expands_to.clone())?;
        }
        for (name, commands) in &config.scripts {
            self.register_script(name.clone(), commands.clone())?;
        }

        Ok(())
    }

    /// Registers namespaces for loaded plugins.
    pub fn register_plugins(&mut self, plugins: &PluginRegistry) {
        for (plugin_name, _) in plugins.iter() {
            self.plugins.insert(plugin_name.to_string());
        }
    }

    #[cfg(test)]
    pub fn register_test_plugin_namespace(&mut self, plugin: impl Into<String>) {
        self.plugins.insert(plugin.into());
    }

    /// Returns true if `name` is a registered command root.
    pub fn is_registered_root(&self, name: &str) -> bool {
        is_canonical_root(name) || self.commands.contains_key(name)
    }

    /// Returns true if `prefix` can still become a registered command root.
    pub fn is_registered_root_prefix(&self, prefix: &str) -> bool {
        canonical_roots()
            .iter()
            .any(|command| command.starts_with(prefix))
            || self
                .commands
                .keys()
                .any(|command| command.starts_with(prefix))
    }

    /// Returns the configured script command list for a command root.
    pub fn script(&self, name: &str) -> Option<Vec<String>> {
        match self.commands.get(name) {
            Some(RegisteredCommand::Script { commands }) => Some(commands.clone()),
            _ => None,
        }
    }

    /// Returns true when the plugin namespace is registered.
    pub fn has_plugin(&self, plugin: &str) -> bool {
        self.plugins.contains(plugin)
    }

    fn register(
        &mut self,
        name: impl Into<String>,
        command: RegisteredCommand,
    ) -> Result<(), CommandError> {
        let name = name.into();
        if is_canonical_root(&name) || self.commands.contains_key(&name) {
            return Err(CommandError::CommandRegistrationConflict(name));
        }

        self.commands.insert(name, command);
        Ok(())
    }

    fn alias_expansion(&self, name: &str) -> Option<Vec<String>> {
        match self.commands.get(name) {
            Some(RegisteredCommand::Alias { expands_to }) => Some(expands_to.clone()),
            _ => None,
        }
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the canonical built-in command roots.
pub fn canonical_roots() -> &'static [&'static str] {
    &[
        "buffer", "action", "pick", "lsp", "pane", "window", "app", "plugin",
    ]
}

/// Returns true if `name` is a canonical command root.
pub fn is_canonical_root(name: &str) -> bool {
    canonical_roots().contains(&name)
}

/// Installs a fresh registry containing built-ins and configured commands.
pub fn install_configured_commands(config: &Config) -> Result<(), CommandError> {
    let mut registry = CommandRegistry::new();
    registry.register_configured_commands(config)?;
    set_registry(registry);
    Ok(())
}

/// Installs a fresh registry containing built-ins, configured commands, and plugin namespaces.
pub fn install_configured_commands_with_plugins(
    config: &Config,
    plugins: &PluginRegistry,
) -> Result<(), CommandError> {
    let mut registry = CommandRegistry::new();
    registry.register_configured_commands(config)?;
    registry.register_plugins(plugins);
    set_registry(registry);
    Ok(())
}

/// Returns true if `name` is a registered command root.
pub fn is_registered_root(name: &str) -> bool {
    with_registry(|registry| registry.is_registered_root(name))
}

/// Returns true if `prefix` can still become a registered command root.
pub fn is_registered_root_prefix(prefix: &str) -> bool {
    with_registry(|registry| registry.is_registered_root_prefix(prefix))
}

/// Expands registered aliases in the command root position.
pub fn expand(invocation: &CommandInvocation) -> Result<CommandInvocation, CommandError> {
    with_registry(|registry| expand_with_registry(registry, invocation))
}

/// Returns the registered script command list for a command root.
pub fn script(name: &str) -> Option<Vec<String>> {
    with_registry(|registry| registry.script(name))
}

/// Returns true when the plugin namespace is registered.
pub fn has_plugin(plugin: &str) -> bool {
    with_registry(|registry| registry.has_plugin(plugin))
}

#[cfg(test)]
pub fn set_test_registry(registry: CommandRegistry) -> TestRegistryGuard {
    TEST_REGISTRY.with(|slot| {
        let previous = slot.replace(registry);
        TestRegistryGuard { previous }
    })
}

#[cfg(test)]
pub struct TestRegistryGuard {
    previous: CommandRegistry,
}

#[cfg(test)]
impl Drop for TestRegistryGuard {
    fn drop(&mut self) {
        TEST_REGISTRY.with(|slot| {
            slot.replace(self.previous.clone());
        });
    }
}

fn expand_with_registry(
    registry: &CommandRegistry,
    invocation: &CommandInvocation,
) -> Result<CommandInvocation, CommandError> {
    let mut tokens = invocation.tokens.clone();

    for _ in 0..MAX_ALIAS_EXPANSION_DEPTH {
        let Some(root) = tokens.first() else {
            return Ok(CommandInvocation { tokens });
        };

        let Some(expands_to) = registry.alias_expansion(root) else {
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

fn with_registry<R>(f: impl FnOnce(&CommandRegistry) -> R) -> R {
    #[cfg(test)]
    {
        return TEST_REGISTRY.with(|slot| f(&slot.borrow()));
    }

    #[cfg(not(test))]
    {
        let registry = REGISTRY.read().unwrap();
        f(&registry)
    }
}

fn set_registry(registry: CommandRegistry) {
    #[cfg(test)]
    {
        TEST_REGISTRY.with(|slot| {
            slot.replace(registry);
        });
    }

    #[cfg(not(test))]
    {
        let mut stored = REGISTRY.write().unwrap();
        *stored = registry;
    }
}

/// Snapshot of a registered command for introspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredCommandSnapshot {
    /// User-facing command root name.
    pub name: String,
    /// Command kind: `alias` or `script`.
    pub kind: &'static str,
    /// Source of the command: `core` or `config`.
    pub source: &'static str,
}

/// Snapshot of the command registry for plugin introspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRegistrySnapshot {
    /// Canonical builtin command roots.
    pub builtins: Vec<String>,
    /// Registered user/configured commands (aliases and scripts).
    pub commands: Vec<RegisteredCommandSnapshot>,
    /// Loaded plugin namespaces.
    pub plugins: Vec<String>,
}

/// Returns a snapshot of the current command registry.
pub fn snapshot() -> CommandRegistrySnapshot {
    with_registry(|registry| {
        let builtins = canonical_roots().iter().map(|s| s.to_string()).collect();

        let mut commands = Vec::new();
        for (name, command) in &registry.commands {
            let (kind, source) = match command {
                RegisteredCommand::Alias { .. } => ("alias", "config"),
                RegisteredCommand::Script { .. } => ("script", "config"),
            };
            commands.push(RegisteredCommandSnapshot {
                name: name.clone(),
                kind,
                source,
            });
        }

        let plugins = registry.plugins.iter().cloned().collect();

        CommandRegistrySnapshot {
            builtins,
            commands,
            plugins,
        }
    })
}

fn builtin_commands() -> BTreeMap<String, RegisteredCommand> {
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
}

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

    #[test]
    fn rejects_configured_command_that_conflicts_with_builtin() {
        let mut registry = CommandRegistry::new();

        assert_eq!(
            registry.register_alias("write", vec!["buffer".to_string()]),
            Err(CommandError::CommandRegistrationConflict(
                "write".to_string()
            ))
        );
    }

    #[test]
    fn registers_configured_scripts() {
        let mut registry = CommandRegistry::new();
        registry
            .register_script("wq", vec!["write".to_string(), "quit".to_string()])
            .expect("script should register");

        assert_eq!(
            registry.script("wq"),
            Some(vec!["write".to_string(), "quit".to_string()])
        );
    }

    #[test]
    fn registers_plugin_namespaces_without_top_level_roots() {
        let root =
            std::env::temp_dir().join(format!("urvim-command-plugin-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("plugin dir should exist");
        std::fs::write(
            root.join(urvim_plugin::MANIFEST_FILE_NAME),
            r#"
name = "tools"
version = "0.1.0"
entry = "plugin.bear"
"#,
        )
        .expect("manifest should write");
        let config = BTreeMap::from([(
            "tools".to_string(),
            urvim_plugin::PluginConfigEntry {
                enabled: true,
                path: root.clone(),
            },
        )]);
        let plugins = PluginRegistry::load_from_config(&config).expect("plugin should load");
        let mut registry = CommandRegistry::new();

        registry.register_plugins(&plugins);

        assert!(registry.has_plugin("tools"));
        assert_eq!(registry.script("tools"), None);

        std::fs::remove_dir_all(root).ok();
    }
}
