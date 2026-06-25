//! Plugin manifest parsing and validation.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

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
    /// BearScript entry point relative to the plugin root.
    pub entry: String,
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
    /// BearScript entry point relative to the plugin root.
    pub entry: PathBuf,
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

        let entry = validate_entry(&raw.entry)?;

        Ok(Self {
            name: raw.name,
            version: raw.version,
            description: raw.description,
            root,
            entry,
        })
    }
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

fn validate_entry(entry: &str) -> Result<PathBuf, PluginLoadError> {
    if entry.trim().is_empty() {
        return Err(PluginLoadError::invalid("plugin entry must not be empty"));
    }
    if entry.contains('\0') {
        return Err(PluginLoadError::invalid(format!(
            "plugin entry {entry:?} must not contain NUL"
        )));
    }
    let path = PathBuf::from(entry);
    if path.is_absolute() {
        return Err(PluginLoadError::invalid(format!(
            "plugin entry {entry:?} must be relative to the plugin root"
        )));
    }

    Ok(path)
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
    fn parses_valid_manifest() {
        let manifest = parse(
            r#"
name = "demo"
version = "0.1.0"
entry = "plugin.bear"
"#,
        )
        .expect("manifest should parse");

        assert_eq!(manifest.name, "demo");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.entry, PathBuf::from("plugin.bear"));
    }

    #[test]
    fn rejects_manifest_themes() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"
entry = "plugin.bear"
themes = ["themes/demo.toml", "more/alt.toml"]
"#,
        )
        .expect_err("manifest themes should be rejected");

        assert!(matches!(error, PluginLoadError::Parse { .. }));
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
entry = "plugin.bear"
"#,
        )
        .expect("manifest should be written");

        let manifest = PluginManifest::load_from_dir(&root).expect("manifest should load");

        assert_eq!(manifest.name, "demo");
        assert_eq!(manifest.root, root);
        assert_eq!(manifest.entry, PathBuf::from("plugin.bear"));

        std::fs::remove_dir_all(manifest.root).ok();
    }

    #[test]
    fn rejects_empty_plugin_name() {
        let error = parse(
            r#"
name = " "
version = "0.1.0"
entry = "plugin.bear"
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
entry = "plugin.bear"
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
    fn rejects_empty_entry() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"
entry = " "
"#,
        )
        .expect_err("empty entry should fail");

        assert!(error.to_string().contains("plugin entry must not be empty"));
    }

    #[test]
    fn rejects_unknown_manifest_fields() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"
entry = "plugin.bear"
unknown = true
"#,
        )
        .expect_err("unknown field should fail");

        assert!(matches!(error, PluginLoadError::Parse { .. }));
    }

    #[test]
    fn rejects_manifest_commands() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"
entry = "plugin.bear"

[commands.echo]
request = "demo/echo"
"#,
        )
        .expect_err("manifest commands should be rejected");

        assert!(matches!(error, PluginLoadError::Parse { .. }));
    }

    #[test]
    fn rejects_absolute_entry() {
        let error = parse(
            r#"
name = "demo"
version = "0.1.0"
entry = "/tmp/plugin.bear"
"#,
        )
        .expect_err("absolute entry should fail");

        assert!(
            error
                .to_string()
                .contains("must be relative to the plugin root")
        );
    }
}
