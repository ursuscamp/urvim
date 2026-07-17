use std::collections::HashMap;

use bearscript::Value;
use urvim_core::event::{
    BufferErrorSnapshot, BufferEventSnapshot, ChangedRange, EditorEvent, EventPosition,
    EventSelection, EventSource, EventSourceKind,
};

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
        EditorEvent::BufferSaveFailed { error } => Some(buffer_error_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferSaveFailed,
            error,
        )),
        EditorEvent::BufferOpenFailed { error } => Some(buffer_error_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferOpenFailed,
            error,
        )),
        EditorEvent::BufferPathChanged { snapshot } => {
            payload.insert(
                "previous_path".to_string(),
                snapshot
                    .previous_path
                    .map(|path| Value::String(path.to_string_lossy().into_owned().into()))
                    .unwrap_or(Value::Null),
            );
            Some(buffer_event_payload(
                &mut payload,
                urvim_plugin::PluginEventKind::BufferPathChanged,
                snapshot.buffer,
            ))
        }
        EditorEvent::BufferReloaded { snapshot } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferReloaded,
            snapshot,
        )),
        EditorEvent::ExternalFileConflict { snapshot } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::ExternalFileConflict,
            snapshot,
        )),
        EditorEvent::BufferChanged {
            buffer_id,
            changed_range,
            source,
        } => {
            let kind = urvim_plugin::PluginEventKind::BufferChanged;
            payload.insert(
                "buffer_id".to_string(),
                Value::Number(buffer_id.get() as f64),
            );
            payload.insert(
                "changed_range".to_string(),
                changed_range_value(changed_range),
            );
            payload.insert("source".to_string(), source_value(source));
            Some(kind_payload(&mut payload, kind))
        }
        EditorEvent::BufferModifiedChanged {
            buffer_id,
            previous_modified,
            modified,
            source,
        } => {
            let kind = urvim_plugin::PluginEventKind::BufferModifiedChanged;
            payload.insert(
                "buffer_id".to_string(),
                Value::Number(buffer_id.get() as f64),
            );
            payload.insert(
                "previous_modified".to_string(),
                Value::Bool(previous_modified),
            );
            payload.insert("modified".to_string(), Value::Bool(modified));
            payload.insert("source".to_string(), source_value(source));
            Some(kind_payload(&mut payload, kind))
        }
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
        EditorEvent::CommandExecuted {
            command,
            success,
            error,
        } => {
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
            payload.insert("success".to_string(), Value::Bool(success));
            payload.insert(
                "error".to_string(),
                error
                    .map(|error| Value::String(error.into_boxed_str().into()))
                    .unwrap_or(Value::Null),
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
        EditorEvent::ModeChanged {
            window_id,
            buffer_id,
            previous_mode,
            mode,
            source,
        } => {
            let kind = urvim_plugin::PluginEventKind::ModeChanged;
            pane_event_fields(&mut payload, window_id, buffer_id, source);
            payload.insert(
                "previous_mode".to_string(),
                Value::String(previous_mode.into()),
            );
            payload.insert("mode".to_string(), Value::String(mode.into()));
            Some(kind_payload(&mut payload, kind))
        }
        EditorEvent::CursorMoved {
            window_id,
            buffer_id,
            previous_position,
            position,
            source,
        } => {
            let kind = urvim_plugin::PluginEventKind::CursorMoved;
            pane_event_fields(&mut payload, window_id, buffer_id, source);
            payload.insert(
                "previous_position".to_string(),
                position_value(previous_position),
            );
            payload.insert("position".to_string(), position_value(position));
            Some(kind_payload(&mut payload, kind))
        }
        EditorEvent::SelectionChanged {
            window_id,
            buffer_id,
            previous_selection,
            selection,
            source,
        } => {
            let kind = urvim_plugin::PluginEventKind::SelectionChanged;
            pane_event_fields(&mut payload, window_id, buffer_id, source);
            payload.insert(
                "previous_selection".to_string(),
                optional_selection_value(previous_selection),
            );
            payload.insert("selection".to_string(), optional_selection_value(selection));
            Some(kind_payload(&mut payload, kind))
        }
    }
}

