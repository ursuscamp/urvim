use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

const SESSION_DIR_NAME: &str = "urvim/sessions";
const DEFAULT_XDG_DATA_HOME_SUFFIX: &str = ".local/share";

pub(super) fn data_home() -> std::io::Result<PathBuf> {
    if let Some(value) = env::var_os("XDG_DATA_HOME")
        && !value.is_empty()
    {
        return Ok(PathBuf::from(value));
    }

    let home = env::var_os("HOME")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME is not set"))?;
    Ok(PathBuf::from(home).join(DEFAULT_XDG_DATA_HOME_SUFFIX))
}

pub(super) fn session_dir() -> std::io::Result<PathBuf> {
    Ok(data_home()?.join(SESSION_DIR_NAME))
}

pub(super) fn session_path_for_cwd(cwd: &Path) -> std::io::Result<PathBuf> {
    let label = session_label(cwd);
    let hash = short_hash_hex(cwd.as_os_str());
    Ok(session_dir()?.join(format!("{label}--{hash}.toml")))
}

pub(super) fn session_label(cwd: &Path) -> String {
    cwd.file_name()
        .and_then(OsStr::to_str)
        .map(sanitize_label)
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| "cwd".to_string())
}

fn sanitize_label(value: &str) -> String {
    let mut label = String::with_capacity(value.len());
    let mut last_dash = false;

    for ch in value.chars() {
        let mapped = if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            Some(ch.to_ascii_lowercase())
        } else if ch.is_whitespace() || matches!(ch, '/' | '\\') {
            Some('-')
        } else {
            None
        };

        let Some(ch) = mapped else { continue };
        if ch == '-' {
            if last_dash {
                continue;
            }
            last_dash = true;
        } else {
            last_dash = false;
        }
        label.push(ch);
    }

    label.trim_matches('-').to_string()
}

fn short_hash_hex(value: &OsStr) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let hash = format!("{hash:016x}");
    hash[..8].to_string()
}

pub(super) fn ensure_session_dir() -> std::io::Result<PathBuf> {
    let dir = session_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
