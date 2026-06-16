//! Startup configuration loading and resolution.
//!
//! This module loads the editor's TOML config file from the XDG config
//! directories, merges it with command-line overrides, and produces a resolved
//! configuration object that can be stored globally.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::editor::validate_key_string;
use crate::lsp::builtin::builtin_lsp_config;
use crate::theme::Tag;
use smol_str::SmolStr;
use toml::Value;

const DEFAULT_THEME: &str = "Friday Night";
const DEFAULT_TODO_MARKERS: [&str; 4] = ["TODO", "FIXME", "BUG", "NOTE"];
const CONFIG_RELATIVE_PATH: &str = "urvim/config.toml";
const DEFAULT_XDG_CONFIG_DIRS: &str = "/etc/xdg";

/// Advanced glyph capabilities that can be enabled through configuration.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvancedGlyphCapability {
    /// Enable nerdfont glyph rendering.
    Nerdfont,
    /// Enable all Unicode advanced glyph capabilities.
    Unicode,
    /// Enable Unicode line-drawing split borders.
    UnicodeBorders,
    /// Enable Unicode line-drawing indent guides.
    UnicodeIndent,
    /// Enable Unicode fold gutter glyphs.
    UnicodeFolds,
}

/// Enabled inlay-hint kinds that can be configured through startup config.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InlayHintCapability {
    /// Enable type inlay hints.
    Type,
    /// Enable parameter inlay hints.
    Parameter,
}

/// How insert-mode tab presses should insert text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabInsertion {
    /// Insert literal tab characters.
    Tabs,
    /// Insert spaces instead of literal tab characters.
    #[default]
    Spaces,
}

/// How insert-mode tab presses should resolve the insertion style.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabBehavior {
    /// Always use the configured tab insertion setting.
    #[default]
    Simple,
    /// Infer indentation style from the active buffer when possible.
    Smart,
}

/// How insert-mode newline creation should resolve indentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AutoIndentMode {
    /// Do not add automatic indentation to new lines.
    #[default]
    Off,
    /// Reuse nearby indentation from the active buffer when possible.
    Neighbor,
}

/// Controls whether insert-mode completion may start automatically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompletionTrigger {
    /// Only start completion from explicit user actions.
    #[default]
    Manual,
    /// Allow completion to start automatically as well as manually.
    Auto,
}

/// How long logical lines should be wrapped when visual wrapping is enabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WrapMode {
    /// Break at the exact render width.
    #[default]
    Hard,
    /// Prefer the nearest word boundary at or before the render width.
    Soft,
}

/// The resolved default register selectors used by yank, delete, and change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultRegisters {
    /// The register selector used by yank operations.
    pub yank: char,
    /// The register selector used by delete operations.
    pub delete: char,
    /// The register selector used by change operations.
    pub change: char,
}

/// The TOML-backed default register table stored in the config file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PartialDefaultRegisters {
    /// The register selector configured for yank operations.
    pub yank: Option<String>,
    /// The register selector configured for delete operations.
    pub delete: Option<String>,
    /// The register selector configured for change operations.
    pub change: Option<String>,
}

impl Default for DefaultRegisters {
    fn default() -> Self {
        Self {
            yank: 'y',
            delete: 'd',
            change: 'c',
        }
    }
}

/// Default visual width for tab characters when no config is available.
pub const DEFAULT_TAB_WIDTH: usize = 4;
/// Default top/bottom and left/right visual scroll margin.
pub const DEFAULT_SCROLL_MARGIN: usize = 5;

/// Visual scroll margins that determine when viewport scrolling starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollMargin {
    /// Number of lines to keep between the cursor and top/bottom viewport edges.
    pub vertical: usize,
    /// Number of columns to keep between the cursor and left/right viewport edges.
    pub horizontal: usize,
}

/// The TOML-backed visual scroll margin table stored in the config file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PartialScrollMargin {
    /// The vertical margin stored in the config file.
    pub vertical: Option<usize>,
    /// The horizontal margin stored in the config file.
    pub horizontal: Option<usize>,
}

impl Default for ScrollMargin {
    fn default() -> Self {
        Self {
            vertical: DEFAULT_SCROLL_MARGIN,
            horizontal: DEFAULT_SCROLL_MARGIN,
        }
    }
}

/// The resolved LSP configuration used by the editor.
#[derive(Clone, Debug, PartialEq)]
pub struct LspConfig {
    /// The resolved server configuration map keyed by server name.
    pub servers: BTreeMap<String, LspServerConfig>,
}

/// The TOML-backed LSP config table stored in the config file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct PartialLspConfig {
    /// The server override table keyed by server name.
    #[serde(flatten)]
    pub servers: BTreeMap<String, PartialLspServerConfig>,
}

/// The resolved configuration for a single LSP server.
#[derive(Clone, Debug, PartialEq)]
pub struct LspServerConfig {
    /// Whether the server is enabled.
    pub enabled: bool,
    /// The executable command used to launch the server.
    pub command: String,
    /// Additional command-line arguments.
    pub args: Vec<String>,
    /// Environment variables passed to the server process.
    pub env: BTreeMap<String, String>,
    /// The filetypes that should attach to this server.
    pub filetypes: Vec<String>,
    /// The root markers used to discover workspace roots.
    pub root_markers: Vec<String>,
    /// Free-form server settings.
    pub settings: Value,
}

/// Resolved custom keymaps loaded from startup config.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeymapsConfig {
    /// Normal-mode mappings from canonical key strings to command strings.
    pub normal: BTreeMap<String, String>,
    /// Insert-mode mappings from canonical key strings to command strings.
    pub insert: BTreeMap<String, String>,
    /// Visual-mode mappings from canonical key strings to command strings.
    pub visual: BTreeMap<String, String>,
    /// Linewise visual-mode mappings from canonical key strings to command strings.
    pub visual_line: BTreeMap<String, String>,
    /// Resize-mode mappings from canonical key strings to command strings.
    pub resizing: BTreeMap<String, String>,
}

/// TOML-backed custom keymap tables stored in the config file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PartialKeymapsConfig {
    /// Normal-mode mappings from canonical key strings to command strings.
    pub normal: Option<BTreeMap<String, String>>,
    /// Insert-mode mappings from canonical key strings to command strings.
    pub insert: Option<BTreeMap<String, String>>,
    /// Visual-mode mappings from canonical key strings to command strings.
    pub visual: Option<BTreeMap<String, String>>,
    /// Linewise visual-mode mappings from canonical key strings to command strings.
    pub visual_line: Option<BTreeMap<String, String>>,
    /// Resize-mode mappings from canonical key strings to command strings.
    pub resizing: Option<BTreeMap<String, String>>,
}

/// The TOML-backed config table for a single LSP server.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PartialLspServerConfig {
    /// Whether the server is enabled.
    pub enabled: Option<bool>,
    /// The executable command stored in the config file.
    pub command: Option<String>,
    /// The command-line arguments stored in the config file.
    pub args: Option<Vec<String>>,
    /// Environment variables stored in the config file.
    pub env: Option<BTreeMap<String, String>>,
    /// The filetypes stored in the config file.
    pub filetypes: Option<Vec<String>>,
    /// The root markers stored in the config file.
    pub root_markers: Option<Vec<String>>,
    /// Free-form server settings stored in the config file.
    pub settings: Option<Value>,
}

impl Default for LspConfig {
    fn default() -> Self {
        builtin_lsp_config().clone()
    }
}

impl Default for LspServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            filetypes: Vec::new(),
            root_markers: Vec::new(),
            settings: Value::Table(Default::default()),
        }
    }
}

/// The resolved startup configuration used by the editor.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The active theme name selected after merging file and CLI inputs.
    pub theme: String,
    /// Custom editor mode keymaps loaded from config.
    pub keymaps: KeymapsConfig,
    /// The resolved default register selectors for yank, delete, and change.
    pub default_registers: DefaultRegisters,
    /// Whether syntax highlighting is enabled for rendered buffers.
    pub syntax: bool,
    /// Whether insert mode should auto-close supported bracket and quote pairs.
    pub auto_close_pairs: bool,
    /// Whether the active line should be highlighted in the focused window.
    pub active_line: bool,
    /// Whether to render relative gutter line numbers.
    pub relative_number: bool,
    /// Whether to render the active indent scope guide.
    pub indent_guides: bool,
    /// The configured comment todo markers used for highlighting.
    pub todo_markers: Vec<SmolStr>,
    /// User-defined command aliases parsed into command-token prefixes.
    pub aliases: BTreeMap<String, Vec<String>>,
    /// User-defined command scripts stored as ordered command lines.
    pub scripts: BTreeMap<String, Vec<String>>,
    /// How insert mode should resolve indentation for newly created lines.
    pub auto_indent: AutoIndentMode,
    /// Whether insert-mode completion may trigger automatically.
    pub completion_trigger: CompletionTrigger,
    /// The ordered list of completion source names.
    pub completion_sources: Vec<String>,
    /// Enabled advanced glyph capabilities.
    pub advanced_glyphs: BTreeSet<AdvancedGlyphCapability>,
    /// Enabled inlay-hint kinds.
    pub inlay_hints: BTreeSet<InlayHintCapability>,
    /// The configured insert-mode tab insertion setting.
    pub tab_insertion: TabInsertion,
    /// The configured insert-mode tab behavior setting.
    pub tab_behavior: TabBehavior,
    /// The number of visual columns a tab occupies.
    pub tab_width: usize,
    /// Visual scroll margins that trigger viewport movement before edge crossing.
    pub scroll_margin: ScrollMargin,
    /// How visual line wrapping should break lines when enabled per window.
    pub wrap_mode: WrapMode,
    /// The resolved LSP server configuration.
    pub lsp: LspConfig,
}

