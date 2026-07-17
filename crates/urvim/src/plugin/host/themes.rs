use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;

use bearscript::Value;
use urvim_core::globals;
use urvim_theme::{RawColorValue, RawStyle, RawTheme};

use super::native_fn;
use crate::plugin::conversion::{BearMapRef, BearNumber, BearValueRef, FromBearValue};

pub(in crate::plugin) fn themes_module(
    plugin: String,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
) -> Value {
    let set_plugin = plugin.clone();
    let register_plugin = plugin.clone();
    let register_contributions = Rc::clone(&contributions);
    let create_plugin = plugin.clone();
    let create_contributions = Rc::clone(&contributions);
    let unregister_plugin = plugin;
    let unregister_contributions = contributions;

    Value::Module(
        HashMap::from([
            (
                "list".to_string(),
                native_fn("themes.list", move || list_themes()),
            ),
            (
                "set".to_string(),
                native_fn("themes.set", move |name: String| {
                    globals::activate_theme(&name, urvim_core::event::ThemeChangeSource::Plugin)?;
                    tracing::debug!(plugin = set_plugin, theme = name, "plugin set active theme");
                    Ok(())
                }),
            ),
            (
                "register".to_string(),
                native_fn("themes.register", move |path: String| {
                    register_theme(
                        &register_plugin,
                        Rc::clone(&register_contributions),
                        PathBuf::from(path),
                    )
                }),
            ),
            (
                "create".to_string(),
                native_fn("themes.create", move |theme: Value| {
                    create_theme(&create_plugin, Rc::clone(&create_contributions), theme)
                }),
            ),
            (
                "unregister".to_string(),
                native_fn("themes.unregister", move |name: String| {
                    unregister_theme(
                        &unregister_plugin,
                        Rc::clone(&unregister_contributions),
                        &name,
                    )
                }),
            ),
        ])
        .into(),
    )
}

fn list_themes() -> Result<Value, String> {
    let active = globals::with_active_theme(|theme| theme.map(|theme| theme.name().to_string()));
    let themes = globals::with_theme_registry(|registry| {
        registry
            .map(|registry| {
                registry
                    .names()
                    .into_iter()
                    .map(|name| theme_to_value(name, active.as_deref()))
                    .collect::<Vec<_>>()
            })
            .ok_or_else(|| "theme registry is unavailable".to_string())
    })?;

    Ok(Value::List(themes.into()))
}

fn theme_to_value(name: &str, active: Option<&str>) -> Value {
    Value::Map(
        HashMap::from([
            (
                "name".to_string(),
                Value::String(name.to_string().into_boxed_str().into()),
            ),
            (
                "active".to_string(),
                Value::Bool(active.is_some_and(|active| active == name)),
            ),
        ])
        .into(),
    )
}

fn register_theme(
    plugin: &str,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    path: PathBuf,
) -> Result<String, String> {
    let name = globals::with_theme_registry_mut(|registry| {
        let registry = registry.ok_or_else(|| "theme registry is unavailable".to_string())?;
        urvim_plugin::load_theme_file(registry, plugin, &path).map_err(|error| error.to_string())
    })?;

    contributions
        .borrow_mut()
        .register_theme(
            plugin.to_string(),
            urvim_plugin::DynamicPluginTheme {
                name: name.clone(),
                source: urvim_plugin::DynamicPluginThemeSource::File(path),
            },
        )
        .map_err(|error| {
            globals::with_theme_registry_mut(|registry| {
                if let Some(registry) = registry {
                    registry.remove(&name);
                }
            });
            error
        })?;

    Ok(name)
}

fn create_theme(
    plugin: &str,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    theme: Value,
) -> Result<String, String> {
    let raw_theme = raw_theme_from_value(theme)?;
    let theme = urvim_theme::resolve_theme(raw_theme).map_err(|error| error.to_string())?;
    let name = theme.name().to_string();

    globals::with_theme_registry_mut(|registry| {
        let registry = registry.ok_or_else(|| "theme registry is unavailable".to_string())?;
        registry.insert(theme).map_err(|error| error.to_string())
    })?;

    contributions
        .borrow_mut()
        .register_theme(
            plugin.to_string(),
            urvim_plugin::DynamicPluginTheme {
                name: name.clone(),
                source: urvim_plugin::DynamicPluginThemeSource::Script,
            },
        )
        .map_err(|error| {
            globals::with_theme_registry_mut(|registry| {
                if let Some(registry) = registry {
                    registry.remove(&name);
                }
            });
            error
        })?;

    Ok(name)
}

