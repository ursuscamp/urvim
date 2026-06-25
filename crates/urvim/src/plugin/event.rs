use std::collections::HashMap;

use bearscript::Value;
use urvim_core::event::EditorEvent;

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
    for event in [
        urvim_plugin::PluginEventKind::EditorStarted,
        urvim_plugin::PluginEventKind::BufferOpened,
        urvim_plugin::PluginEventKind::BufferLoaded,
        urvim_plugin::PluginEventKind::BufferSaved,
        urvim_plugin::PluginEventKind::BufferClosed,
        urvim_plugin::PluginEventKind::BufferUnloaded,
        urvim_plugin::PluginEventKind::BufferFiletypeChanged,
        urvim_plugin::PluginEventKind::CommandExecuted,
        urvim_plugin::PluginEventKind::DiagnosticsChanged,
    ] {
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
        EditorEvent::BufferSaved { buffer_id } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferSaved,
            buffer_id,
        )),
        EditorEvent::BufferClosed { buffer_id } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferClosed,
            buffer_id,
        )),
        EditorEvent::BufferLoaded { buffer_id } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferLoaded,
            buffer_id,
        )),
        EditorEvent::BufferUnloaded { snapshot, .. } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferUnloaded,
            snapshot.buffer_id,
        )),
        EditorEvent::BufferFiletypeChanged { buffer_id } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferFiletypeChanged,
            buffer_id,
        )),
        EditorEvent::DiagnosticsChanged { buffer_id, .. } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::DiagnosticsChanged,
            buffer_id,
        )),
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
        EditorEvent::BufferOpened { buffer_id } => Some(buffer_event_payload(
            &mut payload,
            urvim_plugin::PluginEventKind::BufferOpened,
            buffer_id,
        )),
    }
}

fn buffer_event_payload(
    payload: &mut HashMap<String, Value>,
    kind: urvim_plugin::PluginEventKind,
    buffer_id: urvim_core::buffer::BufferId,
) -> (urvim_plugin::PluginEventKind, Value) {
    payload.insert("event".to_string(), Value::String(kind.as_str().into()));
    payload.insert(
        "buffer_id".to_string(),
        Value::Number(buffer_id.get() as f64),
    );
    (kind, Value::Map(std::mem::take(payload).into()))
}
