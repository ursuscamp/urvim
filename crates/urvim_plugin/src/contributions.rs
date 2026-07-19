//! Dynamic plugin contribution registry.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

/// A BearScript command dynamically registered by a plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DynamicPluginCommand {
    /// User-facing command name inside the plugin namespace.
    pub name: String,
    /// Optional human-readable description.
    pub description: Option<String>,
}

/// A BearScript API endpoint exposed by a plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DynamicPluginApi {
    /// Endpoint name inside the plugin namespace.
    pub name: String,
}

/// A theme dynamically registered by a plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DynamicPluginTheme {
    /// Resolved theme name.
    pub name: String,
    /// Source that produced the dynamic theme.
    pub source: DynamicPluginThemeSource,
}

/// A syntax highlighting provider dynamically registered by a plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DynamicSyntaxProvider {
    /// Runtime provider id owned by the plugin instance.
    pub id: u64,
    /// Filetype this provider highlights.
    pub filetype: String,
}

/// A filetype dynamically registered by a plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DynamicFiletype {
    /// Filetype name.
    pub name: String,
}

/// Source that produced a dynamic plugin theme.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DynamicPluginThemeSource {
    /// Theme loaded from a TOML file path.
    File(PathBuf),
    /// Theme created directly by plugin script data.
    #[default]
    Script,
}

/// Low-frequency editor events that plugins can subscribe to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PluginEventKind {
    /// Editor startup finished.
    EditorStarted,
    /// A buffer was opened from disk.
    BufferOpened,
    /// A buffer was loaded into the buffer pool.
    BufferLoaded,
    /// A buffer was saved successfully.
    BufferSaved,
    /// A buffer save failed.
    BufferSaveFailed,
    /// Opening a buffer failed.
    BufferOpenFailed,
    /// A buffer was closed.
    BufferClosed,
    /// A buffer was unloaded from the buffer pool.
    BufferUnloaded,
    /// A buffer path changed.
    BufferPathChanged,
    /// A buffer was reloaded from disk.
    BufferReloaded,
    /// A buffer conflicted with an externally changed file.
    ExternalFileConflict,
    /// A buffer's contents changed.
    BufferChanged,
    /// A buffer's contents changed during an insert session.
    InsertBufferChanged,
    /// An insert session completed or changed focus.
    InsertSessionChanged,
    /// A buffer's modified state changed.
    BufferModifiedChanged,
    /// A buffer filetype changed.
    BufferFiletypeChanged,
    /// A pane was created.
    PaneCreated,
    /// A pane was closed.
    PaneClosed,
    /// A pane received focus.
    PaneFocused,
    /// A tab was opened.
    TabOpened,
    /// A tab was closed.
    TabClosed,
    /// A tab was activated.
    TabActivated,
    /// The active buffer changed.
    ActiveBufferChanged,
    /// The editor mode changed.
    ModeChanged,
    /// The cursor moved.
    CursorMoved,
    /// The selection changed.
    SelectionChanged,
    /// The editor is about to shut down.
    EditorWillShutdown,
    /// The active theme changed.
    ThemeChanged,
    /// An LSP server started.
    LspServerStarted,
    /// An LSP server failed to start.
    LspServerStartFailed,
    /// An LSP server stopped.
    LspServerStopped,
    /// An LSP server attached to a buffer.
    LspBufferAttached,
    /// An LSP server detached from a buffer.
    LspBufferDetached,
    /// A command was executed.
    CommandExecuted,
    /// Diagnostics changed for a buffer.
    DiagnosticsChanged,
}

