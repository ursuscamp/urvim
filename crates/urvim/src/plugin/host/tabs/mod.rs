//! Plugin-facing tab inspection and mutation APIs.

use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;
use urvim_core::editor_tab::TabId;
use urvim_core::layout::{PaneId, TabSnapshot};

use super::super::{SharedLayout, native_fn};

pub(in crate::plugin) fn tabs_module(layout: SharedLayout) -> Value {
    let list_layout = Rc::clone(&layout);
    let active_layout = Rc::clone(&layout);
    let buffer_layout = Rc::clone(&layout);
    let activate_layout = Rc::clone(&layout);
    let close_layout = Rc::clone(&layout);
    let move_layout = layout;

    Value::Module(
        HashMap::from([
            (
                "list".to_string(),
                native_fn("tabs.list", move |pane_id: Option<f64>| {
                    let pane_id = pane_id.map(pane_id_from_number).transpose()?;
                    let tabs = list_layout
                        .borrow()
                        .tab_snapshots(pane_id)?
                        .into_iter()
                        .map(tab_to_value)
                        .collect::<Vec<_>>();
                    Ok(Value::List(tabs.into()))
                }),
            ),
            (
                "active".to_string(),
                native_fn("tabs.active", move |pane_id: Option<f64>| {
                    let layout = active_layout.borrow();
                    let pane_id = pane_id
                        .map(pane_id_from_number)
                        .transpose()?
                        .unwrap_or_else(|| layout.last_editor_pane_id());
                    Ok(layout
                        .active_tab_id_for_pane(pane_id)?
                        .map(|id| Value::Number(id.get() as f64))
                        .unwrap_or(Value::Null))
                }),
            ),
            (
                "buffer".to_string(),
                native_fn("tabs.buffer", move |tab_id: f64| {
                    let tab_id = tab_id_from_number(tab_id)?;
                    buffer_layout
                        .borrow()
                        .tab_location(tab_id)
                        .map(|(_, buffer_id)| buffer_id.get() as f64)
                        .ok_or_else(|| unknown_tab_error(tab_id))
                }),
            ),
            (
                "activate".to_string(),
                native_fn("tabs.activate", move |tab_id: f64| {
                    activate_layout
                        .borrow_mut()
                        .activate_tab(tab_id_from_number(tab_id)?)
                }),
            ),
            (
                "close".to_string(),
                native_fn("tabs.close", move |tab_id: f64| {
                    close_layout
                        .borrow_mut()
                        .close_tab(tab_id_from_number(tab_id)?)
                }),
            ),
            (
                "move".to_string(),
                native_fn(
                    "tabs.move",
                    move |tab_id: f64, target_pane_id: Option<f64>| {
                        let tab_id = tab_id_from_number(tab_id)?;
                        let mut layout = move_layout.borrow_mut();
                        let target_pane_id = target_pane_id
                            .map(pane_id_from_number)
                            .transpose()?
                            .unwrap_or_else(|| layout.last_editor_pane_id());
                        layout.move_tab(tab_id, target_pane_id)
                    },
                ),
            ),
        ])
        .into(),
    )
}

fn tab_to_value(tab: TabSnapshot) -> Value {
    Value::Map(
        HashMap::from([
            ("id".to_string(), Value::Number(tab.id.get() as f64)),
            ("pane_id".to_string(), Value::Number(tab.pane_id.0 as f64)),
            (
                "buffer_id".to_string(),
                Value::Number(tab.buffer_id.get() as f64),
            ),
            ("index".to_string(), Value::Number(tab.index as f64)),
            ("active".to_string(), Value::Bool(tab.active)),
        ])
        .into(),
    )
}

fn pane_id_from_number(value: f64) -> Result<PaneId, String> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return Err("pane_id must be a non-negative integer".to_string());
    }
    Ok(PaneId(value as usize))
}

fn tab_id_from_number(value: f64) -> Result<TabId, String> {
    if !value.is_finite() || value < 1.0 || value.fract() != 0.0 || value > u64::MAX as f64 {
        return Err("tab_id must be a positive integer".to_string());
    }
    TabId::from_raw(value as u64).ok_or_else(|| "tab_id must be a positive integer".to_string())
}

fn unknown_tab_error(tab_id: TabId) -> String {
    format!("unknown tab_id {}", tab_id.get())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use bearscript::Engine;
    use urvim_core::buffer::Buffer;
    use urvim_core::editor_pane::EditorPane;
    use urvim_core::layout::Layout;
    use urvim_core::ui::{Command, Intent};

    use super::*;

    #[test]
    fn lists_activates_and_moves_tabs() {
        let mut layout = Layout::new(EditorPane::from_buffers(vec![
            Buffer::from_str("first"),
            Buffer::from_str("second"),
        ]));
        assert!(layout.dispatch_intent(&Intent::Command(Command::SplitVertical)));
        let target = layout.last_editor_pane_id();
        let source_tab = layout
            .tab_snapshots(None)
            .unwrap()
            .into_iter()
            .find(|tab| tab.pane_id != target && tab.index == 1)
            .unwrap();
        let layout = Rc::new(RefCell::new(layout));
        let mut engine = Engine::new();
        engine.set_global("tabs", tabs_module(Rc::clone(&layout)));

        let value = engine
            .eval(&format!(
                r#"
                tabs.activate({})
                tabs.move({}, {})
                [tabs.buffer({}), tabs.active({}), tabs.list({})]
                "#,
                source_tab.id.get(),
                source_tab.id.get(),
                target.0,
                source_tab.id.get(),
                target.0,
                target.0,
            ))
            .unwrap();
        let Value::List(values) = value else {
            panic!("tab API result should be a list");
        };
        assert_eq!(values[0], Value::Number(source_tab.buffer_id.get() as f64));
        assert_eq!(values[1], Value::Number(source_tab.id.get() as f64));
        let Value::List(target_tabs) = &values[2] else {
            panic!("tab listing should be a list");
        };
        assert!(target_tabs.iter().any(|value| {
            let Value::Map(tab) = value else {
                return false;
            };
            tab.get("id") == Some(&Value::Number(source_tab.id.get() as f64))
                && tab.get("active") == Some(&Value::Bool(true))
        }));
    }

    #[test]
    fn rejects_invalid_and_unknown_tab_ids() {
        let layout = Rc::new(RefCell::new(Layout::new(EditorPane::from_buffers(vec![
            Buffer::new(),
        ]))));
        let mut engine = Engine::new();
        engine.set_global("tabs", tabs_module(layout));

        let invalid = engine.eval("tabs.buffer(0)").unwrap_err().to_string();
        let unknown = engine
            .eval("tabs.buffer(999999999)")
            .unwrap_err()
            .to_string();

        assert!(invalid.contains("tab_id must be a positive integer"));
        assert!(unknown.contains("unknown tab_id 999999999"));
    }
}
