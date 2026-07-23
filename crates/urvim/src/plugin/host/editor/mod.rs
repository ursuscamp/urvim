//! Plugin-facing editor state inspection APIs.

use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;
use urvim_core::editor::ModeKind;

use super::super::{SharedLayout, native_fn};

pub(in crate::plugin) fn editor_module(layout: SharedLayout) -> Value {
    let mode_layout = Rc::clone(&layout);
    let pane_layout = Rc::clone(&layout);
    let tab_layout = layout;

    Value::Module(
        HashMap::from([
            (
                "mode".to_string(),
                native_fn("editor.mode", move || {
                    Ok(mode_name(mode_layout.borrow().active_tab_mode_kind()).to_string())
                }),
            ),
            (
                "cwd".to_string(),
                native_fn("editor.cwd", move || {
                    std::env::current_dir()
                        .map(|path| path.to_string_lossy().into_owned())
                        .map_err(|error| format!("failed to read editor cwd: {error}"))
                }),
            ),
            (
                "active_pane".to_string(),
                native_fn("editor.active_pane", move || {
                    Ok(pane_layout.borrow().last_editor_pane_id().0 as f64)
                }),
            ),
            (
                "active_tab".to_string(),
                native_fn("editor.active_tab", move || {
                    let layout = tab_layout.borrow();
                    Ok(layout
                        .active_tab_id_for_pane(layout.last_editor_pane_id())?
                        .map(|id| Value::Number(id.get() as f64))
                        .unwrap_or(Value::Null))
                }),
            ),
        ])
        .into(),
    )
}

fn mode_name(mode: ModeKind) -> &'static str {
    match mode {
        ModeKind::Normal => "normal",
        ModeKind::Insert => "insert",
        ModeKind::Replace => "replace",
        ModeKind::Visual => "visual",
        ModeKind::VisualLine => "visual_line",
        ModeKind::Resizing => "resizing",
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use bearscript::Engine;
    use urvim_core::buffer::Buffer;
    use urvim_core::editor_pane::EditorPane;
    use urvim_core::layout::Layout;

    use super::*;

    #[test]
    fn exposes_current_editor_state() {
        let layout = Rc::new(RefCell::new(Layout::new(EditorPane::from_buffers(vec![
            Buffer::new(),
        ]))));
        let pane_id = layout.borrow().last_editor_pane_id();
        let tab_id = layout
            .borrow()
            .active_tab_id_for_pane(pane_id)
            .unwrap()
            .unwrap();
        let mut engine = Engine::new();
        engine.set_global("editor", editor_module(layout));

        let value = engine
            .eval("[editor.mode(), editor.active_pane(), editor.active_tab(), editor.cwd()]")
            .unwrap();
        let Value::List(values) = value else {
            panic!("editor state should be a list");
        };
        assert_eq!(values[0], Value::String("normal".into()));
        assert_eq!(values[1], Value::Number(pane_id.0 as f64));
        assert_eq!(values[2], Value::Number(tab_id.get() as f64));
        assert!(matches!(&values[3], Value::String(path) if !path.is_empty()));
    }
}
