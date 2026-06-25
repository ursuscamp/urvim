use std::collections::HashMap;

use bearscript::Value;
use urvim_core::globals;

use super::native_fn;

pub(in crate::plugin) fn registers_module() -> Value {
    Value::Module(
        HashMap::from([
            (
                "get".to_string(),
                native_fn("registers.get", |name: String| {
                    let name = register_name_from_string(&name)?;
                    Ok(globals::with_register_store(|store| {
                        store
                            .get(name)
                            .map(|content| content.text)
                            .unwrap_or_default()
                    }))
                }),
            ),
            (
                "set".to_string(),
                native_fn("registers.set", |name: String, value: String| {
                    let name = register_name_from_string(&name)?;
                    globals::with_register_store_mut(|store| {
                        store.set(
                            name,
                            urvim_core::register::RegisterContent::new(
                                value,
                                urvim_core::register::RegisterContentKind::Characterwise,
                            ),
                        );
                    });
                    Ok(())
                }),
            ),
            (
                "append".to_string(),
                native_fn("registers.append", |name: String, value: String| {
                    let name = register_name_from_string(&name)?;
                    globals::with_register_store_mut(|store| {
                        let mut text = store
                            .get(name)
                            .map(|content| content.text)
                            .unwrap_or_default();
                        text.push_str(&value);
                        store.set(
                            name,
                            urvim_core::register::RegisterContent::new(
                                text,
                                urvim_core::register::RegisterContentKind::Characterwise,
                            ),
                        );
                    });
                    Ok(())
                }),
            ),
            (
                "names".to_string(),
                native_fn("registers.names", || {
                    Ok(Value::List(globals::with_register_store(|store| {
                        store
                            .names()
                            .into_iter()
                            .map(|name| {
                                Value::String(name.as_char().to_string().into_boxed_str().into())
                            })
                            .collect::<Vec<_>>()
                            .into()
                    })))
                }),
            ),
        ])
        .into(),
    )
}

fn register_name_from_string(name: &str) -> Result<urvim_core::register::RegisterName, String> {
    let mut chars = name.chars();
    let Some(ch) = chars.next() else {
        return Err("register name must be one character".to_string());
    };
    if chars.next().is_some() {
        return Err("register name must be one character".to_string());
    }
    if ch == '"' || ch.is_ascii_lowercase() {
        Ok(urvim_core::register::RegisterName::new(ch))
    } else {
        Err(format!("invalid register name {name}"))
    }
}
