//! Session persistence for workspace restore.

mod format;
mod path;

use crate::layout::Layout;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub use format::*;

const SESSION_VERSION: u32 = 1;
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(10);

struct SessionRuntime {
    enabled: bool,
    dirty: bool,
    dirty_since: Option<Instant>,
    last_save: Option<Instant>,
}

static SESSION_RUNTIME: OnceLock<Mutex<SessionRuntime>> = OnceLock::new();

fn runtime() -> &'static Mutex<SessionRuntime> {
    SESSION_RUNTIME.get_or_init(|| {
        Mutex::new(SessionRuntime {
            enabled: false,
            dirty: false,
            dirty_since: None,
            last_save: None,
        })
    })
}

/// Enables or disables autosave for the current editor run.
pub fn set_enabled(enabled: bool) {
    if let Ok(mut runtime) = runtime().lock() {
        runtime.enabled = enabled;
        runtime.dirty = false;
        runtime.dirty_since = None;
        runtime.last_save = None;
    }
}

#[cfg(test)]
pub fn set_runtime_state_for_test(
    enabled: bool,
    dirty: bool,
    dirty_since: Option<Instant>,
    last_save: Option<Instant>,
) {
    if let Ok(mut runtime) = runtime().lock() {
        runtime.enabled = enabled;
        runtime.dirty = dirty;
        runtime.dirty_since = dirty_since;
        runtime.last_save = last_save;
    }
}

/// Marks the active session dirty.
pub fn mark_dirty() {
    if let Ok(mut runtime) = runtime().lock() {
        if runtime.enabled {
            runtime.dirty = true;
            if runtime.dirty_since.is_none() {
                runtime.dirty_since = Some(Instant::now());
            }
        }
    }
}

/// Loads a session for the current working directory.
pub fn load_current_cwd() -> std::io::Result<Option<SessionFile>> {
    let cwd = env::current_dir()?;
    load_session_for_cwd(&cwd)
}

/// Loads a session for the given cwd.
pub fn load_session_for_cwd(cwd: &Path) -> std::io::Result<Option<SessionFile>> {
    let path = path::session_path_for_cwd(cwd)?;
    if !path.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(&path)?;
    let session = toml::from_str::<SessionFile>(&text)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(Some(session))
}

/// Saves a session for the given cwd.
pub fn save_session_for_cwd(cwd: &Path, session: &SessionFile) -> std::io::Result<()> {
    let path = path::session_path_for_cwd(cwd)?;
    let _ = path::ensure_session_dir()?;

    let text = toml::to_string_pretty(session)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    fs::write(path, text)
}

/// Saves the active layout when autosave is due.
pub fn maybe_autosave(layout: &Layout) {
    if !layout.can_save_session() {
        return;
    }

    let Ok(mut runtime) = runtime().lock() else {
        return;
    };

    if !runtime.enabled || !runtime.dirty {
        return;
    }

    let now = Instant::now();
    if let Some(dirty_since) = runtime.dirty_since
        && now.duration_since(dirty_since) < AUTOSAVE_INTERVAL
    {
        return;
    }

    let Ok(cwd) = env::current_dir() else {
        tracing::warn!("session autosave skipped: cwd unavailable");
        return;
    };

    let session = layout.to_session();
    match save_session_for_cwd(&cwd, &session) {
        Ok(()) => {
            runtime.dirty = false;
            runtime.dirty_since = None;
            runtime.last_save = Some(now);
        }
        Err(error) => {
            tracing::warn!(?error, "failed to autosave session");
        }
    }
}

/// Forces a final session save if session mode is enabled.
pub fn save_now(layout: &Layout) {
    if !layout.can_save_session() {
        return;
    }

    let Ok(mut runtime) = runtime().lock() else {
        return;
    };

    if !runtime.enabled {
        return;
    }

    let Ok(cwd) = env::current_dir() else {
        tracing::warn!("session save skipped: cwd unavailable");
        return;
    };

    let session = layout.to_session();
    match save_session_for_cwd(&cwd, &session) {
        Ok(()) => {
            runtime.dirty = false;
            runtime.dirty_since = None;
            runtime.last_save = Some(Instant::now());
        }
        Err(error) => tracing::warn!(?error, "failed to save session"),
    }
}

pub(crate) fn current_session_label() -> Option<String> {
    env::current_dir().ok().map(|cwd| path::session_label(&cwd))
}

pub(crate) fn session_version() -> u32 {
    SESSION_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::layout::Layout;
    use crate::window_group::WindowGroup;
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn session_path_uses_human_readable_label_and_hash_suffix() {
        let cwd = std::env::temp_dir().join("urvim-session-demo");
        let path = path::session_path_for_cwd(&cwd).expect("session path");
        let file_name = path.file_name().and_then(|name| name.to_str()).unwrap();
        assert!(file_name.starts_with("urvim-session-demo--"));
        assert!(file_name.ends_with(".toml"));
    }

    #[test]
    fn session_file_round_trips_through_toml() {
        let session = SessionFile {
            version: SESSION_VERSION,
            cwd: "/tmp/demo".to_string(),
            label: "demo".to_string(),
            focused_pane: 1,
            root: SessionNode::Pane(SessionPane {
                pane_id: 1,
                window_group: SessionWindowGroup {
                    active_tab: 0,
                    tabs: vec![SessionWindow {
                        path: "/tmp/demo.txt".to_string(),
                        cursor: SessionCursor { row: 2, col: 4 },
                        scroll_offset: SessionPosition { row: 1, col: 0 },
                        wrapped_row_offset: 1,
                        wrap_enabled: false,
                    }],
                },
            }),
        };

        let text = toml::to_string_pretty(&session).expect("serialize session");
        let parsed = toml::from_str::<SessionFile>(&text).expect("deserialize session");
        assert_eq!(parsed, session);
    }

    fn cwd_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn autosave_only_writes_when_dirty() {
        let _guard = cwd_lock();
        let temp_dir = std::env::temp_dir().join(format!(
            "urvim-autosave-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        let layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "autosave",
        )]));
        set_enabled(true);
        set_runtime_state_for_test(true, false, None, None);
        maybe_autosave(&layout);
        assert!(load_current_cwd().unwrap().is_none());

        set_runtime_state_for_test(
            true,
            true,
            Some(Instant::now() - Duration::from_secs(11)),
            None,
        );
        maybe_autosave(&layout);
        assert!(load_current_cwd().unwrap().is_some());

        std::env::set_current_dir(original_dir).unwrap();
    }
}
