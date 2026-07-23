//! Buffer loading and file-lifecycle host APIs.

use std::io;

use bearscript::Value;
use urvim_core::buffer::BufferId;
use urvim_core::globals;

use super::super::{buffer_id_from_value, native_fn, unknown_buffer_error};

pub(in crate::plugin) fn open_fn() -> Value {
    native_fn("buffers.open", |path: String| {
        globals::with_buffer_pool(|pool| pool.open_buffer(&path))
            .map(|buffer_id| buffer_id.get() as f64)
            .map_err(|error| format!("failed to open buffer path {path:?}: {error}"))
    })
}

pub(in crate::plugin) fn create_fn() -> Value {
    native_fn("buffers.create", || {
        Ok(globals::with_buffer_pool(|pool| pool.create_buffer()).get() as f64)
    })
}

pub(in crate::plugin) fn save_as_fn() -> Value {
    native_fn("buffers.save_as", |buffer_id: Value, path: String| {
        let buffer_id = buffer_id_from_value(&buffer_id)?;
        save_as(buffer_id, &path)
    })
}

pub(in crate::plugin) fn reload_fn() -> Value {
    native_fn("buffers.reload", |buffer_id: Value, opts: Option<Value>| {
        let buffer_id = buffer_id_from_value(&buffer_id)?;
        let force = reload_force_from_opts(opts.as_ref().unwrap_or(&Value::Null))?;
        reload(buffer_id, force)
    })
}

fn save_as(buffer_id: BufferId, path: &str) -> Result<(), String> {
    let result = globals::with_buffer_pool(|pool| pool.save_buffer_to_path(buffer_id, path));
    match result {
        Ok(()) => {
            globals::with_lsp_runtime_mut(|runtime| runtime.did_save_buffer(buffer_id));
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(unknown_buffer_error(buffer_id))
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(format!(
            "cannot save buffer_id {} as {path:?}: another loaded buffer owns that path",
            buffer_id.get()
        )),
        Err(error) => Err(format!(
            "failed to save buffer_id {} as {path:?}: {error}",
            buffer_id.get()
        )),
    }
}

fn reload(buffer_id: BufferId, force: bool) -> Result<(), String> {
    let result = globals::with_buffer_pool(|pool| pool.reload_buffer(buffer_id, force));
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(unknown_buffer_error(buffer_id))
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Err(format!(
            "cannot reload buffer_id {}: {error}",
            buffer_id.get()
        )),
        Err(error) => Err(format!(
            "failed to reload buffer_id {}: {error}",
            buffer_id.get()
        )),
    }
}

fn reload_force_from_opts(opts: &Value) -> Result<bool, String> {
    match opts {
        Value::Null => Ok(false),
        Value::Map(map) => {
            for key in map.keys() {
                if key != "force" {
                    return Err(format!("unknown buffers.reload option {key}"));
                }
            }
            match map.get("force") {
                None => Ok(false),
                Some(Value::Bool(force)) => Ok(*force),
                Some(_) => Err("buffers.reload force must be a boolean".to_string()),
            }
        }
        _ => Err("buffers.reload opts must be a map or null".to_string()),
    }
}
