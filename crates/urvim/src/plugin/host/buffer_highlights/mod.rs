//! Plugin-owned buffer range-highlight API.

use std::collections::HashMap;

use bearscript::Value;
use urvim_core::buffer::{Gravity, Highlight, Marker, MarkerId, RangeAnchor, VirtualText};
use urvim_theme::StyleOverlay;

use super::buffer_virtual_text::{
    cursor_from_value, cursor_to_value, gravity_from_value, marker_id_from_value,
    optional_style_from_value, reject_unknown_keys, style_to_value,
};
use super::native_fn;
use crate::plugin::{buffer_id_from_value, ensure_valid_cursor, unknown_buffer_error};

/// Builds the `urvim.buffers.highlights` module for one plugin.
pub(in crate::plugin) fn highlights_module(plugin: String) -> Value {
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
                    "buffers.highlights.add",
                    move |buffer_id: Value, options: Value| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        let options = AddOptions::from_value(&options)?;
                        let id = urvim_core::globals::with_buffer_mut(buffer_id, |buffer| {
                            validate_range(buffer_id, buffer, options.range)?;
                            buffer
                                .insert_namespaced_highlight(
                                    &add_plugin,
                                    options.range,
                                    options.style,
                                )
                                .ok_or_else(|| "highlight range must be non-empty".to_string())
                        })
                        .ok_or_else(|| unknown_buffer_error(buffer_id))??;
                        Ok(id as f64)
                    },
                ),
            ),
            (
                "update".to_string(),
                native_fn(
                    "buffers.highlights.update",
                    move |buffer_id: Value, marker_id: Value, changes: Value| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        let marker_id = marker_id_from_value(&marker_id)?;
                        let changes = UpdateOptions::from_value(&changes)?;
                        urvim_core::globals::with_buffer_mut(buffer_id, |buffer| {
                            let marker = buffer
                                .namespaced_highlight(&update_plugin, marker_id)
                                .cloned()
                                .ok_or_else(|| unavailable_marker_error(marker_id))?;
                            let Some((current, payload)) = marker.as_range() else {
                                return Err(unavailable_marker_error(marker_id));
                            };
                            let (start, end) =
                                changes.range.unwrap_or((current.start, current.end));
                            let range = RangeAnchor {
                                start,
                                end,
                                start_gravity: changes
                                    .start_gravity
                                    .unwrap_or(current.start_gravity),
                                end_gravity: changes.end_gravity.unwrap_or(current.end_gravity),
                            };
                            validate_range(buffer_id, buffer, range)?;
                            let style = changes.style.unwrap_or(payload.style);
                            if !buffer.update_namespaced_highlight(
                                &update_plugin,
                                marker_id,
                                range,
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
                    "buffers.highlights.remove",
                    move |buffer_id: Value, marker_id: Value| {
                        let buffer_id = buffer_id_from_value(&buffer_id)?;
                        let marker_id = marker_id_from_value(&marker_id)?;
                        urvim_core::globals::with_buffer_mut(buffer_id, |buffer| {
                            buffer
                                .remove_namespaced_highlight(&remove_plugin, marker_id)
                                .is_some()
                        })
                        .map(Value::Bool)
                        .ok_or_else(|| unknown_buffer_error(buffer_id))
                    },
                ),
            ),
            (
                "clear".to_string(),
                native_fn("buffers.highlights.clear", move |buffer_id: Value| {
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    urvim_core::globals::with_buffer_mut(buffer_id, |buffer| {
                        buffer.clear_namespaced_highlights(&clear_plugin) as f64
                    })
                    .ok_or_else(|| unknown_buffer_error(buffer_id))
                }),
            ),
            (
                "list".to_string(),
                native_fn("buffers.highlights.list", move |buffer_id: Value| {
                    let buffer_id = buffer_id_from_value(&buffer_id)?;
                    urvim_core::globals::with_buffer(buffer_id, |buffer| {
                        Value::List(
                            buffer
                                .namespaced_highlights(&list_plugin)
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
    range: RangeAnchor,
    style: StyleOverlay,
}

impl AddOptions {
    fn from_value(value: &Value) -> Result<Self, String> {
        let Value::Map(map) = value else {
            return Err("highlight options must be a map".to_string());
        };
        reject_unknown_keys(
            map,
            &["range", "start_gravity", "end_gravity", "style"],
            "options",
        )?;
        let (start, end) = range_from_value(
            map.get("range")
                .ok_or_else(|| "highlight options require range".to_string())?,
            "options.range",
        )?;
        let start_gravity = map
            .get("start_gravity")
            .map(|value| gravity_from_value(value, "options.start_gravity"))
            .transpose()?
            .unwrap_or(Gravity::Right);
        let end_gravity = map
            .get("end_gravity")
            .map(|value| gravity_from_value(value, "options.end_gravity"))
            .transpose()?
            .unwrap_or(Gravity::Left);
        let style = optional_style_from_value(
            map.get("style")
                .ok_or_else(|| "highlight options require style".to_string())?,
            "options.style",
        )?
        .ok_or_else(|| "options.style must not be null".to_string())?;
        Ok(Self {
            range: RangeAnchor {
                start,
                end,
                start_gravity,
                end_gravity,
            },
            style,
        })
    }
}

#[derive(Default)]
struct UpdateOptions {
    range: Option<(urvim_core::buffer::Cursor, urvim_core::buffer::Cursor)>,
    start_gravity: Option<Gravity>,
    end_gravity: Option<Gravity>,
    style: Option<StyleOverlay>,
}

impl UpdateOptions {
    fn from_value(value: &Value) -> Result<Self, String> {
        let Value::Map(map) = value else {
            return Err("highlight changes must be a map".to_string());
        };
        reject_unknown_keys(
            map,
            &["range", "start_gravity", "end_gravity", "style"],
            "changes",
        )?;
        let range = map
            .get("range")
            .map(|value| range_from_value(value, "changes.range"))
            .transpose()?;
        let style = map
            .get("style")
            .map(|value| optional_style_from_value(value, "changes.style"))
            .transpose()?
            .flatten();
        if map.contains_key("style") && style.is_none() {
            return Err("changes.style must not be null".to_string());
        }
        Ok(Self {
            range,
            start_gravity: map
                .get("start_gravity")
                .map(|value| gravity_from_value(value, "changes.start_gravity"))
                .transpose()?,
            end_gravity: map
                .get("end_gravity")
                .map(|value| gravity_from_value(value, "changes.end_gravity"))
                .transpose()?,
            style,
        })
    }
}

fn range_from_value(
    value: &Value,
    label: &str,
) -> Result<(urvim_core::buffer::Cursor, urvim_core::buffer::Cursor), String> {
    let Value::Map(map) = value else {
        return Err(format!("{label} must be a map"));
    };
    reject_unknown_keys(map, &["start", "end"], label)?;
    let start = cursor_from_value(
        map.get("start")
            .ok_or_else(|| format!("{label} requires start"))?,
        &format!("{label}.start"),
    )?;
    let end = cursor_from_value(
        map.get("end")
            .ok_or_else(|| format!("{label} requires end"))?,
        &format!("{label}.end"),
    )?;
    if start >= end {
        return Err(format!("{label} must be a non-empty forward range"));
    }
    Ok((start, end))
}

fn validate_range(
    buffer_id: urvim_core::buffer::BufferId,
    buffer: &urvim_core::buffer::Buffer,
    range: RangeAnchor,
) -> Result<(), String> {
    if range.start >= range.end {
        return Err("highlight range must be a non-empty forward range".to_string());
    }
    ensure_valid_cursor(buffer_id, buffer, range.start, "range.start")?;
    ensure_valid_cursor(buffer_id, buffer, range.end, "range.end")
}

fn marker_to_value(marker: &Marker<VirtualText, Highlight>) -> Value {
    let Some((range, payload)) = marker.as_range() else {
        unreachable!("highlight is range anchored");
    };
    Value::Map(
        HashMap::from([
            ("id".to_string(), Value::Number(marker.id() as f64)),
            (
                "range".to_string(),
                Value::Map(
                    HashMap::from([
                        ("start".to_string(), cursor_to_value(range.start)),
                        ("end".to_string(), cursor_to_value(range.end)),
                    ])
                    .into(),
                ),
            ),
            (
                "start_gravity".to_string(),
                gravity_to_value(range.start_gravity),
            ),
            (
                "end_gravity".to_string(),
                gravity_to_value(range.end_gravity),
            ),
            ("style".to_string(), style_to_value(payload.style)),
        ])
        .into(),
    )
}

fn gravity_to_value(gravity: Gravity) -> Value {
    Value::String(
        match gravity {
            Gravity::Left => "left",
            Gravity::Right => "right",
        }
        .into(),
    )
}

fn unavailable_marker_error(id: MarkerId) -> String {
    format!("highlight marker {id} is unavailable")
}
