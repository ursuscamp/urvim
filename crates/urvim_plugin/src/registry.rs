//! Loaded plugin registry for static manifest contributions.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{PluginLoadError, PluginManifest};

/// A loaded manifest-only plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedPlugin {
    /// Validated plugin manifest.
    pub manifest: PluginManifest,
}

impl LoadedPlugin {
    /// Returns the plugin name used for namespaced commands.
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    /// Returns the resolved plugin root directory.
    pub fn root(&self) -> &Path {
        &self.manifest.root
    }

    /// Returns the BearScript entry file path.
    pub fn entry(&self) -> &Path {
        &self.manifest.entry
    }
}

/// Registry of loaded manifest-only plugins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginRegistry {
    plugins: BTreeMap<String, LoadedPlugin>,
}

/// Resolved plugin configuration consumed by the plugin registry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginConfigEntry {
    /// Whether the plugin should be loaded.
    pub enabled: bool,
    /// The resolved plugin directory.
    pub path: PathBuf,
}

impl PluginRegistry {
    /// Loads all enabled plugins declared by the resolved editor config.
    pub fn load_from_config<'a>(
        plugins: impl IntoIterator<Item = (&'a String, &'a PluginConfigEntry)>,
    ) -> Result<Self, PluginLoadError> {
        let mut registry = Self::default();

        for (id, plugin_config) in plugins {
            if !plugin_config.enabled {
                tracing::debug!(plugin = id, path = ?plugin_config.path, "skipping disabled plugin");
                continue;
            }

            let manifest =
                PluginManifest::load_from_dir(&plugin_config.path).map_err(|source| {
                    PluginLoadError::Plugin {
                        id: id.clone(),
                        path: plugin_config.path.clone(),
                        source: Box::new(source),
                    }
                })?;

            if manifest.name != *id {
                return Err(PluginLoadError::Plugin {
                    id: id.clone(),
                    path: plugin_config.path.clone(),
                    source: Box::new(PluginLoadError::invalid(format!(
                        "configured plugin id {id:?} does not match manifest name {:?}",
                        manifest.name
                    ))),
                });
            }

            registry.insert(LoadedPlugin { manifest })?;
            tracing::debug!(plugin = id, path = ?plugin_config.path, "loaded plugin manifest");
        }

        Ok(registry)
    }

    /// Returns true when no plugins are loaded.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Returns the number of loaded plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Looks up a loaded plugin by name.
    pub fn get(&self, name: &str) -> Option<&LoadedPlugin> {
        self.plugins.get(name)
    }

    /// Returns an iterator over loaded plugins in name order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &LoadedPlugin)> {
        self.plugins
            .iter()
            .map(|(name, plugin)| (name.as_str(), plugin))
    }

    fn insert(&mut self, plugin: LoadedPlugin) -> Result<(), PluginLoadError> {
        let name = plugin.name().to_string();
        if self.plugins.contains_key(&name) {
            return Err(PluginLoadError::invalid(format!(
                "duplicate plugin name {name:?}"
            )));
        }

        self.plugins.insert(name, plugin);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const CARGO_FMT_EXAMPLE_PLUGIN_ROOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/plugins/cargo-fmt"
    );
    const SYMBOL_LENS_EXAMPLE_PLUGIN_ROOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/plugins/symbol-lens"
    );

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "urvim-plugin-registry-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn write_manifest(root: &Path, contents: &str) {
        std::fs::create_dir_all(root).expect("plugin root should be created");
        std::fs::write(root.join(crate::MANIFEST_FILE_NAME), contents)
            .expect("manifest should be written");
    }

    fn enabled(path: PathBuf) -> PluginConfigEntry {
        PluginConfigEntry {
            enabled: true,
            path,
        }
    }

    fn disabled(path: PathBuf) -> PluginConfigEntry {
        PluginConfigEntry {
            enabled: false,
            path,
        }
    }

    fn config_with_plugins(
        plugins: BTreeMap<String, PluginConfigEntry>,
    ) -> BTreeMap<String, PluginConfigEntry> {
        plugins
    }

    #[test]
    fn enabled_plugin_loads_from_configured_directory() {
        let root = unique_temp_dir("enabled");
        write_manifest(
            &root,
            r#"
name = "test-plugin"
version = "0.1.0"
entry = "plugin.bear"
"#,
        );
        let config = BTreeMap::from([("test-plugin".to_string(), enabled(root.clone()))]);

        let registry = PluginRegistry::load_from_config(&config).expect("plugin should load");

        assert_eq!(registry.len(), 1);
        let plugin = registry
            .get("test-plugin")
            .expect("plugin should be registered");
        assert_eq!(plugin.root(), root.as_path());
        assert_eq!(plugin.entry(), Path::new("plugin.bear"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn disabled_plugin_is_skipped_and_path_is_not_read() {
        let missing = unique_temp_dir("disabled-missing");
        let config = BTreeMap::from([("test-plugin".to_string(), disabled(missing))]);

        let registry = PluginRegistry::load_from_config(&config).expect("disabled plugin skips IO");

        assert!(registry.is_empty());
    }

    #[test]
    fn config_id_must_match_manifest_name() {
        let root = unique_temp_dir("mismatch");
        write_manifest(
            &root,
            r#"
name = "other-name"
version = "0.1.0"
entry = "plugin.bear"
"#,
        );
        let config = config_with_plugins(BTreeMap::from([(
            "test-plugin".to_string(),
            enabled(root.clone()),
        )]));

        let error = PluginRegistry::load_from_config(&config).expect_err("mismatch should fail");

        assert!(error.to_string().contains("does not match manifest name"));
        assert!(error.to_string().contains("test-plugin"));
        assert!(error.to_string().contains(root.to_string_lossy().as_ref()));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn duplicate_manifest_names_are_rejected() {
        let first = unique_temp_dir("duplicate-first");
        let second = unique_temp_dir("duplicate-second");
        write_manifest(
            &first,
            r#"
name = "duplicate"
version = "0.1.0"
entry = "plugin.bear"
"#,
        );
        write_manifest(
            &second,
            r#"
name = "duplicate"
version = "0.1.0"
entry = "plugin.bear"
"#,
        );

        let mut registry = PluginRegistry::default();
        let first_manifest = PluginManifest::load_from_dir(&first).expect("first should load");
        let second_manifest = PluginManifest::load_from_dir(&second).expect("second should load");
        registry
            .insert(LoadedPlugin {
                manifest: first_manifest,
            })
            .expect("first insert should work");

        let error = registry
            .insert(LoadedPlugin {
                manifest: second_manifest,
            })
            .expect_err("duplicate should fail");

        assert!(error.to_string().contains("duplicate plugin name"));

        std::fs::remove_dir_all(first).ok();
        std::fs::remove_dir_all(second).ok();
    }

    #[test]
    fn entry_path_is_visible() {
        let root = unique_temp_dir("contributions");
        write_manifest(
            &root,
            r#"
name = "tools"
version = "0.1.0"
entry = "plugin.bear"
"#,
        );
        let config = config_with_plugins(BTreeMap::from([(
            "tools".to_string(),
            enabled(root.clone()),
        )]));

        let registry = PluginRegistry::load_from_config(&config).expect("plugin should load");
        assert_eq!(
            registry.get("tools").unwrap().entry(),
            Path::new("plugin.bear")
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn load_error_includes_plugin_id_and_path_context() {
        let missing = unique_temp_dir("missing");
        let config = config_with_plugins(BTreeMap::from([(
            "test-plugin".to_string(),
            enabled(missing.clone()),
        )]));

        let error = PluginRegistry::load_from_config(&config).expect_err("missing should fail");

        assert!(error.to_string().contains("test-plugin"));
        assert!(
            error
                .to_string()
                .contains(missing.to_string_lossy().as_ref())
        );
    }

    #[test]
    fn example_plugin_manifest_loads_from_disk() {
        let manifest = PluginManifest::load_from_dir(CARGO_FMT_EXAMPLE_PLUGIN_ROOT)
            .expect("cargo-fmt example plugin manifest should load");

        assert_eq!(manifest.name, "cargo-fmt");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.entry, PathBuf::from("plugin.bear"));

        let manifest = PluginManifest::load_from_dir(SYMBOL_LENS_EXAMPLE_PLUGIN_ROOT)
            .expect("symbol-lens example plugin manifest should load");

        assert_eq!(manifest.name, "symbol-lens");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.entry, PathBuf::from("plugin.bear"));
    }

    #[test]
    fn example_plugin_python_project_files_exist() {
        for relative in [
            "pyproject.toml",
            "src/cargo_fmt/__init__.py",
            "src/cargo_fmt/__main__.py",
        ] {
            assert!(
                Path::new(CARGO_FMT_EXAMPLE_PLUGIN_ROOT)
                    .join(relative)
                    .exists(),
                "cargo-fmt example plugin should include {relative}"
            );
        }

        for relative in [
            "pyproject.toml",
            "src/symbol_lens/__init__.py",
            "src/symbol_lens/__main__.py",
        ] {
            assert!(
                Path::new(SYMBOL_LENS_EXAMPLE_PLUGIN_ROOT)
                    .join(relative)
                    .exists(),
                "symbol-lens example plugin should include {relative}"
            );
        }
    }
}
