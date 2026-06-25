use std::collections::HashMap;

use bearscript::Value;

use super::native_fn;

pub(in crate::plugin) fn json_module() -> Value {
    Value::Module(
        HashMap::from([
            (
                "parse".to_string(),
                native_fn("json.parse", |text: String| {
                    serde_json::from_str::<serde_json::Value>(&text)
                        .map(json_value_to_bearscript)
                        .map_err(|error| format!("invalid JSON: {error}"))
                }),
            ),
            (
                "stringify".to_string(),
                native_fn("json.stringify", |value: Value| {
                    serde_json::to_string(&value).map_err(|error| error.to_string())
                }),
            ),
            (
                "stringify_pretty".to_string(),
                native_fn("json.stringify_pretty", |value: Value| {
                    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
                }),
            ),
        ])
        .into(),
    )
}

fn json_value_to_bearscript(value: serde_json::Value) -> Value {
    match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Bool(value),
        serde_json::Value::Number(value) => Value::Number(value.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(value) => Value::String(value.into_boxed_str().into()),
        serde_json::Value::Array(values) => Value::List(
            values
                .into_iter()
                .map(json_value_to_bearscript)
                .collect::<Vec<_>>()
                .into(),
        ),
        serde_json::Value::Object(values) => Value::Map(
            values
                .into_iter()
                .map(|(key, value)| (key, json_value_to_bearscript(value)))
                .collect::<HashMap<_, _>>()
                .into(),
        ),
    }
}
