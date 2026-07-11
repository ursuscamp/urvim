use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;
use urvim_core::layout::{PaneId, SplitAxis, SplitSize};
use urvim_core::ui::plugin_pane::PluginPaneOptions;
use urvim_core::ui::plugin_window::{id_from_number, parse_key_sequence};

use super::super::super::{SharedLayout, native_fn};
use super::windows::{content_from_value, parse_window_command};

pub(in crate::plugin::host::ui) fn panes_module(
    plugin: String,
    contributions: Rc<RefCell<urvim_plugin::PluginContributionRegistry>>,
    layout: SharedLayout,
) -> Value {
    let create_plugin = plugin.clone();
    let create_layout = Rc::clone(&layout);
    let configure_plugin = plugin.clone();
    let configure_layout = Rc::clone(&layout);
    let content_plugin = plugin.clone();
    let content_layout = Rc::clone(&layout);
    let focus_plugin = plugin.clone();
    let focus_layout = Rc::clone(&layout);
    let close_plugin = plugin.clone();
    let close_layout = Rc::clone(&layout);
    let list_plugin = plugin.clone();
    let list_layout = Rc::clone(&layout);
    let active_plugin = plugin.clone();
    let active_layout = Rc::clone(&layout);
    let set_keymap_plugin = plugin.clone();
    let set_keymap_layout = Rc::clone(&layout);
    let set_keymap_contributions = Rc::clone(&contributions);
    let delete_keymap_plugin = plugin.clone();
    let delete_keymap_layout = Rc::clone(&layout);
    let list_keymap_plugin = plugin;
    let list_keymap_layout = layout;

    Value::Module(
        HashMap::from([
            (
                "create".to_string(),
                native_fn(
                    "ui.panes.create",
                    move |target: Option<f64>, opts: Value| {
                        let (axis, split_size, options) = pane_options_from_value(&opts)?;
                        let target = target.map(pane_id_from_number).transpose()?;
                        let id = create_layout.borrow_mut().create_plugin_pane(
                            create_plugin.clone(),
                            target,
                            axis,
                            split_size,
                            options,
                        )?;
                        Ok(id.0 as f64)
                    },
                ),
            ),
            (
                "configure".to_string(),
                native_fn("ui.panes.configure", move |pane_id: f64, opts: Value| {
                    let id = pane_id_from_number(pane_id)?;
                    let current = configure_layout
                        .borrow()
                        .plugin_pane_options(&configure_plugin, id)?;
                    let options = pane_display_options_from_value(&opts, Some(current))?;
                    configure_layout.borrow_mut().configure_plugin_pane(
                        &configure_plugin,
                        id,
                        options,
                    )
                }),
            ),
            (
                "set_content".to_string(),
                native_fn(
                    "ui.panes.set_content",
                    move |pane_id: f64, content: Value| {
                        let content = content_from_value(&content)?;
                        content_layout.borrow_mut().set_plugin_pane_content(
                            &content_plugin,
                            pane_id_from_number(pane_id)?,
                            content,
                        )
                    },
                ),
            ),
            (
                "focus".to_string(),
                native_fn("ui.panes.focus", move |pane_id: f64| {
                    let id = pane_id_from_number(pane_id)?;
                    focus_layout
                        .borrow_mut()
                        .focus_plugin_pane(&focus_plugin, id)
                }),
            ),
            (
                "close".to_string(),
                native_fn("ui.panes.close", move |pane_id: f64| {
                    let id = pane_id_from_number(pane_id)?;
                    close_layout
                        .borrow_mut()
                        .close_plugin_pane(&close_plugin, id)
                }),
            ),
            (
                "list".to_string(),
                native_fn("ui.panes.list", move || {
                    let layout = list_layout.borrow();
                    Ok(Value::List(
                        layout
                            .plugin_pane_ids(&list_plugin)
                            .into_iter()
                            .map(|id| Value::Number(id.0 as f64))
                            .collect::<Vec<_>>()
                            .into(),
                    ))
                }),
            ),
            (
                "active".to_string(),
                native_fn("ui.panes.active", move || {
                    let layout = active_layout.borrow();
                    Ok(layout
                        .focused_plugin_pane()
                        .filter(|id| layout.plugin_pane(&active_plugin, *id).is_ok())
                        .map(|id| Value::Number(id.0 as f64))
                        .unwrap_or(Value::Null))
                }),
            ),
            (
                "set_keymap".to_string(),
                native_fn(
                    "ui.panes.set_keymap",
                    move |pane_id: f64, lhs: String, rhs: String| {
                        let id = pane_id_from_number(pane_id)?;
                        let keys = parse_key_sequence(&lhs)?;
                        let intent = parse_window_command(
                            &set_keymap_plugin,
                            &rhs,
                            &set_keymap_contributions,
                        )?;
                        set_keymap_layout.borrow_mut().set_plugin_pane_keymap(
                            &set_keymap_plugin,
                            id,
                            keys,
                            rhs,
                            intent,
                        )
                    },
                ),
            ),
            (
                "delete_keymap".to_string(),
                native_fn(
                    "ui.panes.delete_keymap",
                    move |pane_id: f64, lhs: String| {
                        let keys = parse_key_sequence(&lhs)?;
                        delete_keymap_layout.borrow_mut().delete_plugin_pane_keymap(
                            &delete_keymap_plugin,
                            pane_id_from_number(pane_id)?,
                            &keys,
                        )
                    },
                ),
            ),
            (
                "list_keymaps".to_string(),
                native_fn("ui.panes.list_keymaps", move |pane_id: f64| {
                    let bindings = list_keymap_layout
                        .borrow()
                        .plugin_pane_keymaps(&list_keymap_plugin, pane_id_from_number(pane_id)?)?;
                    Ok(Value::List(
                        bindings
                            .into_iter()
                            .map(|(keys, rhs)| {
                                Value::Map(
                                    HashMap::from([
                                        ("lhs".to_string(), Value::String(keys.concat().into())),
                                        ("rhs".to_string(), Value::String(rhs.into())),
                                    ])
                                    .into(),
                                )
                            })
                            .collect::<Vec<_>>()
                            .into(),
                    ))
                }),
            ),
        ])
        .into(),
    )
}

