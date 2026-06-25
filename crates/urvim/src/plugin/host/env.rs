use std::collections::HashMap;

use bearscript::Value;

use super::native_fn;

pub(in crate::plugin) fn env_module() -> Value {
    Value::Module(
        HashMap::from([(
            "get".to_string(),
            native_fn("env.get", |name: String| {
                Ok(std::env::var(name)
                    .map(|value| Value::String(value.into_boxed_str().into()))
                    .unwrap_or(Value::Null))
            }),
        )])
        .into(),
    )
}
