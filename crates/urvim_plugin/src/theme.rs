//! Plugin-provided theme loading.

use std::fs;

use urvim_theme::{ThemeRegistry, resolve_theme_from_str};

use super::{PluginLoadError, PluginRegistry};

/// Loads all theme files contributed by loaded plugins into the theme registry.
pub fn load_plugin_themes(
    registry: &mut ThemeRegistry,
    plugins: &PluginRegistry,
) -> Result<(), PluginLoadError> {
    for (plugin, path) in plugins.theme_paths() {
        let source = fs::read_to_string(path).map_err(|source| PluginLoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let theme = resolve_theme_from_str(&path.to_string_lossy(), &source).map_err(|source| {
            PluginLoadError::Theme {
                plugin: plugin.to_string(),
                path: path.to_path_buf(),
                source,
            }
        })?;
        registry
            .insert(theme)
            .map_err(|source| PluginLoadError::Theme {
                plugin: plugin.to_string(),
                path: path.to_path_buf(),
                source,
            })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PluginConfigEntry, PluginRegistry};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    const EXAMPLE_PLUGIN_ROOT: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/plugins/demo-plugin");

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "urvim-plugin-theme-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn write_plugin(root: &Path, plugin_name: &str, theme_name: &str) {
        std::fs::create_dir_all(root.join("themes")).expect("plugin dirs should be created");
        std::fs::write(
            root.join(crate::MANIFEST_FILE_NAME),
            format!(
                r#"
name = "{plugin_name}"
version = "0.1.0"
themes = ["themes/theme.toml"]
"#
            ),
        )
        .expect("manifest should be written");
        std::fs::write(root.join("themes/theme.toml"), minimal_theme(theme_name))
            .expect("theme should be written");
    }

    fn write_invalid_theme_plugin(root: &Path, plugin_name: &str) {
        std::fs::create_dir_all(root.join("themes")).expect("plugin dirs should be created");
        std::fs::write(
            root.join(crate::MANIFEST_FILE_NAME),
            format!(
                r#"
name = "{plugin_name}"
version = "0.1.0"
themes = ["themes/theme.toml"]
"#
            ),
        )
        .expect("manifest should be written");
        std::fs::write(root.join("themes/theme.toml"), "name = 1")
            .expect("theme should be written");
    }

    fn minimal_theme(name: &str) -> String {
        format!(
            r##"
name = "{name}"

[palette]
bg = "#101010"
fg = "#eeeeee"

[default]
fg = "fg"
bg = "bg"
"##
        )
    }

    fn config_with_plugin(name: &str, path: PathBuf) -> BTreeMap<String, PluginConfigEntry> {
        BTreeMap::from([(
            name.to_string(),
            PluginConfigEntry {
                enabled: true,
                path,
            },
        )])
    }

    #[test]
    fn plugin_theme_is_available_by_name_after_loading() {
        let root = unique_temp_dir("valid");
        write_plugin(&root, "theme-demo", "Plugin Theme");
        let plugins =
            PluginRegistry::load_from_config(&config_with_plugin("theme-demo", root.clone()))
                .expect("plugin should load");
        let mut registry = ThemeRegistry::load_builtin().expect("builtins should load");

        load_plugin_themes(&mut registry, &plugins).expect("plugin theme should load");

        assert_eq!(
            registry.get("Plugin Theme").map(|theme| theme.name()),
            Some("Plugin Theme")
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn duplicate_plugin_theme_name_is_rejected() {
        let first = unique_temp_dir("duplicate-first");
        let second = unique_temp_dir("duplicate-second");
        write_plugin(&first, "first-theme", "Duplicate Theme");
        write_plugin(&second, "second-theme", "Duplicate Theme");
        let config = BTreeMap::from([
            (
                "first-theme".to_string(),
                PluginConfigEntry {
                    enabled: true,
                    path: first.clone(),
                },
            ),
            (
                "second-theme".to_string(),
                PluginConfigEntry {
                    enabled: true,
                    path: second.clone(),
                },
            ),
        ]);
        let plugins = PluginRegistry::load_from_config(&config).expect("plugins should load");
        let mut registry = ThemeRegistry::load_builtin().expect("builtins should load");

        let error = load_plugin_themes(&mut registry, &plugins).expect_err("duplicate should fail");

        assert!(error.to_string().contains("duplicate theme name"));

        std::fs::remove_dir_all(first).ok();
        std::fs::remove_dir_all(second).ok();
    }

    #[test]
    fn duplicate_builtin_theme_name_is_rejected() {
        let root = unique_temp_dir("duplicate-builtin");
        write_plugin(&root, "builtin-duplicate", "Friday Night");
        let plugins = PluginRegistry::load_from_config(&config_with_plugin(
            "builtin-duplicate",
            root.clone(),
        ))
        .expect("plugin should load");
        let mut registry = ThemeRegistry::load_builtin().expect("builtins should load");

        let error = load_plugin_themes(&mut registry, &plugins).expect_err("duplicate should fail");

        assert!(error.to_string().contains("duplicate theme name"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn invalid_plugin_theme_toml_is_rejected_with_path_context() {
        let root = unique_temp_dir("invalid");
        write_invalid_theme_plugin(&root, "invalid-theme");
        let theme_path = root.join("themes/theme.toml");
        let plugins =
            PluginRegistry::load_from_config(&config_with_plugin("invalid-theme", root.clone()))
                .expect("plugin should load");
        let mut registry = ThemeRegistry::load_builtin().expect("builtins should load");

        let error = load_plugin_themes(&mut registry, &plugins).expect_err("theme should fail");

        assert!(error.to_string().contains("invalid-theme"));
        assert!(
            error
                .to_string()
                .contains(theme_path.to_string_lossy().as_ref())
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn example_plugin_theme_can_be_selected_after_loading() {
        let plugins = PluginRegistry::load_from_config(&config_with_plugin(
            "demo-plugin",
            PathBuf::from(EXAMPLE_PLUGIN_ROOT),
        ))
        .expect("example plugin should load");
        let mut registry = ThemeRegistry::load_builtin().expect("builtins should load");

        load_plugin_themes(&mut registry, &plugins).expect("example theme should load");

        assert_eq!(
            registry.get("Demo Night").map(|theme| theme.name()),
            Some("Demo Night")
        );
    }
}