fn pane_id_from_number(value: f64) -> Result<PaneId, String> {
    let id = id_from_number(value)?;
    Ok(PaneId(id.0))
}

fn pane_options_from_value(
    value: &Value,
) -> Result<(SplitAxis, SplitSize, PluginPaneOptions), String> {
    let Value::Map(map) = value else {
        return Err("plugin pane options must be a map".to_string());
    };
    let allowed = [
        "axis",
        "ratio",
        "title",
        "body_style",
        "header_style",
        "focused_header_style",
    ];
    if let Some(key) = map.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown plugin pane option {key}"));
    }

    let axis = match map.get("axis") {
        Some(Value::String(axis)) if axis.as_ref() == "horizontal" => SplitAxis::Horizontal,
        Some(Value::String(axis)) if axis.as_ref() == "vertical" => SplitAxis::Vertical,
        Some(_) => return Err("plugin pane axis must be horizontal or vertical".to_string()),
        None => return Err("plugin pane axis is required".to_string()),
    };
    let split_size = match map.get("ratio") {
        None | Some(Value::Null) => SplitSize::even(),
        Some(Value::Map(ratio)) => {
            let first = positive_u16(ratio.get("first"), "ratio.first")?;
            let second = positive_u16(ratio.get("second"), "ratio.second")?;
            SplitSize::new(first, second)
        }
        Some(_) => return Err("plugin pane ratio must be a map or null".to_string()),
    };

    let options = pane_display_options_from_value(value, None)?;
    Ok((axis, split_size, options))
}

