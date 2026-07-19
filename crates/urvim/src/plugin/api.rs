use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use bearscript::Value;

/// A deferred call from one plugin to an API endpoint exposed by another.
#[derive(Clone, Debug)]
pub(in crate::plugin) struct PluginApiRequest {
    pub(in crate::plugin) id: u64,
    pub(in crate::plugin) caller: String,
    pub(in crate::plugin) plugin: String,
    pub(in crate::plugin) api: String,
    pub(in crate::plugin) value: Value,
}

/// Main-thread queue for deferred cross-plugin API calls.
#[derive(Default)]
pub(in crate::plugin) struct PluginApiQueue {
    next_id: Cell<u64>,
    requests: RefCell<VecDeque<PluginApiRequest>>,
}

impl PluginApiQueue {
    pub(in crate::plugin) fn enqueue(
        &self,
        caller: &str,
        plugin: String,
        api: String,
        value: Value,
    ) -> Result<u64, String> {
        validate_api_value(&value, "plugin API request")?;
        let id = self.next_id.get().max(1);
        self.next_id.set(id.saturating_add(1));
        self.requests.borrow_mut().push_back(PluginApiRequest {
            id,
            caller: caller.to_string(),
            plugin,
            api,
            value,
        });
        Ok(id)
    }

    pub(in crate::plugin) fn take_batch(&self) -> Vec<PluginApiRequest> {
        self.requests.borrow_mut().drain(..).collect()
    }

    pub(in crate::plugin) fn remove_caller(&self, plugin: &str) {
        self.requests
            .borrow_mut()
            .retain(|request| request.caller != plugin);
    }
}

pub(in crate::plugin) fn validate_api_value(value: &Value, label: &str) -> Result<(), String> {
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
        Value::Number(number) if number.is_finite() => Ok(()),
        Value::Number(_) => Err(format!("{label} numbers must be finite")),
        Value::List(values) => {
            for value in values.iter() {
                validate_api_value(value, label)?;
            }
            Ok(())
        }
        Value::Map(values) => {
            for value in values.values() {
                validate_api_value(value, label)?;
            }
            Ok(())
        }
        Value::Range(_, _)
        | Value::ScriptFn(_)
        | Value::NativeFn(_)
        | Value::Generator(_)
        | Value::GeneratorNext(_)
        | Value::Module(_) => Err(format!(
            "{label} must contain only null, booleans, finite numbers, strings, lists, and maps"
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn queue_defers_requests_in_order() {
        let queue = PluginApiQueue::default();
        let first = queue
            .enqueue(
                "caller",
                "provider".to_string(),
                "one".to_string(),
                Value::Null,
            )
            .expect("request should enqueue");
        let second = queue
            .enqueue(
                "caller",
                "provider".to_string(),
                "two".to_string(),
                Value::Null,
            )
            .expect("request should enqueue");

        let batch = queue.take_batch();
        assert_eq!((first, second), (1, 2));
        assert_eq!(
            batch
                .iter()
                .map(|request| request.api.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
        assert!(queue.take_batch().is_empty());
    }

    #[test]
    fn rejects_non_portable_nested_values() {
        let value = Value::Map(HashMap::from([("bad".to_string(), Value::Range(1.0, 2.0))]).into());

        assert!(validate_api_value(&value, "request").is_err());
    }
}
