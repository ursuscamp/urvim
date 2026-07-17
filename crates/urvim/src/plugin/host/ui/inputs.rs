use std::cell::RefCell;
use std::rc::Rc;

use bearscript::Value;

use super::super::super::{SharedLayout, native_fn, validate_callback};
use crate::plugin::callbacks::{BearscriptPluginCallbacks, PluginInputCallbacks};
use crate::plugin::conversion::BearNumber;

pub(in crate::plugin::host::ui) fn input_fn(
    plugin: String,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
    layout: SharedLayout,
) -> Value {
    native_fn("ui.input", move |opts: Value| {
        let options = options_from_value(&opts)?;
        let (input_id, cancellation_sender) = {
            let mut callbacks = callbacks.borrow_mut();
            let cancellation_sender = callbacks
                .input_cancellation_sender
                .clone()
                .ok_or_else(|| "plugin input runtime is unavailable".to_string())?;
            let input_id = callbacks.next_input_id;
            callbacks.next_input_id = callbacks.next_input_id.saturating_add(1);
            callbacks.inputs.insert(
                input_id,
                PluginInputCallbacks {
                    on_submit: options.on_submit,
                    on_cancel: options.on_cancel,
                },
            );
            (input_id, cancellation_sender)
        };
        layout.borrow_mut().open_plugin_input(
            plugin.clone(),
            input_id,
            options.title,
            options.prompt,
            options.initial,
            cancellation_sender,
        );
        Ok(input_id as f64)
    })
}

pub(in crate::plugin::host::ui) fn close_input_fn(plugin: String, layout: SharedLayout) -> Value {
    native_fn("ui.close_input", move |input_id: f64| {
        layout.borrow_mut().close_plugin_input(
            &plugin,
            BearNumber::new(input_id, "plugin input id")
                .non_negative_u64()
                .map_err(|_| {
                    format!("plugin input id must be a non-negative integer, got {input_id}")
                })?,
        )
    })
}

struct InputOptions {
    title: String,
    prompt: String,
    initial: String,
    on_submit: Value,
    on_cancel: Option<Value>,
}

fn options_from_value(value: &Value) -> Result<InputOptions, String> {
    let Value::Map(map) = value else {
        return Err("input options must be a map".to_string());
    };
    for key in map.keys() {
        if !matches!(
            key.as_str(),
            "title" | "prompt" | "initial" | "on_submit" | "on_cancel"
        ) {
            return Err(format!("unknown input option {key}"));
        }
    }
    let title = optional_single_line_string(map.get("title"), "input title")?
        .unwrap_or_else(|| "Input".to_string());
    let prompt =
        optional_single_line_string(map.get("prompt"), "input prompt")?.unwrap_or_default();
    let initial =
        optional_single_line_string(map.get("initial"), "input initial text")?.unwrap_or_default();
    let on_submit = map
        .get("on_submit")
        .cloned()
        .ok_or_else(|| "input requires on_submit".to_string())?;
    validate_callback(&on_submit, "input on_submit")?;
    let on_cancel = match map.get("on_cancel") {
        None | Some(Value::Null) => None,
        Some(callback) => {
            validate_callback(callback, "input on_cancel")?;
            Some(callback.clone())
        }
    };
    Ok(InputOptions {
        title,
        prompt,
        initial,
        on_submit,
        on_cancel,
    })
}

fn optional_single_line_string(
    value: Option<&Value>,
    label: &str,
) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value))
            if !value
                .chars()
                .any(|character| matches!(character, '\r' | '\n')) =>
        {
            Ok(Some(value.to_string()))
        }
        Some(Value::String(_)) => Err(format!("{label} must not contain newlines")),
        Some(_) => Err(format!("{label} must be a string")),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn input_options_reject_unknown_fields() {
        let options =
            Value::Map(HashMap::from([("unknown".to_string(), Value::Bool(true))]).into());

        assert_eq!(
            options_from_value(&options).err().as_deref(),
            Some("unknown input option unknown")
        );
    }
}
