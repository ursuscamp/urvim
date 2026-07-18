use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;
use urvim_core::buffer::Cursor;
use urvim_core::globals;
use urvim_core::layout::{PaneId, PaneKind, SplitAxis, SplitSize};
use urvim_core::ui::overlay::{id_from_number, parse_key_sequence};
use urvim_core::ui::plugin_pane::PluginPaneOptions;

use super::super::{
    SharedLayout, buffer_id_from_value, cursor_to_value, ensure_valid_cursor, native_fn,
    row_range_to_value, unknown_buffer_error, usize_from_value,
};
use super::ui::overlays::{content_from_value, parse_overlay_command};
use crate::plugin::conversion::{BearValueRef, FromBearValue};

pub(in crate::plugin) fn panes_module(
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
    let focus_layout = Rc::clone(&layout);
    let close_layout = Rc::clone(&layout);
    let list_layout = Rc::clone(&layout);
    let active_layout = Rc::clone(&layout);
    let set_keymap_plugin = plugin.clone();
    let set_keymap_layout = Rc::clone(&layout);
    let set_keymap_contributions = Rc::clone(&contributions);
    let delete_keymap_plugin = plugin.clone();
    let delete_keymap_layout = Rc::clone(&layout);
    let list_keymap_plugin = plugin;
    let list_keymap_layout = Rc::clone(&layout);
    let buffer_layout = Rc::clone(&layout);
    let cursor_layout = Rc::clone(&layout);
    let set_cursor_layout = Rc::clone(&layout);
    let visible_range_layout = Rc::clone(&layout);
    let open_buffer_layout = layout;

    let mut module = HashMap::from([
        (
            "create".to_string(),
            native_fn("panes.create", move |target: Option<f64>, opts: Value| {
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
            }),
        ),
        (
            "configure".to_string(),
            native_fn("panes.configure", move |pane_id: f64, opts: Value| {
                let id = pane_id_from_number(pane_id)?;
                let current = configure_layout
                    .borrow()
                    .plugin_pane_options(&configure_plugin, id)?;
                let options = pane_display_options_from_value(&opts, Some(current))?;
                configure_layout
                    .borrow_mut()
                    .configure_plugin_pane(&configure_plugin, id, options)
            }),
        ),
        (
            "set_content".to_string(),
            native_fn("panes.set_content", move |pane_id: f64, content: Value| {
                let content = content_from_value(&content)?;
                content_layout.borrow_mut().set_plugin_pane_content(
                    &content_plugin,
                    pane_id_from_number(pane_id)?,
                    content,
                )
            }),
        ),
        (
            "focus".to_string(),
            native_fn("panes.focus", move |pane_id: f64| {
                let id = pane_id_from_number(pane_id)?;
                if focus_layout.borrow_mut().focus_layout_pane(id) {
                    Ok(())
                } else {
                    Err(format!("unknown pane_id {}", id.0))
                }
            }),
        ),
        (
            "close".to_string(),
            native_fn("panes.close", move |pane_id: f64| {
                let id = pane_id_from_number(pane_id)?;
                close_layout.borrow_mut().close_pane(id)
            }),
        ),
        (
            "list".to_string(),
            native_fn("panes.list", move || {
                let layout = list_layout.borrow();
                Ok(Value::List(
                    layout
                        .pane_ids()
                        .into_iter()
                        .map(|id| pane_descriptor(id, layout.pane_kind(id).unwrap()))
                        .collect::<Vec<_>>()
                        .into(),
                ))
            }),
        ),
        (
            "active".to_string(),
            native_fn("panes.active", move || {
                let layout = active_layout.borrow();
                let id = layout.focused_pane_id();
                Ok(layout
                    .pane_kind(id)
                    .map(|kind| pane_descriptor(id, kind))
                    .unwrap_or(Value::Null))
            }),
        ),
        (
            "set_keymap".to_string(),
            native_fn(
                "panes.set_keymap",
                move |pane_id: f64, lhs: String, rhs: String| {
                    let id = pane_id_from_number(pane_id)?;
                    let keys = parse_key_sequence(&lhs)?;
                    let intent =
                        parse_overlay_command(&set_keymap_plugin, &rhs, &set_keymap_contributions)?;
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
            native_fn("panes.delete_keymap", move |pane_id: f64, lhs: String| {
                let keys = parse_key_sequence(&lhs)?;
                delete_keymap_layout.borrow_mut().delete_plugin_pane_keymap(
                    &delete_keymap_plugin,
                    pane_id_from_number(pane_id)?,
                    &keys,
                )
            }),
        ),
        (
            "list_keymaps".to_string(),
            native_fn("panes.list_keymaps", move |pane_id: f64| {
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
    ]);
    module.insert(
        "buffer".to_string(),
        native_fn("panes.buffer", move |pane_id: Value| {
            let pane_id = pane_id_from_value(&pane_id)?;
            let layout = buffer_layout.borrow();
            let view = layout
                .buffer_view_for_pane(pane_id)
                .ok_or_else(|| editor_pane_error(&layout, pane_id))?;
            Ok(view.buffer_id().get() as f64)
        }),
    );
    module.insert(
        "cursor".to_string(),
        native_fn("panes.cursor", move |pane_id: Value| {
            let pane_id = pane_id_from_value(&pane_id)?;
            let layout = cursor_layout.borrow();
            let view = layout
                .buffer_view_for_pane(pane_id)
                .ok_or_else(|| editor_pane_error(&layout, pane_id))?;
            Ok(cursor_to_value(view.cursor()))
        }),
    );
    module.insert(
        "set_cursor".to_string(),
        native_fn(
            "panes.set_cursor",
            move |pane_id: Value, row: Value, col: Value| {
                let pane_id = pane_id_from_value(&pane_id)?;
                let cursor = Cursor::new(
                    usize_from_value(&row, "row")?,
                    usize_from_value(&col, "col")?,
                );
                let mut layout = set_cursor_layout.borrow_mut();
                let error = editor_pane_error(&layout, pane_id);
                let view = layout.buffer_view_for_pane_mut(pane_id).ok_or(error)?;
                let buffer_id = view.buffer_id();
                globals::with_buffer(buffer_id, |buffer| {
                    ensure_valid_cursor(buffer_id, buffer, cursor, "cursor")
                })
                .ok_or_else(|| unknown_buffer_error(buffer_id))??;
                view.set_cursor(cursor);
                Ok(())
            },
        ),
    );
    module.insert(
        "visible_range".to_string(),
        native_fn("panes.visible_range", move |pane_id: Value| {
            let pane_id = pane_id_from_value(&pane_id)?;
            let layout = visible_range_layout.borrow();
            let view = layout
                .buffer_view_for_pane(pane_id)
                .ok_or_else(|| editor_pane_error(&layout, pane_id))?;
            let start_row = view.scroll_offset().row as usize;
            let buffer_id = view.buffer_id();
            let line_count = globals::with_buffer(buffer_id, |buffer| buffer.line_count())
                .ok_or_else(|| unknown_buffer_error(buffer_id))?;
            let height = layout
                .pane_region(pane_id)
                .map(|region| region.size.rows)
                .unwrap_or_else(|| layout.size().rows.saturating_sub(1))
                as usize;
            Ok(row_range_to_value(
                start_row,
                start_row.saturating_add(height).min(line_count),
            ))
        }),
    );
    module.insert(
        "open_buffer".to_string(),
        native_fn("panes.open_buffer", move |buffer_id: Value| {
            let buffer_id = buffer_id_from_value(&buffer_id)?;
            if globals::with_buffer(buffer_id, |_| ()).is_none() {
                return Err(unknown_buffer_error(buffer_id));
            }
            open_buffer_layout
                .borrow_mut()
                .activate_or_open_buffer(buffer_id);
            Ok(())
        }),
    );
    Value::Module(module.into())
}

fn pane_descriptor(id: PaneId, kind: PaneKind) -> Value {
    Value::Map(
        HashMap::from([
            ("id".to_string(), Value::Number(id.0 as f64)),
            ("kind".to_string(), Value::String(kind.as_str().into())),
        ])
        .into(),
    )
}

fn pane_id_from_value(value: &Value) -> Result<PaneId, String> {
    usize::from_bear(BearValueRef::new(value, "pane_id"))
        .map(PaneId)
        .map_err(|_| "pane_id must be a non-negative integer".to_string())
}

fn editor_pane_error(layout: &urvim_core::Layout, id: PaneId) -> String {
    match layout.pane_kind(id) {
        Some(PaneKind::Plugin) => format!("pane_id {} is not an editor pane", id.0),
        Some(PaneKind::Editor) => format!("editor pane_id {} has no active tab", id.0),
        None => format!("unknown pane_id {}", id.0),
    }
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
    let Some(value) = value else {
        return Err(format!("{label} must be a positive integer"));
    };
    BearValueRef::new(value, label)
        .number()
        .and_then(|number| number.positive_u16())
        .map_err(|_| format!("{label} must be a positive integer"))
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
