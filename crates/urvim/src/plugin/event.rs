use std::collections::HashMap;

use bearscript::Value;
use urvim_core::event::{BufferEventSnapshot, EditorEvent};

pub(in crate::plugin) fn bear_args(args: &[String]) -> Value {
    Value::List(
        args.iter()
            .map(|arg| Value::String(arg.clone().into_boxed_str().into()))
            .collect::<Vec<_>>()
            .into(),
    )
}

pub(in crate::plugin) fn event_constants() -> Value {
    let mut events = HashMap::new();
    for &event in urvim_plugin::PluginEventKind::ALL {
        events.insert(
            event.as_str().to_string(),
            Value::String(event.as_str().into()),
        );
    }
    Value::Module(events.into())
}

pub(in crate::plugin) fn event_payload(
    event: EditorEvent,
) -> Option<(urvim_plugin::PluginEventKind, Value)> {
    let mut payload = HashMap::new();
    match event {
        EditorEvent::EditorStarted => {
            payload.insert(
                "event".to_string(),
                Value::String(urvim_plugin::PluginEventKind::EditorStarted.as_str().into()),
            );
            Some((
                urvim_plugin::PluginEventKind::EditorStarted,
                Value::Map(payload.into()),
            ))
        }
        EditorEvent::BufferSaved { snapshot } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferSaved,
            snapshot,
        )),
        EditorEvent::BufferClosed { snapshot } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferClosed,
            snapshot,
        )),
        EditorEvent::BufferLoaded { snapshot } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferLoaded,
            snapshot,
        )),
        EditorEvent::BufferUnloaded { snapshot } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferUnloaded,
            snapshot,
        )),
        EditorEvent::BufferFiletypeChanged { snapshot } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferFiletypeChanged,
            snapshot,
        )),
        EditorEvent::DiagnosticsChanged { buffer_id, .. } => {
            let kind = urvim_plugin::PluginEventKind::DiagnosticsChanged;
            payload.insert("event".to_string(), Value::String(kind.as_str().into()));
            payload.insert(
                "buffer_id".to_string(),
                Value::Number(buffer_id.get() as f64),
            );
            Some((kind, Value::Map(payload.into())))
        }
        EditorEvent::CommandExecuted { command } => {
            payload.insert(
                "event".to_string(),
                Value::String(
                    urvim_plugin::PluginEventKind::CommandExecuted
                        .as_str()
                        .into(),
                ),
            );
            payload.insert(
                "command".to_string(),
                Value::String(command.into_boxed_str().into()),
            );
            Some((
                urvim_plugin::PluginEventKind::CommandExecuted,
                Value::Map(payload.into()),
            ))
        }
        EditorEvent::BufferOpened { snapshot } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferOpened,
            snapshot,
        )),
        EditorEvent::WindowCreated {
            window_id,
            buffer_id,
            tab_id,
        } => Some(window_tab_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::WindowCreated,
            window_id,
            tab_id,
            buffer_id,
        )),
        EditorEvent::WindowClosed {
            window_id,
            buffer_id,
            tab_id,
        } => Some(window_tab_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::WindowClosed,
            window_id,
            tab_id,
            buffer_id,
        )),
        EditorEvent::WindowFocused {
            previous_window_id,
            window_id,
            buffer_id,
            tab_id,
        } => {
            payload.insert(
                "previous_window_id".to_string(),
                optional_window_id_value(previous_window_id),
            );
            Some(window_tab_event_payload(
                &mut payload,
                urvim_plugin::PluginEventKind::WindowFocused,
                window_id,
                tab_id,
                buffer_id,
            ))
        }
        EditorEvent::TabOpened {
            window_id,
            tab_id,
            snapshot,
        } => Some(tab_snapshot_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::TabOpened,
            window_id,
            tab_id,
            snapshot,
        )),
        EditorEvent::TabClosed {
            window_id,
            tab_id,
            snapshot,
        } => Some(tab_snapshot_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::TabClosed,
            window_id,
            tab_id,
            snapshot,
        )),
        EditorEvent::TabActivated {
            previous_tab_id,
            window_id,
            tab_id,
            buffer_id,
        } => {
            payload.insert(
                "previous_tab_id".to_string(),
                optional_tab_id_value(previous_tab_id),
            );
            Some(window_tab_event_payload(
                &mut payload,
                urvim_plugin::PluginEventKind::TabActivated,
                window_id,
                tab_id,
                buffer_id,
            ))
        }
        EditorEvent::ActiveBufferChanged {
            previous_buffer_id,
            buffer_id,
            window_id,
            tab_id,
        } => {
            payload.insert(
                "previous_buffer_id".to_string(),
                previous_buffer_id
                    .map(|id| Value::Number(id.get() as f64))
                    .unwrap_or(Value::Null),
            );
            Some(window_tab_event_payload(
                &mut payload,
                urvim_plugin::PluginEventKind::ActiveBufferChanged,
                window_id,
                tab_id,
                buffer_id,
            ))
        }
    }
}

