//! Startup configuration loading and resolution.
//!
//! This module loads the editor's TOML config file from the XDG config
//! directories, merges it with command-line overrides, and produces a resolved
//! configuration object that can be stored globally.

use serde::Deserialize;
use std::env;
use std::fmt;
use std::fs;
use std::path::PathBuf;

const DEFAULT_THEME: &str = "Friday Night";
const CONFIG_RELATIVE_PATH: &str = "urvim/config.toml";
const DEFAULT_XDG_CONFIG_DIRS: &str = "/etc/xdg";

/// The resolved startup configuration used by the editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// The active theme name selected after merging file and CLI inputs.
    pub theme: String,
}

/// The TOML-backed config file schema.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PartialConfig {
    /// The theme name stored in the config file.
    pub theme: Option<String>,
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
    pub fn load(cli_theme: Option<&str>) -> Result<Self, ConfigLoadError> {
        let config_home = xdg_config_home()?;
        let config_dirs = xdg_config_dirs();
        Self::load_from_locations(config_home, config_dirs, cli_theme)
    }

    /// Loads, merges, and resolves startup configuration from explicit XDG paths.
    pub fn load_from_locations(
        config_home: PathBuf,
        config_dirs: Vec<PathBuf>,
        cli_theme: Option<&str>,
    ) -> Result<Self, ConfigLoadError> {
        let file = load_config_file(config_home, config_dirs)?;
        Ok(Self::resolve(file.as_ref(), cli_theme))
    }

    /// Resolves the final config by applying CLI overrides on top of file values.
    pub fn resolve(file: Option<&PartialConfig>, cli_theme: Option<&str>) -> Self {
        let theme = cli_theme
            .map(ToOwned::to_owned)
            .or_else(|| file.and_then(|config| config.theme.clone()))
            .unwrap_or_else(|| DEFAULT_THEME.to_string());

        Self { theme }
    }
}

impl ConfigLoadError {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid {
            message: message.into(),
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
    if let Some(theme) = config.theme.as_ref() {
        if theme.trim().is_empty() {
            return Err(ConfigLoadError::invalid(
                "config theme must not be empty or whitespace",
            ));
        }
    }

    Ok(())
}

fn xdg_config_home() -> Result<PathBuf, ConfigLoadError> {
    if let Some(config_home) = env::var_os("XDG_CONFIG_HOME") {
        if !config_home.is_empty() {
            return Ok(PathBuf::from(config_home));
        }
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

    #[test]
    fn resolve_prefers_cli_then_file_then_default() {
        let file = PartialConfig {
            theme: Some("file-theme".to_string()),
        };

        assert_eq!(
            Config::resolve(Some(&file), Some("cli-theme")).theme,
            "cli-theme"
        );
        assert_eq!(Config::resolve(Some(&file), None).theme, "file-theme");
        assert_eq!(Config::resolve(None, None).theme, DEFAULT_THEME);
    }

    #[test]
    fn load_from_locations_uses_first_config_file_in_search_order() {
        let home = unique_temp_dir("home");
        let dir1 = unique_temp_dir("dir1");
        let dir2 = unique_temp_dir("dir2");
        write_config(&home, "theme = \"home-theme\"");
        write_config(&dir1, "theme = \"dir1-theme\"");
        write_config(&dir2, "theme = \"dir2-theme\"");

        let config = Config::load_from_locations(home.clone(), vec![dir1, dir2], None)
            .expect("config should load");

        assert_eq!(config.theme, "home-theme");
    }

    #[test]
    fn load_from_locations_skips_missing_files() {
        let home = unique_temp_dir("missing-home");
        let config =
            Config::load_from_locations(home.clone(), vec![unique_temp_dir("missing-dir")], None)
                .expect("missing config should fall back to defaults");

        assert_eq!(config.theme, DEFAULT_THEME);
    }

    #[test]
    fn load_from_locations_rejects_unknown_fields() {
        let home = unique_temp_dir("unknown-field-home");
        write_config(&home, "theme = \"demo\"\nextra = true");

        let error = Config::load_from_locations(home, vec![], None).expect_err("should fail");

        match error {
            ConfigLoadError::Parse { .. } => {}
            other => panic!("expected parse error, got {other:?}"),
        }
    }

    #[test]
    fn load_from_locations_rejects_empty_theme() {
        let home = unique_temp_dir("empty-theme-home");
        write_config(&home, "theme = \"   \"");

        let error = Config::load_from_locations(home, vec![], None).expect_err("should fail");

        match error {
            ConfigLoadError::Invalid { message } => {
                assert!(message.contains("theme"));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }
}