impl PluginEventKind {
    /// Complete catalog of stable plugin event names.
    pub const ALL: &'static [Self] = &[
        Self::EditorStarted,
        Self::BufferOpened,
        Self::BufferLoaded,
        Self::BufferSaved,
        Self::BufferSaveFailed,
        Self::BufferOpenFailed,
        Self::BufferClosed,
        Self::BufferUnloaded,
        Self::BufferPathChanged,
        Self::BufferReloaded,
        Self::ExternalFileConflict,
        Self::BufferChanged,
        Self::InsertBufferChanged,
        Self::InsertSessionChanged,
        Self::BufferModifiedChanged,
        Self::BufferFiletypeChanged,
        Self::PaneCreated,
        Self::PaneClosed,
        Self::PaneFocused,
        Self::TabOpened,
        Self::TabClosed,
        Self::TabActivated,
        Self::ActiveBufferChanged,
        Self::ModeChanged,
        Self::CursorMoved,
        Self::SelectionChanged,
        Self::EditorWillShutdown,
        Self::ThemeChanged,
        Self::LspServerStarted,
        Self::LspServerStartFailed,
        Self::LspServerStopped,
        Self::LspBufferAttached,
        Self::LspBufferDetached,
        Self::CommandExecuted,
        Self::DiagnosticsChanged,
    ];

    /// Returns the stable event name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EditorStarted => "EditorStarted",
            Self::BufferOpened => "BufferOpened",
            Self::BufferLoaded => "BufferLoaded",
            Self::BufferSaved => "BufferSaved",
            Self::BufferSaveFailed => "BufferSaveFailed",
            Self::BufferOpenFailed => "BufferOpenFailed",
            Self::BufferClosed => "BufferClosed",
            Self::BufferUnloaded => "BufferUnloaded",
            Self::BufferPathChanged => "BufferPathChanged",
            Self::BufferReloaded => "BufferReloaded",
            Self::ExternalFileConflict => "ExternalFileConflict",
            Self::BufferChanged => "BufferChanged",
            Self::InsertBufferChanged => "InsertBufferChanged",
            Self::InsertSessionChanged => "InsertSessionChanged",
            Self::BufferModifiedChanged => "BufferModifiedChanged",
            Self::BufferFiletypeChanged => "BufferFiletypeChanged",
            Self::PaneCreated => "PaneCreated",
            Self::PaneClosed => "PaneClosed",
            Self::PaneFocused => "PaneFocused",
            Self::TabOpened => "TabOpened",
            Self::TabClosed => "TabClosed",
            Self::TabActivated => "TabActivated",
            Self::ActiveBufferChanged => "ActiveBufferChanged",
            Self::ModeChanged => "ModeChanged",
            Self::CursorMoved => "CursorMoved",
            Self::SelectionChanged => "SelectionChanged",
            Self::EditorWillShutdown => "EditorWillShutdown",
            Self::ThemeChanged => "ThemeChanged",
            Self::LspServerStarted => "LspServerStarted",
            Self::LspServerStartFailed => "LspServerStartFailed",
            Self::LspServerStopped => "LspServerStopped",
            Self::LspBufferAttached => "LspBufferAttached",
            Self::LspBufferDetached => "LspBufferDetached",
            Self::CommandExecuted => "CommandExecuted",
            Self::DiagnosticsChanged => "DiagnosticsChanged",
        }
    }
}

impl fmt::Display for PluginEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PluginEventKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "EditorStarted" => Ok(Self::EditorStarted),
            "BufferOpened" => Ok(Self::BufferOpened),
            "BufferLoaded" => Ok(Self::BufferLoaded),
            "BufferSaved" => Ok(Self::BufferSaved),
            "BufferSaveFailed" => Ok(Self::BufferSaveFailed),
            "BufferOpenFailed" => Ok(Self::BufferOpenFailed),
            "BufferClosed" => Ok(Self::BufferClosed),
            "BufferUnloaded" => Ok(Self::BufferUnloaded),
            "BufferPathChanged" => Ok(Self::BufferPathChanged),
            "BufferReloaded" => Ok(Self::BufferReloaded),
            "ExternalFileConflict" => Ok(Self::ExternalFileConflict),
            "BufferChanged" => Ok(Self::BufferChanged),
            "InsertBufferChanged" => Ok(Self::InsertBufferChanged),
            "InsertSessionChanged" => Ok(Self::InsertSessionChanged),
            "BufferModifiedChanged" => Ok(Self::BufferModifiedChanged),
            "BufferFiletypeChanged" => Ok(Self::BufferFiletypeChanged),
            "PaneCreated" => Ok(Self::PaneCreated),
            "PaneClosed" => Ok(Self::PaneClosed),
            "PaneFocused" => Ok(Self::PaneFocused),
            "TabOpened" => Ok(Self::TabOpened),
            "TabClosed" => Ok(Self::TabClosed),
            "TabActivated" => Ok(Self::TabActivated),
            "ActiveBufferChanged" => Ok(Self::ActiveBufferChanged),
            "ModeChanged" => Ok(Self::ModeChanged),
            "CursorMoved" => Ok(Self::CursorMoved),
            "SelectionChanged" => Ok(Self::SelectionChanged),
            "EditorWillShutdown" => Ok(Self::EditorWillShutdown),
            "ThemeChanged" => Ok(Self::ThemeChanged),
            "LspServerStarted" => Ok(Self::LspServerStarted),
            "LspServerStartFailed" => Ok(Self::LspServerStartFailed),
            "LspServerStopped" => Ok(Self::LspServerStopped),
            "LspBufferAttached" => Ok(Self::LspBufferAttached),
            "LspBufferDetached" => Ok(Self::LspBufferDetached),
            "CommandExecuted" => Ok(Self::CommandExecuted),
            "DiagnosticsChanged" => Ok(Self::DiagnosticsChanged),
            other => Err(format!("unknown plugin event {other:?}")),
        }
    }
}

