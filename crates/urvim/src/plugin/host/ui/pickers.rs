use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use bearscript::Value;
use urvim_core::ui::picker::plugin::{PluginPickerId, PluginPickerItem};

use super::super::super::{SharedLayout, native_fn, validate_callback};
use crate::plugin::callbacks::{BearscriptPluginCallbacks, PluginPickerCallbacks};
use crate::plugin::conversion::BearNumber;

pub(in crate::plugin::host::ui) fn pickers_module(
    plugin: String,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
    layout: SharedLayout,
) -> Value {
    let open_plugin = plugin.clone();
    let open_callbacks = Rc::clone(&callbacks);
    let open_layout = Rc::clone(&layout);
    let set_plugin = plugin.clone();
    let set_callbacks = Rc::clone(&callbacks);
    let set_layout = Rc::clone(&layout);
    let append_plugin = plugin.clone();
    let append_callbacks = Rc::clone(&callbacks);
    let append_layout = Rc::clone(&layout);
    let close_plugin = plugin;
    let close_layout = layout;

    Value::Module(
        HashMap::from([
            (
                "open".to_string(),
                native_fn("ui.pickers.open", move |opts: Value| {
                    let options = open_options_from_value(&opts)?;
                    let (picker_id, items, cancellation_sender) = {
                        let mut callbacks = open_callbacks.borrow_mut();
                        let cancellation_sender = callbacks
                            .picker_cancellation_sender
                            .clone()
                            .ok_or_else(|| "plugin picker runtime is unavailable".to_string())?;
                        let picker_id = callbacks.next_picker_id;
                        callbacks.next_picker_id = callbacks.next_picker_id.saturating_add(1);
                        let (items, values, keys) =
                            items_from_value(&options.items, picker_id, &mut callbacks)?;
                        callbacks.pickers.insert(
                            picker_id,
                            PluginPickerCallbacks {
                                on_select: options.on_select,
                                on_cancel: options.on_cancel,
                                values,
                                keys,
                            },
                        );
                        (picker_id, items, cancellation_sender)
                    };
                    open_layout.borrow_mut().open_plugin_picker(
                        open_plugin.clone(),
                        picker_id,
                        options.title,
                        items,
                        cancellation_sender,
                    );
                    Ok(picker_id as f64)
                }),
            ),
            (
                "set_items".to_string(),
                native_fn(
                    "ui.pickers.set_items",
                    move |picker_id: f64, values: Value| {
                        let picker_id = picker_id_from_number(picker_id)?;
                        let (items, item_values, keys) = {
                            let mut callbacks = set_callbacks.borrow_mut();
                            if !callbacks.pickers.contains_key(&picker_id) {
                                return Err(format!("plugin picker {picker_id} is not open"));
                            }
                            items_from_value(&values, picker_id, &mut callbacks)?
                        };
                        set_layout.borrow_mut().set_plugin_picker_items(
                            &set_plugin,
                            picker_id,
                            items,
                        )?;
                        let mut callbacks = set_callbacks.borrow_mut();
                        let picker = callbacks
                            .pickers
                            .get_mut(&picker_id)
                            .ok_or_else(|| format!("plugin picker {picker_id} is not open"))?;
                        picker.values = item_values;
                        picker.keys = keys;
                        Ok(())
                    },
                ),
            ),
            (
                "append_items".to_string(),
                native_fn(
                    "ui.pickers.append_items",
                    move |picker_id: f64, values: Value| {
                        let picker_id = picker_id_from_number(picker_id)?;
                        let (items, item_values, keys) = {
                            let mut callbacks = append_callbacks.borrow_mut();
                            let existing_keys = callbacks
                                .pickers
                                .get(&picker_id)
                                .ok_or_else(|| format!("plugin picker {picker_id} is not open"))?
                                .keys
                                .clone();
                            let (items, values, mut keys) =
                                items_from_value(&values, picker_id, &mut callbacks)?;
                            if let Some(duplicate) = keys.intersection(&existing_keys).next() {
                                return Err(format!(
                                    "duplicate plugin picker item key {duplicate:?}"
                                ));
                            }
                            keys.extend(existing_keys);
                            (items, values, keys)
                        };
                        append_layout.borrow_mut().append_plugin_picker_items(
                            &append_plugin,
                            picker_id,
                            items,
                        )?;
                        let mut callbacks = append_callbacks.borrow_mut();
                        let picker = callbacks
                            .pickers
                            .get_mut(&picker_id)
                            .ok_or_else(|| format!("plugin picker {picker_id} is not open"))?;
                        picker.values.extend(item_values);
                        picker.keys = keys;
                        Ok(())
                    },
                ),
            ),
            (
                "close".to_string(),
                native_fn("ui.pickers.close", move |picker_id: f64| {
                    close_layout
                        .borrow_mut()
                        .close_plugin_picker(&close_plugin, picker_id_from_number(picker_id)?)
                }),
            ),
        ])
        .into(),
    )
}

