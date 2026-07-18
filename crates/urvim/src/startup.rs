use urvim_core::config::Config;
use urvim_core::layout::Layout;
use urvim_plugin::PluginConfigEntry;
use urvim_theme::{Theme, ThemeRegistry};

use crate::plugin::{BearscriptPluginRuntime, SharedLayout};

#[cfg(test)]
use urvim_plugin::PluginRegistry;

pub(super) struct StartupPluginsAndThemes {
    #[cfg(test)]
    pub(super) plugin_registry: PluginRegistry,
    pub(super) plugin_runtime: BearscriptPluginRuntime,
    pub(super) theme_registry: ThemeRegistry,
    pub(super) active_theme: Theme,
}

pub(super) fn select_active_theme(
    registry: &ThemeRegistry,
    requested: Option<&str>,
) -> Result<Theme, String> {
    let theme_name = requested.unwrap_or("Friday Night");
    registry.get(theme_name).cloned().ok_or_else(|| {
        format!(
            "unknown theme {theme_name:?}; available themes: {}",
            registry.names().join(", ")
        )
    })
}

pub(super) fn load_startup_plugins_and_themes(
    config: &Config,
    layout: SharedLayout,
) -> Result<StartupPluginsAndThemes, String> {
    let plugin_config = config
        .plugins
        .iter()
        .map(|(name, plugin)| {
            (
                name.clone(),
                PluginConfigEntry {
                    enabled: plugin.enabled,
                    path: plugin.path.clone(),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let plugin_registry = urvim_plugin::PluginRegistry::load_from_config(&plugin_config)
        .map_err(|error| error.to_string())?;
    urvim_core::command::install_configured_commands_with_plugins(config, &plugin_registry)
        .map_err(|error| error.to_string())?;
    let mut theme_registry = ThemeRegistry::load_builtin().map_err(|error| error.to_string())?;
    urvim_plugin::load_plugin_themes(&mut theme_registry, &plugin_registry)
        .map_err(|error| error.to_string())?;
    urvim_core::globals::set_theme_registry(theme_registry);

    let plugin_runtime = BearscriptPluginRuntime::load_from_registry(&plugin_registry, layout);

    let theme_registry = urvim_core::globals::with_theme_registry(|registry| {
        registry
            .cloned()
            .ok_or_else(|| "theme registry is unavailable".to_string())
    })?;
    let active_theme = select_active_theme(&theme_registry, Some(config.theme.as_str()))?;

    Ok(StartupPluginsAndThemes {
        #[cfg(test)]
        plugin_registry,
        plugin_runtime,
        theme_registry,
        active_theme,
    })
}

pub(super) fn startup_layout(files: &[urvim_core::cli::CliFileSpec]) -> Layout {
    let Ok(cwd) = std::env::current_dir() else {
        tracing::warn!("failed to resolve current directory for startup");
        return Layout::from_cli_files(files);
    };

    startup_layout_for_cwd(&cwd, files)
}

pub(super) fn startup_layout_for_cwd(
    cwd: &std::path::Path,
    files: &[urvim_core::cli::CliFileSpec],
) -> Layout {
    if files.is_empty() {
        match urvim_core::session::load_session_for_cwd(cwd) {
            Ok(Some(session)) => Layout::from_session(session),
            Ok(None) => Layout::from_cli_files(&[]),
            Err(error) => {
                tracing::warn!(?error, "failed to load session");
                Layout::from_cli_files(&[])
            }
        }
    } else {
        Layout::from_cli_files(files)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use urvim_core::buffer::Buffer;
    use urvim_core::config::{Config, PluginConfig};
    use urvim_core::editor_pane::EditorPane;

    use super::*;

    fn shared_test_layout() -> SharedLayout {
        Rc::new(RefCell::new(Layout::new(EditorPane::from_buffers(vec![
            Buffer::new(),
        ]))))
    }

    fn theme_registry_test_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::theme_test_lock()
    }

    fn buffer_pool_test_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::buffer_pool_test_lock()
    }

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("urvim-{name}-{}-{stamp}", std::process::id()))
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

    fn bearscript_theme_literal(name: &str) -> String {
        format!(
            r##"{{
                    "name": "{name}",
                    "palette": {{
                        "bg": "#101010",
                        "fg": "#eeeeee",
                        "accent": "#7aa2f7",
                        "muted": 244
                    }},
                    "default": {{
                        "fg": "fg",
                        "bg": "bg"
                    }},
                    "highlights": {{
                        "ui.status_bar": {{
                            "fg": "bg",
                            "bg": "accent",
                            "bold": true
                        }},
                        "syntax.comment": {{
                            "fg": "muted",
                            "italic": true
                        }}
                    }}
                }}"##
        )
    }

    #[test]
    fn load_startup_plugins_and_themes_loads_plugin_theme() {
        let _guard = theme_registry_test_lock();
        let _pool_guard = buffer_pool_test_lock();
        let root = unique_temp_dir("startup-plugin-test");
        std::fs::create_dir_all(root.join("themes")).expect("plugin dirs should be created");
        std::fs::write(
            root.join("urvim-plugin.toml"),
            r#"
name = "test-plugin"
version = "0.1.0"
entry = "plugin.bear"
"#,
        )
        .expect("manifest should be written");
        std::fs::write(root.join("plugin.bear"), "fn init() {}")
            .expect("plugin entry should be written");
        std::fs::write(
            root.join("themes/test-theme.toml"),
            minimal_theme("Test Theme"),
        )
        .expect("theme should be written");

        let config = Config {
            theme: "Test Theme".to_string(),
            plugins: std::collections::BTreeMap::from([(
                "test-plugin".to_string(),
                PluginConfig {
                    enabled: true,
                    path: root.clone(),
                },
            )]),
            ..Config::default()
        };

        let startup = load_startup_plugins_and_themes(&config, shared_test_layout())
            .expect("startup plugins should load");

        assert_eq!(startup.active_theme.name(), "Test Theme");
        assert!(startup.theme_registry.get("Test Theme").is_some());
        assert!(startup.plugin_registry.get("test-plugin").is_some());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn load_startup_plugins_and_themes_can_select_theme_registered_from_init() {
        let _guard = theme_registry_test_lock();
        let _pool_guard = buffer_pool_test_lock();
        let root = unique_temp_dir("startup-dynamic-theme-test");
        std::fs::create_dir_all(&root).expect("plugin dir should be created");
        let theme_path = root.join("dynamic.toml");
        std::fs::write(
            root.join("urvim-plugin.toml"),
            r#"
name = "dynamic-theme-plugin"
version = "0.1.0"
entry = "plugin.bear"
"#,
        )
        .expect("manifest should be written");
        std::fs::write(
            root.join("plugin.bear"),
            format!(
                r#"
fn init() {{
    urvim.themes.register({:?})
}}
"#,
                theme_path.to_string_lossy()
            ),
        )
        .expect("plugin entry should be written");
        std::fs::write(&theme_path, minimal_theme("Dynamic Init Theme"))
            .expect("theme should be written");

        let config = Config {
            theme: "Dynamic Init Theme".to_string(),
            plugins: std::collections::BTreeMap::from([(
                "dynamic-theme-plugin".to_string(),
                PluginConfig {
                    enabled: true,
                    path: root.clone(),
                },
            )]),
            ..Config::default()
        };

        let startup = load_startup_plugins_and_themes(&config, shared_test_layout())
            .expect("startup plugin should register selected theme");

        assert_eq!(startup.active_theme.name(), "Dynamic Init Theme");
        assert!(startup.theme_registry.get("Dynamic Init Theme").is_some());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn load_startup_plugins_and_themes_can_select_theme_created_from_init() {
        let _guard = theme_registry_test_lock();
        let _pool_guard = buffer_pool_test_lock();
        let root = unique_temp_dir("startup-created-theme-test");
        std::fs::create_dir_all(&root).expect("plugin dir should be created");
        std::fs::write(
            root.join("urvim-plugin.toml"),
            r#"
name = "created-theme-plugin"
version = "0.1.0"
entry = "plugin.bear"
"#,
        )
        .expect("manifest should be written");
        std::fs::write(
            root.join("plugin.bear"),
            format!(
                r#"
fn init() {{
    urvim.themes.create({})
}}
"#,
                bearscript_theme_literal("Created Init Theme")
            ),
        )
        .expect("plugin entry should be written");

        let config = Config {
            theme: "Created Init Theme".to_string(),
            plugins: std::collections::BTreeMap::from([(
                "created-theme-plugin".to_string(),
                PluginConfig {
                    enabled: true,
                    path: root.clone(),
                },
            )]),
            ..Config::default()
        };

        let startup = load_startup_plugins_and_themes(&config, shared_test_layout())
            .expect("startup plugin should create selected theme");

        assert_eq!(startup.active_theme.name(), "Created Init Theme");
        assert!(startup.theme_registry.get("Created Init Theme").is_some());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn load_startup_plugins_and_themes_skips_disabled_missing_plugins() {
        let _guard = theme_registry_test_lock();
        let _pool_guard = buffer_pool_test_lock();
        let config = Config {
            plugins: std::collections::BTreeMap::from([(
                "missing-demo".to_string(),
                PluginConfig {
                    enabled: false,
                    path: std::path::PathBuf::from("/tmp/urvim-missing-disabled-plugin"),
                },
            )]),
            ..Config::default()
        };

        let startup = load_startup_plugins_and_themes(&config, shared_test_layout())
            .expect("disabled missing plugin should not fail startup");

        assert_eq!(startup.active_theme.name(), "Friday Night");
        assert!(startup.plugin_registry.is_empty());
    }
}
