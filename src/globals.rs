//! Global state for the editor.
//!
//! This module stores persistent state that needs to survive across mode switches
//! and future multi-window support.

use crate::buffer::{Buffer, BufferId, BufferPool};
use crate::config::Config;
use crate::editor::Action;
use crate::job::JobManager;
use crate::register::RegisterStore;
use crate::theme::Theme;
use std::sync::{Mutex, OnceLock, RwLock};

#[cfg(test)]
use std::cell::RefCell;

/// Direction of character search
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// Kind of character search motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindKind {
    /// f or F - lands ON the character
    Find,
    /// t or T - lands BEFORE/AFTER the character
    Till,
}

/// State of the last character search motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FindState {
    pub target_char: char,
    pub kind: FindKind,
    pub direction: Direction,
}

/// State of the last repeatable edit used by dot repeat.
#[derive(Debug, Clone, PartialEq)]
pub struct RepeatState {
    pub action: Action,
    pub count: usize,
    pub insert_text: Option<String>,
}

/// Global storage for the last character search state
static LAST_FIND: Mutex<Option<FindState>> = Mutex::new(None);
#[cfg(not(test))]
static LAST_REPEAT: Mutex<Option<RepeatState>> = Mutex::new(None);
static BUFFER_POOL: OnceLock<RwLock<BufferPool>> = OnceLock::new();
static ACTIVE_BUFFER_ID: OnceLock<RwLock<Option<BufferId>>> = OnceLock::new();
static CONFIG: OnceLock<RwLock<Option<Config>>> = OnceLock::new();
static ACTIVE_THEME: OnceLock<RwLock<Option<Theme>>> = OnceLock::new();
static JOB_MANAGER: OnceLock<RwLock<Option<JobManager>>> = OnceLock::new();
#[cfg(not(test))]
static REGISTER_STORE: OnceLock<RwLock<RegisterStore>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static TEST_CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
    static TEST_ACTIVE_THEME: RefCell<Option<Theme>> = const { RefCell::new(None) };
    static TEST_LAST_REPEAT: RefCell<Option<RepeatState>> = const { RefCell::new(None) };
    static TEST_REGISTER_STORE: RefCell<RegisterStore> = RefCell::new(RegisterStore::new());
}

/// Set the last character search state
pub fn set_last_find(state: FindState) {
    let mut last = LAST_FIND.lock().unwrap();
    *last = Some(state);
}

/// Get the last character search state
pub fn get_last_find() -> Option<FindState> {
    let last = LAST_FIND.lock().unwrap();
    last.clone()
}

/// Set the last repeatable edit state used by dot repeat.
pub fn set_last_repeat(state: RepeatState) {
    #[cfg(test)]
    {
        TEST_LAST_REPEAT.with(|slot| {
            *slot.borrow_mut() = Some(state);
        });
    }

    #[cfg(not(test))]
    {
        let mut last = LAST_REPEAT.lock().unwrap();
        *last = Some(state);
    }
}

/// Get the last repeatable edit state used by dot repeat.
pub fn get_last_repeat() -> Option<RepeatState> {
    #[cfg(test)]
    {
        TEST_LAST_REPEAT.with(|slot| slot.borrow().clone())
    }

    #[cfg(not(test))]
    {
        let last = LAST_REPEAT.lock().unwrap();
        last.clone()
    }
}

/// Returns the global buffer pool read-write lock, initializing it on first use.
pub fn buffer_pool() -> &'static RwLock<BufferPool> {
    BUFFER_POOL.get_or_init(|| RwLock::new(BufferPool::new()))
}

/// Runs a closure with mutable access to the global buffer pool.
pub fn with_buffer_pool<R>(f: impl FnOnce(&mut BufferPool) -> R) -> R {
    let mut pool = buffer_pool().write().unwrap();
    f(&mut pool)
}

/// Runs a closure with shared access to a live buffer entry.
pub fn with_buffer<R>(id: BufferId, f: impl FnOnce(&Buffer) -> R) -> Option<R> {
    let pool = buffer_pool().read().unwrap();
    pool.get(id).map(f)
}

/// Runs a closure with mutable access to a live buffer entry.
pub fn with_buffer_mut<R>(id: BufferId, f: impl FnOnce(&mut Buffer) -> R) -> Option<R> {
    let mut pool = buffer_pool().write().unwrap();
    pool.with_buffer_mut(id, f)
}

