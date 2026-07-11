use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;
use urvim_core::ui::Intent;
use urvim_core::ui::floating_window::{FloatingMargins, FloatingPlacement};
use urvim_core::ui::plugin_window::{
    PluginWindowContent, PluginWindowOptions, PluginWindowSegment, id_from_number, id_to_number,
    parse_anchor, parse_key_sequence,
};

use super::super::super::{SharedLayout, native_fn, validate_plugin_command_execution_intent};

pub(in crate::plugin::host::ui) fn windows_module(
    plugin: String,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    layout: SharedLayout,
) -> Value {
    let create_plugin = plugin.clone();
    let create_layout = Rc::clone(&layout);
    let configure_plugin = plugin.clone();
    let configure_layout = Rc::clone(&layout);
    let content_plugin = plugin.clone();
    let content_layout = Rc::clone(&layout);
    let show_plugin = plugin.clone();
    let show_layout = Rc::clone(&layout);
    let hide_plugin = plugin.clone();
    let hide_layout = Rc::clone(&layout);
    let focus_plugin = plugin.clone();
    let focus_layout = Rc::clone(&layout);
    let blur_plugin = plugin.clone();
    let blur_layout = Rc::clone(&layout);
    let close_plugin = plugin.clone();
    let close_layout = Rc::clone(&layout);
    let list_plugin = plugin.clone();
    let list_layout = Rc::clone(&layout);
    let active_plugin = plugin.clone();
    let active_layout = Rc::clone(&layout);
    let set_keymap_plugin = plugin.clone();
    let set_keymap_layout = Rc::clone(&layout);
    let set_keymap_contributions = Rc::clone(&contributions);
    let delete_keymap_plugin = plugin.clone();
    let delete_keymap_layout = Rc::clone(&layout);
    let list_keymap_plugin = plugin;
    let list_keymap_layout = layout;

    Value::Module(
        HashMap::from([
            (
                "create".to_string(),
                native_fn("ui.windows.create", move |opts: Option<Value>| {
                    let options = options_from_value(opts, None)?;
                    let id = create_layout
                        .borrow_mut()
                        .create_plugin_window(create_plugin.clone(), options);
                    Ok(id_to_number(id))
                }),
            ),
            (
                "configure".to_string(),
                native_fn(
                    "ui.windows.configure",
                    move |window_id: f64, opts: Value| {
                        let id = id_from_number(window_id)?;
                        let current = configure_layout
                            .borrow()
                            .plugin_windows()
                            .owned_window(&configure_plugin, id)?
                            .options()
                            .clone();
                        let options = options_from_value(Some(opts), Some(current))?;
                        configure_layout
                            .borrow_mut()
                            .plugin_windows_mut()
                            .configure(&configure_plugin, id, options)
                    },
                ),
            ),
            (
                "set_content".to_string(),
                native_fn(
                    "ui.windows.set_content",
                    move |window_id: f64, content: Value| {
                        let id = id_from_number(window_id)?;
                        let content = content_from_value(&content)?;
                        content_layout
                            .borrow_mut()
                            .plugin_windows_mut()
                            .set_content(&content_plugin, id, content)
                    },
                ),
            ),
            (
                "show".to_string(),
                native_fn("ui.windows.show", move |window_id: f64| {
                    show_layout
                        .borrow_mut()
                        .plugin_windows_mut()
                        .show(&show_plugin, id_from_number(window_id)?)
                }),
            ),
            (
                "hide".to_string(),
                native_fn("ui.windows.hide", move |window_id: f64| {
                    hide_layout
                        .borrow_mut()
                        .plugin_windows_mut()
                        .hide(&hide_plugin, id_from_number(window_id)?)
                }),
            ),
            (
                "focus".to_string(),
                native_fn("ui.windows.focus", move |window_id: f64| {
                    focus_layout
                        .borrow_mut()
                        .plugin_windows_mut()
                        .focus(&focus_plugin, id_from_number(window_id)?)
                }),
            ),
            (
                "blur".to_string(),
                native_fn("ui.windows.blur", move |window_id: f64| {
                    blur_layout
                        .borrow_mut()
                        .plugin_windows_mut()
                        .blur(&blur_plugin, id_from_number(window_id)?)
                }),
            ),
            (
                "close".to_string(),
                native_fn("ui.windows.close", move |window_id: f64| {
                    close_layout
                        .borrow_mut()
                        .plugin_windows_mut()
                        .close(&close_plugin, id_from_number(window_id)?)
                }),
            ),
            (
                "list".to_string(),
                native_fn("ui.windows.list", move || {
                    let layout = list_layout.borrow();
                    let ids = layout
                        .plugin_windows()
                        .ids()
                        .filter(|id| {
                            layout
                                .plugin_windows()
                                .owned_window(&list_plugin, *id)
                                .is_ok()
                        })
                        .map(|id| Value::Number(id_to_number(id)))
                        .collect::<Vec<_>>();
                    Ok(Value::List(ids.into()))
                }),
            ),
            (
                "active".to_string(),
                native_fn("ui.windows.active", move || {
                    let layout = active_layout.borrow();
                    let value = layout
                        .plugin_windows()
                        .focused()
                        .filter(|id| {
                            layout
                                .plugin_windows()
                                .owned_window(&active_plugin, *id)
                                .is_ok()
                        })
                        .map(id_to_number)
                        .map(Value::Number)
                        .unwrap_or(Value::Null);
                    Ok(value)
                }),
            ),
            (
                "set_keymap".to_string(),
                native_fn(
                    "ui.windows.set_keymap",
                    move |window_id: f64, lhs: String, rhs: String| {
                        let id = id_from_number(window_id)?;
                        let keys = parse_key_sequence(&lhs)?;
                        let intent = parse_window_command(
                            &set_keymap_plugin,
                            &rhs,
                            &set_keymap_contributions,
                        )?;
                        set_keymap_layout
                            .borrow_mut()
                            .plugin_windows_mut()
                            .set_keymap(&set_keymap_plugin, id, keys, rhs, intent)
                    },
                ),
            ),
            (
                "delete_keymap".to_string(),
                native_fn(
                    "ui.windows.delete_keymap",
                    move |window_id: f64, lhs: String| {
                        let keys = parse_key_sequence(&lhs)?;
                        delete_keymap_layout
                            .borrow_mut()
                            .plugin_windows_mut()
                            .delete_keymap(&delete_keymap_plugin, id_from_number(window_id)?, &keys)
                    },
                ),
            ),
            (
                "list_keymaps".to_string(),
                native_fn("ui.windows.list_keymaps", move |window_id: f64| {
                    let bindings = list_keymap_layout
                        .borrow()
                        .plugin_windows()
                        .keymaps(&list_keymap_plugin, id_from_number(window_id)?)?;
                    Ok(Value::List(
                        bindings
                            .into_iter()
                            .map(|(keys, rhs)| {
                                Value::Map(
                                    HashMap::from([
                                        ("lhs".to_string(), Value::String(keys.concat().into())),
                                        ("rhs".to_string(), Value::String(rhs.into())),
                                    ])
                                    .into(),
                                )
                            })
                            .collect::<Vec<_>>()
                            .into(),
                    ))
                }),
            ),
        ])
        .into(),
    )
}