/// Runtime registry of plugin-provided contributions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginContributionRegistry {
    apis: BTreeMap<String, BTreeMap<String, DynamicPluginApi>>,
    commands: BTreeMap<String, BTreeMap<String, DynamicPluginCommand>>,
    themes: BTreeMap<String, BTreeMap<String, DynamicPluginTheme>>,
    filetypes: BTreeMap<String, BTreeMap<String, DynamicFiletype>>,
    filetype_extensions: BTreeMap<String, String>,
    syntax_providers: BTreeMap<String, BTreeMap<u64, DynamicSyntaxProvider>>,
    event_hooks: BTreeMap<String, BTreeMap<PluginEventKind, BTreeSet<u64>>>,
}

impl PluginContributionRegistry {
    /// Registers or replaces an API endpoint for `plugin`.
    pub fn register_api(
        &mut self,
        plugin: impl Into<String>,
        api: DynamicPluginApi,
    ) -> Result<(), String> {
        validate_contribution_name(&api.name, "plugin API name")?;
        self.apis
            .entry(plugin.into())
            .or_default()
            .insert(api.name.clone(), api);
        Ok(())
    }

    /// Unregisters an API endpoint owned by `plugin`.
    pub fn unregister_api(&mut self, plugin: &str, api: &str) -> bool {
        self.apis
            .get_mut(plugin)
            .and_then(|apis| apis.remove(api))
            .is_some()
    }

    /// Unregisters every API endpoint owned by `plugin`.
    pub fn unregister_plugin_apis(&mut self, plugin: &str) -> bool {
        self.apis.remove(plugin).is_some()
    }

    /// Looks up an API endpoint owned by `plugin`.
    pub fn api(&self, plugin: &str, api: &str) -> Option<&DynamicPluginApi> {
        self.apis.get(plugin).and_then(|apis| apis.get(api))
    }

    /// Returns API endpoints exposed by `plugin` in name order.
    pub fn apis(&self, plugin: &str) -> impl Iterator<Item = &DynamicPluginApi> {
        self.apis
            .get(plugin)
            .into_iter()
            .flat_map(|apis| apis.values())
    }

    /// Returns whether `plugin` exposes `api`.
    pub fn has_api(&self, plugin: &str, api: &str) -> bool {
        self.api(plugin, api).is_some()
    }

    /// Registers or replaces a command for `plugin`.
    pub fn register_command(
        &mut self,
        plugin: impl Into<String>,
        command: DynamicPluginCommand,
    ) -> Result<(), String> {
        validate_contribution_name(&command.name, "plugin command name")?;
        self.commands
            .entry(plugin.into())
            .or_default()
            .insert(command.name.clone(), command);
        Ok(())
    }

    /// Unregisters a command owned by `plugin`.
    pub fn unregister_command(&mut self, plugin: &str, command: &str) -> bool {
        self.commands
            .get_mut(plugin)
            .and_then(|commands| commands.remove(command))
            .is_some()
    }

