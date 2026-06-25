use std::collections::HashMap;
use std::path::PathBuf;

use bearscript::Value;

use super::native_fn;
use super::strings::string_list;

pub(in crate::plugin) fn path_module() -> Value {
    Value::Module(
        HashMap::from([
            (
                "join".to_string(),
                native_fn("path.join", |parts: Value| {
                    let parts = string_list(parts, "path.join parts")?;
                    Ok(parts
                        .iter()
                        .collect::<PathBuf>()
                        .to_string_lossy()
                        .to_string())
                }),
            ),
            (
                "dirname".to_string(),
                native_fn("path.dirname", |path: String| {
                    Ok(std::path::Path::new(&path)
                        .parent()
                        .map(|parent| parent.to_string_lossy().to_string())
                        .unwrap_or_default())
                }),
            ),
            (
                "basename".to_string(),
                native_fn("path.basename", |path: String| {
                    Ok(std::path::Path::new(&path)
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_default())
                }),
            ),
            (
                "extension".to_string(),
                native_fn("path.extension", |path: String| {
                    Ok(std::path::Path::new(&path)
                        .extension()
                        .map(|ext| {
                            Value::String(ext.to_string_lossy().to_string().into_boxed_str().into())
                        })
                        .unwrap_or(Value::Null))
                }),
            ),
            (
                "stem".to_string(),
                native_fn("path.stem", |path: String| {
                    Ok(std::path::Path::new(&path)
                        .file_stem()
                        .map(|stem| stem.to_string_lossy().to_string())
                        .unwrap_or_default())
                }),
            ),
        ])
        .into(),
    )
}