fn window_tab_event_payload(
    payload: &mut HashMap<String, Value>,
    kind: urvim_plugin::PluginEventKind,
    window_id: urvim_core::layout::PaneId,
    tab_id: urvim_core::window::TabId,
    buffer_id: urvim_core::buffer::BufferId,
) -> (urvim_plugin::PluginEventKind, Value) {
    payload.insert("event".to_string(), Value::String(kind.as_str().into()));
    payload.insert("window_id".to_string(), Value::Number(window_id.0 as f64));
    payload.insert("tab_id".to_string(), Value::Number(tab_id.get() as f64));
    payload.insert(
        "buffer_id".to_string(),
        Value::Number(buffer_id.get() as f64),
    );
    (kind, Value::Map(std::mem::take(payload).into()))
}

fn tab_snapshot_event_payload(
    payload: &mut HashMap<String, Value>,
    kind: urvim_plugin::PluginEventKind,
    window_id: urvim_core::layout::PaneId,
    tab_id: urvim_core::window::TabId,
    snapshot: BufferEventSnapshot,
) -> (urvim_plugin::PluginEventKind, Value) {
    payload.insert("window_id".to_string(), Value::Number(window_id.0 as f64));
    payload.insert("tab_id".to_string(), Value::Number(tab_id.get() as f64));
    buffer_event_payload(payload, kind, snapshot)
}

fn optional_window_id_value(window_id: Option<urvim_core::layout::PaneId>) -> Value {
    window_id
        .map(|id| Value::Number(id.0 as f64))
        .unwrap_or(Value::Null)
}

fn optional_tab_id_value(tab_id: Option<urvim_core::window::TabId>) -> Value {
    tab_id
        .map(|id| Value::Number(id.get() as f64))
        .unwrap_or(Value::Null)
}