    /// Looks up a command owned by `plugin`.
    pub fn command(&self, plugin: &str, command: &str) -> Option<&DynamicPluginCommand> {
        self.commands
            .get(plugin)
            .and_then(|commands| commands.get(command))
    }

    /// Returns registered commands for `plugin` in name order.
    pub fn commands(&self, plugin: &str) -> impl Iterator<Item = &DynamicPluginCommand> {
        self.commands
            .get(plugin)
            .into_iter()
            .flat_map(|commands| commands.values())
    }

    /// Registers or replaces dynamic theme ownership for `plugin`.
    pub fn register_theme(
        &mut self,
        plugin: impl Into<String>,
        theme: DynamicPluginTheme,
    ) -> Result<(), String> {
        if theme.name.trim().is_empty() {
            return Err("plugin theme name must not be empty".to_string());
        }
        self.themes
            .entry(plugin.into())
            .or_default()
            .insert(theme.name.clone(), theme);
        Ok(())
    }

    /// Unregisters dynamic theme ownership for `plugin`.
    pub fn unregister_theme(&mut self, plugin: &str, theme: &str) -> Option<DynamicPluginTheme> {
        self.themes
            .get_mut(plugin)
            .and_then(|themes| themes.remove(theme))
    }

    /// Looks up a dynamic theme owned by `plugin`.
    pub fn theme(&self, plugin: &str, theme: &str) -> Option<&DynamicPluginTheme> {
        self.themes.get(plugin).and_then(|themes| themes.get(theme))
    }

    /// Registers or replaces a filetype owned by `plugin`.
    pub fn register_filetype(
        &mut self,
        plugin: impl Into<String>,
        filetype: DynamicFiletype,
    ) -> Result<(), String> {
        validate_contribution_name(&filetype.name, "filetype")?;
        self.filetypes
            .entry(plugin.into())
            .or_default()
            .insert(filetype.name.clone(), filetype);
        Ok(())
    }

    /// Registers an extension-based filetype detector.
    pub fn detect_filetype_extension(
        &mut self,
        extension: impl Into<String>,
        filetype: impl Into<String>,
    ) -> Result<(), String> {
        let mut extension = extension.into();
        if let Some(stripped) = extension.strip_prefix('.') {
            extension = stripped.to_string();
        }
        validate_contribution_name(&extension, "filetype extension")?;
        let filetype = filetype.into();
        validate_contribution_name(&filetype, "filetype")?;
        self.filetype_extensions.insert(extension, filetype);
        Ok(())
    }

    /// Resolves a plugin-registered filetype from a path extension.
    pub fn filetype_for_extension(&self, extension: &str) -> Option<&str> {
        self.filetype_extensions.get(extension).map(String::as_str)
    }

