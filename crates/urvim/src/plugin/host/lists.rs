use std::collections::HashMap;

use bearscript::Value;

use super::native_fn;

/// Creates the `urvim.lists` BearScript module.
pub(in crate::plugin) fn lists_module() -> Value {
    Value::Module(
        HashMap::from([(
            "push".to_string(),
            native_fn("lists.push", |list: Value, value: Value| {
                let Value::List(mut items) = list else {
                    return Err("lists.push first argument must be a list".to_string());
                };
                items.push(value);
                Ok(Value::List(items))
            }),
        )])
        .into(),
    )
}