fn config_slot() -> &'static RwLock<Option<Config>> {
    CONFIG.get_or_init(|| RwLock::new(None))
}

fn active_buffer_slot() -> &'static RwLock<Option<BufferId>> {
    ACTIVE_BUFFER_ID.get_or_init(|| RwLock::new(None))
}

/// Sets the resolved startup configuration used by the editor.
pub fn set_config(config: Config) {
    let mut stored = config_slot().write().unwrap();
    *stored = Some(config);
}

/// Sets the currently active buffer ID used by global editor helpers.
pub fn set_active_buffer_id(buffer_id: BufferId) {
    let mut stored = active_buffer_slot().write().unwrap();
    *stored = Some(buffer_id);
}

/// Runs a closure with access to the currently active buffer ID, if one has been set.
pub fn with_active_buffer_id<R>(f: impl FnOnce(Option<BufferId>) -> R) -> R {
    let stored = active_buffer_slot().read().unwrap();
    f(*stored)
}

/// Runs a closure with optional access to the resolved startup configuration.
///
/// The closure receives `Some(&Config)` when startup configuration is available
/// and `None` otherwise.
pub fn with_opt_config<R>(f: impl FnOnce(Option<&Config>) -> R) -> R {
    #[cfg(test)]
    {
        let test_config = TEST_CONFIG.with(|slot| slot.borrow().clone());
        if let Some(config) = test_config.as_ref() {
            return f(Some(config));
        }
        f(None)
    }

    #[cfg(not(test))]
    {
        let config = config_slot().read().unwrap();
        f(config.as_ref())
    }
}

/// Runs a closure with access to the resolved startup configuration.
///
/// The closure is only called when configuration exists, and the return value
/// is wrapped in `Some`. If no configuration has been set, this returns `None`.
pub fn with_config<R>(f: impl FnOnce(&Config) -> R) -> Option<R> {
    #[cfg(test)]
    {
        let test_config = TEST_CONFIG.with(|slot| slot.borrow().clone());
        return test_config.as_ref().map(f);
    }

    #[cfg(not(test))]
    {
        let config = config_slot().read().unwrap();
        config.as_ref().map(f)
    }
}

fn active_theme_slot() -> &'static RwLock<Option<Theme>> {
    ACTIVE_THEME.get_or_init(|| RwLock::new(None))
}

fn job_manager_slot() -> &'static RwLock<Option<JobManager>> {
    JOB_MANAGER.get_or_init(|| RwLock::new(None))
}

#[cfg(not(test))]
fn register_store_slot() -> &'static RwLock<RegisterStore> {
    REGISTER_STORE.get_or_init(|| RwLock::new(RegisterStore::new()))
}

/// Sets the active theme used by renderers.
///
/// The editor treats the active theme as startup configuration, so this should
/// be called once after CLI theme selection succeeds.
pub fn set_active_theme(theme: Theme) {
    let mut active_theme = active_theme_slot().write().unwrap();
    *active_theme = Some(theme);
}

/// Runs a closure with access to the active theme if one has been configured.
pub fn with_active_theme<R>(f: impl FnOnce(Option<&Theme>) -> R) -> R {
    #[cfg(test)]
    {
        let test_theme = TEST_ACTIVE_THEME.with(|slot| slot.borrow().clone());
        if let Some(theme) = test_theme.as_ref() {
            return f(Some(theme));
        }
        f(None)
    }

    #[cfg(not(test))]
    {
        let theme = active_theme_slot().read().unwrap();
        f(theme.as_ref())
    }
}

/// Sets the active job manager used by the editor loop.
pub fn set_job_manager(manager: JobManager) {
    let mut job_manager = job_manager_slot().write().unwrap();
    *job_manager = Some(manager);
}

/// Runs a closure with access to the active job manager if one has been configured.
pub fn with_job_manager<R>(f: impl FnOnce(Option<&JobManager>) -> R) -> R {
    let job_manager = job_manager_slot().read().unwrap();
    f(job_manager.as_ref())
}

/// Runs a closure with shared access to the session-wide register store.
pub fn with_register_store<R>(f: impl FnOnce(&RegisterStore) -> R) -> R {
    #[cfg(test)]
    {
        TEST_REGISTER_STORE.with(|slot| f(&slot.borrow()))
    }

    #[cfg(not(test))]
    {
        let store = register_store_slot().read().unwrap();
        f(&store)
    }
}