    /// Returns all registered plugin filetype names in sorted order.
    pub fn filetype_names(&self) -> Vec<String> {
        let mut names = self
            .filetypes
            .values()
            .flat_map(|filetypes| filetypes.keys().cloned())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    /// Registers or replaces a syntax provider owned by `plugin`.
    pub fn register_syntax_provider(
        &mut self,
        plugin: impl Into<String>,
        provider: DynamicSyntaxProvider,
    ) -> Result<(), String> {
        validate_contribution_name(&provider.filetype, "syntax provider filetype")?;
        self.syntax_providers
            .entry(plugin.into())
            .or_default()
            .insert(provider.id, provider);
        Ok(())
    }

    /// Unregisters a syntax provider owned by `plugin`.
    pub fn unregister_syntax_provider(&mut self, plugin: &str, provider_id: u64) -> bool {
        self.syntax_providers
            .get_mut(plugin)
            .and_then(|providers| providers.remove(&provider_id))
            .is_some()
    }

    /// Returns the first syntax provider registered for `filetype` in stable order.
    pub fn syntax_provider_for_filetype(
        &self,
        filetype: &str,
    ) -> Option<(&str, &DynamicSyntaxProvider)> {
        self.syntax_providers
            .iter()
            .find_map(|(plugin, providers)| {
                providers
                    .values()
                    .find(|provider| provider.filetype == filetype)
                    .map(|provider| (plugin.as_str(), provider))
            })
    }

    /// Returns registered syntax providers for `plugin` in id order.
    pub fn syntax_providers(&self, plugin: &str) -> impl Iterator<Item = &DynamicSyntaxProvider> {
        self.syntax_providers
            .get(plugin)
            .into_iter()
            .flat_map(|providers| providers.values())
    }

    /// Registers an event hook callback for `plugin`.
    pub fn register_event_hook(
        &mut self,
        plugin: impl Into<String>,
        event: PluginEventKind,
        hook_id: u64,
    ) -> Result<(), String> {
        self.event_hooks
            .entry(plugin.into())
            .or_default()
            .entry(event)
            .or_default()
            .insert(hook_id);
        Ok(())
    }

    /// Unregisters an event hook callback owned by `plugin`.
    pub fn unregister_event_hook(&mut self, plugin: &str, hook_id: u64) -> bool {
        let Some(events) = self.event_hooks.get_mut(plugin) else {
            return false;
        };

        let mut removed = false;
        events.retain(|_, hooks| {
            removed |= hooks.remove(&hook_id);
            !hooks.is_empty()
        });
        removed
    }

    /// Returns hook ids registered for `plugin` and `event` in sorted order.
    pub fn event_hooks(
        &self,
        plugin: &str,
        event: PluginEventKind,
    ) -> impl Iterator<Item = u64> + '_ {
        self.event_hooks
            .get(plugin)
            .and_then(|events| events.get(&event))
            .into_iter()
            .flat_map(|hooks| hooks.iter().copied())
    }

    /// Returns all registered hook targets for `event` in sorted plugin/hook order.
    pub fn event_hook_targets(&self, event: PluginEventKind) -> impl Iterator<Item = (&str, u64)> {
        self.event_hooks.iter().flat_map(move |(plugin, events)| {
            events.get(&event).into_iter().flat_map(move |hooks| {
                hooks
                    .iter()
                    .copied()
                    .map(move |hook_id| (plugin.as_str(), hook_id))
            })
        })
    }

    /// Returns the total number of registered event hook functions for `plugin`.
    pub fn event_hook_count(&self, plugin: &str) -> usize {
        self.event_hooks
            .get(plugin)
            .map(|events| events.values().map(BTreeSet::len).sum())
            .unwrap_or(0)
    }

