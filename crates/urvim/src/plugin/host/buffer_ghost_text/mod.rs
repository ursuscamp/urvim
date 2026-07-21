//! Plugin-owned buffer ghost-text API.

use std::collections::HashMap;

use bearscript::Value;
use urvim_core::buffer::{Cursor, Gravity, Marker, MarkerId, MarkerPayload, MarkerShape};
use urvim_terminal::{Color, Rgb};
use urvim_theme::StyleOverlay;

use super::native_fn;
use crate::plugin::{
    buffer_id_from_value, ensure_valid_cursor, unknown_buffer_error, usize_from_value,
};

/// Builds the `urvim.buffers.ghost_text` module for one plugin.
pub(in crate::plugin) fn ghost_text_module(plugin: String) -> Value {
    let add_plugin = plugin.clone();
    let update_plugin = plugin.clone();
    let remove_plugin = plugin.clone();
    let clear_plugin = plugin.clone();
    let list_plugin = plugin;

    Value::Module(
        HashMap::from([
            (
                "add".to_string(),
                native_fn(
                    "buffers.ghost_text.add",
                    move |buffer_id: Value, options: Value| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        let options = AddOptions::from_value(&options)?;
                        let id = urvim_core::globals::with_buffer_mut(buffer_id, |buffer| {
                            ensure_valid_cursor(
                                buffer_id,
                                buffer,
                                options.position,
                                "options.position",
                            )?;
                            Ok::<_, String>(buffer.insert_namespaced_ghost_text(
                                &add_plugin,
                                options.position,
                                options.gravity,
                                options.text,
                                options.style,
                            ))
                        })
                        .ok_or_else(|| unknown_buffer_error(buffer_id))??;
                        Ok(id as f64)
                    },
                ),
            ),
            (
                "update".to_string(),
                native_fn(
                    "buffers.ghost_text.update",
                    move |buffer_id: Value, marker_id: Value, changes: Value| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        let marker_id = marker_id_from_value(&marker_id)?;
                        let changes = UpdateOptions::from_value(&changes)?;
                        urvim_core::globals::with_buffer_mut(buffer_id, |buffer| {
                            let marker = buffer
                                .namespaced_ghost_text(&update_plugin, marker_id)
                                .cloned()
                                .ok_or_else(|| unavailable_marker_error(marker_id))?;
                            let MarkerShape::Point(point) = marker.kind else {
                                return Err(unavailable_marker_error(marker_id));
                            };
                            let position = changes.position.unwrap_or(point.pos);
                            ensure_valid_cursor(buffer_id, buffer, position, "changes.position")?;
                            let gravity = changes.gravity.unwrap_or(point.gravity);
                            let text = changes
                                .text
                                .unwrap_or_else(|| marker.payload.label.clone().into());
                            let style = changes.style.unwrap_or(marker.payload.style);
                            if !buffer.update_namespaced_ghost_text(
                                &update_plugin,
                                marker_id,
                                position,
                                gravity,
                                text,
                                style,
                            ) {
                                return Err(unavailable_marker_error(marker_id));
                            }
                            Ok::<_, String>(())
                        })
                        .ok_or_else(|| unknown_buffer_error(buffer_id))??;
                        Ok(())
                    },
                ),
            ),
            (
                "remove".to_string(),
                native_fn(
                    "buffers.ghost_text.remove",
                    move |buffer_id: Value, marker_id: Value| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        let marker_id = marker_id_from_value(&marker_id)?;
                        urvim_core::globals::with_buffer_mut(buffer_id, |buffer| {
                            buffer
                                .remove_namespaced_ghost_text(&remove_plugin, marker_id)
                                .is_some()
                        })
                        .map(Value::Bool)
                        .ok_or_else(|| unknown_buffer_error(buffer_id))
                    },
                ),
            ),
            (
                "clear".to_string(),
                native_fn("buffers.ghost_text.clear", move |buffer_id: Value| {
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    urvim_core::globals::with_buffer_mut(buffer_id, |buffer| {
                        buffer.clear_namespaced_ghost_texts(&clear_plugin) as f64
                    })
                    .ok_or_else(|| unknown_buffer_error(buffer_id))
                }),
            ),
            (
                "list".to_string(),
                native_fn("buffers.ghost_text.list", move |buffer_id: Value| {
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    urvim_core::globals::with_buffer(buffer_id, |buffer| {
                        Value::List(
                            buffer
                                .namespaced_ghost_texts(&list_plugin)
                                .iter()
                                .map(marker_to_value)
                                .collect::<Vec<_>>()
                                .into(),
                        )
                    })
                    .ok_or_else(|| unknown_buffer_error(buffer_id))
                }),
            ),
        ])
        .into(),
    )
}