/// Runs a closure with mutable access to the session-wide register store.
pub fn with_register_store_mut<R>(f: impl FnOnce(&mut RegisterStore) -> R) -> R {
    #[cfg(test)]
    {
        TEST_REGISTER_STORE.with(|slot| f(&mut slot.borrow_mut()))
    }

    #[cfg(not(test))]
    {
        let mut store = register_store_slot().write().unwrap();
        f(&mut store)
    }
}

/// Requests shutdown of the active job manager if one is configured.
pub fn shutdown_job_manager() {
    with_job_manager(|job_manager| {
        if let Some(job_manager) = job_manager {
            job_manager.shutdown();
        }
    });
}

#[cfg(test)]
/// A guard that installs a test-only resolved config for the current thread.
pub struct TestConfigGuard;

#[cfg(test)]
impl Drop for TestConfigGuard {
    fn drop(&mut self) {
        TEST_CONFIG.with(|slot| {
            *slot.borrow_mut() = None;
        });
    }
}

#[cfg(test)]
/// Installs a resolved config for the current test thread and clears it when the guard drops.
pub fn set_test_config(config: Config) -> TestConfigGuard {
    TEST_CONFIG.with(|slot| {
        *slot.borrow_mut() = Some(config);
    });
    TestConfigGuard
}

#[cfg(test)]
/// A guard that installs a test-only active theme for the current thread.
pub struct TestActiveThemeGuard;

#[cfg(test)]
/// A guard that installs a test-only register store for the current thread.
pub struct TestRegisterStoreGuard;

#[cfg(test)]
impl Drop for TestActiveThemeGuard {
    fn drop(&mut self) {
        TEST_ACTIVE_THEME.with(|slot| {
            *slot.borrow_mut() = None;
        });
    }
}

#[cfg(test)]
impl Drop for TestRegisterStoreGuard {
    fn drop(&mut self) {
        TEST_REGISTER_STORE.with(|slot| {
            *slot.borrow_mut() = RegisterStore::new();
        });
    }
}

#[cfg(test)]
/// Installs a theme for the current test thread and clears it when the guard drops.
pub fn set_test_active_theme(theme: Theme) -> TestActiveThemeGuard {
    TEST_ACTIVE_THEME.with(|slot| {
        *slot.borrow_mut() = Some(theme);
    });
    TestActiveThemeGuard
}