    /// Returns a snapshot of all contributions, optionally filtered by plugin.
    pub fn snapshot(&self, plugin_filter: Option<&str>) -> Vec<PluginContributionSnapshot> {
        let mut snapshots = Vec::new();
        let plugins: Vec<String> = match plugin_filter {
            Some(name) => vec![name.to_string()],
            None => {
                let mut names = BTreeSet::new();
                names.extend(self.apis.keys().cloned());
                names.extend(self.commands.keys().cloned());
                names.extend(self.themes.keys().cloned());
                names.extend(self.filetypes.keys().cloned());
                names.extend(self.syntax_providers.keys().cloned());
                names.extend(self.event_hooks.keys().cloned());
                names.into_iter().collect()
            }
        };

        for plugin in plugins {
            let apis: Vec<DynamicPluginApi> = self
                .apis
                .get(&plugin)
                .map(|apis| apis.values().cloned().collect())
                .unwrap_or_default();
            let commands: Vec<DynamicPluginCommand> = self
                .commands
                .get(&plugin)
                .map(|commands| commands.values().cloned().collect())
                .unwrap_or_default();
            let themes: Vec<DynamicPluginTheme> = self
                .themes
                .get(&plugin)
                .map(|themes| themes.values().cloned().collect())
                .unwrap_or_default();
            let event_hooks: Vec<(PluginEventKind, u64)> = self
                .event_hooks
                .get(&plugin)
                .map(|events| {
                    events
                        .iter()
                        .flat_map(|(event, hooks)| {
                            hooks.iter().map(move |hook_id| (*event, *hook_id))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let syntax_providers: Vec<DynamicSyntaxProvider> = self
                .syntax_providers
                .get(&plugin)
                .map(|providers| providers.values().cloned().collect())
                .unwrap_or_default();
            let filetypes: Vec<DynamicFiletype> = self
                .filetypes
                .get(&plugin)
                .map(|filetypes| filetypes.values().cloned().collect())
                .unwrap_or_default();

            if !apis.is_empty()
                || !commands.is_empty()
                || !themes.is_empty()
                || !filetypes.is_empty()
                || !syntax_providers.is_empty()
                || !event_hooks.is_empty()
            {
                snapshots.push(PluginContributionSnapshot {
                    plugin,
                    apis,
                    commands,
                    themes,
                    filetypes,
                    syntax_providers,
                    event_hooks,
                });
            }
        }

        snapshots
    }
}

/// Snapshot of a plugin's contributions for introspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginContributionSnapshot {
    /// Plugin name.
    pub plugin: String,
    /// Exposed API endpoints.
    pub apis: Vec<DynamicPluginApi>,
    /// Registered dynamic commands.
    pub commands: Vec<DynamicPluginCommand>,
    /// Registered dynamic themes.
    pub themes: Vec<DynamicPluginTheme>,
    /// Registered filetypes.
    pub filetypes: Vec<DynamicFiletype>,
    /// Registered syntax providers.
    pub syntax_providers: Vec<DynamicSyntaxProvider>,
    /// Registered event hooks as (event, hook id) pairs.
    pub event_hooks: Vec<(PluginEventKind, u64)>,
}

/// Validates a plugin contribution name.
pub fn validate_contribution_name(name: &str, label: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if name.chars().any(char::is_whitespace) {
        return Err(format!("{label} {name:?} must not contain whitespace"));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(format!("{label} {name:?} must not contain path separators"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_event_names_round_trip() {
        for &event in PluginEventKind::ALL {
            assert_eq!(event.as_str().parse::<PluginEventKind>(), Ok(event));
            assert_eq!(event.to_string(), event.as_str());
        }
    }

    #[test]
    fn registers_replaces_and_unregisters_commands() {
        let mut registry = PluginContributionRegistry::default();
        registry
            .register_command(
                "demo",
                DynamicPluginCommand {
                    name: "hello".to_string(),
                    description: Some("Say hello".to_string()),
                },
            )
            .expect("command should register");

        assert_eq!(registry.command("demo", "hello").unwrap().name, "hello");
        assert!(registry.unregister_command("demo", "hello"));
        assert!(registry.command("demo", "hello").is_none());
    }

    #[test]
    fn registers_lists_and_unregisters_apis() {
        let mut registry = PluginContributionRegistry::default();
        registry
            .register_api(
                "demo",
                DynamicPluginApi {
                    name: "lookup.v1".to_string(),
                },
            )
            .expect("API should register");

        assert!(registry.has_api("demo", "lookup.v1"));
        assert_eq!(
            registry
                .apis("demo")
                .map(|api| api.name.as_str())
                .collect::<Vec<_>>(),
            vec!["lookup.v1"]
        );
        assert!(registry.unregister_plugin_apis("demo"));
        assert!(!registry.has_api("demo", "lookup.v1"));
    }

    #[test]
    fn registers_event_hooks() {
        let mut registry = PluginContributionRegistry::default();
        registry
            .register_event_hook("demo", PluginEventKind::BufferSaved, 7)
            .expect("hook should register");

        assert_eq!(
            registry
                .event_hooks("demo", PluginEventKind::BufferSaved)
                .collect::<Vec<_>>(),
            vec![7]
        );
    }

    #[test]
    fn registers_syntax_providers() {
        let mut registry = PluginContributionRegistry::default();
        registry
            .register_syntax_provider(
                "demo",
                DynamicSyntaxProvider {
                    id: 3,
                    filetype: "simplelang".to_string(),
                },
            )
            .expect("syntax provider should register");

        let (plugin, provider) = registry
            .syntax_provider_for_filetype("simplelang")
            .expect("provider should be found");
        assert_eq!(plugin, "demo");
        assert_eq!(provider.id, 3);
        assert!(registry.unregister_syntax_provider("demo", 3));
        assert!(
            registry
                .syntax_provider_for_filetype("simplelang")
                .is_none()
        );
    }
}
