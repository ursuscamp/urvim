//! Plugin manifest parsing and validation.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::PluginLoadError;

/// The manifest filename expected in each plugin directory.
pub const MANIFEST_FILE_NAME: &str = "urvim-plugin.toml";

/// A raw plugin manifest as represented in TOML.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawPluginManifest {
    /// Stable plugin namespace used for commands.
    pub name: String,
    /// Plugin version string.
    pub version: String,
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Relative theme file paths provided by the plugin.
    #[serde(default)]
    pub themes: Vec<PathBuf>,
    /// Manifest-provided command scripts.
    #[serde(default)]
    pub scripts: BTreeMap<String, Vec<String>>,
    /// Manifest-provided process commands.
    #[serde(default)]
    pub commands: BTreeMap<String, RawPluginCommand>,
    /// Optional process plugin launch configuration.
    pub process: Option<RawPluginProcess>,
}

/// A raw process command table as represented in TOML.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawPluginCommand {
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Process request method sent when the command runs.
    pub request: String,
}

/// A validated process command contribution.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginCommand {
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Process request method sent when the command runs.
    pub request: String,
}

/// A raw process plugin launch table as represented in TOML.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RawPluginProcess {
    /// Executable command used to launch the plugin process.
    pub command: String,
    /// Command-line arguments passed to the process.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables passed to the process.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// A validated process plugin launch configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginProcess {
    /// Executable command used to launch the plugin process.
    pub command: String,
    /// Command-line arguments passed to the process.
    pub args: Vec<String>,
    /// Environment variables passed to the process.
    pub env: BTreeMap<String, String>,
}

/// A validated plugin manifest with paths resolved against its plugin root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginManifest {
    /// Stable plugin namespace used for commands.
    pub name: String,
    /// Plugin version string.
    pub version: String,
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Absolute plugin root directory.
    pub root: PathBuf,
    /// Resolved theme file paths provided by the plugin.
    pub themes: Vec<PathBuf>,
    /// Manifest-provided command scripts.
    pub scripts: BTreeMap<String, Vec<String>>,
    /// Manifest-provided process commands.
    pub commands: BTreeMap<String, PluginCommand>,
    /// Optional process plugin launch configuration.
    pub process: Option<PluginProcess>,
}

impl PluginManifest {
    /// Loads, parses, and validates the manifest in a plugin directory.
    pub fn load_from_dir(root: impl AsRef<Path>) -> Result<Self, PluginLoadError> {
        let root = root.as_ref();
        let manifest_path = root.join(MANIFEST_FILE_NAME);
        let input = fs::read_to_string(&manifest_path).map_err(|source| PluginLoadError::Io {
            path: manifest_path.clone(),
            source,
        })?;

        Self::parse_from_str(manifest_path.to_string_lossy().as_ref(), &input, root)
    }

    /// Parses and validates a manifest string using `root` for relative paths.
    pub fn parse_from_str(
        source: &str,
        input: &str,
        root: impl AsRef<Path>,
    ) -> Result<Self, PluginLoadError> {
        let raw =
            toml::from_str::<RawPluginManifest>(input).map_err(|error| PluginLoadError::Parse {
                source: source.to_string(),
                message: error.to_string(),
            })?;

        Self::resolve(raw, root)
    }

    /// Resolves a raw manifest into a validated manifest.
    pub fn resolve(
        raw: RawPluginManifest,
        root: impl AsRef<Path>,
    ) -> Result<Self, PluginLoadError> {
        let root = root.as_ref().to_path_buf();
        validate_plugin_name(&raw.name)?;
        validate_version(&raw.version)?;

        let themes = raw
            .themes
            .iter()
            .map(|path| resolve_manifest_relative_path(&root, "theme", path))
            .collect::<Result<Vec<_>, _>>()?;

        for name in raw.scripts.keys() {
            validate_script_name(name)?;
        }
        let mut commands = BTreeMap::new();
        for (name, command) in raw.commands {
            validate_script_name(&name)?;
            if raw.scripts.contains_key(&name) {
                return Err(PluginLoadError::invalid(format!(
                    "plugin command {name:?} conflicts with a script of the same name"
                )));
            }
            commands.insert(name, validate_command(command)?);
        }
        let process = raw.process.map(validate_process).transpose()?;

        Ok(Self {
            name: raw.name,
            version: raw.version,
            description: raw.description,
            root,
            themes,
            scripts: raw.scripts,
            commands,
            process,
        })
    }
}

