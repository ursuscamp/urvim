use bearscript::Value;

use super::native_fn;

pub(in crate::plugin) fn inspect_fn() -> Value {
    native_fn("inspect", |value: Value| Ok(value.to_string()))
}