#[cfg(test)]
/// Installs a register store for the current test thread and clears it when the guard drops.
pub fn set_test_register_store(store: RegisterStore) -> TestRegisterStoreGuard {
    TEST_REGISTER_STORE.with(|slot| {
        *slot.borrow_mut() = store;
    });
    TestRegisterStoreGuard
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::terminal::Color;
    use crate::terminal::Style;
    use crate::theme::{HighlightStyles, Tag, Theme, ThemeKind};

    fn themed_theme() -> Theme {
        let default_style = Style::new().fg(Color::ansi(10)).bg(Color::ansi(20));
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.status_bar").expect("valid tag"),
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
        );
        highlights.insert(
            Tag::parse("ui.status_bar.modified_marker").expect("valid tag"),
            Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
        );
        highlights.insert(
            Tag::parse("ui.selection").expect("valid tag"),
            Style::new().reverse(),
        );
        highlights.insert(
            Tag::parse("ui.window.active_line").expect("valid tag"),
            Style::new().bg(Color::ansi(21)),
        );
        highlights.insert(
            Tag::parse("ui.tab.active").expect("valid tag"),
            Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
        );
        highlights.insert(
            Tag::parse("ui.tab.inactive").expect("valid tag"),
            Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
        );
        highlights.insert(
            Tag::parse("ui.tab.scroll_indicator").expect("valid tag"),
            Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
        );
        highlights.insert(
            Tag::parse("ui.window.gutter").expect("valid tag"),
            Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
        );
        highlights.insert(
            Tag::parse("ui.window").expect("valid tag"),
            Style::new().fg(Color::ansi(13)).bg(Color::ansi(14)),
        );
        highlights.insert(
            Tag::parse("ui.window.split_border").expect("valid tag"),
            Style::new().fg(Color::ansi(15)).bg(Color::ansi(16)),
        );
        highlights.insert(
            Tag::parse("ui.window.split_border.resize").expect("valid tag"),
            Style::new().fg(Color::ansi(17)).bg(Color::ansi(18)),
        );

        Theme::new("demo", ThemeKind::Ansi256, default_style, highlights)
    }

    fn themed_config(theme: &str) -> Config {
        Config {
            theme: theme.to_string(),
            insert_escape: None,
            syntax: true,
            auto_close_pairs: true,
            active_line: false,
            advanced_glyphs: std::collections::BTreeSet::new(),
            ..Default::default()
        }
    }

    #[test]
    fn test_set_and_get_last_find() {
        let state = FindState {
            target_char: 'x',
            kind: FindKind::Find,
            direction: Direction::Forward,
        };
        set_last_find(state.clone());
        assert_eq!(get_last_find(), Some(state));
    }

    #[test]
    fn test_get_last_find_empty() {
        // Ensure we start with None
        let mut last = LAST_FIND.lock().unwrap();
        *last = None;
        drop(last);

        assert_eq!(get_last_find(), None);
    }

    #[test]
    fn test_with_buffer_reads_live_buffer() {
        let id = with_buffer_pool(|pool| {
            let id = pool.create_buffer();
            pool.with_buffer_mut(id, |buffer| {
                buffer.insert_text(crate::buffer::Cursor::new(0, 0), "alpha");
            });
            id
        });

        let text = with_buffer(id, |buffer| buffer.as_str());

        assert_eq!(text.as_deref(), Some("alpha"));
    }

    #[test]
    fn test_with_buffer_missing_id_returns_none() {
        assert!(with_buffer(BufferId::new(usize::MAX), |_| ()).is_none());
    }

    #[test]
    fn test_set_active_theme_updates_global_slot() {
        let theme = themed_theme();
        let expected_name = theme.name().to_string();
        set_active_theme(theme);

        assert_eq!(
            active_theme_slot()
                .read()
                .unwrap()
                .as_ref()
                .map(|theme| theme.name()),
            Some(expected_name.as_str())
        );

        drop(active_theme_slot().write().unwrap().take());
    }

    #[test]
    fn test_set_config_updates_global_slot() {
        let config = themed_config("demo");
        let expected_theme = config.theme.clone();
        set_config(config);

        assert_eq!(
            config_slot()
                .read()
                .unwrap()
                .as_ref()
                .map(|config| config.theme.as_str()),
            Some(expected_theme.as_str())
        );

        drop(config_slot().write().unwrap().take());
    }

    #[test]
    fn test_test_config_guard_clears_on_drop() {
        let config = themed_config("demo");
        {
            let _guard = set_test_config(config);
            with_opt_config(|active_config| {
                assert_eq!(
                    active_config.map(|config| config.theme.as_str()),
                    Some("demo")
                );
            });
        }

        assert!(with_config(|active_config| active_config.theme.clone()).is_none());
    }

    #[test]
    fn test_test_active_theme_guard_clears_on_drop() {
        let theme = themed_theme();
        {
            let _guard = set_test_active_theme(theme);
            with_active_theme(|active_theme| {
                assert_eq!(active_theme.map(|theme| theme.name()), Some("demo"));
            });
        }

        with_active_theme(|active_theme| {
            assert!(active_theme.is_none());
        });
    }

    #[test]
    fn test_with_config_returns_value_when_config_present() {
        let config = themed_config("demo");
        let expected_theme = config.theme.clone();
        let _guard = set_test_config(config);

        let theme_name = with_config(|active_config| active_config.theme.clone());

        assert_eq!(theme_name, Some(expected_theme));
    }

    #[test]
    fn test_with_config_returns_none_when_config_missing() {
        TEST_CONFIG.with(|slot| {
            *slot.borrow_mut() = None;
        });

        let theme_name = with_config(|active_config| active_config.theme.clone());

        assert_eq!(theme_name, None);
    }

    #[test]
    fn test_with_opt_config_preserves_optional_behavior() {
        let config = themed_config("demo");
        let expected_theme = config.theme.clone();
        let _guard = set_test_config(config);

        let theme_name =
            with_opt_config(|active_config| active_config.map(|config| config.theme.clone()));

        assert_eq!(theme_name, Some(expected_theme));
    }

    #[test]
    fn test_repeat_state_round_trip() {
        use crate::editor::ActionKind;

        set_last_repeat(RepeatState {
            action: Action::new(ActionKind::DeleteLine),
            count: 4,
            insert_text: Some("hello".to_string()),
        });

        let state = get_last_repeat().expect("repeat state should be available");
        assert_eq!(state.count, 4);
        assert!(matches!(
            state.action.kind.as_ref(),
            Some(ActionKind::DeleteLine)
        ));
        assert_eq!(state.insert_text.as_deref(), Some("hello"));
    }
}