fn validate_command(command: RawPluginCommand) -> Result<PluginCommand, PluginLoadError> {
    if command.request.trim().is_empty() {
        return Err(PluginLoadError::invalid(
            "plugin command request must not be empty",
        ));
    }
    if command.request.chars().any(char::is_whitespace) {
        return Err(PluginLoadError::invalid(format!(
            "plugin command request {:?} must not contain whitespace",
            command.request
        )));
    }

    Ok(PluginCommand {
        description: command.description,
        request: command.request,
    })
}

fn validate_process(process: RawPluginProcess) -> Result<PluginProcess, PluginLoadError> {
    if process.command.trim().is_empty() {
        return Err(PluginLoadError::invalid(
            "plugin process command must not be empty",
        ));
    }
    for (name, value) in &process.env {
        if name.trim().is_empty() {
            return Err(PluginLoadError::invalid(
                "plugin process env names must not be empty",
            ));
        }
        if name.contains('=') {
            return Err(PluginLoadError::invalid(format!(
                "plugin process env name {name:?} must not contain '='"
            )));
        }
        if value.contains('\0') {
            return Err(PluginLoadError::invalid(format!(
                "plugin process env value for {name:?} must not contain NUL"
            )));
        }
    }

    Ok(PluginProcess {
        command: process.command,
        args: process.args,
        env: process.env,
    })
}

fn validate_plugin_name(name: &str) -> Result<(), PluginLoadError> {
    if name.trim().is_empty() {
        return Err(PluginLoadError::invalid("plugin name must not be empty"));
    }
    if name.chars().any(char::is_whitespace) {
        return Err(PluginLoadError::invalid(format!(
            "plugin name {name:?} must not contain whitespace"
        )));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(PluginLoadError::invalid(format!(
            "plugin name {name:?} must not contain path separators"
        )));
    }

    Ok(())
}

fn validate_version(version: &str) -> Result<(), PluginLoadError> {
    if version.trim().is_empty() {
        return Err(PluginLoadError::invalid("plugin version must not be empty"));
    }

    Ok(())
}

fn validate_script_name(name: &str) -> Result<(), PluginLoadError> {
    if name.trim().is_empty() {
        return Err(PluginLoadError::invalid("script name must not be empty"));
    }
    if name.chars().any(char::is_whitespace) {
        return Err(PluginLoadError::invalid(format!(
            "script name {name:?} must not contain whitespace"
        )));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(PluginLoadError::invalid(format!(
            "script name {name:?} must not contain path separators"
        )));
    }

    Ok(())
}

