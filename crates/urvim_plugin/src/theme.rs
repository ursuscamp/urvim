//! Plugin-provided theme loading.

use std::fs;
use std::path::{Path, PathBuf};

use urvim_theme::{ThemeRegistry, resolve_theme_from_str};

use super::{PluginLoadError, PluginRegistry};

/// Loads all theme files contributed by loaded plugins into the theme registry.
pub fn load_plugin_themes(
    registry: &mut ThemeRegistry,
    plugins: &PluginRegistry,
) -> Result<(), PluginLoadError> {
    for (plugin_name, plugin) in plugins.iter() {
        for path in discover_theme_paths(plugin.root())? {
            load_theme_file(registry, plugin_name, &path)?;
        }
    }

    Ok(())
}

/// Loads a single plugin theme file into the registry.
pub fn load_theme_file(
    registry: &mut ThemeRegistry,
    plugin: &str,
    path: &Path,
) -> Result<String, PluginLoadError> {
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
    let name = theme.name().to_string();
    registry
        .insert(theme)
        .map_err(|source| PluginLoadError::Theme {
            plugin: plugin.to_string(),
            path: path.to_path_buf(),
            source,
        })?;

    Ok(name)
}

fn discover_theme_paths(plugin_root: &Path) -> Result<Vec<PathBuf>, PluginLoadError> {
    let themes_dir = plugin_root.join("themes");
    let entries = match fs::read_dir(&themes_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(PluginLoadError::Io {
                path: themes_dir,
                source,
            });
        }
    };

    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| PluginLoadError::Io {
            path: themes_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "toml")
        {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PluginConfigEntry, PluginRegistry};
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

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
entry = "plugin.bear"
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
entry = "plugin.bear"
"#
            ),
        )
        .expect("manifest should be written");
        std::fs::write(root.join("themes/theme.toml"), "name = 1")
            .expect("theme should be written");
    }

    fn write_plugin_without_themes(root: &Path, plugin_name: &str) {
        std::fs::create_dir_all(root).expect("plugin dirs should be created");
        std::fs::write(
            root.join(crate::MANIFEST_FILE_NAME),
            format!(
                r#"
name = "{plugin_name}"
version = "0.1.0"
entry = "plugin.bear"
"#
            ),
        )
        .expect("manifest should be written");
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
    fn missing_themes_directory_is_ignored() {
        let root = unique_temp_dir("missing-dir");
        write_plugin_without_themes(&root, "no-themes");
        let plugins =
            PluginRegistry::load_from_config(&config_with_plugin("no-themes", root.clone()))
                .expect("plugin should load");
        let mut registry = ThemeRegistry::load_builtin().expect("builtins should load");

        load_plugin_themes(&mut registry, &plugins).expect("missing themes dir should be ignored");

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn non_toml_and_nested_theme_files_are_ignored() {
        let root = unique_temp_dir("ignored-files");
        write_plugin(&root, "ignored-files", "Direct Theme");
        std::fs::write(
            root.join("themes/ignored.txt"),
            minimal_theme("Ignored Text"),
        )
        .expect("ignored file should be written");
        std::fs::create_dir_all(root.join("themes/nested")).expect("nested dir should be created");
        std::fs::write(
            root.join("themes/nested/nested.toml"),
            minimal_theme("Nested Theme"),
        )
        .expect("nested theme should be written");
        let plugins =
            PluginRegistry::load_from_config(&config_with_plugin("ignored-files", root.clone()))
                .expect("plugin should load");
        let mut registry = ThemeRegistry::load_builtin().expect("builtins should load");

        load_plugin_themes(&mut registry, &plugins).expect("direct theme should load");

        assert!(registry.get("Direct Theme").is_some());
        assert!(registry.get("Ignored Text").is_none());
        assert!(registry.get("Nested Theme").is_none());

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
    fn plugin_theme_can_be_selected_after_loading() {
        let root = unique_temp_dir("theme-after-loading");
        write_plugin(&root, "theme-loader", "Theme Loader");
        let plugins =
            PluginRegistry::load_from_config(&config_with_plugin("theme-loader", root.clone()))
                .expect("plugin should load");
        let mut registry = ThemeRegistry::load_builtin().expect("builtins should load");

        load_plugin_themes(&mut registry, &plugins).expect("plugin theme should load");

        assert_eq!(
            registry.get("Theme Loader").map(|theme| theme.name()),
            Some("Theme Loader")
        );

        std::fs::remove_dir_all(root).ok();
    }
}