/// The TOML-backed config file schema.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PartialConfig {
    /// The theme name stored in the config file.
    pub theme: Option<String>,
    /// Custom editor mode keymaps stored in the config file.
    pub keymaps: Option<PartialKeymapsConfig>,
    /// The default register table stored in the config file.
    pub default_registers: Option<PartialDefaultRegisters>,
    /// Whether syntax highlighting is enabled in the config file.
    pub syntax: Option<bool>,
    /// Whether insert mode should auto-close supported bracket and quote pairs.
    pub auto_close_pairs: Option<bool>,
    /// Whether the active line should be highlighted in the focused window.
    pub active_line: Option<bool>,
    /// Whether to render relative gutter line numbers.
    pub relative_number: Option<bool>,
    /// Whether to render the active indent scope guide.
    pub indent_guides: Option<bool>,
    /// The todo marker list stored in the config file.
    pub todo_markers: Option<Vec<String>>,
    /// User-defined command aliases stored in the config file.
    pub aliases: Option<BTreeMap<String, String>>,
    /// User-defined command scripts stored in the config file.
    pub scripts: Option<BTreeMap<String, Vec<String>>>,
    /// How insert mode should resolve indentation for newly created lines.
    pub auto_indent: Option<AutoIndentMode>,
    /// Whether insert-mode completion may trigger automatically.
    pub completion_trigger: Option<CompletionTrigger>,
    /// The ordered list of completion source names stored in the config file.
    pub completion_sources: Option<Vec<String>>,
    /// Enabled advanced glyph capabilities in the config file.
    pub advanced_glyphs: Option<Vec<AdvancedGlyphCapability>>,
    /// Enabled inlay-hint kinds in the config file.
    pub inlay_hints: Option<Vec<InlayHintCapability>>,
    /// The tab insertion setting stored in the config file.
    pub tab_insertion: Option<TabInsertion>,
    /// The tab behavior setting stored in the config file.
    pub tab_behavior: Option<TabBehavior>,
    /// The tab width stored in the config file.
    pub tab_width: Option<usize>,
    /// The visual scroll margin table stored in the config file.
    pub scroll_margin: Option<PartialScrollMargin>,
    /// The wrap strategy stored in the config file.
    pub wrap_mode: Option<WrapMode>,
    /// The LSP config table stored in the config file.
    pub lsp: Option<PartialLspConfig>,
}

/// Errors that can occur while loading or resolving startup configuration.
#[derive(Debug)]
pub enum ConfigLoadError {
    /// The config file could not be read from disk.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The config file could not be parsed as TOML.
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    /// The configuration file could not be resolved into a valid startup config.
    Invalid { message: String },
    /// No HOME directory was available to derive the default XDG config home.
    MissingHomeDir,
}

impl Config {
    /// Loads, merges, and resolves startup configuration from the environment.
    pub fn load(
        cli_theme: Option<&str>,
        cli_syntax: Option<bool>,
    ) -> Result<Self, ConfigLoadError> {
        let config_home = xdg_config_home()?;
        let config_dirs = xdg_config_dirs();
        Self::load_from_locations(config_home, config_dirs, cli_theme, cli_syntax)
    }

    /// Loads, merges, and resolves startup configuration from explicit XDG paths.
    pub fn load_from_locations(
        config_home: PathBuf,
        config_dirs: Vec<PathBuf>,
        cli_theme: Option<&str>,
        cli_syntax: Option<bool>,
    ) -> Result<Self, ConfigLoadError> {
        let file = load_config_file(config_home, config_dirs)?;
        Ok(Self::resolve(file.as_ref(), cli_theme, cli_syntax))
    }

    /// Resolves the final config by applying CLI overrides on top of file values.
    pub fn resolve(
        file: Option<&PartialConfig>,
        cli_theme: Option<&str>,
        cli_syntax: Option<bool>,
    ) -> Self {
        let theme = cli_theme
            .map(ToOwned::to_owned)
            .or_else(|| file.and_then(|config| config.theme.clone()))
            .unwrap_or_else(|| DEFAULT_THEME.to_string());
        let keymaps = file
            .and_then(|config| config.keymaps.as_ref())
            .map(resolve_keymaps)
            .unwrap_or_default();
        let default_registers = file
            .and_then(|config| config.default_registers.as_ref())
            .map(resolve_default_registers)
            .unwrap_or_default();
        let syntax = cli_syntax
            .or_else(|| file.and_then(|config| config.syntax))
            .unwrap_or(true);
        let auto_close_pairs = file
            .and_then(|config| config.auto_close_pairs)
            .unwrap_or(true);
        let active_line = file.and_then(|config| config.active_line).unwrap_or(false);
        let relative_number = file
            .and_then(|config| config.relative_number)
            .unwrap_or(false);
        let indent_guides = file.and_then(|config| config.indent_guides).unwrap_or(true);
        let todo_markers = file
            .and_then(|config| config.todo_markers.clone())
            .map(|markers| markers.into_iter().map(SmolStr::new).collect())
            .unwrap_or_else(default_todo_markers);
        let aliases = file
            .and_then(|config| config.aliases.as_ref())
            .map(resolve_aliases)
            .unwrap_or_default();
        let scripts = file
            .and_then(|config| config.scripts.clone())
            .unwrap_or_default();
        let auto_indent = file
            .and_then(|config| config.auto_indent)
            .unwrap_or_default();
        let completion_trigger = file
            .and_then(|config| config.completion_trigger)
            .unwrap_or_default();
        let completion_sources = file
            .and_then(|config| config.completion_sources.clone())
            .unwrap_or_else(default_completion_sources);
        let advanced_glyphs = file
            .and_then(|config| config.advanced_glyphs.as_ref())
            .map(|glyphs| resolve_advanced_glyphs(glyphs))
            .unwrap_or_else(default_advanced_glyphs);
        let inlay_hints = file
            .and_then(|config| config.inlay_hints.as_ref())
            .map(|kinds| kinds.iter().cloned().collect())
            .unwrap_or_else(default_inlay_hint_kinds);
        let tab_insertion = file
            .and_then(|config| config.tab_insertion)
            .unwrap_or_default();
        let tab_behavior = file
            .and_then(|config| config.tab_behavior)
            .unwrap_or_default();
        let tab_width = file
            .and_then(|config| config.tab_width)
            .unwrap_or(DEFAULT_TAB_WIDTH);
        let scroll_margin =
            resolve_scroll_margin(file.and_then(|config| config.scroll_margin.as_ref()));
        let wrap_mode = file.and_then(|config| config.wrap_mode).unwrap_or_default();
        let lsp = resolve_lsp(file.and_then(|config| config.lsp.as_ref()));

        Self {
            theme,
            keymaps,
            default_registers,
            syntax,
            auto_close_pairs,
            active_line,
            relative_number,
            indent_guides,
            todo_markers,
            aliases,
            scripts,
            auto_indent,
            completion_trigger,
            completion_sources,
            advanced_glyphs,
            inlay_hints,
            tab_insertion,
            tab_behavior,
            tab_width,
            scroll_margin,
            wrap_mode,
            lsp,
        }
    }

    /// Returns whether nerdfont glyph rendering is enabled.
    pub fn nerdfont_enabled(&self) -> bool {
        self.advanced_glyphs
            .contains(&AdvancedGlyphCapability::Nerdfont)
    }

    /// Returns whether general Unicode glyph rendering is enabled.
    pub fn unicode_enabled(&self) -> bool {
        self.advanced_glyphs
            .contains(&AdvancedGlyphCapability::Unicode)
    }

    /// Returns whether Unicode split borders are enabled.
    pub fn unicode_borders_enabled(&self) -> bool {
        self.advanced_glyphs
            .contains(&AdvancedGlyphCapability::UnicodeBorders)
    }