struct OpenOptions {
    title: String,
    items: Value,
    on_select: Value,
    on_cancel: Option<Value>,
}

fn open_options_from_value(value: &Value) -> Result<OpenOptions, String> {
    let Value::Map(map) = value else {
        return Err("plugin picker options must be a map".to_string());
    };
    for key in map.keys() {
        if !matches!(key.as_str(), "title" | "items" | "on_select" | "on_cancel") {
            return Err(format!("unknown plugin picker option {key}"));
        }
    }
    let title = match map.get("title") {
        Some(Value::String(title)) => title.to_string(),
        Some(_) => return Err("plugin picker title must be a string".to_string()),
        None => "Picker".to_string(),
    };
    let items = map
        .get("items")
        .cloned()
        .unwrap_or_else(|| Value::List(Vec::new().into()));
    let on_select = map
        .get("on_select")
        .cloned()
        .ok_or_else(|| "plugin picker requires on_select".to_string())?;
    validate_callback(&on_select, "plugin picker on_select")?;
    let on_cancel = match map.get("on_cancel") {
        Some(Value::Null) | None => None,
        Some(callback) => {
            validate_callback(callback, "plugin picker on_cancel")?;
            Some(callback.clone())
        }
    };
    Ok(OpenOptions {
        title,
        items,
        on_select,
        on_cancel,
    })
}

fn items_from_value(
    value: &Value,
    picker_id: PluginPickerId,
    callbacks: &mut BearscriptPluginCallbacks,
) -> Result<(Vec<PluginPickerItem>, HashMap<u64, Value>, BTreeSet<String>), String> {
    let Value::List(values) = value else {
        return Err("plugin picker items must be a list".to_string());
    };
    let mut items = Vec::with_capacity(values.len());
    let mut original_values = HashMap::with_capacity(values.len());
    let mut keys = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let Value::Map(map) = value else {
            return Err(format!("plugin picker item {index} must be a map"));
        };
        for key in map.keys() {
            if !matches!(key.as_str(), "key" | "label" | "detail" | "value") {
                return Err(format!("unknown plugin picker item option {key}"));
            }
        }
        let label = required_string(
            map.get("label"),
            &format!("plugin picker item {index} label"),
        )?;
        let detail = optional_string(
            map.get("detail"),
            &format!("plugin picker item {index} detail"),
        )?;
        let item_id = callbacks.next_picker_item_id;
        callbacks.next_picker_item_id = callbacks.next_picker_item_id.saturating_add(1);
        let key = match map.get("key") {
            Some(value) => {
                required_string(Some(value), &format!("plugin picker item {index} key"))?
            }
            None => format!("{picker_id}:{item_id}"),
        };
        if !keys.insert(key.clone()) {
            return Err(format!("duplicate plugin picker item key {key:?}"));
        }
        let original = map
            .get("value")
            .cloned()
            .ok_or_else(|| format!("plugin picker item {index} requires value"))?;
        original_values.insert(item_id, original);
        items.push(PluginPickerItem {
            id: item_id,
            key,
            label,
            detail,
        });
    }
    Ok((items, original_values, keys))
}

fn required_string(value: Option<&Value>, label: &str) -> Result<String, String> {
    match value {
        Some(Value::String(value)) if !value.contains('\n') => Ok(value.to_string()),
        Some(Value::String(_)) => Err(format!("{label} must not contain newlines")),
        _ => Err(format!("{label} must be a string")),
    }
}

fn optional_string(value: Option<&Value>, label: &str) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => required_string(Some(value), label).map(Some),
    }
}

fn picker_id_from_number(value: f64) -> Result<PluginPickerId, String> {
    BearNumber::new(value, "plugin picker id")
        .non_negative_u64()
        .map_err(|_| format!("plugin picker id must be a non-negative integer, got {value}"))
}
