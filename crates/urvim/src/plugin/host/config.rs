//! Immutable startup configuration for the calling BearScript plugin.

use std::collections::{BTreeMap, HashMap};

use bearscript::Value;

use super::json::json_value_to_bearscript;
use super::native_fn;

pub(in crate::plugin) fn config_module(config: BTreeMap<String, serde_json::Value>) -> Value {
    Value::Module(
        HashMap::from([(
            "get".to_string(),
            native_fn("config.get", move |path: Option<String>| {
                let value = match path {
                    None => serde_json::Value::Object(config.clone().into_iter().collect()),
                    Some(path) => value_at_path(&config, &path)
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                };
                Ok(json_value_to_bearscript(value))
            }),
        )])
        .into(),
    )
}

fn value_at_path<'a>(
    config: &'a BTreeMap<String, serde_json::Value>,
    path: &str,
) -> Option<&'a serde_json::Value> {
    let mut parts = path.split('.');
    let mut value = config.get(parts.next()?)?;
    for part in parts {
        value = value.as_object()?.get(part)?;
    }
    Some(value)
}