    /// Returns whether Unicode indent-guide glyph rendering is enabled.
    pub fn unicode_indent_enabled(&self) -> bool {
        self.advanced_glyphs
            .contains(&AdvancedGlyphCapability::UnicodeIndent)
    }

    /// Returns whether Unicode fold gutter glyph rendering is enabled.
    pub fn unicode_folds_enabled(&self) -> bool {
        self.advanced_glyphs
            .contains(&AdvancedGlyphCapability::UnicodeFolds)
    }

    /// Returns whether any inlay hints are enabled.
    pub fn inlay_hints_enabled(&self) -> bool {
        !self.inlay_hints.is_empty()
    }

    /// Returns whether the given inlay-hint kind is enabled.
    pub fn inlay_hint_kind_enabled(&self, kind: &InlayHintCapability) -> bool {
        self.inlay_hints.contains(kind)
    }
}

impl ConfigLoadError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: DEFAULT_THEME.to_string(),
            keymaps: KeymapsConfig::default(),
            default_registers: DefaultRegisters::default(),
            syntax: true,
            auto_close_pairs: true,
            active_line: false,
            relative_number: false,
            indent_guides: true,
            todo_markers: default_todo_markers(),
            aliases: BTreeMap::new(),
            scripts: BTreeMap::new(),
            auto_indent: AutoIndentMode::default(),
            completion_trigger: CompletionTrigger::default(),
            completion_sources: default_completion_sources(),
            advanced_glyphs: default_advanced_glyphs(),
            inlay_hints: default_inlay_hint_kinds(),
            tab_insertion: TabInsertion::default(),
            tab_behavior: TabBehavior::default(),
            tab_width: DEFAULT_TAB_WIDTH,
            scroll_margin: ScrollMargin::default(),
            wrap_mode: WrapMode::default(),
            lsp: LspConfig::default(),
        }
    }
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "failed to read config {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse config {}: {source}", path.display())
            }
            Self::Invalid { message } => write!(f, "{message}"),
            Self::MissingHomeDir => write!(f, "could not determine XDG config home directory"),
        }
    }
}

impl std::error::Error for ConfigLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Invalid { .. } | Self::MissingHomeDir => None,
        }
    }
}

fn load_config_file(
    config_home: PathBuf,
    config_dirs: Vec<PathBuf>,
) -> Result<Option<PartialConfig>, ConfigLoadError> {
    for candidate in config_paths(config_home, config_dirs) {
        if candidate.exists() {
            let contents =
                fs::read_to_string(&candidate).map_err(|source| ConfigLoadError::Io {
                    path: candidate.clone(),
                    source,
                })?;
            let config = toml::from_str::<PartialConfig>(&contents).map_err(|source| {
                ConfigLoadError::Parse {
                    path: candidate.clone(),
                    source,
                }
            })?;
            validate_partial_config(&config)?;
            return Ok(Some(config));
        }
    }

    Ok(None)
}

fn config_paths(config_home: PathBuf, config_dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(1 + config_dirs.len());
    paths.push(config_home.join(CONFIG_RELATIVE_PATH));
    paths.extend(
        config_dirs
            .into_iter()
            .map(|dir| dir.join(CONFIG_RELATIVE_PATH)),
    );
    paths
}

fn validate_partial_config(config: &PartialConfig) -> Result<(), ConfigLoadError> {
    if let Some(theme) = config.theme.as_ref()
        && theme.trim().is_empty()
    {
        return Err(ConfigLoadError::invalid(
            "config theme must not be empty or whitespace",
        ));
    }

    if let Some(default_registers) = config.default_registers.as_ref() {
        validate_default_registers(default_registers)?;
    }

    if let Some(tab_width) = config.tab_width
        && tab_width == 0
    {
        return Err(ConfigLoadError::invalid(
            "config tab_width must be greater than zero",
        ));
    }

    if let Some(markers) = config.todo_markers.as_ref() {
        validate_todo_markers(markers)?;
    }

    validate_configured_commands(config)?;

    if let Some(keymaps) = config.keymaps.as_ref() {
        validate_keymaps(keymaps)?;
    }

    if let Some(sources) = config.completion_sources.as_ref() {
        validate_completion_sources(sources)?;
    }

    if let Some(trigger) = config.completion_trigger {
        validate_completion_trigger(trigger)?;
    }

    if let Some(lsp) = config.lsp.as_ref() {
        validate_partial_lsp_config(lsp)?;
    }

    Ok(())
}

fn validate_keymaps(keymaps: &PartialKeymapsConfig) -> Result<(), ConfigLoadError> {
    if let Some(normal) = keymaps.normal.as_ref() {
        validate_keymap_table("normal", normal)?;
    }
    if let Some(insert) = keymaps.insert.as_ref() {
        validate_keymap_table("insert", insert)?;
    }
    if let Some(visual) = keymaps.visual.as_ref() {
        validate_keymap_table("visual", visual)?;
    }
    if let Some(visual_line) = keymaps.visual_line.as_ref() {
        validate_keymap_table("visual_line", visual_line)?;
    }
    if let Some(resizing) = keymaps.resizing.as_ref() {
        validate_keymap_table("resizing", resizing)?;
    }

    Ok(())
}

fn validate_keymap_table(
    mode: &str,
    mappings: &BTreeMap<String, String>,
) -> Result<(), ConfigLoadError> {
    for (keys, command) in mappings {
        validate_key_string(keys).map_err(|error| {
            ConfigLoadError::invalid(format!(
                "config keymaps.{mode} key {keys:?} must be a valid canonical key string: {error}"
            ))
        })?;
        crate::command::parse(command).map_err(|error| {
            ConfigLoadError::invalid(format!(
                "config keymaps.{mode} mapping {keys:?} must target a valid command: {error}"
            ))
        })?;
    }

    Ok(())
}

fn validate_aliases(aliases: &BTreeMap<String, String>) -> Result<(), ConfigLoadError> {
    for (name, expansion) in aliases {
        validate_custom_command_name("alias", name)?;
        crate::command::validate_alias_expansion(expansion).map_err(|error| {
            ConfigLoadError::invalid(format!(
                "config alias {name:?} must expand to a valid command prefix: {error}"
            ))
        })?;
    }

    Ok(())
}

fn validate_configured_commands(config: &PartialConfig) -> Result<(), ConfigLoadError> {
    if let Some(aliases) = config.aliases.as_ref() {
        validate_aliases(aliases)?;
    }

    if let Some(scripts) = config.scripts.as_ref() {
        validate_scripts(scripts)?;
    }

    let resolved = Config {
        aliases: config
            .aliases
            .as_ref()
            .map(resolve_aliases)
            .unwrap_or_default(),
        scripts: config.scripts.clone().unwrap_or_default(),
        ..Config::default()
    };

    crate::command::CommandRegistry::new()
        .register_configured_commands(&resolved)
        .map_err(|error| ConfigLoadError::invalid(format!("config command conflict: {error}")))
}

fn validate_scripts(scripts: &BTreeMap<String, Vec<String>>) -> Result<(), ConfigLoadError> {
    for (name, commands) in scripts {
        validate_custom_command_name("script", name)?;
        if commands.is_empty() {
            return Err(ConfigLoadError::invalid(format!(
                "config script {name:?} must contain at least one command"
            )));
        }
        for command in commands {
            crate::command::validate_script_command(command).map_err(|error| {
                ConfigLoadError::invalid(format!(
                    "config script {name:?} contains an invalid command: {error}"
                ))
            })?;
        }
    }

    Ok(())
}

fn validate_custom_command_name(kind: &str, name: &str) -> Result<(), ConfigLoadError> {
    if name.trim().is_empty() {
        return Err(ConfigLoadError::invalid(format!(
            "config {kind}s must not contain empty names"
        )));
    }
    if name.chars().any(char::is_whitespace) {
        return Err(ConfigLoadError::invalid(format!(
            "config {kind} {name:?} must not contain whitespace"
        )));
    }
    if crate::command::is_canonical_command_root(name) {
        return Err(ConfigLoadError::invalid(format!(
            "config {kind} {name:?} conflicts with a canonical command root"
        )));
    }

    Ok(())
}

fn resolve_aliases(aliases: &BTreeMap<String, String>) -> BTreeMap<String, Vec<String>> {
    aliases
        .iter()
        .map(|(name, expansion)| {
            let tokens = crate::command::parse_alias_expansion(expansion)
                .expect("validated command alias expansion should parse");
            (name.clone(), tokens)
        })
        .collect()
}

fn resolve_keymaps(keymaps: &PartialKeymapsConfig) -> KeymapsConfig {
    KeymapsConfig {
        normal: keymaps.normal.clone().unwrap_or_default(),
        insert: keymaps.insert.clone().unwrap_or_default(),
        visual: keymaps.visual.clone().unwrap_or_default(),
        visual_line: keymaps.visual_line.clone().unwrap_or_default(),
        resizing: keymaps.resizing.clone().unwrap_or_default(),
    }
}

