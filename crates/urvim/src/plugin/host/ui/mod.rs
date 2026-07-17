use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;
use urvim_core::globals;

use super::super::callbacks::BearscriptPluginCallbacks;
use super::super::{SharedLayout, native_fn};

mod confirmations;
mod inputs;
mod line_format;
mod panes;
mod pickers;
mod windows;

pub(in crate::plugin) fn ui_module(
    plugin: String,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    layout: SharedLayout,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
) -> Value {
    let panes_plugin = plugin.clone();
    let panes_contributions = Rc::clone(&contributions);
    let panes_layout = Rc::clone(&layout);
    let confirm_plugin = plugin.clone();
    let confirm_callbacks = Rc::clone(&callbacks);
    let confirm_layout = Rc::clone(&layout);
    let close_confirmation_plugin = plugin.clone();
    let close_confirmation_layout = Rc::clone(&layout);
    let input_plugin = plugin.clone();
    let input_callbacks = Rc::clone(&callbacks);
    let input_layout = Rc::clone(&layout);
    let close_input_plugin = plugin.clone();
    let close_input_layout = Rc::clone(&layout);
    let windows_plugin = plugin.clone();
    let windows_contributions = Rc::clone(&contributions);
    let windows_layout = Rc::clone(&layout);
    Value::Module(
        HashMap::from([
            (
                "show_message".to_string(),
                native_fn("ui.show_message", |message: String, opts: Option<Value>| {
                    let opts = opts.unwrap_or(Value::Null);
                    let level = show_message_level_from_opts(&opts)?;
                    globals::enqueue_notification(level, message);
                    Ok(())
                }),
            ),
            (
                "confirm".to_string(),
                confirmations::confirm_fn(confirm_plugin, confirm_callbacks, confirm_layout),
            ),
            (
                "close_confirmation".to_string(),
                confirmations::close_confirmation_fn(
                    close_confirmation_plugin,
                    close_confirmation_layout,
                ),
            ),
            (
                "input".to_string(),
                inputs::input_fn(input_plugin, input_callbacks, input_layout),
            ),
            (
                "close_input".to_string(),
                inputs::close_input_fn(close_input_plugin, close_input_layout),
            ),
            ("line_format".to_string(), line_format::line_format_module()),
            (
                "windows".to_string(),
                windows::windows_module(windows_plugin, windows_contributions, windows_layout),
            ),
            (
                "panes".to_string(),
                panes::panes_module(panes_plugin, panes_contributions, panes_layout),
            ),
            (
                "pickers".to_string(),
                pickers::pickers_module(plugin, callbacks, layout),
            ),
        ])
        .into(),
    )
}

fn show_message_level_from_opts(
    opts: &Value,
) -> Result<urvim_core::notification::NotificationLevel, String> {
    match opts {
        Value::Null => Ok(urvim_core::notification::NotificationLevel::Info),
        Value::Map(map) => {
            for key in map.keys() {
                if key != "level" {
                    return Err(format!("unknown show_message option {key}"));
                }
            }
            let Some(level) = map.get("level") else {
                return Ok(urvim_core::notification::NotificationLevel::Info);
            };
            let Value::String(level) = level else {
                return Err("show_message level must be a string".to_string());
            };
            strict_notification_level_from_string(level)
        }
        _ => Err("show_message opts must be a map or null".to_string()),
    }
}

fn strict_notification_level_from_string(
    level: &str,
) -> Result<urvim_core::notification::NotificationLevel, String> {
    match level {
        "info" => Ok(urvim_core::notification::NotificationLevel::Info),
        "warn" | "warning" => Ok(urvim_core::notification::NotificationLevel::Warn),
        "error" => Ok(urvim_core::notification::NotificationLevel::Error),
        other => Err(format!("unknown notification level {other}")),
    }
}