fn options_from_value(
    value: Option<Value>,
    mut options: Option<PluginWindowOptions>,
) -> Result<PluginWindowOptions, String> {
    let value = value.unwrap_or(Value::Null);
    if matches!(value, Value::Null) {
        return Ok(options.unwrap_or_default());
    }
    let Value::Map(map) = value else {
        return Err("plugin window options must be a map or null".to_string());
    };
    let allowed = [
        "placement",
        "rows",
        "cols",
        "title",
        "body_style",
        "border_style",
        "focused_border_style",
    ];
    if let Some(key) = map.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown plugin window option {key}"));
    }
    let mut current = options.take().unwrap_or_default();
    if let Some(value) = map.get("rows") {
        current.rows = dimension_value(value, "rows")?;
    }
    if let Some(value) = map.get("cols") {
        current.cols = dimension_value(value, "cols")?;
    }
    if let Some(value) = map.get("placement") {
        current.placement = placement_from_value(value)?;
    }
    if let Some(value) = map.get("title") {
        current.title = optional_string_value(value, "title")?;
    }
    if let Some(value) = map.get("body_style") {
        current.body_style = parse_tag(string_value(value, "body_style")?, "body_style")?;
    }
    if let Some(value) = map.get("border_style") {
        current.border_style = parse_tag(string_value(value, "border_style")?, "border_style")?;
    }
    if let Some(value) = map.get("focused_border_style") {
        current.focused_border_style = parse_tag(
            string_value(value, "focused_border_style")?,
            "focused_border_style",
        )?;
    }
    if current.rows == 0 || current.cols == 0 {
        return Err("plugin window rows and cols must be positive".to_string());
    }
    Ok(current)
}