fn resolve_scroll_margin(scroll_margin: Option<&PartialScrollMargin>) -> ScrollMargin {
    let default_margin = ScrollMargin::default();
    ScrollMargin {
        vertical: scroll_margin
            .and_then(|margin| margin.vertical)
            .unwrap_or(default_margin.vertical),
        horizontal: scroll_margin
            .and_then(|margin| margin.horizontal)
            .unwrap_or(default_margin.horizontal),
    }
}

fn resolve_lsp(file: Option<&PartialLspConfig>) -> LspConfig {
    let builtin = builtin_lsp_config().clone();
    let mut servers = builtin.servers;

    if let Some(file) = file {
        for (name, override_config) in &file.servers {
            let builtin_server = servers.remove(name);
            let resolved = resolve_lsp_server(builtin_server, Some(override_config));
            servers.insert(name.clone(), resolved);
        }
    }

    LspConfig { servers }
}

fn resolve_lsp_server(
    builtin: Option<LspServerConfig>,
    file: Option<&PartialLspServerConfig>,
) -> LspServerConfig {
    let builtin = builtin.unwrap_or_default();
    let settings = merge_toml_values(
        builtin.settings,
        file.and_then(|config| config.settings.as_ref()),
    );

    LspServerConfig {
        enabled: file
            .and_then(|config| config.enabled)
            .unwrap_or(builtin.enabled),
        command: file
            .and_then(|config| config.command.clone())
            .unwrap_or(builtin.command),
        args: file
            .and_then(|config| config.args.clone())
            .unwrap_or(builtin.args),
        env: resolve_string_map(builtin.env, file.and_then(|config| config.env.as_ref())),
        filetypes: file
            .and_then(|config| config.filetypes.clone())
            .unwrap_or(builtin.filetypes),
        root_markers: file
            .and_then(|config| config.root_markers.clone())
            .unwrap_or(builtin.root_markers),
        settings,
    }
}

fn resolve_string_map(
    builtin: BTreeMap<String, String>,
    file: Option<&BTreeMap<String, String>>,
) -> BTreeMap<String, String> {
    let mut resolved = builtin;
    if let Some(file) = file {
        for (key, value) in file {
            resolved.insert(key.clone(), value.clone());
        }
    }
    resolved
}

fn merge_toml_values(base: Value, overlay: Option<&Value>) -> Value {
    let Some(overlay) = overlay else {
        return base;
    };

    match (base, overlay) {
        (Value::Table(mut base), Value::Table(overlay)) => {
            for (key, value) in overlay {
                let merged = match base.remove(key) {
                    Some(existing) => merge_toml_values(existing, Some(value)),
                    None => value.clone(),
                };
                base.insert(key.clone(), merged);
            }

            Value::Table(base)
        }
        (_, overlay) => overlay.clone(),
    }
}

fn validate_partial_lsp_config(config: &PartialLspConfig) -> Result<(), ConfigLoadError> {
    let builtin_servers = &builtin_lsp_config().servers;

    for (server_name, server) in &config.servers {
        if !builtin_servers.contains_key(server_name) {
            return Err(ConfigLoadError::Invalid {
                message: format!("config lsp.{server_name} is not a built-in server"),
            });
        }
        validate_partial_lsp_server_config(server_name.as_str(), server)?;
    }

    Ok(())
}

fn validate_partial_lsp_server_config(
    server_name: &str,
    config: &PartialLspServerConfig,
) -> Result<(), ConfigLoadError> {
    if let Some(command) = config.command.as_ref()
        && command.trim().is_empty()
    {
        return Err(ConfigLoadError::invalid(format!(
            "config lsp.{server_name}.command must not be empty or whitespace",
        )));
    }

    if let Some(filetypes) = config.filetypes.as_ref() {
        validate_non_empty_string_list(&format!("lsp.{server_name}.filetypes"), filetypes)?;
    }

    if let Some(root_markers) = config.root_markers.as_ref() {
        validate_non_empty_string_list(&format!("lsp.{server_name}.root_markers"), root_markers)?;
    }

    if let Some(args) = config.args.as_ref() {
        validate_string_list(&format!("lsp.{server_name}.args"), args)?;
    }

    if let Some(env) = config.env.as_ref() {
        validate_env_map(&format!("lsp.{server_name}.env"), env)?;
    }

    if let Some(settings) = config.settings.as_ref()
        && !matches!(settings, Value::Table(_))
    {
        return Err(ConfigLoadError::invalid(format!(
            "config lsp.{server_name}.settings must be a table",
        )));
    }

    Ok(())
}

fn validate_string_list(field: &str, values: &[String]) -> Result<(), ConfigLoadError> {
    for value in values {
        if value.trim().is_empty() {
            return Err(ConfigLoadError::invalid(format!(
                "config {field} entries must not be empty or whitespace",
            )));
        }
    }

    Ok(())
}

fn validate_non_empty_string_list(field: &str, values: &[String]) -> Result<(), ConfigLoadError> {
    validate_string_list(field, values)
}

fn validate_env_map(field: &str, values: &BTreeMap<String, String>) -> Result<(), ConfigLoadError> {
    for (key, value) in values {
        if key.trim().is_empty() {
            return Err(ConfigLoadError::invalid(format!(
                "config {field} keys must not be empty or whitespace",
            )));
        }
        if value.trim().is_empty() {
            return Err(ConfigLoadError::invalid(format!(
                "config {field}.{key} values must not be empty or whitespace",
            )));
        }
    }

    Ok(())
}

fn default_todo_markers() -> Vec<SmolStr> {
    DEFAULT_TODO_MARKERS
        .iter()
        .map(|marker| SmolStr::new(*marker))
        .collect()
}

fn default_completion_sources() -> Vec<String> {
    vec![
        "lsp".to_string(),
        "paths".to_string(),
        "buffer_words".to_string(),
    ]
}

fn validate_completion_trigger(_trigger: CompletionTrigger) -> Result<(), ConfigLoadError> {
    Ok(())
}

fn resolve_default_registers(registers: &PartialDefaultRegisters) -> DefaultRegisters {
    DefaultRegisters {
        yank: parse_default_register(registers.yank.as_deref().unwrap_or("y")).unwrap_or('y'),
        delete: parse_default_register(registers.delete.as_deref().unwrap_or("d")).unwrap_or('d'),
        change: parse_default_register(registers.change.as_deref().unwrap_or("c")).unwrap_or('c'),
    }
}

fn validate_default_registers(registers: &PartialDefaultRegisters) -> Result<(), ConfigLoadError> {
    if let Some(yank) = registers.yank.as_deref() {
        validate_default_register_value("yank", yank)?;
    }
    if let Some(delete) = registers.delete.as_deref() {
        validate_default_register_value("delete", delete)?;
    }
    if let Some(change) = registers.change.as_deref() {
        validate_default_register_value("change", change)?;
    }

    Ok(())
}

fn validate_default_register_value(field: &str, value: &str) -> Result<(), ConfigLoadError> {
    if parse_default_register(value).is_none() {
        return Err(ConfigLoadError::invalid(format!(
            "config default_registers.{field} must be a single lowercase ASCII letter"
        )));
    }

    Ok(())
}

fn parse_default_register(value: &str) -> Option<char> {
    let mut chars = value.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || !ch.is_ascii_lowercase() {
        return None;
    }

    Some(ch)
}

fn validate_todo_markers(markers: &[String]) -> Result<(), ConfigLoadError> {
    for marker in markers {
        validate_todo_marker(marker)?;
    }

    Ok(())
}

fn validate_completion_sources(sources: &[String]) -> Result<(), ConfigLoadError> {
    for source in sources {
        if source.trim().is_empty() {
            return Err(ConfigLoadError::invalid(
                "config completion_sources entries must not be empty or whitespace",
            ));
        }

        match source.as_str() {
            "lsp" => {}
            "paths" => {}
            "buffer_words" => {}
            _ => {
                return Err(ConfigLoadError::invalid(format!(
                    "config completion_sources contains an unknown source name: {source}"
                )));
            }
        }
    }

    Ok(())
}

fn validate_todo_marker(marker: &str) -> Result<(), ConfigLoadError> {
    if marker.trim().is_empty() {
        return Err(ConfigLoadError::invalid(
            "config todo_markers entries must not be empty or whitespace",
        ));
    }

    let normalized = marker.to_ascii_lowercase();
    Tag::parse(&normalized).map_err(|_| {
        ConfigLoadError::invalid(format!(
            "config todo_markers entries must be standalone word tokens that normalize to valid theme tags: {marker}"
        ))
    })?;

    Ok(())
}

