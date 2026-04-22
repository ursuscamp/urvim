//! Startup configuration loading and resolution.
//!
//! This module loads the editor's TOML config file from the XDG config
//! directories, merges it with command-line overrides, and produces a resolved
//! configuration object that can be stored globally.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::editor::validate_key_string;
use crate::theme::Tag;
use imbl::Vector;
use smol_str::SmolStr;

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

/// The resolved startup configuration used by the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// The active theme name selected after merging file and CLI inputs.
    pub theme: String,
    /// The optional insert-mode escape binding loaded from config.
    pub insert_escape: Option<String>,
    /// The resolved default register selectors for yank, delete, and change.
    pub default_registers: DefaultRegisters,
    /// Whether syntax highlighting is enabled for rendered buffers.
    pub syntax: bool,
    /// Whether insert mode should auto-close supported bracket and quote pairs.
    pub auto_close_pairs: bool,
    /// Whether the active line should be highlighted in the focused window.
    pub active_line: bool,
    /// Whether to render the active indent scope guide.
    pub indent_guides: bool,
    /// The configured comment todo markers used for highlighting.
    pub todo_markers: Vector<SmolStr>,
    /// How insert mode should resolve indentation for newly created lines.
    pub auto_indent: AutoIndentMode,
    /// Enabled advanced glyph capabilities.
    pub advanced_glyphs: BTreeSet<AdvancedGlyphCapability>,
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
}