fn buffer_event_payload(
    payload: &mut HashMap<String, Value>,
    kind: urvim_plugin::PluginEventKind,
    snapshot: BufferEventSnapshot,
) -> (urvim_plugin::PluginEventKind, Value) {
    payload.insert("event".to_string(), Value::String(kind.as_str().into()));
    payload.insert(
        "buffer_id".to_string(),
        Value::Number(snapshot.buffer_id.get() as f64),
    );
    payload.insert(
        "path".to_string(),
        snapshot
            .path
            .map(|path| Value::String(path.to_string_lossy().into_owned().into()))
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "file_name".to_string(),
        snapshot
            .file_name
            .map(|name| Value::String(name.into_boxed_str().into()))
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "filetype".to_string(),
        Value::String(snapshot.filetype.into_boxed_str().into()),
    );
    payload.insert("modified".to_string(), Value::Bool(snapshot.modified));
    (kind, Value::Map(std::mem::take(payload).into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use urvim_core::buffer::BufferId;

    #[test]
    fn event_constants_include_the_complete_catalog() {
        let Value::Module(constants) = event_constants() else {
            panic!("event constants should be a module");
        };

        assert_eq!(constants.len(), urvim_plugin::PluginEventKind::ALL.len());
        for event in urvim_plugin::PluginEventKind::ALL {
            assert_eq!(
                constants.get(event.as_str()),
                Some(&Value::String(event.as_str().into()))
            );
        }
    }

    #[test]
    fn unloaded_buffer_payload_uses_its_snapshot() {
        let snapshot = BufferEventSnapshot {
            buffer_id: BufferId::new(7),
            path: Some(PathBuf::from("/tmp/removed.rs")),
            file_name: Some("removed.rs".to_string()),
            filetype: "rust".to_string(),
            modified: true,
        };

        let (_, Value::Map(payload)) = event_payload(EditorEvent::BufferUnloaded { snapshot })
            .expect("unload event should have a payload")
        else {
            panic!("unload payload should be a map");
        };

        assert_eq!(payload.get("buffer_id"), Some(&Value::Number(7.0)));
        assert_eq!(
            payload.get("path"),
            Some(&Value::String("/tmp/removed.rs".into()))
        );
        assert_eq!(
            payload.get("file_name"),
            Some(&Value::String("removed.rs".into()))
        );
        assert_eq!(payload.get("filetype"), Some(&Value::String("rust".into())));
        assert_eq!(payload.get("modified"), Some(&Value::Bool(true)));
    }

    #[test]
    fn current_buffer_payload_preserves_null_metadata() {
        let snapshot = BufferEventSnapshot {
            buffer_id: BufferId::new(3),
            path: None,
            file_name: None,
            filetype: "plain text".to_string(),
            modified: false,
        };

        let (kind, Value::Map(payload)) = event_payload(EditorEvent::BufferSaved { snapshot })
            .expect("save event should have a payload")
        else {
            panic!("save payload should be a map");
        };

        assert_eq!(kind, urvim_plugin::PluginEventKind::BufferSaved);
        assert_eq!(
            payload.get("event"),
            Some(&Value::String("BufferSaved".into()))
        );
        assert_eq!(payload.get("path"), Some(&Value::Null));
        assert_eq!(payload.get("file_name"), Some(&Value::Null));
        assert_eq!(payload.get("modified"), Some(&Value::Bool(false)));
    }

    #[test]
    fn tab_payload_contains_window_tab_and_buffer_ids() {
        let window = urvim_core::window::Window::from_buffer_id(BufferId::new(11));
        let tab_id = window.tab_id();
        let snapshot = BufferEventSnapshot {
            buffer_id: BufferId::new(11),
            path: Some(PathBuf::from("/tmp/tab.rs")),
            file_name: Some("tab.rs".to_string()),
            filetype: "rust".to_string(),
            modified: true,
        };
        let (kind, Value::Map(payload)) = event_payload(EditorEvent::TabOpened {
            window_id: urvim_core::layout::PaneId(4),
            tab_id,
            snapshot,
        })
        .expect("tab event should have a payload") else {
            panic!("tab payload should be a map");
        };

        assert_eq!(kind, urvim_plugin::PluginEventKind::TabOpened);
        assert_eq!(payload.get("window_id"), Some(&Value::Number(4.0)));
        assert_eq!(
            payload.get("tab_id"),
            Some(&Value::Number(tab_id.get() as f64))
        );
        assert_eq!(payload.get("buffer_id"), Some(&Value::Number(11.0)));
        assert_eq!(
            payload.get("path"),
            Some(&Value::String("/tmp/tab.rs".into()))
        );
        assert_eq!(payload.get("modified"), Some(&Value::Bool(true)));
    }

    #[test]
    fn closed_tab_payload_uses_pre_removal_snapshot() {
        let window = urvim_core::window::Window::from_buffer_id(BufferId::new(12));
        let snapshot = BufferEventSnapshot {
            buffer_id: BufferId::new(12),
            path: Some(PathBuf::from("/tmp/closed.rs")),
            file_name: Some("closed.rs".to_string()),
            filetype: "rust".to_string(),
            modified: true,
        };
        let (kind, Value::Map(payload)) = event_payload(EditorEvent::TabClosed {
            window_id: urvim_core::layout::PaneId(4),
            tab_id: window.tab_id(),
            snapshot,
        })
        .expect("closed tab event should have a payload") else {
            panic!("closed tab payload should be a map");
        };

        assert_eq!(kind, urvim_plugin::PluginEventKind::TabClosed);
        assert_eq!(
            payload.get("path"),
            Some(&Value::String("/tmp/closed.rs".into()))
        );
        assert_eq!(
            payload.get("file_name"),
            Some(&Value::String("closed.rs".into()))
        );
        assert_eq!(payload.get("modified"), Some(&Value::Bool(true)));
    }

    #[test]
    fn active_buffer_payload_contains_previous_and_current_ids() {
        let window = urvim_core::window::Window::from_buffer_id(BufferId::new(3));
        let tab_id = window.tab_id();
        let (kind, Value::Map(payload)) = event_payload(EditorEvent::ActiveBufferChanged {
            previous_buffer_id: Some(BufferId::new(2)),
            buffer_id: BufferId::new(3),
            window_id: urvim_core::layout::PaneId(5),
            tab_id,
        })
        .expect("active buffer event should have a payload") else {
            panic!("active buffer payload should be a map");
        };

        assert_eq!(kind, urvim_plugin::PluginEventKind::ActiveBufferChanged);
        assert_eq!(payload.get("previous_buffer_id"), Some(&Value::Number(2.0)));
        assert_eq!(payload.get("buffer_id"), Some(&Value::Number(3.0)));
        assert_eq!(payload.get("window_id"), Some(&Value::Number(5.0)));
        assert_eq!(
            payload.get("tab_id"),
            Some(&Value::Number(tab_id.get() as f64))
        );
    }

    #[test]
    fn tab_activation_payload_contains_previous_tab_id() {
        let previous = urvim_core::window::Window::from_buffer_id(BufferId::new(1));
        let active = urvim_core::window::Window::from_buffer_id(BufferId::new(2));
        let (kind, Value::Map(payload)) = event_payload(EditorEvent::TabActivated {
            previous_tab_id: Some(previous.tab_id()),
            window_id: urvim_core::layout::PaneId(7),
            tab_id: active.tab_id(),
            buffer_id: BufferId::new(2),
        })
        .expect("tab activation should have a payload") else {
            panic!("tab activation payload should be a map");
        };

        assert_eq!(kind, urvim_plugin::PluginEventKind::TabActivated);
        assert_eq!(
            payload.get("previous_tab_id"),
            Some(&Value::Number(previous.tab_id().get() as f64))
        );
        assert_eq!(
            payload.get("tab_id"),
            Some(&Value::Number(active.tab_id().get() as f64))
        );
        assert_eq!(payload.get("window_id"), Some(&Value::Number(7.0)));
        assert_eq!(payload.get("buffer_id"), Some(&Value::Number(2.0)));
    }

    #[test]
    fn window_payload_contains_window_id() {
        let window = urvim_core::window::Window::from_buffer_id(BufferId::new(6));
        let tab_id = window.tab_id();
        let (kind, Value::Map(payload)) = event_payload(EditorEvent::WindowFocused {
            previous_window_id: None,
            window_id: urvim_core::layout::PaneId(9),
            buffer_id: BufferId::new(6),
            tab_id,
        })
        .expect("window event should have a payload") else {
            panic!("window payload should be a map");
        };

        assert_eq!(kind, urvim_plugin::PluginEventKind::WindowFocused);
        assert_eq!(payload.get("previous_window_id"), Some(&Value::Null));
        assert_eq!(payload.get("window_id"), Some(&Value::Number(9.0)));
        assert_eq!(payload.get("buffer_id"), Some(&Value::Number(6.0)));
        assert_eq!(
            payload.get("tab_id"),
            Some(&Value::Number(tab_id.get() as f64))
        );
    }
}