fn kind_payload(
    payload: &mut HashMap<String, Value>,
    kind: urvim_plugin::PluginEventKind,
) -> (urvim_plugin::PluginEventKind, Value) {
    payload.insert("event".to_string(), Value::String(kind.as_str().into()));
    (kind, Value::Map(std::mem::take(payload).into()))
}

fn pane_event_fields(
    payload: &mut HashMap<String, Value>,
    window_id: urvim_core::layout::PaneId,
    buffer_id: urvim_core::buffer::BufferId,
    source: EventSource,
) {
    payload.insert("window_id".to_string(), Value::Number(window_id.0 as f64));
    payload.insert(
        "buffer_id".to_string(),
        Value::Number(buffer_id.get() as f64),
    );
    payload.insert("source".to_string(), source_value(source));
}

fn source_value(source: EventSource) -> Value {
    let mut value = HashMap::new();
    let kind = match source.kind {
        EventSourceKind::User => "user",
        EventSourceKind::Paste => "paste",
        EventSourceKind::Undo => "undo",
        EventSourceKind::Redo => "redo",
        EventSourceKind::Plugin => "plugin",
        EventSourceKind::Lsp => "lsp",
        EventSourceKind::Reload => "reload",
        EventSourceKind::Internal => "internal",
    };
    value.insert("kind".to_string(), Value::String(kind.into()));
    value.insert(
        "name".to_string(),
        source
            .name
            .map(|name| Value::String(name.into()))
            .unwrap_or(Value::Null),
    );
    Value::Map(value.into())
}

fn position_value(position: EventPosition) -> Value {
    let mut value = HashMap::new();
    value.insert("row".to_string(), Value::Number(position.row as f64));
    value.insert("col".to_string(), Value::Number(position.col as f64));
    Value::Map(value.into())
}

fn changed_range_value(range: ChangedRange) -> Value {
    let mut value = HashMap::new();
    value.insert("start".to_string(), position_value(range.start));
    value.insert("old_end".to_string(), position_value(range.old_end));
    value.insert("new_end".to_string(), position_value(range.new_end));
    Value::Map(value.into())
}

fn optional_selection_value(selection: Option<EventSelection>) -> Value {
    let Some(selection) = selection else {
        return Value::Null;
    };
    let mut value = HashMap::new();
    value.insert("anchor".to_string(), position_value(selection.anchor));
    value.insert("cursor".to_string(), position_value(selection.cursor));
    value.insert("linewise".to_string(), Value::Bool(selection.linewise));
    Value::Map(value.into())
}

