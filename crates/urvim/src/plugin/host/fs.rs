use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use bearscript::Value;

use super::native_fn;
use crate::plugin::callbacks::BearscriptPluginCallbacks;
use crate::plugin::fs::PluginFsRegistry;
use crate::plugin::validate_callback;

pub(in crate::plugin) fn fs_module(
    plugin: String,
    fs: Rc<PluginFsRegistry>,
    callbacks: Rc<RefCell<BearscriptPluginCallbacks>>,
) -> Value {
    let read_plugin = plugin.clone();
    let read_fs = Rc::clone(&fs);
    let read_callbacks = Rc::clone(&callbacks);
    let write_plugin = plugin.clone();
    let write_fs = Rc::clone(&fs);
    let write_callbacks = Rc::clone(&callbacks);
    let read_dir_plugin = plugin;
    let read_dir_fs = Rc::clone(&fs);
    let read_dir_callbacks = Rc::clone(&callbacks);
    Value::Module(
        HashMap::from([
            (
                "exists".to_string(),
                native_fn("fs.exists", |path: String| {
                    Ok(std::path::Path::new(&path).exists())
                }),
            ),
            (
                "is_file".to_string(),
                native_fn("fs.is_file", |path: String| {
                    Ok(std::path::Path::new(&path).is_file())
                }),
            ),
            (
                "is_dir".to_string(),
                native_fn("fs.is_dir", |path: String| {
                    Ok(std::path::Path::new(&path).is_dir())
                }),
            ),
            (
                "read_file".to_string(),
                native_fn("fs.read_file", move |path: String, callback: Value| {
                    validate_callback(&callback, "filesystem callback")?;
                    let request = read_fs.read_file(&read_plugin, path);
                    read_callbacks.borrow_mut().fs.insert(request.id, callback);
                    Ok(request.id as f64)
                }),
            ),
            (
                "write_file".to_string(),
                native_fn(
                    "fs.write_file",
                    move |path: String, text: String, callback: Value| {
                        validate_callback(&callback, "filesystem callback")?;
                        let request = write_fs.write_file(&write_plugin, path, text);
                        write_callbacks.borrow_mut().fs.insert(request.id, callback);
                        Ok(request.id as f64)
                    },
                ),
            ),
            (
                "read_dir".to_string(),
                native_fn("fs.read_dir", move |path: String, callback: Value| {
                    validate_callback(&callback, "filesystem callback")?;
                    let request = read_dir_fs.read_dir(&read_dir_plugin, path);
                    read_dir_callbacks
                        .borrow_mut()
                        .fs
                        .insert(request.id, callback);
                    Ok(request.id as f64)
                }),
            ),
        ])
        .into(),
    )
}
