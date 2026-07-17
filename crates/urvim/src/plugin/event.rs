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
    }
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
}
