use std::collections::HashMap;

use bearscript::Value;

use super::native_fn;
use crate::plugin::conversion::{BearNumber, BearValueRef};

pub(in crate::plugin) fn strings_module() -> Value {
    Value::Module(
        HashMap::from([
            (
                "len".to_string(),
                native_fn(
                    "strings.len",
                    |text: String| Ok(text.chars().count() as f64),
                ),
            ),
            (
                "byte_len".to_string(),
                native_fn("strings.byte_len", |text: String| Ok(text.len() as f64)),
            ),
            (
                "char_at".to_string(),
                native_fn("strings.char_at", |text: String, index: f64| {
                    let index = byte_index_from_number(index, "strings.char_at index")?;
                    if index >= text.len() {
                        return Ok(Value::Null);
                    }
                    if !text.is_char_boundary(index) {
                        return Err(
                            "strings.char_at index must be a UTF-8 character boundary".to_string()
                        );
                    }
                    Ok(text[index..]
                        .chars()
                        .next()
                        .map(|ch| Value::String(ch.to_string().into_boxed_str().into()))
                        .unwrap_or(Value::Null))
                }),
            ),
            (
                "find".to_string(),
                native_fn(
                    "strings.find",
                    |text: String, needle: String, start: Option<f64>| {
                        let start = start
                            .map(|start| byte_index_from_number(start, "strings.find start"))
                            .transpose()?
                            .unwrap_or(0);
                        if start > text.len() {
                            return Err("strings.find start is out of bounds".to_string());
                        }
                        if !text.is_char_boundary(start) {
                            return Err(
                                "strings.find start must be a UTF-8 character boundary".to_string()
                            );
                        }
                        Ok(text[start..]
                            .find(&needle)
                            .map(|index| (start + index) as f64)
                            .unwrap_or(-1.0))
                    },
                ),
            ),
            (
                "trim".to_string(),
                native_fn("strings.trim", |text: String| Ok(text.trim().to_string())),
            ),
            (
                "trim_start".to_string(),
                native_fn("strings.trim_start", |text: String| {
                    Ok(text.trim_start().to_string())
                }),
            ),
            (
                "trim_end".to_string(),
                native_fn("strings.trim_end", |text: String| {
                    Ok(text.trim_end().to_string())
                }),
            ),
            (
                "starts_with".to_string(),
                native_fn("strings.starts_with", |text: String, prefix: String| {
                    Ok(text.starts_with(&prefix))
                }),
            ),
            (
                "ends_with".to_string(),
                native_fn("strings.ends_with", |text: String, suffix: String| {
                    Ok(text.ends_with(&suffix))
                }),
            ),
            (
                "contains".to_string(),
                native_fn("strings.contains", |text: String, needle: String| {
                    Ok(text.contains(&needle))
                }),
            ),
            (
                "split".to_string(),
                native_fn("strings.split", |text: String, separator: String| {
                    Ok(Value::List(
                        text.split(&separator)
                            .map(|part| Value::String(part.to_string().into_boxed_str().into()))
                            .collect::<Vec<_>>()
                            .into(),
                    ))
                }),
            ),
            (
                "join".to_string(),
                native_fn("strings.join", |parts: Value, separator: String| {
                    let parts = string_list(parts, "strings.join parts")?;
                    Ok(parts.join(&separator))
                }),
            ),
            (
                "replace".to_string(),
                native_fn(
                    "strings.replace",
                    |text: String, from: String, to: String| Ok(text.replace(&from, &to)),
                ),
            ),
            (
                "to_lower".to_string(),
                native_fn("strings.to_lower", |text: String| Ok(text.to_lowercase())),
            ),
            (
                "to_upper".to_string(),
                native_fn("strings.to_upper", |text: String| Ok(text.to_uppercase())),
            ),
        ])
        .into(),
    )
}

fn byte_index_from_number(value: f64, label: &str) -> Result<usize, String> {
    BearNumber::new(value, label)
        .non_negative_usize()
        .map_err(|error| error.to_string())
}

pub(in crate::plugin) fn string_list(value: Value, label: &str) -> Result<Vec<String>, String> {
    let Value::List(values) = value else {
        return Err(format!("{label} must be a list of strings"));
    };
    values
        .into_vec()
        .into_iter()
        .map(|value| match BearValueRef::new(&value, label).string() {
            Ok(text) => Ok(text),
            Err(_) => Err(format!("{label} must be a list of strings, got {value}")),
        })
        .collect()
}