fn unregister_theme(
    plugin: &str,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    name: &str,
) -> Result<(), String> {
    let removed = contributions
        .borrow_mut()
        .unregister_theme(plugin, name)
        .ok_or_else(|| format!("plugin {plugin:?} does not own theme {name:?}"))?;

    globals::with_theme_registry_mut(|registry| {
        let registry = registry.ok_or_else(|| "theme registry is unavailable".to_string())?;
        registry.remove(&removed.name);
        Ok::<(), String>(())
    })?;

    let removed_active =
        globals::with_active_theme(|theme| theme.is_some_and(|theme| theme.name() == name));
    if removed_active {
        let fallback_name = globals::with_theme_registry(|registry| {
            registry
                .map(|registry| registry.default_theme().name().to_string())
                .ok_or_else(|| "theme registry is unavailable".to_string())
        })?;
        globals::activate_theme(
            &fallback_name,
            urvim_core::event::ThemeChangeSource::Fallback,
        )?;
    }

    Ok(())
}

fn raw_theme_from_value(value: Value) -> Result<RawTheme, String> {
    let map = BearValueRef::new(&value, "theme")
        .map()
        .map_err(|error| error.to_string())?;
    reject_unknown_keys(&map, &["name", "palette", "default", "highlights"], "theme")?;

    let name = String::from_bear(map.required("name").map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let palette = raw_palette_from_value(
        map.required("palette")
            .map_err(|error| error.to_string())?
            .value(),
    )?;
    let default = raw_style_from_value(
        map.required("default")
            .map_err(|error| error.to_string())?
            .value(),
        "theme.default",
    )?;
    let highlights = match map
        .optional("highlights")
        .map_err(|error| error.to_string())?
    {
        Some(value) => raw_highlights_from_value(value.value())?,
        None => BTreeMap::new(),
    };

    Ok(RawTheme {
        name,
        palette,
        default,
        highlights,
    })
}

fn raw_palette_from_value(value: &Value) -> Result<BTreeMap<String, RawColorValue>, String> {
    let Value::Map(map) = value else {
        return Err("theme.palette must be a map".to_string());
    };

    let mut palette = BTreeMap::new();
    for (name, value) in map.as_map() {
        palette.insert(name.clone(), raw_color_from_value(value, name)?);
    }
    Ok(palette)
}

fn raw_color_from_value(value: &Value, name: &str) -> Result<RawColorValue, String> {
    match value {
        Value::String(value) => Ok(RawColorValue::Rgb(value.to_string())),
        Value::Number(number) => Ok(RawColorValue::Ansi(u8_from_number(
            *number,
            &format!("theme.palette.{name}"),
        )?)),
        _ => Err(format!(
            "theme.palette.{name} must be a hex color string or ANSI number"
        )),
    }
}

fn raw_highlights_from_value(value: &Value) -> Result<BTreeMap<String, RawStyle>, String> {
    let Value::Map(map) = value else {
        return Err("theme.highlights must be a map".to_string());
    };

    let mut highlights = BTreeMap::new();
    for (name, value) in map.as_map() {
        highlights.insert(
            name.clone(),
            raw_style_from_value(value, &format!("theme.highlights.{name}"))?,
        );
    }
    Ok(highlights)
}

fn raw_style_from_value(value: &Value, label: &str) -> Result<RawStyle, String> {
    let map = BearValueRef::new(value, label)
        .map()
        .map_err(|error| error.to_string())?;
    reject_unknown_keys(
        &map,
        &[
            "fg",
            "bg",
            "underline_color",
            "bold",
            "italic",
            "underline",
            "double_underline",
            "dim",
            "reverse",
            "blink",
            "strikethrough",
            "overline",
            "overlay",
        ],
        label,
    )?;

    Ok(RawStyle {
        fg: optional_string(&map, "fg")?,
        bg: optional_string(&map, "bg")?,
        underline_color: optional_string(&map, "underline_color")?,
        bold: optional_bool(&map, "bold")?,
        italic: optional_bool(&map, "italic")?,
        underline: optional_bool(&map, "underline")?,
        double_underline: optional_bool(&map, "double_underline")?,
        dim: optional_bool(&map, "dim")?,
        reverse: optional_bool(&map, "reverse")?,
        blink: optional_bool(&map, "blink")?,
        strikethrough: optional_bool(&map, "strikethrough")?,
        overline: optional_bool(&map, "overline")?,
        overlay: optional_bool(&map, "overlay")?.unwrap_or(false),
    })
}

fn reject_unknown_keys(map: &BearMapRef<'_>, allowed: &[&str], label: &str) -> Result<(), String> {
    map.reject_unknown(allowed).map_err(|error| {
        let key = error.path().rsplit('.').next().unwrap_or(error.path());
        format!("unknown {label} field {key:?}")
    })
}

fn optional_string(map: &BearMapRef<'_>, key: &str) -> Result<Option<String>, String> {
    map.optional(key)
        .map_err(|error| error.to_string())?
        .map(String::from_bear)
        .transpose()
        .map_err(|error| error.to_string())
}

fn optional_bool(map: &BearMapRef<'_>, key: &str) -> Result<Option<bool>, String> {
    map.optional(key)
        .map_err(|error| error.to_string())?
        .map(bool::from_bear)
        .transpose()
        .map_err(|error| error.to_string())
}

fn u8_from_number(number: f64, label: &str) -> Result<u8, String> {
    BearNumber::new(number, label)
        .byte()
        .map_err(|error| error.to_string())
}
