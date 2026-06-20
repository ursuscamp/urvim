use super::{CommandError, CommandInvocation};
use crate::config::Config;
use std::collections::BTreeMap;
use urvim_plugin::{PluginCommand, PluginRegistry};

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
    plugin_scripts: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    plugin_commands: BTreeMap<String, BTreeMap<String, PluginCommand>>,
}

impl CommandRegistry {
    /// Creates a registry populated with built-in command entries.
    pub fn new() -> Self {
        Self {
            commands: builtin_commands(),
            plugin_scripts: BTreeMap::new(),
            plugin_commands: BTreeMap::new(),
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

    /// Registers scripts contributed by loaded plugins.
    pub fn register_plugin_scripts(&mut self, plugins: &PluginRegistry) {
        for (plugin_name, plugin) in plugins.iter() {
            self.plugin_scripts
                .insert(plugin_name.to_string(), plugin.scripts().clone());
            self.plugin_commands
                .insert(plugin_name.to_string(), plugin.commands().clone());
        }
    }

    #[cfg(test)]
    pub fn register_test_plugin_script(
        &mut self,
        plugin: impl Into<String>,
        script: impl Into<String>,
        commands: Vec<String>,
    ) {
        self.plugin_scripts
            .entry(plugin.into())
            .or_default()
            .insert(script.into(), commands);
    }

    #[cfg(test)]
    pub fn register_test_plugin_command(
        &mut self,
        plugin: impl Into<String>,
        command: impl Into<String>,
        request: impl Into<String>,
    ) {
        self.plugin_commands
            .entry(plugin.into())
            .or_default()
            .insert(
                command.into(),
                PluginCommand {
                    description: None,
                    request: request.into(),
                },
            );
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
        self.plugin_scripts.contains_key(plugin) || self.plugin_commands.contains_key(plugin)
    }

    /// Returns the registered script command list for a namespaced plugin script.
    pub fn plugin_script(&self, plugin: &str, script: &str) -> Option<Vec<String>> {
        self.plugin_scripts
            .get(plugin)
            .and_then(|scripts| scripts.get(script))
            .cloned()
    }

    /// Returns the registered process command for a namespaced plugin command.
    pub fn plugin_command(&self, plugin: &str, command: &str) -> Option<PluginCommand> {
        self.plugin_commands
            .get(plugin)
            .and_then(|commands| commands.get(command))
            .cloned()
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
    &["buffer", "action", "pick", "lsp", "pane", "app", "plugin"]
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

/// Installs a fresh registry containing built-ins, configured commands, and plugin scripts.
pub fn install_configured_commands_with_plugins(
    config: &Config,
    plugins: &PluginRegistry,
) -> Result<(), CommandError> {
    let mut registry = CommandRegistry::new();
    registry.register_configured_commands(config)?;
    registry.register_plugin_scripts(plugins);
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

/// Returns the registered script command list for a namespaced plugin script.
pub fn plugin_script(plugin: &str, script: &str) -> Option<Vec<String>> {
    with_registry(|registry| registry.plugin_script(plugin, script))
}

/// Returns the registered process command for a namespaced plugin command.
pub fn plugin_command(plugin: &str, command: &str) -> Option<PluginCommand> {
    with_registry(|registry| registry.plugin_command(plugin, command))
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
    fn registers_plugin_scripts_without_top_level_roots() {
        let root =
            std::env::temp_dir().join(format!("urvim-command-plugin-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("plugin dir should exist");
        std::fs::write(
            root.join(urvim_plugin::MANIFEST_FILE_NAME),
            r#"
name = "tools"
version = "0.1.0"

[scripts]
wq = ["write", "quit"]
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

        registry.register_plugin_scripts(&plugins);

        assert_eq!(
            registry.plugin_script("tools", "wq"),
            Some(vec!["write".to_string(), "quit".to_string()])
        );
        assert_eq!(registry.script("wq"), None);

        std::fs::remove_dir_all(root).ok();
    }
}