fn placement_from_value(value: &Value) -> Result<FloatingPlacement, String> {
    let Value::Map(map) = value else {
        return Err("placement must be a map".to_string());
    };
    for key in map.keys() {
        if !matches!(key.as_str(), "type" | "anchor" | "margins" | "row" | "col") {
            return Err(format!("unknown plugin window placement option {key}"));
        }
    }

    let placement_type = string_value(
        map.get("type")
            .ok_or_else(|| "placement requires type".to_string())?,
        "placement.type",
    )?;
    match placement_type.as_str() {
        "anchored" => {
            if map.contains_key("row") || map.contains_key("col") {
                return Err("anchored placement cannot specify row or col".to_string());
            }
            let anchor = parse_anchor(&string_value(
                map.get("anchor")
                    .ok_or_else(|| "anchored placement requires anchor".to_string())?,
                "placement.anchor",
            )?)?;
            let margins = map
                .get("margins")
                .map(|value| margins_from_value(value, FloatingMargins::default()))
                .transpose()?
                .unwrap_or_default();
            Ok(FloatingPlacement::Anchored { anchor, margins })
        }
        "fixed" => {
            if map.contains_key("anchor") || map.contains_key("margins") {
                return Err("fixed placement cannot specify anchor or margins".to_string());
            }
            let row = coordinate_value(
                map.get("row")
                    .ok_or_else(|| "fixed placement requires row".to_string())?,
                "placement.row",
            )?;
            let col = coordinate_value(
                map.get("col")
                    .ok_or_else(|| "fixed placement requires col".to_string())?,
                "placement.col",
            )?;
            Ok(FloatingPlacement::Fixed { row, col })
        }
        other => Err(format!("unknown plugin window placement type {other}")),
    }
}

pub(super) fn content_from_value(value: &Value) -> Result<PluginWindowContent, String> {
    let Value::List(lines) = value else {
        return Err("plugin window content must be a list of lines".to_string());
    };
    lines
        .iter()
        .enumerate()
        .map(|(index, value)| line_from_value(value, index))
        .collect()
}

fn line_from_value(value: &Value, line_index: usize) -> Result<Vec<PluginWindowSegment>, String> {
    let Value::List(segments) = value else {
        return Err(format!("plugin window line {line_index} must be a list"));
    };
    segments
        .iter()
        .enumerate()
        .map(|(segment_index, value)| {
            let Value::Map(map) = value else {
                return Err(format!(
                    "plugin window segment {line_index}:{segment_index} must be a map"
                ));
            };
            for key in map.keys() {
                if key != "text" && key != "style" {
                    return Err(format!("unknown plugin window segment option {key}"));
                }
            }
            let text = string_value(
                map.get("text")
                    .ok_or_else(|| "plugin window segment requires text".to_string())?,
                "text",
            )?;
            if text.contains('\n') {
                return Err("plugin window segment text must not contain newlines".to_string());
            }
            let style = map
                .get("style")
                .map(|value| match value {
                    Value::Null => Ok(None),
                    _ => parse_tag(string_value(value, "style")?, "style").map(Some),
                })
                .transpose()?
                .flatten();
            Ok(PluginWindowSegment { text, style })
        })
        .collect()
}