struct AddOptions {
    position: Cursor,
    text: String,
    gravity: Gravity,
    style: Option<StyleOverlay>,
}

impl AddOptions {
    fn from_value(value: &Value) -> Result<Self, String> {
        let Value::Map(map) = value else {
            return Err("ghost text options must be a map".to_string());
        };
        reject_unknown_keys(map, &["position", "text", "gravity", "style"], "options")?;
        let position = cursor_from_value(
            map.get("position")
                .ok_or_else(|| "ghost text options require position".to_string())?,
            "options.position",
        )?;
        let text = string_from_value(
            map.get("text")
                .ok_or_else(|| "ghost text options require text".to_string())?,
            "options.text",
        )?;
        let gravity = map
            .get("gravity")
            .map(|value| gravity_from_value(value, "options.gravity"))
            .transpose()?
            .unwrap_or(Gravity::Right);
        let style = map
            .get("style")
            .map(|value| optional_style_from_value(value, "options.style"))
            .transpose()?
            .flatten();
        Ok(Self {
            position,
            text,
            gravity,
            style,
        })
    }
}

#[derive(Default)]
struct UpdateOptions {
    position: Option<Cursor>,
    text: Option<String>,
    gravity: Option<Gravity>,
    style: Option<Option<StyleOverlay>>,
}

impl UpdateOptions {
    fn from_value(value: &Value) -> Result<Self, String> {
        let Value::Map(map) = value else {
            return Err("ghost text changes must be a map".to_string());
        };
        reject_unknown_keys(map, &["position", "text", "gravity", "style"], "changes")?;
        Ok(Self {
            position: map
                .get("position")
                .map(|value| cursor_from_value(value, "changes.position"))
                .transpose()?,
            text: map
                .get("text")
                .map(|value| string_from_value(value, "changes.text"))
                .transpose()?,
            gravity: map
                .get("gravity")
                .map(|value| gravity_from_value(value, "changes.gravity"))
                .transpose()?,
            style: map
                .get("style")
                .map(|value| optional_style_from_value(value, "changes.style"))
                .transpose()?,
        })
    }
}

fn marker_to_value(marker: &Marker<MarkerPayload>) -> Value {
    let MarkerShape::Point(point) = marker.kind else {
        unreachable!("ghost text is point anchored");
    };
    Value::Map(
        HashMap::from([
            ("id".to_string(), Value::Number(marker.id as f64)),
            ("position".to_string(), cursor_to_value(point.pos)),
            (
                "text".to_string(),
                Value::String(marker.payload.label.to_string().into_boxed_str().into()),
            ),
            (
                "gravity".to_string(),
                Value::String(
                    match point.gravity {
                        Gravity::Left => "left",
                        Gravity::Right => "right",
                    }
                    .into(),
                ),
            ),
            (
                "style".to_string(),
                marker
                    .payload
                    .style
                    .map(style_to_value)
                    .unwrap_or(Value::Null),
            ),
        ])
        .into(),
    )
}

fn cursor_from_value(value: &Value, label: &str) -> Result<Cursor, String> {
    let Value::Map(map) = value else {
        return Err(format!("{label} must be a map"));
    };
    reject_unknown_keys(map, &["row", "col"], label)?;
    let row = map
        .get("row")
        .ok_or_else(|| format!("{label} requires row"))?;
    let col = map
        .get("col")
        .ok_or_else(|| format!("{label} requires col"))?;
    Ok(Cursor::new(
        usize_from_value(row, &format!("{label}.row"))?,
        usize_from_value(col, &format!("{label}.col"))?,
    ))
}

fn cursor_to_value(cursor: Cursor) -> Value {
    Value::Map(
        HashMap::from([
            ("row".to_string(), Value::Number(cursor.line as f64)),
            ("col".to_string(), Value::Number(cursor.col as f64)),
        ])
        .into(),
    )
}

fn marker_id_from_value(value: &Value) -> Result<MarkerId, String> {
    let id = usize_from_value(value, "marker_id")?;
    Ok(id as MarkerId)
}

fn gravity_from_value(value: &Value, label: &str) -> Result<Gravity, String> {
    match string_from_value(value, label)?.as_str() {
        "left" => Ok(Gravity::Left),
        "right" => Ok(Gravity::Right),
        value => Err(format!("{label} must be left or right, got {value:?}")),
    }
}

