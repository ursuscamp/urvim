use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bearscript::Value;

use super::native_fn;
use super::strings::string_list;

pub(in crate::plugin) fn project_module() -> Value {
    Value::Module(
        HashMap::from([
            (
                "find_up".to_string(),
                native_fn(
                    "project.find_up",
                    |markers: Value, start: Option<String>| {
                        let markers = markers_from_value(markers)?;
                        let start = start_dir(start)?;
                        Ok(find_up(&markers, &start)
                            .map(|path| {
                                Value::String(
                                    path.to_string_lossy().to_string().into_boxed_str().into(),
                                )
                            })
                            .unwrap_or(Value::Null))
                    },
                ),
            ),
            (
                "root".to_string(),
                native_fn("project.root", |markers: Value, start: Option<String>| {
                    let markers = markers_from_value(markers)?;
                    let start = start_dir(start)?;
                    Ok(find_up(&markers, &start)
                        .and_then(|path| path.parent().map(Path::to_path_buf))
                        .map(|path| {
                            Value::String(
                                path.to_string_lossy().to_string().into_boxed_str().into(),
                            )
                        })
                        .unwrap_or(Value::Null))
                }),
            ),
        ])
        .into(),
    )
}

fn markers_from_value(value: Value) -> Result<Vec<String>, String> {
    match value {
        Value::String(marker) => Ok(vec![marker.to_string()]),
        other => string_list(other, "project markers"),
    }
}

fn start_dir(start: Option<String>) -> Result<PathBuf, String> {
    match start {
        Some(start) => Ok(PathBuf::from(start)),
        None => std::env::current_dir().map_err(|error| error.to_string()),
    }
}

fn find_up(markers: &[String], start: &Path) -> Option<PathBuf> {
    let mut dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        for marker in markers {
            let candidate = dir.join(marker);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}