fn pane_display_options_from_value(
    value: &Value,
    current: Option<PluginPaneOptions>,
) -> Result<PluginPaneOptions, String> {
    let Value::Map(map) = value else {
        return Err("plugin pane options must be a map".to_string());
    };
    let allowed = [
        "axis",
        "ratio",
        "title",
        "body_style",
        "header_style",
        "focused_header_style",
    ];
    if let Some(key) = map.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown plugin pane option {key}"));
    }

    let mut options = current.unwrap_or_default();
    if let Some(value) = map.get("title") {
        options.title = match value {
            Value::Null => None,
            Value::String(title) => Some(title.to_string()),
            _ => return Err("plugin pane title must be a string or null".to_string()),
        };
    }
    if let Some(value) = map.get("body_style") {
        options.body_style = parse_style(value, "body_style")?;
    }
    if let Some(value) = map.get("header_style") {
        options.header_style = parse_style(value, "header_style")?;
    }
    if let Some(value) = map.get("focused_header_style") {
        options.focused_header_style = parse_style(value, "focused_header_style")?;
    }
    Ok(options)
}

fn parse_style(value: &Value, label: &str) -> Result<urvim_theme::Tag, String> {
    let Value::String(style) = value else {
        return Err(format!("plugin pane {label} must be a string"));
    };
    urvim_theme::Tag::parse(style)
        .map_err(|error| format!("plugin pane {label} is invalid: {error}"))
}

fn positive_u16(value: Option<&Value>, label: &str) -> Result<u16, String> {
    let Some(Value::Number(number)) = value else {
        return Err(format!("{label} must be a positive integer"));
    };
    if !number.is_finite() || *number <= 0.0 || number.fract() != 0.0 || *number > u16::MAX as f64 {
        return Err(format!("{label} must be a positive integer"));
    }
    Ok(*number as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use urvim_theme::Tag;

    fn options(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
        Value::Map(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect::<HashMap<_, _>>()
                .into(),
        )
    }

    #[test]
    fn pane_options_parse_pane_specific_display_values() {
        let value = options([
            ("axis", Value::String("vertical".into())),
            ("title", Value::String("Demo".into())),
            ("body_style", Value::String("ui.window".into())),
            ("header_style", Value::String("ui.tab.inactive".into())),
            (
                "focused_header_style",
                Value::String("ui.tab.active".into()),
            ),
        ]);

        let (axis, split_size, parsed) = pane_options_from_value(&value).unwrap();

        assert_eq!(axis, SplitAxis::Vertical);
        assert_eq!(split_size, SplitSize::even());
        assert_eq!(parsed.title.as_deref(), Some("Demo"));
        assert_eq!(parsed.body_style.as_str(), "ui.window");
        assert_eq!(parsed.header_style.as_str(), "ui.tab.inactive");
        assert_eq!(parsed.focused_header_style.as_str(), "ui.tab.active");
    }

    #[test]
    fn pane_options_reject_border_style() {
        let value = options([
            ("axis", Value::String("vertical".into())),
            (
                "border_style",
                Value::String("ui.window.lines.border".into()),
            ),
        ]);

        assert_eq!(
            pane_options_from_value(&value).unwrap_err(),
            "unknown plugin pane option border_style"
        );
    }

    #[test]
    fn pane_configure_preserves_unspecified_values() {
        let current = PluginPaneOptions {
            title: Some("Old".to_string()),
            body_style: Tag::parse("ui.window").unwrap(),
            header_style: Tag::parse("ui.picker.location").unwrap(),
            focused_header_style: Tag::parse("ui.picker.accent").unwrap(),
        };
        let value = options([("title", Value::String("New".into()))]);

        let parsed = pane_display_options_from_value(&value, Some(current)).unwrap();

        assert_eq!(parsed.title.as_deref(), Some("New"));
        assert_eq!(parsed.body_style.as_str(), "ui.window");
        assert_eq!(parsed.header_style.as_str(), "ui.picker.location");
        assert_eq!(parsed.focused_header_style.as_str(), "ui.picker.accent");
    }
}