fn optional_style_from_value(value: &Value, label: &str) -> Result<Option<StyleOverlay>, String> {
    if matches!(value, Value::Null) {
        return Ok(None);
    }
    let Value::Map(map) = value else {
        return Err(format!("{label} must be a map or null"));
    };
    reject_unknown_keys(
        map,
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
        ],
        label,
    )?;
    Ok(Some(StyleOverlay {
        fg: optional_color(map.get("fg"), &format!("{label}.fg"))?,
        bg: optional_color(map.get("bg"), &format!("{label}.bg"))?,
        underline_color: optional_color(
            map.get("underline_color"),
            &format!("{label}.underline_color"),
        )?,
        bold: optional_bool(map.get("bold"), &format!("{label}.bold"))?,
        italic: optional_bool(map.get("italic"), &format!("{label}.italic"))?,
        underline: optional_bool(map.get("underline"), &format!("{label}.underline"))?,
        double_underline: optional_bool(
            map.get("double_underline"),
            &format!("{label}.double_underline"),
        )?,
        dim: optional_bool(map.get("dim"), &format!("{label}.dim"))?,
        reverse: optional_bool(map.get("reverse"), &format!("{label}.reverse"))?,
        blink: optional_bool(map.get("blink"), &format!("{label}.blink"))?,
        strikethrough: optional_bool(map.get("strikethrough"), &format!("{label}.strikethrough"))?,
        overline: optional_bool(map.get("overline"), &format!("{label}.overline"))?,
    }))
}

fn optional_color(value: Option<&Value>, label: &str) -> Result<Option<Color>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        Value::Number(_) => Ok(Some(Color::ansi(
            crate::plugin::conversion::BearValueRef::new(value, label)
                .number()
                .map_err(|error| error.to_string())?
                .byte()
                .map_err(|error| error.to_string())?,
        ))),
        Value::String(value) => Rgb::parse_hex(value)
            .map(Color::Rgb)
            .map(Some)
            .map_err(|error| format!("{label} is invalid: {error}")),
        _ => Err(format!("{label} must be an ANSI number or #RRGGBB string")),
    }
}

fn optional_bool(value: Option<&Value>, label: &str) -> Result<Option<bool>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Value::Bool(value) = value else {
        return Err(format!("{label} must be a bool"));
    };
    Ok(Some(*value))
}

fn style_to_value(style: StyleOverlay) -> Value {
    let mut map = HashMap::new();
    insert_optional_color(&mut map, "fg", style.fg);
    insert_optional_color(&mut map, "bg", style.bg);
    insert_optional_color(&mut map, "underline_color", style.underline_color);
    insert_optional_bool(&mut map, "bold", style.bold);
    insert_optional_bool(&mut map, "italic", style.italic);
    insert_optional_bool(&mut map, "underline", style.underline);
    insert_optional_bool(&mut map, "double_underline", style.double_underline);
    insert_optional_bool(&mut map, "dim", style.dim);
    insert_optional_bool(&mut map, "reverse", style.reverse);
    insert_optional_bool(&mut map, "blink", style.blink);
    insert_optional_bool(&mut map, "strikethrough", style.strikethrough);
    insert_optional_bool(&mut map, "overline", style.overline);
    Value::Map(map.into())
}

fn insert_optional_color(map: &mut HashMap<String, Value>, key: &str, color: Option<Color>) {
    let Some(color) = color else {
        return;
    };
    let value = match color {
        Color::Ansi(value) => Value::Number(value as f64),
        Color::Rgb(value) => Value::String(
            format!("#{:02x}{:02x}{:02x}", value.r, value.g, value.b)
                .into_boxed_str()
                .into(),
        ),
    };
    map.insert(key.to_string(), value);
}

fn insert_optional_bool(map: &mut HashMap<String, Value>, key: &str, value: Option<bool>) {
    if let Some(value) = value {
        map.insert(key.to_string(), Value::Bool(value));
    }
}

fn string_from_value(value: &Value, label: &str) -> Result<String, String> {
    let Value::String(value) = value else {
        return Err(format!("{label} must be a string"));
    };
    Ok(value.to_string())
}

fn reject_unknown_keys(
    map: &bearscript::CowMap,
    allowed: &[&str],
    label: &str,
) -> Result<(), String> {
    if let Some(key) = map.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown {label} field {key:?}"));
    }
    Ok(())
}

fn unavailable_marker_error(id: MarkerId) -> String {
    format!("ghost text marker {id} is unavailable")
}