fn buffer_error_payload(
    payload: &mut HashMap<String, Value>,
    kind: urvim_plugin::PluginEventKind,
    error: BufferErrorSnapshot,
) -> (urvim_plugin::PluginEventKind, Value) {
    payload.insert("event".to_string(), Value::String(kind.as_str().into()));
    payload.insert(
        "buffer_id".to_string(),
        error
            .buffer_id
            .map(|id| Value::Number(id.get() as f64))
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "path".to_string(),
        error
            .path
            .map(|path| Value::String(path.to_string_lossy().into_owned().into()))
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "error_kind".to_string(),
        Value::String(error.error_kind.into_boxed_str().into()),
    );
    payload.insert(
        "error".to_string(),
        Value::String(error.message.into_boxed_str().into()),
    );
    (kind, Value::Map(std::mem::take(payload).into()))
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
    use urvim_core::event::{BufferErrorSnapshot, BufferPathChangeSnapshot};

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
    fn buffer_changed_payload_uses_byte_range_and_named_source() {
        let (kind, Value::Map(payload)) = event_payload(EditorEvent::BufferChanged {
            buffer_id: BufferId::new(9),
            changed_range: ChangedRange {
                start: EventPosition { row: 1, col: 2 },
                old_end: EventPosition { row: 1, col: 4 },
                new_end: EventPosition { row: 2, col: 3 },
            },
            source: EventSource::plugin("demo"),
        })
        .expect("buffer change should have a payload") else {
            panic!("buffer change payload should be a map");
        };

        assert_eq!(kind, urvim_plugin::PluginEventKind::BufferChanged);
        assert_eq!(payload.get("buffer_id"), Some(&Value::Number(9.0)));
        let Some(Value::Map(source)) = payload.get("source") else {
            panic!("source should be a map");
        };
        assert_eq!(source.get("kind"), Some(&Value::String("plugin".into())));
        assert_eq!(source.get("name"), Some(&Value::String("demo".into())));
        let Some(Value::Map(range)) = payload.get("changed_range") else {
            panic!("changed range should be a map");
        };
        let Some(Value::Map(start)) = range.get("start") else {
            panic!("range start should be a map");
        };
        assert_eq!(start.get("row"), Some(&Value::Number(1.0)));
        assert_eq!(start.get("col"), Some(&Value::Number(2.0)));
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
    fn save_failure_payload_contains_stable_error_details() {
        let (kind, Value::Map(payload)) = event_payload(EditorEvent::BufferSaveFailed {
            error: BufferErrorSnapshot {
                buffer_id: Some(BufferId::new(9)),
                path: Some(PathBuf::from("/tmp/failed.rs")),
                error_kind: "permission_denied".to_string(),
                message: "access denied".to_string(),
            },
        })
        .expect("save failure should have a payload") else {
            panic!("save failure payload should be a map");
        };

        assert_eq!(kind, urvim_plugin::PluginEventKind::BufferSaveFailed);
        assert_eq!(payload.get("buffer_id"), Some(&Value::Number(9.0)));
        assert_eq!(
            payload.get("path"),
            Some(&Value::String("/tmp/failed.rs".into()))
        );
        assert_eq!(
            payload.get("error_kind"),
            Some(&Value::String("permission_denied".into()))
        );
        assert_eq!(
            payload.get("error"),
            Some(&Value::String("access denied".into()))
        );
    }

    #[test]
    fn path_changed_payload_contains_previous_and_current_paths() {
        let snapshot = BufferEventSnapshot {
            buffer_id: BufferId::new(5),
            path: Some(PathBuf::from("/tmp/new.rs")),
            file_name: Some("new.rs".to_string()),
            filetype: "rust".to_string(),
            modified: false,
        };
        let (kind, Value::Map(payload)) = event_payload(EditorEvent::BufferPathChanged {
            snapshot: BufferPathChangeSnapshot {
                buffer: snapshot,
                previous_path: Some(PathBuf::from("/tmp/old.rs")),
            },
        })
        .expect("path change should have a payload") else {
            panic!("path change payload should be a map");
        };

        assert_eq!(kind, urvim_plugin::PluginEventKind::BufferPathChanged);
        assert_eq!(
            payload.get("previous_path"),
            Some(&Value::String("/tmp/old.rs".into()))
        );
        assert_eq!(
            payload.get("path"),
            Some(&Value::String("/tmp/new.rs".into()))
        );
    }

    #[test]
    fn command_payload_uses_stable_name_and_result() {
        let (_, Value::Map(payload)) = event_payload(EditorEvent::CommandExecuted {
            command: "buffer.save".to_string(),
            success: false,
            error: Some("buffer has no path".to_string()),
        })
        .expect("command should have a payload") else {
            panic!("command payload should be a map");
        };

        assert_eq!(
            payload.get("command"),
            Some(&Value::String("buffer.save".into()))
        );
        assert_eq!(payload.get("success"), Some(&Value::Bool(false)));
        assert_eq!(
            payload.get("error"),
            Some(&Value::String("buffer has no path".into()))
        );
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