fn resolve_advanced_glyphs(
    glyphs: &[AdvancedGlyphCapability],
) -> BTreeSet<AdvancedGlyphCapability> {
    let mut resolved = BTreeSet::new();
    for capability in glyphs {
        match capability {
            AdvancedGlyphCapability::Unicode => {
                for unicode_capability in all_unicode_advanced_glyph_capabilities() {
                    resolved.insert(unicode_capability);
                }
            }
            other => {
                resolved.insert(other.clone());
            }
        }
    }

    resolved
}

fn default_advanced_glyphs() -> BTreeSet<AdvancedGlyphCapability> {
    BTreeSet::new()
}

fn default_inlay_hint_kinds() -> BTreeSet<InlayHintCapability> {
    [InlayHintCapability::Type, InlayHintCapability::Parameter]
        .into_iter()
        .collect()
}

fn all_unicode_advanced_glyph_capabilities() -> [AdvancedGlyphCapability; 3] {
    [
        AdvancedGlyphCapability::UnicodeBorders,
        AdvancedGlyphCapability::UnicodeIndent,
        AdvancedGlyphCapability::UnicodeFolds,
    ]
}

fn xdg_config_home() -> Result<PathBuf, ConfigLoadError> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME")
        && !config_home.is_empty()
    {
        return Ok(PathBuf::from(config_home));
    }

    let home = env::var_os("HOME").ok_or(ConfigLoadError::MissingHomeDir)?;
    Ok(PathBuf::from(home).join(".config"))
}

