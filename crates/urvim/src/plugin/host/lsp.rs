//! Plugin-facing asynchronous LSP APIs.

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use bearscript::Value;
use urvim_core::buffer::{BufferId, Cursor};
use urvim_core::globals;

use super::super::conversion::{BearValueRef, FromBearValue};
use super::super::lsp::{PluginLspRegistry, PluginLspRequestKind};
use super::super::{
    SharedLayout, buffer_id_from_value, cursor_from_value, native_fn, validate_callback,
};

const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const MAX_TIMEOUT_MS: u64 = 60_000;

pub(in crate::plugin) fn lsp_module(
    plugin: String,
    layout: SharedLayout,
    registry: Rc<PluginLspRegistry>,
) -> Value {
    let hover = request_fn(
        "lsp.hover",
        plugin.clone(),
        Rc::clone(&layout),
        Rc::clone(&registry),
        PluginLspRequestKind::Hover,
    );
    let definition = request_fn(
        "lsp.definition",
        plugin.clone(),
        Rc::clone(&layout),
        Rc::clone(&registry),
        PluginLspRequestKind::Definition,
    );
    let completion = request_fn(
        "lsp.completion",
        plugin.clone(),
        layout,
        Rc::clone(&registry),
        PluginLspRequestKind::Completion,
    );
    let cancel_registry = registry;

    Value::Module(
        HashMap::from([
            ("hover".to_string(), hover),
            ("definition".to_string(), definition),
            ("completion".to_string(), completion),
            (
                "cancel".to_string(),
                native_fn("lsp.cancel", move |id: f64| {
                    let id = super::super::BearNumber::new(id, "lsp.cancel.id")
                        .non_negative_u64()
                        .map_err(|error| error.to_string())?;
                    cancel_registry.cancel(&plugin, id)
                }),
            ),
        ])
        .into(),
    )
}

fn request_fn(
    name: &'static str,
    plugin: String,
    layout: SharedLayout,
    registry: Rc<PluginLspRegistry>,
    kind: PluginLspRequestKind,
) -> Value {
    native_fn(name, move |opts: Value, callback: Value| {
        validate_callback(&callback, "LSP callback")?;
        let request = request_options(&opts, &layout, name)?;
        registry
            .request(
                &plugin,
                kind,
                request.buffer_id,
                request.cursor,
                request.timeout,
                callback,
            )
            .map(|id| id as f64)
    })
}

struct LspRequestOptions {
    buffer_id: BufferId,
    cursor: Cursor,
    timeout: Duration,
}

fn request_options(
    opts: &Value,
    layout: &SharedLayout,
    name: &str,
) -> Result<LspRequestOptions, String> {
    let (active_buffer_id, active_cursor) = {
        let layout = layout.borrow();
        let view = layout.active_buffer_view();
        (view.buffer_id(), view.cursor())
    };

    let (buffer_id, cursor, timeout_ms) = if matches!(opts, Value::Null) {
        (active_buffer_id, active_cursor, DEFAULT_TIMEOUT_MS)
    } else {
        let map = BearValueRef::new(opts, name)
            .map()
            .map_err(|error| error.to_string())?;
        map.reject_unknown(&["buffer_id", "position", "timeout_ms"])
            .map_err(|error| error.to_string())?;
        let buffer_id = map
            .optional("buffer_id")
            .map_err(|error| error.to_string())?
            .filter(|value| !value.is_null())
            .map(|value| buffer_id_from_value(value.value()))
            .transpose()?
            .unwrap_or(active_buffer_id);
        let cursor = map
            .optional("position")
            .map_err(|error| error.to_string())?
            .filter(|value| !value.is_null())
            .map(|value| cursor_from_value(value.value(), "lsp.position"))
            .transpose()?
            .unwrap_or_else(|| {
                if buffer_id == active_buffer_id {
                    active_cursor
                } else {
                    Cursor::new(usize::MAX, usize::MAX)
                }
            });
        let timeout_ms = map
            .optional("timeout_ms")
            .map_err(|error| error.to_string())?
            .filter(|value| !value.is_null())
            .map(u64::from_bear)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        (buffer_id, cursor, timeout_ms)
    };

    if timeout_ms == 0 || timeout_ms > MAX_TIMEOUT_MS {
        return Err(format!(
            "{name}.timeout_ms must be an integer from 1 to {MAX_TIMEOUT_MS}"
        ));
    }
    let valid = globals::with_buffer(buffer_id, |buffer| buffer.is_valid_cursor(cursor))
        .ok_or_else(|| format!("unknown buffer_id {}", buffer_id.get()))?;
    if !valid {
        return Err(format!(
            "{name}.position must be a valid UTF-8 buffer position"
        ));
    }

    Ok(LspRequestOptions {
        buffer_id,
        cursor,
        timeout: Duration::from_millis(timeout_ms),
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use bearscript::Engine;
    use urvim_core::buffer::Buffer;
    use urvim_core::editor_pane::EditorPane;
    use urvim_core::layout::Layout;

    use super::*;

    fn test_layout() -> SharedLayout {
        Rc::new(RefCell::new(Layout::new(EditorPane::from_buffers(vec![
            Buffer::from_str("hello"),
        ]))))
    }

    #[test]
    fn cancel_returns_false_for_an_unknown_request() {
        let mut engine = Engine::new();
        engine.set_global(
            "lsp",
            lsp_module(
                "demo".to_string(),
                test_layout(),
                Rc::new(PluginLspRegistry::default()),
            ),
        );

        assert_eq!(engine.eval("lsp.cancel(99)").unwrap(), Value::Bool(false));
    }

    #[test]
    fn request_options_reject_excessive_timeout() {
        let opts =
            Value::Map(HashMap::from([("timeout_ms".to_string(), Value::Number(60_001.0))]).into());

        let error = request_options(&opts, &test_layout(), "lsp.hover")
            .err()
            .expect("timeout should be rejected");

        assert!(error.contains("timeout_ms"));
    }
}
