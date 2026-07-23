//! Persistent state scoped to the calling BearScript plugin.

use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;

use super::json::json_value_to_bearscript;
use super::native_fn;
use crate::plugin::api::validate_cross_plugin_value;
use crate::plugin::state::PluginStateStore;

pub(in crate::plugin) fn state_module(plugin: String, state: Rc<PluginStateStore>) -> Value {
    let get_plugin = plugin.clone();
    let get_state = Rc::clone(&state);
    let set_plugin = plugin.clone();
    let set_state = Rc::clone(&state);
    let delete_plugin = plugin.clone();
    let delete_state = Rc::clone(&state);
    Value::Module(
        HashMap::from([
            (
                "get".to_string(),
                native_fn(
                    "state.get",
                    move |key: String, default: Option<Value>| match get_state
                        .get(&get_plugin, &key)?
                    {
                        Some(value) => Ok(json_value_to_bearscript(value)),
                        None => Ok(default.unwrap_or(Value::Null)),
                    },
                ),
            ),
            (
                "set".to_string(),
                native_fn("state.set", move |key: String, value: Value| {
                    validate_cross_plugin_value(&value, "plugin state")?;
                    let value = serde_json::to_value(value)
                        .map_err(|error| format!("failed to serialize plugin state: {error}"))?;
                    set_state.set(&set_plugin, key, value)
                }),
            ),
            (
                "delete".to_string(),
                native_fn("state.delete", move |key: String| {
                    delete_state.delete(&delete_plugin, &key)
                }),
            ),
            (
                "clear".to_string(),
                native_fn("state.clear", move || {
                    state.clear(&plugin).map(|removed| removed as f64)
                }),
            ),
        ])
        .into(),
    )
}
