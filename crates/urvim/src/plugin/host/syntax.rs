use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;

use super::super::buffer_id_from_value;
use super::super::callbacks::BearscriptPluginCallbacks;

/// Creates the `urvim.syntax` BearScript module.
pub(in crate::plugin) fn syntax_module(
    plugin: String,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
) -> Value {
    let register_plugin = plugin.clone();
    let register_contributions = Rc::clone(&contributions);
    let register_callbacks = Rc::clone(&callbacks);
    let unregister_plugin = plugin.clone();
    let unregister_contributions = Rc::clone(&contributions);
    let unregister_callbacks = Rc::clone(&callbacks);
    let refresh_callbacks = Rc::clone(&callbacks);
    Value::Module(
        HashMap::from([
            (
                "register".to_string(),
                super::super::native_fn(
                    "syntax.register",
                    move |filetype: String, callback: Value, _opts: Option<Value>| {
                        super::super::validate_callback(&callback, "syntax provider callback")?;
                        urvim_plugin::validate_contribution_name(
                            &filetype,
                            "syntax provider filetype",
                        )?;
                        let provider_id = {
                            let mut callbacks = register_callbacks.borrow_mut();
                            let provider_id = callbacks.next_syntax_provider_id;
                            callbacks.next_syntax_provider_id += 1;
                            callbacks.syntax_providers.insert(provider_id, callback);
                            provider_id
                        };
                        register_contributions
                            .borrow_mut()
                            .register_syntax_provider(
                                register_plugin.clone(),
                                urvim_plugin::DynamicSyntaxProvider {
                                    id: provider_id,
                                    filetype,
                                },
                            )?;
                        Ok(provider_id as f64)
                    },
                ),
            ),
            (
                "unregister".to_string(),
                super::super::native_fn("syntax.unregister", move |provider_id: f64| {
                    let provider_id = super::super::provider_id_from_number(provider_id)?;
                    unregister_contributions
                        .borrow_mut()
                        .unregister_syntax_provider(&unregister_plugin, provider_id);
                    unregister_callbacks
                        .borrow_mut()
                        .syntax_providers
                        .remove(&provider_id);
                    Ok(())
                }),
            ),
            (
                "refresh".to_string(),
                super::super::native_fn("syntax.refresh", move |buffer_id: Option<Value>| {
                    let buffer_id = match buffer_id {
                        Some(value) => buffer_id_from_value(&value)?,
                        None => urvim_core::globals::with_active_buffer_id(|id| id).ok_or_else(
                            || "syntax.refresh requires an active buffer".to_string(),
                        )?,
                    };
                    super::super::ensure_buffer_exists(buffer_id)?;
                    refresh_callbacks
                        .borrow_mut()
                        .syntax_refresh_requests
                        .insert(buffer_id);
                    Ok(())
                }),
            ),
            (
                "tags".to_string(),
                super::super::native_fn("syntax.tags", move || {
                    Ok(Value::List(
                        [
                            "syntax.comment",
                            "syntax.constant",
                            "syntax.constant.integer",
                            "syntax.function",
                            "syntax.keyword",
                            "syntax.operator",
                            "syntax.punctuation",
                            "syntax.string",
                            "syntax.type",
                            "syntax.variable",
                        ]
                        .into_iter()
                        .map(|tag| Value::String(tag.into()))
                        .collect::<Vec<_>>()
                        .into(),
                    ))
                }),
            ),
        ])
        .into(),
    )
}