fn resolve_manifest_relative_path(
    root: &Path,
    kind: &str,
    path: &Path,
) -> Result<PathBuf, PluginLoadError> {
    if path.as_os_str().is_empty() {
        return Err(PluginLoadError::invalid(format!(
            "{kind} path must not be empty"
        )));
    }
    if path.is_absolute() {
        return Err(PluginLoadError::invalid(format!(
            "{kind} path {} must be relative",
            path.display()
        )));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(PluginLoadError::invalid(format!(
            "{kind} path {} must stay inside the plugin directory",
            path.display()
        )));
    }

    Ok(root.join(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn parse(input: &str) -> Result<PluginManifest, PluginLoadError> {
        PluginManifest::parse_from_str("test", input, Path::new("/plugins/demo"))
    }

    fn temp_plugin_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "urvim-plugin-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn parses_valid_manifest_with_scripts() {
        let manifest = parse(
            r#"
name = "demo"
version = "0.1.0"

[scripts]
wq = ["write", "quit"]
save_as = ["buffer write path={1}"]
"#,
        )
        .expect("manifest should parse");

        assert_eq!(manifest.name, "demo");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.themes, Vec::<PathBuf>::new());
        assert_eq!(manifest.scripts["wq"], vec!["write", "quit"]);
        assert_eq!(manifest.scripts["save_as"], vec!["buffer write path={1}"]);
    }

    #[test]
    fn parses_process_commands() {
        let manifest = parse(
            r#"
name = "demo"
version = "0.1.0"

[commands.echo]
description = "Echo text"
request = "demo/echo"
"#,
        )
        .expect("manifest should parse");

        let command = manifest
            .commands
            .get("echo")
            .expect("command should be registered");
        assert_eq!(command.description.as_deref(), Some("Echo text"));
        assert_eq!(command.request, "demo/echo");
    }

    #[test]
    fn resolves_theme_paths_relative_to_plugin_root() {
        let manifest = parse(
            r#"
name = "demo"
version = "0.1.0"
themes = ["themes/demo.toml", "more/alt.toml"]
"#,
        )
        .expect("manifest should parse");

        assert_eq!(
            manifest.themes,
            vec![
                PathBuf::from("/plugins/demo/themes/demo.toml"),
                PathBuf::from("/plugins/demo/more/alt.toml"),
            ]
        );
    }

    #[test]
    fn load_from_dir_reports_missing_manifest() {
        let root = temp_plugin_dir("missing");
        std::fs::create_dir_all(&root).expect("temp plugin dir should be created");

        let error = PluginManifest::load_from_dir(&root).expect_err("manifest should be missing");

        match error {
            PluginLoadError::Io { path, .. } => {
                assert_eq!(path, root.join(MANIFEST_FILE_NAME));
            }
            other => panic!("unexpected error: {other}"),
        }

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn load_from_dir_reads_manifest_file() {
        let root = temp_plugin_dir("valid");
        std::fs::create_dir_all(&root).expect("temp plugin dir should be created");
        std::fs::write(
            root.join(MANIFEST_FILE_NAME),
            r#"
name = "demo"
version = "0.1.0"
themes = ["themes/demo.toml"]
"#,
        )
        .expect("manifest should be written");

        let manifest = PluginManifest::load_from_dir(&root).expect("manifest should load");

        assert_eq!(manifest.name, "demo");
        assert_eq!(manifest.root, root);
        assert_eq!(
            manifest.themes,
            vec![manifest.root.join("themes/demo.toml")]
        );

        std::fs::remove_dir_all(manifest.root).ok();
    }

    #[test]
    fn rejects_empty_plugin_name() {
        let error = parse(
            r#"
name = " "
version = "0.1.0"
"#,
        )
        .expect_err("empty name should fail");

        assert!(error.to_string().contains("plugin name must not be empty"));
    }

    #[test]
    fn rejects_empty_version() {
        let error = parse(
            r#"
name = "demo"
version = " "
"#,
        )
        .expect_err("empty version should fail");

        assert!(
            error
                .to_string()
                .contains("plugin version must not be empty")
        );
    }

    #[test]
    fn rejects_empty_script_name() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"

[scripts]
"" = ["write"]
"#,
        )
        .expect_err("empty script name should fail");

        assert!(error.to_string().contains("script name must not be empty"));
    }

    #[test]
    fn rejects_parent_dir_theme_path() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"
themes = ["../outside.toml"]
"#,
        )
        .expect_err("parent path should fail");

        assert!(error.to_string().contains("must stay inside"));
    }

    #[test]
    fn rejects_absolute_theme_path() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"
themes = ["/tmp/outside.toml"]
"#,
        )
        .expect_err("absolute path should fail");

        assert!(error.to_string().contains("must be relative"));
    }

    #[test]
    fn rejects_unknown_manifest_fields() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"
unknown = true
"#,
        )
        .expect_err("unknown field should fail");

        assert!(matches!(error, PluginLoadError::Parse { .. }));
    }

    #[test]
    fn rejects_script_name_with_path_separator() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"

[scripts]
"nested/name" = ["write"]
"#,
        )
        .expect_err("script path separator should fail");

        assert!(
            error
                .to_string()
                .contains("must not contain path separators")
        );
    }

    #[test]
    fn rejects_process_command_that_conflicts_with_script() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"

[scripts]
echo = ["write"]

[commands.echo]
request = "demo/echo"
"#,
        )
        .expect_err("conflicting command should fail");

        assert!(error.to_string().contains("conflicts with a script"));
    }

    #[test]
    fn rejects_empty_process_command_request() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"

[commands.echo]
request = " "
"#,
        )
        .expect_err("empty request should fail");

        assert!(
            error
                .to_string()
                .contains("plugin command request must not be empty")
        );
    }

    #[test]
    fn parses_process_config() {
        let manifest = parse(
            r#"
name = "demo"
version = "0.1.0"

[process]
command = "demo-plugin"
args = ["--stdio"]
env = { RUST_LOG = "info" }
"#,
        )
        .expect("manifest should parse");

        let process = manifest.process.expect("process config should resolve");
        assert_eq!(process.command, "demo-plugin");
        assert_eq!(process.args, vec!["--stdio"]);
        assert_eq!(process.env["RUST_LOG"], "info");
    }

    #[test]
    fn rejects_empty_process_command() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"

[process]
command = " "
"#,
        )
        .expect_err("empty process command should fail");

        assert!(
            error
                .to_string()
                .contains("process command must not be empty")
        );
    }
}