pub(super) fn parse_window_command(
    plugin: &str,
    rhs: &str,
    contributions: &Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
) -> Result<Intent, String> {
    let intent = urvim_core::command::parse(rhs).map_err(|error| error.to_string())?;
    if let Intent::Command(urvim_core::ui::Command::PluginRequest {
        plugin: target,
        command,
        ..
    }) = &intent
    {
        if target != plugin {
            return Err(
                "plugin window keymaps may only invoke commands from their owner".to_string(),
            );
        }
        if contributions.borrow().command(plugin, command).is_none() {
            return Err(format!("unknown plugin command {plugin} {command}"));
        }
    } else {
        validate_plugin_command_execution_intent(&intent)?;
    }
    Ok(intent)
}

fn dimension_value(value: &Value, label: &str) -> Result<u16, String> {
    let number = match value {
        Value::Number(number) => *number,
        _ => return Err(format!("{label} must be a positive integer")),
    };
    if !number.is_finite() || number <= 0.0 || number.fract() != 0.0 || number > u16::MAX as f64 {
        return Err(format!("{label} must be a positive integer"));
    }
    Ok(number as u16)
}

fn coordinate_value(value: &Value, label: &str) -> Result<u16, String> {
    let number = match value {
        Value::Number(number) => *number,
        _ => return Err(format!("{label} must be a non-negative integer")),
    };
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 || number > u16::MAX as f64 {
        return Err(format!("{label} must be a non-negative integer"));
    }
    Ok(number as u16)
}

fn margins_from_value(
    value: &Value,
    mut margins: FloatingMargins,
) -> Result<FloatingMargins, String> {
    if matches!(value, Value::Null) {
        return Ok(FloatingMargins::default());
    }
    let Value::Map(map) = value else {
        return Err("margins must be a map or null".to_string());
    };
    for key in map.keys() {
        if !matches!(key.as_str(), "top" | "right" | "bottom" | "left") {
            return Err(format!("unknown plugin window margin {key}"));
        }
    }
    if let Some(value) = map.get("top") {
        margins.top = optional_margin_value(value, "margins.top")?.unwrap_or(0);
    }
    if let Some(value) = map.get("right") {
        margins.right = optional_margin_value(value, "margins.right")?.unwrap_or(0);
    }
    if let Some(value) = map.get("bottom") {
        margins.bottom = optional_margin_value(value, "margins.bottom")?.unwrap_or(0);
    }
    if let Some(value) = map.get("left") {
        margins.left = optional_margin_value(value, "margins.left")?.unwrap_or(0);
    }
    Ok(margins)
}

fn optional_margin_value(value: &Value, label: &str) -> Result<Option<u16>, String> {
    if matches!(value, Value::Null) {
        return Ok(None);
    }
    let Value::Number(number) = value else {
        return Err(format!("{label} must be a non-negative integer or null"));
    };
    if !number.is_finite() || *number < 0.0 || number.fract() != 0.0 || *number > u16::MAX as f64 {
        return Err(format!("{label} must be a non-negative integer or null"));
    }
    Ok(Some(*number as u16))
}

fn string_value(value: &Value, label: &str) -> Result<String, String> {
    match value {
        Value::String(value) => Ok(value.to_string()),
        _ => Err(format!("{label} must be a string")),
    }
}

fn optional_string_value(value: &Value, label: &str) -> Result<Option<String>, String> {
    match value {
        Value::Null => Ok(None),
        Value::String(value) => Ok(Some(value.to_string())),
        _ => Err(format!("{label} must be a string or null")),
    }
}

fn parse_tag(value: String, label: &str) -> Result<urvim_theme::Tag, String> {
    urvim_theme::Tag::parse(value.as_str()).map_err(|error| format!("{label} is invalid: {error}"))
}
