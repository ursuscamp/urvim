use std::cell::RefCell;
use std::rc::Rc;

use bearscript::Value;

use super::super::super::{SharedLayout, native_fn, validate_callback};
use crate::plugin::callbacks::{BearscriptPluginCallbacks, PluginConfirmationCallbacks};
use crate::plugin::conversion::BearNumber;

pub(in crate::plugin::host::ui) fn confirm_fn(
    plugin: String,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
    layout: SharedLayout,
) -> Value {
    native_fn("ui.confirm", move |opts: Value| {
        let options = options_from_value(&opts)?;
        if options
            .primary
            .key
            .eq_ignore_ascii_case(&options.secondary.key)
        {
            return Err("confirmation response keys must be different".to_string());
        }
        let (confirmation_id, cancellation_sender) = {
            let mut callbacks = callbacks.borrow_mut();
            let cancellation_sender = callbacks
                .confirmation_cancellation_sender
                .clone()
                .ok_or_else(|| "plugin confirmation runtime is unavailable".to_string())?;
            let confirmation_id = callbacks.next_confirmation_id;
            callbacks.next_confirmation_id = callbacks.next_confirmation_id.saturating_add(1);
            callbacks.confirmations.insert(
                confirmation_id,
                PluginConfirmationCallbacks {
                    on_response: options.on_response,
                    on_cancel: options.on_cancel,
                    primary_value: options.primary.value,
                    secondary_value: options.secondary.value,
                },
            );
            (confirmation_id, cancellation_sender)
        };
        layout.borrow_mut().open_plugin_confirmation(
            plugin.clone(),
            confirmation_id,
            options.title,
            options.message,
            options.primary.label,
            options.primary.key,
            options.secondary.label,
            options.secondary.key,
            cancellation_sender,
        );
        Ok(confirmation_id as f64)
    })
}

pub(in crate::plugin::host::ui) fn close_confirmation_fn(
    plugin: String,
    layout: SharedLayout,
) -> Value {
    native_fn("ui.close_confirmation", move |confirmation_id: f64| {
        layout.borrow_mut().close_plugin_confirmation(
            &plugin,
            BearNumber::new(confirmation_id, "plugin confirmation id")
                .non_negative_u64()
                .map_err(|_| {
                    format!(
                        "plugin confirmation id must be a non-negative integer, got {confirmation_id}"
                    )
                })?,
        )
    })
}

struct ConfirmationOptions {
    title: String,
    message: String,
    primary: ResponseOptions,
    secondary: ResponseOptions,
    on_response: Value,
    on_cancel: Option<Value>,
}

struct ResponseOptions {
    label: String,
    key: char,
    value: Value,
}

fn options_from_value(value: &Value) -> Result<ConfirmationOptions, String> {
    let Value::Map(map) = value else {
        return Err("confirmation options must be a map".to_string());
    };
    for key in map.keys() {
        if !matches!(
            key.as_str(),
            "title" | "message" | "confirm" | "reject" | "on_response" | "on_cancel"
        ) {
            return Err(format!("unknown confirmation option {key}"));
        }
    }
    let title = optional_string(map.get("title"), "confirmation title")?
        .unwrap_or_else(|| "Confirm".to_string());
    let message = required_string(map.get("message"), "confirmation message")?;
    let primary =
        response_from_value(map.get("confirm"), "confirm", "Yes", 'y', Value::Bool(true))?;
    let secondary =
        response_from_value(map.get("reject"), "reject", "No", 'n', Value::Bool(false))?;
    let on_response = map
        .get("on_response")
        .cloned()
        .ok_or_else(|| "confirmation requires on_response".to_string())?;
    validate_callback(&on_response, "confirmation on_response")?;
    let on_cancel = match map.get("on_cancel") {
        None | Some(Value::Null) => None,
        Some(callback) => {
            validate_callback(callback, "confirmation on_cancel")?;
            Some(callback.clone())
        }
    };
    Ok(ConfirmationOptions {
        title,
        message,
        primary,
        secondary,
        on_response,
        on_cancel,
    })
}

fn response_from_value(
    value: Option<&Value>,
    name: &str,
    default_label: &str,
    default_key: char,
    default_value: Value,
) -> Result<ResponseOptions, String> {
    let Some(value) = value else {
        return Ok(ResponseOptions {
            label: default_label.to_string(),
            key: default_key,
            value: default_value,
        });
    };
    let Value::Map(map) = value else {
        return Err(format!("confirmation {name} response must be a map"));
    };
    for key in map.keys() {
        if !matches!(key.as_str(), "label" | "key" | "value") {
            return Err(format!("unknown confirmation {name} response option {key}"));
        }
    }
    let label = optional_string(
        map.get("label"),
        &format!("confirmation {name} response label"),
    )?
    .unwrap_or_else(|| default_label.to_string());
    if label.is_empty() {
        return Err(format!(
            "confirmation {name} response label must not be empty"
        ));
    }
    let key = match map.get("key") {
        None => default_key,
        Some(Value::String(key)) => {
            let mut chars = key.chars();
            let key = chars
                .next()
                .ok_or_else(|| format!("confirmation {name} response key must be one character"))?;
            if chars.next().is_some() || key.is_control() {
                return Err(format!(
                    "confirmation {name} response key must be one character"
                ));
            }
            key
        }
        Some(_) => {
            return Err(format!("confirmation {name} response key must be a string"));
        }
    };
    Ok(ResponseOptions {
        label,
        key,
        value: map.get("value").cloned().unwrap_or(default_value),
    })
}

fn required_string(value: Option<&Value>, label: &str) -> Result<String, String> {
    optional_string(value, label)?.ok_or_else(|| format!("{label} must be a string"))
}

fn optional_string(value: Option<&Value>, label: &str) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.contains('\n') => Ok(Some(value.to_string())),
        Some(Value::String(_)) => Err(format!("{label} must not contain newlines")),
        Some(_) => Err(format!("{label} must be a string")),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn confirmation_options_reject_unknown_fields() {
        let options = Value::Map(
            HashMap::from([
                ("message".to_string(), Value::String("Continue?".into())),
                ("on_response".to_string(), Value::Null),
                ("timeout".to_string(), Value::Number(1.0)),
            ])
            .into(),
        );

        assert_eq!(
            options_from_value(&options).err().as_deref(),
            Some("unknown confirmation option timeout")
        );
    }

    #[test]
    fn response_options_preserve_custom_values() {
        let response = Value::Map(
            HashMap::from([
                ("label".to_string(), Value::String("Delete".into())),
                ("key".to_string(), Value::String("d".into())),
                ("value".to_string(), Value::String("deleted".into())),
            ])
            .into(),
        );

        let response =
            response_from_value(Some(&response), "confirm", "Yes", 'y', Value::Bool(true))
                .expect("custom response should parse");
        assert_eq!(response.label, "Delete");
        assert_eq!(response.key, 'd');
        assert_eq!(response.value, Value::String("deleted".into()));
    }
}