fn xdg_config_dirs() -> Vec<PathBuf> {
    match env::var_os("XDG_CONFIG_DIRS") {
        Some(value) if !value.is_empty() => env::split_paths(&value).collect(),
        _ => vec![PathBuf::from(DEFAULT_XDG_CONFIG_DIRS)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        std::env::temp_dir().join(format!("urvim-{name}-{nanos}"))
    }

    fn write_config(dir: &Path, contents: &str) {
        let path = dir.join("urvim/config.toml");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("should create config directory");
        }
        fs::write(path, contents).expect("should write config file");
    }

    fn glyph_caps(values: &[AdvancedGlyphCapability]) -> BTreeSet<AdvancedGlyphCapability> {
        values.iter().cloned().collect()
    }

    fn inlay_hint_caps(values: &[InlayHintCapability]) -> BTreeSet<InlayHintCapability> {
        values.iter().cloned().collect()
    }

    fn todo_marker_strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn default_register_strings(
        yank: Option<&str>,
        delete: Option<&str>,
        change: Option<&str>,
    ) -> PartialDefaultRegisters {
        PartialDefaultRegisters {
            yank: yank.map(str::to_owned),
            delete: delete.map(str::to_owned),
            change: change.map(str::to_owned),
        }
    }

    fn resolved_todo_markers(values: &[&str]) -> Vec<SmolStr> {
        values.iter().map(|value| SmolStr::new(*value)).collect()
    }

    fn table_value(entries: &[(&str, Value)]) -> Value {
        let mut table = toml::map::Map::new();
        for (key, value) in entries {
            table.insert((*key).to_string(), value.clone());
        }
        Value::Table(table)
    }

    fn partial_lsp_settings(check_on_save_command: &str) -> PartialLspConfig {
        let mut settings = toml::map::Map::new();
        settings.insert(
            "checkOnSave".to_string(),
            table_value(&[("command", Value::String(check_on_save_command.to_string()))]),
        );

        PartialLspConfig {
            servers: BTreeMap::from([(
                "rust_analyzer".to_string(),
                PartialLspServerConfig {
                    enabled: Some(true),
                    command: Some("rust-analyzer".to_string()),
                    args: Some(vec!["--stdio".to_string()]),
                    env: Some(BTreeMap::from([(
                        "RUST_LOG".to_string(),
                        "debug".to_string(),
                    )])),
                    filetypes: Some(vec!["rust".to_string()]),
                    root_markers: Some(vec!["Cargo.toml".to_string()]),
                    settings: Some(Value::Table(settings)),
                },
            )]),
        }
    }

    fn lsp_server<'a>(config: &'a Config, name: &str) -> &'a LspServerConfig {
        config
            .lsp
            .servers
            .get(name)
            .unwrap_or_else(|| panic!("missing lsp server {name}"))
    }

    #[test]
    fn builtin_lsp_config_exposes_curated_server_set() {
        let servers = builtin_lsp_config()
            .servers
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();

        assert_eq!(
            servers,
            BTreeSet::from([
                "bashls".to_string(),
                "clangd".to_string(),
                "cssls".to_string(),
                "csharp_ls".to_string(),
                "gopls".to_string(),
                "haskell_language_server".to_string(),
                "html".to_string(),
                "intelephense".to_string(),
                "jdtls".to_string(),
                "jsonls".to_string(),
                "kotlin_language_server".to_string(),
                "marksman".to_string(),
                "metals".to_string(),
                "ocamllsp".to_string(),
                "pyright".to_string(),
                "rust_analyzer".to_string(),
                "sourcekit_lsp".to_string(),
                "ruby_lsp".to_string(),
                "taplo".to_string(),
                "typescript_language_server".to_string(),
                "yaml_language_server".to_string(),
                "zls".to_string(),
            ])
        );
    }

    #[test]
    fn resolve_prefers_cli_then_file_then_default() {
        let file = PartialConfig {
            theme: Some("file-theme".to_string()),
            keymaps: Some(PartialKeymapsConfig {
                normal: Some(BTreeMap::from([(
                    "<Space>w".to_string(),
                    "write".to_string(),
                )])),
                insert: Some(BTreeMap::from([(
                    "jk".to_string(),
                    "mode normal".to_string(),
                )])),
                visual: Some(BTreeMap::from([(
                    "x".to_string(),
                    "mode normal".to_string(),
                )])),
                visual_line: Some(BTreeMap::from([(
                    "x".to_string(),
                    "mode normal".to_string(),
                )])),
                resizing: Some(BTreeMap::from([(
                    "x".to_string(),
                    "pane equalize".to_string(),
                )])),
            }),
            default_registers: Some(default_register_strings(Some("a"), Some("b"), Some("c"))),
            syntax: Some(false),
            auto_close_pairs: Some(false),
            active_line: Some(true),
            relative_number: Some(true),
            indent_guides: Some(false),
            todo_markers: Some(todo_marker_strings(&["TASK", "FIXME"])),
            auto_indent: Some(AutoIndentMode::Neighbor),
            advanced_glyphs: Some(vec![
                AdvancedGlyphCapability::Nerdfont,
                AdvancedGlyphCapability::Unicode,
                AdvancedGlyphCapability::UnicodeBorders,
                AdvancedGlyphCapability::UnicodeIndent,
                AdvancedGlyphCapability::UnicodeFolds,
            ]),
            scroll_margin: Some(PartialScrollMargin {
                vertical: Some(8),
                horizontal: Some(6),
            }),
            wrap_mode: Some(WrapMode::Soft),
            ..Default::default()
        };

        assert_eq!(
            Config::resolve(Some(&file), Some("cli-theme"), Some(true)).theme,
            "cli-theme"
        );
        assert_eq!(Config::resolve(Some(&file), None, None).theme, "file-theme");
        assert_eq!(
            Config::resolve(Some(&file), None, None)
                .keymaps
                .insert
                .get("jk")
                .map(String::as_str),
            Some("mode normal")
        );
        assert_eq!(
            Config::resolve(Some(&file), None, None).default_registers,
            DefaultRegisters {
                yank: 'a',
                delete: 'b',
                change: 'c',
            }
        );
        assert!(!Config::resolve(Some(&file), None, None).syntax);
        assert!(!Config::resolve(Some(&file), None, None).auto_close_pairs);
        assert!(Config::resolve(Some(&file), None, None).active_line);
        assert!(Config::resolve(Some(&file), None, None).relative_number);
        assert!(!Config::resolve(Some(&file), None, None).indent_guides);
        assert_eq!(
            Config::resolve(Some(&file), None, None).todo_markers,
            resolved_todo_markers(&["TASK", "FIXME"])
        );
        assert_eq!(
            Config::resolve(Some(&file), None, None).auto_indent,
            AutoIndentMode::Neighbor
        );
        assert!(Config::resolve(None, None, None).syntax);
        assert!(Config::resolve(None, None, None).auto_close_pairs);
        assert!(!Config::resolve(None, None, None).active_line);
        assert!(!Config::resolve(None, None, None).relative_number);
        assert!(Config::resolve(None, None, None).indent_guides);
        assert_eq!(
            Config::resolve(None, None, None).auto_indent,
            AutoIndentMode::Off
        );
        assert_eq!(
            Config::resolve(None, None, None).completion_sources,
            vec![
                "lsp".to_string(),
                "paths".to_string(),
                "buffer_words".to_string()
            ]
        );
        assert_eq!(
            Config::resolve(None, None, None).completion_trigger,
            CompletionTrigger::Manual
        );
        assert_eq!(Config::resolve(None, None, None).theme, DEFAULT_THEME);
        assert_eq!(
            Config::resolve(None, None, None).keymaps,
            KeymapsConfig::default()
        );
        assert_eq!(
            Config::resolve(None, None, None).default_registers,
            DefaultRegisters::default()
        );
        let default_lsp = Config::resolve(None, None, None);
        let default_rust_analyzer = lsp_server(&default_lsp, "rust_analyzer");
        assert!(!default_rust_analyzer.enabled);
        assert_eq!(default_rust_analyzer.command, "rust-analyzer");
        assert_eq!(default_rust_analyzer.args, Vec::<String>::new());
        assert!(default_rust_analyzer.env.is_empty());
        assert_eq!(default_rust_analyzer.filetypes, vec!["rust".to_string()]);
        assert_eq!(
            default_rust_analyzer.root_markers,
            vec![
                "Cargo.toml".to_string(),
                "rust-project.json".to_string(),
                ".git".to_string(),
            ]
        );
        assert_eq!(
            default_rust_analyzer.settings,
            table_value(&[(
                "workspace",
                table_value(&[(
                    "symbol",
                    table_value(&[(
                        "search",
                        table_value(&[
                            ("kind", Value::String("all_symbols".to_string())),
                            ("limit", Value::Integer(5000)),
                        ]),
                    )]),
                )]),
            )])
        );
        assert!(Config::resolve(None, None, None).advanced_glyphs.is_empty());
        assert_eq!(
            Config::resolve(None, None, None).todo_markers,
            resolved_todo_markers(&DEFAULT_TODO_MARKERS)
        );
        assert_eq!(
            Config::resolve(None, None, None).tab_insertion,
            TabInsertion::Spaces
        );
        assert_eq!(
            Config::resolve(None, None, None).tab_behavior,
            TabBehavior::Simple
        );
        assert_eq!(
            Config::resolve(None, None, None).tab_width,
            DEFAULT_TAB_WIDTH
        );
        assert_eq!(
            Config::resolve(None, None, None).scroll_margin,
            ScrollMargin::default()
        );
        assert_eq!(Config::resolve(None, None, None).wrap_mode, WrapMode::Hard);
        assert_eq!(
            Config::resolve(Some(&file), None, None).scroll_margin,
            ScrollMargin {
                vertical: 8,
                horizontal: 6
            }
        );
        assert_eq!(
            Config::resolve(Some(&file), None, None).wrap_mode,
            WrapMode::Soft
        );
        assert_eq!(
            Config::resolve(Some(&file), None, None).advanced_glyphs,
            glyph_caps(&[
                AdvancedGlyphCapability::Nerdfont,
                AdvancedGlyphCapability::UnicodeBorders,
                AdvancedGlyphCapability::UnicodeIndent,
                AdvancedGlyphCapability::UnicodeFolds
            ])
        );
        assert!(!lsp_server(&Config::resolve(Some(&file), None, None), "rust_analyzer").enabled);
    }

    #[test]
    fn resolve_loads_lsp_rust_analyzer_settings() {
        let file = PartialConfig {
            lsp: Some(partial_lsp_settings("clippy")),
            ..Default::default()
        };

        let config = Config::resolve(Some(&file), None, None);
        let server = lsp_server(&config, "rust_analyzer");
        assert!(server.enabled);
        assert_eq!(server.command, "rust-analyzer");
        assert_eq!(server.args, vec!["--stdio".to_string()]);
        assert_eq!(
            server.env,
            BTreeMap::from([("RUST_LOG".to_string(), "debug".to_string())])
        );
        assert_eq!(server.filetypes, vec!["rust".to_string()]);
        assert_eq!(server.root_markers, vec!["Cargo.toml".to_string()]);
        assert_eq!(
            server.settings,
            table_value(&[
                (
                    "checkOnSave",
                    table_value(&[("command", Value::String("clippy".to_string()))]),
                ),
                (
                    "workspace",
                    table_value(&[(
                        "symbol",
                        table_value(&[(
                            "search",
                            table_value(&[
                                ("kind", Value::String("all_symbols".to_string())),
                                ("limit", Value::Integer(5000)),
                            ]),
                        )]),
                    )]),
                )
            ])
        );
    }

    #[test]
    fn resolve_loads_lsp_gopls_settings() {
        let file = PartialConfig {
            lsp: Some(PartialLspConfig {
                servers: BTreeMap::from([(
                    "gopls".to_string(),
                    PartialLspServerConfig {
                        enabled: Some(true),
                        ..Default::default()
                    },
                )]),
            }),
            ..Default::default()
        };

        let config = Config::resolve(Some(&file), None, None);
        let server = lsp_server(&config, "gopls");
        assert!(server.enabled);
        assert_eq!(server.command, "gopls");
        assert!(server.args.is_empty());
        assert_eq!(server.filetypes, vec!["go".to_string()]);
        assert_eq!(
            server.root_markers,
            vec![
                "go.mod".to_string(),
                "go.work".to_string(),
                ".git".to_string()
            ]
        );
    }

    #[test]
    fn resolve_merges_nested_lsp_settings() {
        let mut base_check_on_save = toml::map::Map::new();
        base_check_on_save.insert("command".to_string(), Value::String("check".to_string()));
        base_check_on_save.insert("extra".to_string(), Value::String("1".to_string()));

        let base = table_value(&[("checkOnSave", Value::Table(base_check_on_save))]);
        let overlay = table_value(&[(
            "checkOnSave",
            table_value(&[("command", Value::String("clippy".to_string()))]),
        )]);

        assert_eq!(
            merge_toml_values(base, Some(&overlay)),
            table_value(&[(
                "checkOnSave",
                table_value(&[
                    ("command", Value::String("clippy".to_string())),
                    ("extra", Value::String("1".to_string())),
                ]),
            )])
        );
    }

    #[test]
    fn resolve_expands_unicode_alias_advanced_glyph_capability() {
        let file = PartialConfig {
            advanced_glyphs: Some(vec![AdvancedGlyphCapability::Unicode]),
            ..Default::default()
        };

        assert_eq!(
            Config::resolve(Some(&file), None, None).advanced_glyphs,
            glyph_caps(&[
                AdvancedGlyphCapability::UnicodeBorders,
                AdvancedGlyphCapability::UnicodeIndent,
                AdvancedGlyphCapability::UnicodeFolds
            ])
        );
    }

    #[test]
    fn resolve_defaults_to_all_inlay_hint_capabilities() {
        assert_eq!(
            Config::resolve(None, None, None).inlay_hints,
            inlay_hint_caps(&[InlayHintCapability::Type, InlayHintCapability::Parameter])
        );
    }

    #[test]
    fn resolve_honors_explicit_inlay_hint_capabilities() {
        let file = PartialConfig {
            inlay_hints: Some(vec![InlayHintCapability::Parameter]),
            ..Default::default()
        };

        assert_eq!(
            Config::resolve(Some(&file), None, None).inlay_hints,
            inlay_hint_caps(&[InlayHintCapability::Parameter])
        );
    }

    #[test]
    fn resolve_allows_disabling_inlay_hints_entirely() {
        let file = PartialConfig {
            inlay_hints: Some(vec![]),
            ..Default::default()
        };

        let config = Config::resolve(Some(&file), None, None);
        assert!(!config.inlay_hints_enabled());
        assert!(!config.inlay_hint_kind_enabled(&InlayHintCapability::Type));
    }

    #[test]
    fn nerdfont_enabled_checks_resolved_advanced_glyphs() {
        assert!(!Config::resolve(None, None, None).nerdfont_enabled());

        let file = PartialConfig {
            advanced_glyphs: Some(vec![AdvancedGlyphCapability::Nerdfont]),
            ..Default::default()
        };

        assert!(Config::resolve(Some(&file), None, None).nerdfont_enabled());
    }

    #[test]
    fn unicode_borders_enabled_checks_resolved_advanced_glyphs() {
        assert!(!Config::resolve(None, None, None).unicode_borders_enabled());

        let file = PartialConfig {
            advanced_glyphs: Some(vec![AdvancedGlyphCapability::UnicodeBorders]),
            ..Default::default()
        };

        assert!(Config::resolve(Some(&file), None, None).unicode_borders_enabled());
    }

    #[test]
    fn unicode_indent_enabled_checks_resolved_advanced_glyphs() {
        assert!(!Config::resolve(None, None, None).unicode_indent_enabled());

        let file = PartialConfig {
            advanced_glyphs: Some(vec![AdvancedGlyphCapability::UnicodeIndent]),
            ..Default::default()
        };

        assert!(Config::resolve(Some(&file), None, None).unicode_indent_enabled());
    }

    #[test]
    fn unicode_folds_enabled_checks_resolved_advanced_glyphs() {
        assert!(!Config::resolve(None, None, None).unicode_folds_enabled());

        let file = PartialConfig {
            advanced_glyphs: Some(vec![AdvancedGlyphCapability::UnicodeFolds]),
            ..Default::default()
        };

        assert!(Config::resolve(Some(&file), None, None).unicode_folds_enabled());
    }

    #[test]
    fn load_from_locations_uses_first_config_file_in_search_order() {
        let home = unique_temp_dir("home");
        let dir1 = unique_temp_dir("dir1");
        let dir2 = unique_temp_dir("dir2");
        write_config(&home, "theme = \"home-theme\"");
        write_config(&dir1, "theme = \"dir1-theme\"");
        write_config(&dir2, "theme = \"dir2-theme\"");

        let config = Config::load_from_locations(home.clone(), vec![dir1, dir2], None, None)
            .expect("config should load");

        assert_eq!(config.theme, "home-theme");
    }

    #[test]
    fn load_from_locations_skips_missing_files() {
        let home = unique_temp_dir("missing-home");
        let config = Config::load_from_locations(
            home.clone(),
            vec![unique_temp_dir("missing-dir")],
            None,
            None,
        )
        .expect("missing config should fall back to defaults");

        assert_eq!(config.theme, DEFAULT_THEME);
        assert!(config.syntax);
    }

    #[test]
    fn load_from_locations_rejects_unknown_fields() {
        let home = unique_temp_dir("unknown-field-home");
        write_config(&home, "theme = \"demo\"\nextra = true");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Parse { .. } => {}
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_loads_command_aliases() {
        let home = unique_temp_dir("aliases-home");
        write_config(
            &home,
            r#"
[aliases]
dl = "action edit delete-line"
w = "write"
"#,
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");
        assert_eq!(
            config.aliases,
            BTreeMap::from([
                (
                    "dl".to_string(),
                    vec![
                        "action".to_string(),
                        "edit".to_string(),
                        "delete-line".to_string()
                    ],
                ),
                ("w".to_string(), vec!["write".to_string()]),
            ])
        );
    }

    #[test]
    fn load_from_locations_loads_command_scripts() {
        let home = unique_temp_dir("scripts-home");
        write_config(
            &home,
            r#"
[scripts]
wq = ["write", "quit"]
save_rust = ["buffer write path={1}", "buffer filetype filetype=rust"]
"#,
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");
        assert_eq!(
            config.scripts,
            BTreeMap::from([
                (
                    "save_rust".to_string(),
                    vec![
                        "buffer write path={1}".to_string(),
                        "buffer filetype filetype=rust".to_string(),
                    ],
                ),
                (
                    "wq".to_string(),
                    vec!["write".to_string(), "quit".to_string()],
                ),
            ])
        );
    }

    #[test]
    fn load_from_locations_rejects_alias_script_name_collision() {
        let home = unique_temp_dir("alias-script-collision-home");
        write_config(
            &home,
            r#"
[aliases]
wq = "write"

[scripts]
wq = ["write", "quit"]
"#,
        );

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");
        assert!(matches!(error, ConfigLoadError::Invalid { .. }));
    }

    #[test]
    fn load_from_locations_rejects_empty_script_command_list() {
        let home = unique_temp_dir("empty-script-home");
        write_config(
            &home,
            r#"
[scripts]
wq = []
"#,
        );

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");
        assert!(matches!(error, ConfigLoadError::Invalid { .. }));
    }

    #[test]
    fn load_from_locations_rejects_alias_for_canonical_root() {
        let home = unique_temp_dir("canonical-alias-home");
        write_config(
            &home,
            r#"
[aliases]
buffer = "write"
"#,
        );

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");
        assert!(matches!(error, ConfigLoadError::Invalid { .. }));
    }

    #[test]
    fn load_from_locations_rejects_alias_for_builtin_command_root() {
        let home = unique_temp_dir("builtin-alias-home");
        write_config(
            &home,
            r#"
[aliases]
write = "buffer write"
"#,
        );

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");
        assert!(matches!(error, ConfigLoadError::Invalid { .. }));
    }

    #[test]
    fn load_from_locations_rejects_empty_alias_expansion() {
        let home = unique_temp_dir("empty-alias-home");
        write_config(
            &home,
            r#"
[aliases]
dl = ""
"#,
        );

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");
        assert!(matches!(error, ConfigLoadError::Invalid { .. }));
    }

    #[test]
    fn load_from_locations_loads_lsp_rust_analyzer_config() {
        let home = unique_temp_dir("lsp-home");
        write_config(
            &home,
            r#"
[lsp.rust_analyzer]
enabled = true
command = "rust-analyzer"
args = ["--stdio"]
filetypes = ["rust"]
root_markers = ["Cargo.toml", "rust-project.json", ".git"]

[lsp.rust_analyzer.env]
RUST_LOG = "debug"

[lsp.rust_analyzer.settings]
checkOnSave = { command = "clippy" }
"#,
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");
        let server = lsp_server(&config, "rust_analyzer");
        assert!(server.enabled);
        assert_eq!(server.command, "rust-analyzer");
        assert_eq!(server.args, vec!["--stdio".to_string()]);
        assert_eq!(
            server.env,
            BTreeMap::from([("RUST_LOG".to_string(), "debug".to_string())])
        );
        assert_eq!(server.filetypes, vec!["rust".to_string()]);
        assert_eq!(
            server.root_markers,
            vec![
                "Cargo.toml".to_string(),
                "rust-project.json".to_string(),
                ".git".to_string(),
            ]
        );
        assert_eq!(
            server.settings,
            table_value(&[
                (
                    "checkOnSave",
                    table_value(&[("command", Value::String("clippy".to_string()))]),
                ),
                (
                    "workspace",
                    table_value(&[(
                        "symbol",
                        table_value(&[(
                            "search",
                            table_value(&[
                                ("kind", Value::String("all_symbols".to_string())),
                                ("limit", Value::Integer(5000)),
                            ]),
                        )]),
                    )]),
                )
            ])
        );
    }

    #[test]
    fn load_from_locations_loads_lsp_pyright_config() {
        let home = unique_temp_dir("pyright-home");
        write_config(
            &home,
            r#"
[lsp.pyright]
enabled = true
"#,
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");
        let server = lsp_server(&config, "pyright");
        assert!(server.enabled);
        assert_eq!(server.command, "pyright-langserver");
        assert_eq!(server.args, vec!["--stdio".to_string()]);
        assert_eq!(server.filetypes, vec!["python".to_string()]);
    }

    #[test]
    fn load_from_locations_rejects_unknown_lsp_fields() {
        let home = unique_temp_dir("lsp-unknown-home");
        write_config(&home, "[lsp.unknown_server]\nenabled = true");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("lsp.unknown_server"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_rejects_empty_lsp_command() {
        let home = unique_temp_dir("lsp-empty-command-home");
        write_config(&home, "[lsp.rust_analyzer]\ncommand = \"   \"");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("lsp.rust_analyzer.command"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_rejects_unknown_scroll_margin_fields() {
        let home = unique_temp_dir("scroll-margin-unknown-field-home");
        write_config(
            &home,
            "scroll_margin = { vertical = 5, horizontal = 5, side = 2 }",
        );

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Parse { .. } => {}
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_rejects_invalid_scroll_margin_value_type() {
        let home = unique_temp_dir("scroll-margin-invalid-type-home");
        write_config(
            &home,
            "scroll_margin = { vertical = \"wide\", horizontal = 5 }",
        );

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Parse { .. } => {}
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_rejects_empty_theme() {
        let home = unique_temp_dir("empty-theme-home");
        write_config(&home, "theme = \"   \"");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("theme"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_loads_keymaps() {
        let home = unique_temp_dir("keymaps-home");
        write_config(
            &home,
            "[keymaps.normal]\n\"<Space>w\" = \"write\"\n[keymaps.insert]\njk = \"mode normal\"\n[keymaps.visual]\nx = \"mode normal\"\n[keymaps.visual_line]\nx = \"mode normal\"\n[keymaps.resizing]\nx = \"pane equalize\"",
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(
            config.keymaps.normal.get("<Space>w").map(String::as_str),
            Some("write")
        );
        assert_eq!(
            config.keymaps.insert.get("jk").map(String::as_str),
            Some("mode normal")
        );
        assert_eq!(
            config.keymaps.visual.get("x").map(String::as_str),
            Some("mode normal")
        );
        assert_eq!(
            config.keymaps.visual_line.get("x").map(String::as_str),
            Some("mode normal")
        );
        assert_eq!(
            config.keymaps.resizing.get("x").map(String::as_str),
            Some("pane equalize")
        );
    }

    #[test]
    fn load_from_locations_loads_default_registers() {
        let home = unique_temp_dir("default-registers-home");
        write_config(
            &home,
            "default_registers = { yank = \"a\", delete = \"b\", change = \"c\" }",
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(
            config.default_registers,
            DefaultRegisters {
                yank: 'a',
                delete: 'b',
                change: 'c',
            }
        );
    }

    #[test]
    fn load_from_locations_defaults_default_registers_to_builtin_set() {
        let home = unique_temp_dir("default-registers-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.default_registers, DefaultRegisters::default());
    }

    #[test]
    fn load_from_locations_rejects_invalid_default_register_value() {
        let home = unique_temp_dir("default-registers-invalid-home");
        write_config(
            &home,
            "default_registers = { yank = \"AA\", delete = \"b\", change = \"c\" }",
        );

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("default_registers.yank"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_loads_tab_settings() {
        let home = unique_temp_dir("tab-settings-home");
        write_config(
            &home,
            "tab_insertion = \"tabs\"\ntab_behavior = \"smart\"\ntab_width = 8",
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.tab_insertion, TabInsertion::Tabs);
        assert_eq!(config.tab_behavior, TabBehavior::Smart);
        assert_eq!(config.tab_width, 8);
    }

    #[test]
    fn load_from_locations_loads_scroll_margin_table() {
        let home = unique_temp_dir("scroll-margin-home");
        write_config(&home, "scroll_margin = { vertical = 3, horizontal = 9 }");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(
            config.scroll_margin,
            ScrollMargin {
                vertical: 3,
                horizontal: 9
            }
        );
    }

    #[test]
    fn load_from_locations_defaults_partial_scroll_margin_values() {
        let home = unique_temp_dir("scroll-margin-partial-home");
        write_config(&home, "scroll_margin = { vertical = 2 }");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(
            config.scroll_margin,
            ScrollMargin {
                vertical: 2,
                horizontal: DEFAULT_SCROLL_MARGIN
            }
        );
    }

    #[test]
    fn load_from_locations_defaults_scroll_margin_to_builtin_values() {
        let home = unique_temp_dir("scroll-margin-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.scroll_margin, ScrollMargin::default());
    }

    #[test]
    fn load_from_locations_loads_wrap_mode() {
        let home = unique_temp_dir("wrap-mode-home");
        write_config(&home, "wrap_mode = \"soft\"");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.wrap_mode, WrapMode::Soft);
    }

    #[test]
    fn load_from_locations_defaults_wrap_mode_to_hard() {
        let home = unique_temp_dir("wrap-mode-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.wrap_mode, WrapMode::Hard);
    }

    #[test]
    fn load_from_locations_rejects_invalid_wrap_mode() {
        let home = unique_temp_dir("wrap-mode-invalid-home");
        write_config(&home, "wrap_mode = \"word\"");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Parse { .. } => {}
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_loads_auto_indent_mode() {
        let home = unique_temp_dir("auto-indent-home");
        write_config(&home, "auto_indent = \"neighbor\"");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.auto_indent, AutoIndentMode::Neighbor);
    }

    #[test]
    fn load_from_locations_rejects_invalid_auto_indent_mode() {
        let home = unique_temp_dir("auto-indent-invalid-home");
        write_config(&home, "auto_indent = \"sideways\"");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Parse { .. } => {}
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_defaults_auto_indent_to_off() {
        let home = unique_temp_dir("auto-indent-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.auto_indent, AutoIndentMode::Off);
    }

    #[test]
    fn load_from_locations_loads_completion_sources() {
        let home = unique_temp_dir("completion-sources-home");
        write_config(
            &home,
            "completion_sources = [\"lsp\", \"paths\", \"buffer_words\"]",
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(
            config.completion_sources,
            vec![
                "lsp".to_string(),
                "paths".to_string(),
                "buffer_words".to_string()
            ]
        );
    }

    #[test]
    fn load_from_locations_defaults_completion_sources_to_lsp_first() {
        let home = unique_temp_dir("completion-sources-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(
            config.completion_sources,
            vec![
                "lsp".to_string(),
                "paths".to_string(),
                "buffer_words".to_string()
            ]
        );
    }

    #[test]
    fn load_from_locations_loads_completion_trigger() {
        let home = unique_temp_dir("completion-trigger-home");
        write_config(&home, "completion_trigger = \"auto\"");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.completion_trigger, CompletionTrigger::Auto);
    }

    #[test]
    fn load_from_locations_defaults_completion_trigger_to_manual() {
        let home = unique_temp_dir("completion-trigger-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.completion_trigger, CompletionTrigger::Manual);
    }

    #[test]
    fn load_from_locations_rejects_unknown_completion_sources() {
        let home = unique_temp_dir("completion-sources-invalid-home");
        write_config(&home, "completion_sources = [\"unknown\"]");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("completion_sources"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_rejects_invalid_keymap_key() {
        let home = unique_temp_dir("invalid-keymap-key-home");
        write_config(&home, "[keymaps.insert]\n\"   \" = \"mode normal\"");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("keymaps.insert"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_rejects_invalid_keymap_command() {
        let home = unique_temp_dir("invalid-keymap-command-home");
        write_config(&home, "[keymaps.normal]\nq = \"unknown command\"");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("keymaps.normal"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_rejects_zero_tab_width() {
        let home = unique_temp_dir("zero-tab-width-home");
        write_config(&home, "tab_width = 0");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("tab_width"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_loads_syntax_flag() {
        let home = unique_temp_dir("syntax-home");
        write_config(&home, "syntax = false");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(!config.syntax);
    }

    #[test]
    fn load_from_locations_defaults_auto_close_pairs_to_true() {
        let home = unique_temp_dir("auto-close-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(config.auto_close_pairs);
    }

    #[test]
    fn load_from_locations_loads_auto_close_pairs_flag() {
        let home = unique_temp_dir("auto-close-home");
        write_config(&home, "auto_close_pairs = false");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(!config.auto_close_pairs);
    }

    #[test]
    fn load_from_locations_loads_active_line_flag() {
        let home = unique_temp_dir("active-line-home");
        write_config(&home, "active_line = true");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(config.active_line);
    }

    #[test]
    fn load_from_locations_loads_relative_number_flag() {
        let home = unique_temp_dir("relative-number-home");
        write_config(&home, "relative_number = true");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(config.relative_number);
    }

    #[test]
    fn load_from_locations_defaults_active_line_to_false() {
        let home = unique_temp_dir("active-line-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(!config.active_line);
    }

    #[test]
    fn load_from_locations_defaults_relative_number_to_false() {
        let home = unique_temp_dir("relative-number-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(!config.relative_number);
    }

    #[test]
    fn load_from_locations_loads_indent_guides_flag() {
        let home = unique_temp_dir("indent-guides-home");
        write_config(&home, "indent_guides = false");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(!config.indent_guides);
    }

    #[test]
    fn load_from_locations_defaults_indent_guides_to_true() {
        let home = unique_temp_dir("indent-guides-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(config.indent_guides);
    }

    #[test]
    fn load_from_locations_loads_todo_markers() {
        let home = unique_temp_dir("todo-markers-home");
        write_config(&home, "todo_markers = [\"TASK\", \"BUG\"]");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.todo_markers, resolved_todo_markers(&["TASK", "BUG"]));
    }

    #[test]
    fn load_from_locations_defaults_todo_markers_to_builtin_set() {
        let home = unique_temp_dir("todo-markers-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(
            config.todo_markers,
            resolved_todo_markers(&DEFAULT_TODO_MARKERS)
        );
    }

    #[test]
    fn load_from_locations_rejects_invalid_todo_marker_values() {
        let home = unique_temp_dir("todo-markers-invalid-home");
        write_config(&home, "todo_markers = [\"TODO!\", \"BUG\"]");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("todo_markers"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_loads_advanced_glyph_caps() {
        let home = unique_temp_dir("glyph-home");
        write_config(
            &home,
            "advanced_glyphs = [\"nerdfont\", \"unicode\", \"unicode_borders\", \"unicode_indent\", \"unicode_folds\", \"nerdfont\"]",
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(
            config.advanced_glyphs,
            glyph_caps(&[
                AdvancedGlyphCapability::Nerdfont,
                AdvancedGlyphCapability::UnicodeBorders,
                AdvancedGlyphCapability::UnicodeIndent,
                AdvancedGlyphCapability::UnicodeFolds
            ])
        );
    }

    #[test]
    fn load_from_locations_rejects_unknown_advanced_glyph_caps() {
        let home = unique_temp_dir("glyph-unknown-home");
        write_config(&home, "advanced_glyphs = [\"unknown\"]");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Parse { .. } => {}
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_cli_overrides_syntax_flag() {
        let home = unique_temp_dir("syntax-cli-home");
        write_config(&home, "syntax = false");

        let config =
            Config::load_from_locations(home, vec![], None, Some(true)).expect("should load");

        assert!(config.syntax);
    }
}