/// The TOML-backed config file schema.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PartialConfig {
    /// The theme name stored in the config file.
    pub theme: Option<String>,
    /// The optional insert-mode escape binding stored in the config file.
    pub insert_escape: Option<String>,
    /// The default register table stored in the config file.
    pub default_registers: Option<PartialDefaultRegisters>,
    /// Whether syntax highlighting is enabled in the config file.
    pub syntax: Option<bool>,
    /// Whether insert mode should auto-close supported bracket and quote pairs.
    pub auto_close_pairs: Option<bool>,
    /// Whether the active line should be highlighted in the focused window.
    pub active_line: Option<bool>,
    /// Whether to render the active indent scope guide.
    pub indent_guides: Option<bool>,
    /// The todo marker list stored in the config file.
    pub todo_markers: Option<Vec<String>>,
    /// How insert mode should resolve indentation for newly created lines.
    pub auto_indent: Option<AutoIndentMode>,
    /// Enabled advanced glyph capabilities in the config file.
    pub advanced_glyphs: Option<Vec<AdvancedGlyphCapability>>,
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
        let insert_escape = file.and_then(|config| config.insert_escape.clone());
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
        let indent_guides = file.and_then(|config| config.indent_guides).unwrap_or(true);
        let todo_markers = file
            .and_then(|config| config.todo_markers.clone())
            .map(|markers| markers.into_iter().map(SmolStr::new).collect())
            .unwrap_or_else(default_todo_markers);
        let auto_indent = file
            .and_then(|config| config.auto_indent)
            .unwrap_or_default();
        let advanced_glyphs = file
            .and_then(|config| config.advanced_glyphs.as_ref())
            .map(|glyphs| resolve_advanced_glyphs(glyphs))
            .unwrap_or_default();
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

        Self {
            theme,
            insert_escape,
            default_registers,
            syntax,
            auto_close_pairs,
            active_line,
            indent_guides,
            todo_markers,
            auto_indent,
            advanced_glyphs,
            tab_insertion,
            tab_behavior,
            tab_width,
            scroll_margin,
            wrap_mode,
        }
    }

    /// Returns whether nerdfont glyph rendering is enabled.
    pub fn nerdfont_enabled(&self) -> bool {
        self.advanced_glyphs
            .contains(&AdvancedGlyphCapability::Nerdfont)
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
            insert_escape: None,
            default_registers: DefaultRegisters::default(),
            syntax: true,
            auto_close_pairs: true,
            active_line: false,
            indent_guides: true,
            todo_markers: default_todo_markers(),
            auto_indent: AutoIndentMode::default(),
            advanced_glyphs: BTreeSet::new(),
            tab_insertion: TabInsertion::default(),
            tab_behavior: TabBehavior::default(),
            tab_width: DEFAULT_TAB_WIDTH,
            scroll_margin: ScrollMargin::default(),
            wrap_mode: WrapMode::default(),
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

    if let Some(insert_escape) = config.insert_escape.as_ref() {
        validate_key_string(insert_escape).map_err(|error| {
            ConfigLoadError::invalid(format!(
                "config insert_escape must be a valid canonical key string: {error}"
            ))
        })?;
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

    Ok(())
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

fn default_todo_markers() -> Vector<SmolStr> {
    DEFAULT_TODO_MARKERS
        .iter()
        .map(|marker| SmolStr::new(*marker))
        .collect()
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

fn all_unicode_advanced_glyph_capabilities() -> [AdvancedGlyphCapability; 2] {
    [
        AdvancedGlyphCapability::UnicodeBorders,
        AdvancedGlyphCapability::UnicodeIndent,
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

    fn resolved_todo_markers(values: &[&str]) -> Vector<SmolStr> {
        values.iter().map(|value| SmolStr::new(*value)).collect()
    }

    #[test]
    fn resolve_prefers_cli_then_file_then_default() {
        let file = PartialConfig {
            theme: Some("file-theme".to_string()),
            insert_escape: Some("jk".to_string()),
            default_registers: Some(default_register_strings(Some("a"), Some("b"), Some("c"))),
            syntax: Some(false),
            auto_close_pairs: Some(false),
            active_line: Some(true),
            indent_guides: Some(false),
            todo_markers: Some(todo_marker_strings(&["TASK", "FIXME"])),
            auto_indent: Some(AutoIndentMode::Neighbor),
            advanced_glyphs: Some(vec![
                AdvancedGlyphCapability::Nerdfont,
                AdvancedGlyphCapability::Unicode,
                AdvancedGlyphCapability::UnicodeBorders,
                AdvancedGlyphCapability::UnicodeIndent,
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
                .insert_escape
                .as_deref(),
            Some("jk")
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
        assert!(Config::resolve(None, None, None).indent_guides);
        assert_eq!(
            Config::resolve(None, None, None).auto_indent,
            AutoIndentMode::Off
        );
        assert_eq!(Config::resolve(None, None, None).theme, DEFAULT_THEME);
        assert_eq!(Config::resolve(None, None, None).insert_escape, None);
        assert_eq!(
            Config::resolve(None, None, None).default_registers,
            DefaultRegisters::default()
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
                AdvancedGlyphCapability::UnicodeIndent
            ])
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
                AdvancedGlyphCapability::UnicodeIndent
            ])
        );
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
    fn load_from_locations_loads_insert_escape_binding() {
        let home = unique_temp_dir("insert-escape-home");
        write_config(&home, "insert_escape = \"jk\"");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(config.insert_escape.as_deref(), Some("jk"));
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
    fn load_from_locations_rejects_invalid_insert_escape_binding() {
        let home = unique_temp_dir("invalid-insert-escape-home");
        write_config(&home, "insert_escape = \"   \"");

        let error = Config::load_from_locations(home, vec![], None, None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("insert_escape"));
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
    fn load_from_locations_defaults_active_line_to_false() {
        let home = unique_temp_dir("active-line-default-home");

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert!(!config.active_line);
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
            "advanced_glyphs = [\"nerdfont\", \"unicode\", \"unicode_borders\", \"unicode_indent\", \"nerdfont\"]",
        );

        let config = Config::load_from_locations(home, vec![], None, None).expect("should load");

        assert_eq!(
            config.advanced_glyphs,
            glyph_caps(&[
                AdvancedGlyphCapability::Nerdfont,
                AdvancedGlyphCapability::UnicodeBorders,
                AdvancedGlyphCapability::UnicodeIndent
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
