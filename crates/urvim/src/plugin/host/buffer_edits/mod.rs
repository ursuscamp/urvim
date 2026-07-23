//! Structured buffer text-edit helpers for BearScript plugins.

use bearscript::Value;
use urvim_core::buffer::{Buffer, BufferId, Cursor};
use urvim_core::globals;

use super::super::native_fn;
use crate::plugin::{
    ScriptRange, buffer_id_from_value, cursor_from_value, ensure_valid_cursor, range_from_value,
    unknown_buffer_error,
};

#[derive(Clone, Debug)]
struct TextEdit {
    range: ScriptRange,
    text: String,
}

/// Builds `urvim.buffers.insert`.
pub(in crate::plugin) fn insert_fn() -> Value {
    native_fn(
        "buffers.insert",
        |buffer_id: Value, position: Value, text: String| {
            let buffer_id = buffer_id_from_value(&buffer_id)?;
            let position = cursor_from_value(&position, "position")?;
            apply_edits(
                buffer_id,
                vec![TextEdit {
                    range: ScriptRange {
                        start: position,
                        end: position,
                    },
                    text,
                }],
            )
        },
    )
}

/// Builds `urvim.buffers.delete`.
pub(in crate::plugin) fn delete_fn() -> Value {
    native_fn("buffers.delete", |buffer_id: Value, range: Value| {
        let buffer_id = buffer_id_from_value(&buffer_id)?;
        let range = range_from_value(&range)?;
        apply_edits(
            buffer_id,
            vec![TextEdit {
                range,
                text: String::new(),
            }],
        )
    })
}

/// Builds `urvim.buffers.text_in_range`.
pub(in crate::plugin) fn text_in_range_fn() -> Value {
    native_fn("buffers.text_in_range", |buffer_id: Value, range: Value| {
        let buffer_id = buffer_id_from_value(&buffer_id)?;
        let range = range_from_value(&range)?;
        globals::with_buffer(buffer_id, |buffer| {
            validate_range(buffer_id, buffer, range, "range")?;
            buffer
                .text_in_range(range.start, range.end)
                .ok_or_else(|| "range is out of range".to_string())
        })
        .ok_or_else(|| unknown_buffer_error(buffer_id))?
    })
}

/// Builds `urvim.buffers.apply_edits`.
pub(in crate::plugin) fn apply_edits_fn() -> Value {
    native_fn("buffers.apply_edits", |buffer_id: Value, edits: Value| {
        let buffer_id = buffer_id_from_value(&buffer_id)?;
        let edits = edits_from_value(&edits)?;
        apply_edits(buffer_id, edits)
    })
}

fn edits_from_value(value: &Value) -> Result<Vec<TextEdit>, String> {
    let Value::List(values) = value else {
        return Err("edits must be a list".to_string());
    };

    values
        .iter()
        .enumerate()
        .map(|(index, value)| edit_from_value(value, index))
        .collect()
}

fn edit_from_value(value: &Value, index: usize) -> Result<TextEdit, String> {
    let Value::Map(map) = value else {
        return Err(format!("edits[{index}] must be a map"));
    };
    let range = map
        .get("range")
        .ok_or_else(|| format!("edits[{index}] requires range"))?;
    let text = map
        .get("text")
        .ok_or_else(|| format!("edits[{index}] requires text"))?;
    let Value::String(text) = text else {
        return Err(format!("edits[{index}].text must be a string"));
    };

    Ok(TextEdit {
        range: range_from_value(range).map_err(|error| format!("edits[{index}].{error}"))?,
        text: text.to_string(),
    })
}

fn apply_edits(buffer_id: BufferId, edits: Vec<TextEdit>) -> Result<(), String> {
    globals::with_buffer(buffer_id, |buffer| {
        validate_edits(buffer_id, buffer, &edits)
    })
    .ok_or_else(|| unknown_buffer_error(buffer_id))??;

    if edits.is_empty() {
        return Ok(());
    }

    let edits = edits
        .into_iter()
        .map(|edit| (edit.range.start, edit.range.end, edit.text))
        .collect::<Vec<_>>();
    globals::with_buffer_mut(buffer_id, |buffer| buffer.apply_text_edits(&edits))
        .ok_or_else(|| unknown_buffer_error(buffer_id))?;
    Ok(())
}

fn validate_edits(buffer_id: BufferId, buffer: &Buffer, edits: &[TextEdit]) -> Result<(), String> {
    for (index, edit) in edits.iter().enumerate() {
        validate_range(
            buffer_id,
            buffer,
            edit.range,
            &format!("edits[{index}].range"),
        )?;
    }

    for left_index in 0..edits.len() {
        for right_index in left_index + 1..edits.len() {
            if edits_conflict(edits[left_index].range, edits[right_index].range) {
                return Err(format!(
                    "edits[{left_index}] overlaps or conflicts with edits[{right_index}]"
                ));
            }
        }
    }
    Ok(())
}

fn validate_range(
    buffer_id: BufferId,
    buffer: &Buffer,
    range: ScriptRange,
    label: &str,
) -> Result<(), String> {
    ensure_valid_cursor(buffer_id, buffer, range.start, &format!("{label}.start"))?;
    ensure_valid_cursor(buffer_id, buffer, range.end, &format!("{label}.end"))?;
    if range.start > range.end {
        return Err(format!("{label} start must be before or equal to end"));
    }
    Ok(())
}

fn edits_conflict(left: ScriptRange, right: ScriptRange) -> bool {
    let left_empty = left.start == left.end;
    let right_empty = right.start == right.end;

    match (left_empty, right_empty) {
        (true, true) => left.start == right.start,
        (true, false) => cursor_is_inside(left.start, right),
        (false, true) => cursor_is_inside(right.start, left),
        (false, false) => left.start < right.end && right.start < left.end,
    }
}

fn cursor_is_inside(cursor: Cursor, range: ScriptRange) -> bool {
    range.start < cursor && cursor < range.end
}
