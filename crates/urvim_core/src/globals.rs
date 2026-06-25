//! Global state for the editor.
//!
//! This module stores persistent state that needs to survive across mode switches
//! and future multi-window support.

use crate::AbsolutePath;
use crate::buffer::{Buffer, BufferId, BufferPool};
use crate::config::Config;
use crate::diagnostics::DiagnosticsStore;
use crate::editor::Action;
use crate::event::EditorEvent;
use crate::lsp::runtime::LspRuntime;
use crate::notification::{NotificationLevel, NotificationMessage, NotificationState};
use crate::register::RegisterStore;
use crate::ui::Intent;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};
use urvim_theme::{Theme, ThemeRegistry};

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

/// Notification for a workspace file operation applied by LSP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceFileOperationNotification {
    /// A file was created on disk.
    Create { path: PathBuf },
    /// A file was renamed on disk.
    Rename {
        old_path: PathBuf,
        new_path: PathBuf,
    },
    /// A file was deleted from disk.
    Delete {
        path: PathBuf,
        buffer_id: Option<BufferId>,
    },
}

/// Global storage for the last character search state
static LAST_FIND: Mutex<Option<FindState>> = Mutex::new(None);
#[cfg(not(test))]
static LAST_REPEAT: Mutex<Option<RepeatState>> = Mutex::new(None);
static BUFFER_POOL: OnceLock<RwLock<BufferPool>> = OnceLock::new();
static ACTIVE_BUFFER_ID: OnceLock<RwLock<Option<BufferId>>> = OnceLock::new();
static CONFIG: OnceLock<RwLock<Option<Config>>> = OnceLock::new();
static ACTIVE_THEME: OnceLock<RwLock<Option<Theme>>> = OnceLock::new();
static THEME_REGISTRY: OnceLock<RwLock<Option<ThemeRegistry>>> = OnceLock::new();
static LSP_RUNTIME: OnceLock<Mutex<Option<LspRuntime>>> = OnceLock::new();
#[cfg(not(test))]
static DIAGNOSTICS_STORE: OnceLock<DiagnosticsStore> = OnceLock::new();
static NOTIFICATION_STATE: OnceLock<Mutex<NotificationState>> = OnceLock::new();
static PLUGIN_KEYMAPS: OnceLock<RwLock<PluginKeymaps>> = OnceLock::new();
static PLUGIN_FILETYPES: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
static INLAY_HINT_RETRY_REQUESTED: AtomicBool = AtomicBool::new(false);
static FILE_OPERATION_QUEUE: OnceLock<Mutex<VecDeque<WorkspaceFileOperationNotification>>> =
    OnceLock::new();
#[cfg_attr(test, allow(dead_code))]
static EDITOR_EVENT_QUEUE: OnceLock<Mutex<VecDeque<EditorEvent>>> = OnceLock::new();
#[cfg(not(test))]
static REGISTER_STORE: OnceLock<RwLock<RegisterStore>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static TEST_CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
    static TEST_ACTIVE_THEME: RefCell<Option<Theme>> = const { RefCell::new(None) };
    static TEST_THEME_REGISTRY: RefCell<Option<ThemeRegistry>> = const { RefCell::new(None) };
    static TEST_LAST_REPEAT: RefCell<Option<RepeatState>> = const { RefCell::new(None) };
    static TEST_REGISTER_STORE: RefCell<RegisterStore> = RefCell::new(RegisterStore::new());
    static TEST_BUFFER_POOL: RefCell<BufferPool> = RefCell::new(BufferPool::new());
    static TEST_PLUGIN_KEYMAPS: RefCell<PluginKeymaps> = RefCell::new(PluginKeymaps::default());
    static TEST_PLUGIN_FILETYPES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Runtime keymaps installed by plugins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginKeymaps {
    pub normal: BTreeMap<String, String>,
    pub insert: BTreeMap<String, String>,
    pub visual: BTreeMap<String, String>,
    pub visual_line: BTreeMap<String, String>,
    pub resizing: BTreeMap<String, String>,
}

fn plugin_keymaps_slot() -> &'static RwLock<PluginKeymaps> {
    PLUGIN_KEYMAPS.get_or_init(|| RwLock::new(PluginKeymaps::default()))
}

fn plugin_filetypes_slot() -> &'static RwLock<Vec<String>> {
    PLUGIN_FILETYPES.get_or_init(|| RwLock::new(Vec::new()))
}

/// Replaces the runtime list of filetypes provided by plugins.
pub fn set_plugin_filetypes(mut filetypes: Vec<String>) {
    filetypes.sort();
    filetypes.dedup();
    #[cfg(test)]
    {
        TEST_PLUGIN_FILETYPES.with(|slot| *slot.borrow_mut() = filetypes);
    }

    #[cfg(not(test))]
    {
        *plugin_filetypes_slot().write().unwrap() = filetypes;
    }
}

/// Returns filetypes provided by plugins.
pub fn plugin_filetypes() -> Vec<String> {
    #[cfg(test)]
    {
        return TEST_PLUGIN_FILETYPES.with(|slot| slot.borrow().clone());
    }

    #[cfg(not(test))]
    {
        plugin_filetypes_slot().read().unwrap().clone()
    }
}

#[cfg(test)]
static LSP_RUNTIME_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
thread_local! {
    static TEST_DIAGNOSTICS_STORE: RefCell<DiagnosticsStore> = RefCell::new(DiagnosticsStore::new());
    static TEST_EDITOR_EVENT_QUEUE: RefCell<VecDeque<EditorEvent>> = RefCell::new(VecDeque::new());
}

#[cfg(test)]
static BUFFER_POOL_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Set the last character search state
pub fn set_last_find(state: FindState) {
    let mut last = LAST_FIND.lock().unwrap();
    *last = Some(state);
}

/// Get the last character search state
pub fn get_last_find() -> Option<FindState> {
    let last = LAST_FIND.lock().unwrap();
    *last
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
    #[cfg(test)]
    {
        return TEST_BUFFER_POOL.with(|pool| f(&mut pool.borrow_mut()));
    }

    #[cfg(not(test))]
    {
        let mut pool = buffer_pool().write().unwrap();
        f(&mut pool)
    }
}

/// Runs a closure with shared access to a live buffer entry.
pub fn with_buffer<R>(id: BufferId, f: impl FnOnce(&Buffer) -> R) -> Option<R> {
    #[cfg(test)]
    {
        return TEST_BUFFER_POOL.with(|pool| pool.borrow().get(id).map(f));
    }

    #[cfg(not(test))]
    {
        let pool = buffer_pool().read().unwrap();
        pool.get(id).map(f)
    }
}

/// Runs a closure with mutable access to a live buffer entry.
pub fn with_buffer_mut<R>(id: BufferId, f: impl FnOnce(&mut Buffer) -> R) -> Option<R> {
    #[cfg(test)]
    {
        return TEST_BUFFER_POOL.with(|pool| pool.borrow_mut().with_buffer_mut(id, f));
    }

    #[cfg(not(test))]
    {
        let mut pool = buffer_pool().write().unwrap();
        pool.with_buffer_mut(id, f)
    }
}

/// Opens a file-backed buffer while minimizing time spent holding the pool lock.
pub fn open_buffer(path: impl AsRef<std::path::Path>) -> std::io::Result<BufferId> {
    let abs_path = AbsolutePath::from_path(path.as_ref()).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "failed to resolve absolute path",
        )
    })?;

    let buffer = match Buffer::load_from_file(abs_path.as_path()) {
        Ok(buffer) => buffer,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Buffer::with_path(abs_path.clone())
        }
        Err(error) => return Err(error),
    };

    #[cfg(test)]
    {
        return TEST_BUFFER_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            if let Some(id) = pool.buffer_id_for_path(&abs_path) {
                return Ok(id);
            }
            Ok(pool.register_buffer(buffer))
        });
    }

    #[cfg(not(test))]
    {
        if let Some(id) = buffer_pool().read().unwrap().buffer_id_for_path(&abs_path) {
            return Ok(id);
        }

        let mut pool = buffer_pool().write().unwrap();
        if let Some(id) = pool.buffer_id_for_path(&abs_path) {
            return Ok(id);
        }

        Ok(pool.register_buffer(buffer))
    }
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
        test_config.as_ref().map(f)
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

fn theme_registry_slot() -> &'static RwLock<Option<ThemeRegistry>> {
    THEME_REGISTRY.get_or_init(|| RwLock::new(None))
}

fn notification_state_slot() -> &'static Mutex<NotificationState> {
    NOTIFICATION_STATE.get_or_init(|| Mutex::new(NotificationState::new()))
}

fn lsp_runtime_slot() -> &'static Mutex<Option<LspRuntime>> {
    LSP_RUNTIME.get_or_init(|| Mutex::new(None))
}

#[cfg(not(test))]
fn diagnostics_store_slot() -> &'static DiagnosticsStore {
    DIAGNOSTICS_STORE.get_or_init(DiagnosticsStore::new)
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

/// Sets the theme registry used by the colorscheme picker.
pub fn set_theme_registry(registry: ThemeRegistry) {
    let slot = theme_registry_slot();
    let mut stored = slot.write().unwrap();
    *stored = Some(registry);
}

/// Runs a closure with access to the theme registry, if one has been set.
pub fn with_theme_registry<R>(f: impl FnOnce(Option<&ThemeRegistry>) -> R) -> R {
    #[cfg(test)]
    {
        let test_registry = TEST_THEME_REGISTRY.with(|slot| slot.borrow().clone());
        if let Some(registry) = test_registry.as_ref() {
            return f(Some(registry));
        }
        f(None)
    }

    #[cfg(not(test))]
    {
        let slot = theme_registry_slot();
        let stored = slot.read().unwrap();
        f(stored.as_ref())
    }
}

/// Runs a closure with mutable access to the theme registry, if one has been set.
pub fn with_theme_registry_mut<R>(f: impl FnOnce(Option<&mut ThemeRegistry>) -> R) -> R {
    #[cfg(test)]
    {
        TEST_THEME_REGISTRY.with(|slot| f(slot.borrow_mut().as_mut()))
    }

    #[cfg(not(test))]
    {
        let slot = theme_registry_slot();
        let mut stored = slot.write().unwrap();
        f(stored.as_mut())
    }
}

/// Updates the theme field in the global session config.
///
/// This is called when a user selects a colorscheme from the picker so that
/// the chosen theme name persists for the remainder of the session.
pub fn update_theme_in_config(theme_name: &str) {
    #[cfg(test)]
    {
        TEST_CONFIG.with(|slot| {
            if let Some(config) = slot.borrow_mut().as_mut() {
                config.theme = theme_name.to_string();
            }
        });
    }

    #[cfg(not(test))]
    {
        let mut stored = config_slot().write().unwrap();
        if let Some(ref mut config) = *stored {
            config.theme = theme_name.to_string();
        }
    }
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

/// Runs a closure with shared access to runtime plugin keymaps.
pub fn with_plugin_keymaps<R>(f: impl FnOnce(&PluginKeymaps) -> R) -> R {
    #[cfg(test)]
    {
        TEST_PLUGIN_KEYMAPS.with(|slot| f(&slot.borrow()))
    }

    #[cfg(not(test))]
    {
        let keymaps = plugin_keymaps_slot().read().unwrap();
        f(&keymaps)
    }
}

/// Runs a closure with mutable access to runtime plugin keymaps.
pub fn with_plugin_keymaps_mut<R>(f: impl FnOnce(&mut PluginKeymaps) -> R) -> R {
    #[cfg(test)]
    {
        TEST_PLUGIN_KEYMAPS.with(|slot| f(&mut slot.borrow_mut()))
    }

    #[cfg(not(test))]
    {
        let mut keymaps = plugin_keymaps_slot().write().unwrap();
        f(&mut keymaps)
    }
}

/// Returns plugin key mappings for a mode as resolved command intents.
pub fn plugin_keymap_intents_for_mode(mode: crate::editor::ModeKind) -> BTreeMap<String, Intent> {
    with_plugin_keymaps(|keymaps| {
        let mappings = match mode {
            crate::editor::ModeKind::Normal => &keymaps.normal,
            crate::editor::ModeKind::Insert => &keymaps.insert,
            crate::editor::ModeKind::Visual => &keymaps.visual,
            crate::editor::ModeKind::VisualLine => &keymaps.visual_line,
            crate::editor::ModeKind::Resizing => &keymaps.resizing,
            crate::editor::ModeKind::Replace => return BTreeMap::new(),
        };
        mappings
            .iter()
            .filter_map(|(lhs, rhs)| {
                crate::command::parse(rhs)
                    .ok()
                    .map(|intent| (lhs.clone(), intent))
            })
            .collect()
    })
}

/// Enqueues a user-facing notification.
pub fn enqueue_notification(level: NotificationLevel, text: String) -> bool {
    match level {
        NotificationLevel::Info => tracing::info!("{}", text),
        NotificationLevel::Warn => tracing::warn!("{}", text),
        NotificationLevel::Error => tracing::error!("{}", text),
    }

    let now = std::time::Instant::now();
    let Ok(mut state) = notification_state_slot().lock() else {
        tracing::warn!("notification queue unavailable; skipping enqueue");
        return false;
    };

    state.enqueue(level, text, now)
}

/// Returns the active notification after pruning/advancing expired entries.
pub fn active_notification(now: std::time::Instant) -> Option<NotificationMessage> {
    let Ok(mut state) = notification_state_slot().lock() else {
        tracing::warn!("notification queue unavailable; cannot read active notification");
        return None;
    };

    state.prune_and_advance(now);
    state.active().cloned()
}

/// Prunes expired notifications and advances the queue.
pub fn prune_notifications() -> bool {
    let Ok(mut state) = notification_state_slot().lock() else {
        tracing::warn!("notification queue unavailable; skip prune");
        return false;
    };

    state.prune_and_advance(std::time::Instant::now())
}

/// Requests a UI redraw due to notification state changes.
pub fn request_notification_redraw() {
    if let Ok(mut state) = notification_state_slot().lock() {
        state.request_redraw();
    }
}

/// Requests a retry of the active inlay-hint request after LSP server state changes.
pub fn request_inlay_hint_retry() {
    INLAY_HINT_RETRY_REQUESTED.store(true, Ordering::SeqCst);
    request_notification_redraw();
}

/// Returns and clears whether inlay hints should be retried.
pub fn take_inlay_hint_retry_requested() -> bool {
    INLAY_HINT_RETRY_REQUESTED.swap(false, Ordering::SeqCst)
}

fn file_operation_queue_slot() -> &'static Mutex<VecDeque<WorkspaceFileOperationNotification>> {
    FILE_OPERATION_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg_attr(test, allow(dead_code))]
fn editor_event_queue_slot() -> &'static Mutex<VecDeque<EditorEvent>> {
    EDITOR_EVENT_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Enqueues an editor event for delivery to plugin hooks.
///
/// Events are consumed in FIFO order by [`take_editor_event`].
pub fn enqueue_editor_event(event: EditorEvent) {
    #[cfg(test)]
    {
        TEST_EDITOR_EVENT_QUEUE.with(|queue| {
            queue.borrow_mut().push_back(event);
        });
        return;
    }

    #[cfg(not(test))]
    {
        if let Ok(mut queue) = editor_event_queue_slot().lock() {
            queue.push_back(event);
        }
    }
}

/// Returns and removes the next pending editor event in FIFO order.
///
/// Returns `None` when the queue is empty.
pub fn take_editor_event() -> Option<EditorEvent> {
    #[cfg(test)]
    {
        return TEST_EDITOR_EVENT_QUEUE.with(|queue| queue.borrow_mut().pop_front());
    }

    #[cfg(not(test))]
    {
        let Ok(mut queue) = editor_event_queue_slot().lock() else {
            return None;
        };
        queue.pop_front()
    }
}

/// Enqueues a workspace file-operation notification for the UI.
pub fn enqueue_workspace_file_operation_notification(
    notification: WorkspaceFileOperationNotification,
) {
    if let Ok(mut queue) = file_operation_queue_slot().lock() {
        queue.push_back(notification);
    }
}

/// Returns and removes the next pending workspace file-operation notification.
pub fn take_workspace_file_operation_notification() -> Option<WorkspaceFileOperationNotification> {
    let Ok(mut queue) = file_operation_queue_slot().lock() else {
        return None;
    };

    queue.pop_front()
}

/// Returns and clears the notification redraw-requested flag.
pub fn take_notification_redraw_requested() -> bool {
    let Ok(mut state) = notification_state_slot().lock() else {
        tracing::warn!("notification queue unavailable; cannot read redraw flag");
        return false;
    };

    state.take_redraw_requested()
}

/// Installs the resolved LSP runtime for the current editor session.
pub fn set_lsp_runtime(runtime: LspRuntime) {
    if let Ok(mut slot) = lsp_runtime_slot().lock() {
        *slot = Some(runtime);
    }
}

/// Runs a closure with shared access to the diagnostics store.
pub fn with_diagnostics_store<R>(f: impl FnOnce(&DiagnosticsStore) -> R) -> Option<R> {
    #[cfg(test)]
    {
        return TEST_DIAGNOSTICS_STORE.with(|store| {
            let store = store.borrow();
            Some(f(&store))
        });
    }

    #[cfg(not(test))]
    {
        Some(f(diagnostics_store_slot()))
    }
}

#[cfg(test)]
/// Clears the diagnostics store in tests.
pub fn clear_diagnostics_store() {
    TEST_DIAGNOSTICS_STORE.with(|store| {
        *store.borrow_mut() = DiagnosticsStore::new();
    });
}

/// Runs a closure with mutable access to the LSP runtime, if one has been set.
pub fn with_lsp_runtime_mut<R>(f: impl FnOnce(&mut LspRuntime) -> R) -> Option<R> {
    let Ok(mut slot) = lsp_runtime_slot().lock() else {
        return None;
    };
    slot.as_mut().map(f)
}

/// Runs a closure with mutable access to the LSP runtime without blocking.
pub fn try_with_lsp_runtime_mut<R>(f: impl FnOnce(&mut LspRuntime) -> R) -> Option<R> {
    let Ok(mut slot) = lsp_runtime_slot().try_lock() else {
        return None;
    };
    slot.as_mut().map(f)
}

#[cfg(test)]
/// Clears the installed LSP runtime in tests.
pub fn clear_lsp_runtime() {
    if let Ok(mut slot) = lsp_runtime_slot().lock() {
        *slot = None;
    }
}

/// Runs a closure with shared access to the LSP runtime, if one has been set.
pub fn with_lsp_runtime<R>(f: impl FnOnce(Option<&LspRuntime>) -> R) -> R {
    let Ok(slot) = lsp_runtime_slot().lock() else {
        return f(None);
    };
    f(slot.as_ref())
}

/// Shuts down the LSP runtime if it exists.
pub fn shutdown_lsp_runtime() {
    if let Ok(mut slot) = lsp_runtime_slot().lock()
        && let Some(runtime) = slot.as_mut()
    {
        runtime.shutdown();
    }
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
/// Serializes tests that mutate the global LSP runtime.
pub fn lsp_runtime_test_lock() -> std::sync::MutexGuard<'static, ()> {
    LSP_RUNTIME_TEST_LOCK.lock().unwrap()
}

#[cfg(test)]
/// A guard that installs a test-only theme registry for the current thread.
pub struct TestThemeRegistryGuard;

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
impl Drop for TestThemeRegistryGuard {
    fn drop(&mut self) {
        TEST_THEME_REGISTRY.with(|slot| {
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
/// Installs a theme registry for the current test thread and clears it when the guard drops.
pub fn set_test_theme_registry(registry: ThemeRegistry) -> TestThemeRegistryGuard {
    TEST_THEME_REGISTRY.with(|slot| {
        *slot.borrow_mut() = Some(registry);
    });
    TestThemeRegistryGuard
}

#[cfg(test)]
/// Installs a register store for the current test thread and clears it when the guard drops.
pub fn set_test_register_store(store: RegisterStore) -> TestRegisterStoreGuard {
    TEST_REGISTER_STORE.with(|slot| {
        *slot.borrow_mut() = store;
    });
    TestRegisterStoreGuard
}

/// Clears active and queued notifications.
pub fn clear_notifications() {
    if let Ok(mut state) = notification_state_slot().lock() {
        state.clear();
    }
}

/// Clears queued workspace file-operation notifications.
pub fn clear_workspace_file_operation_notifications() {
    if let Ok(mut queue) = file_operation_queue_slot().lock() {
        queue.clear();
    }
}

/// Clears the queued editor events. Intended for tests that need a clean queue
/// between assertions.
pub fn clear_editor_events_for_tests() {
    #[cfg(test)]
    {
        TEST_EDITOR_EVENT_QUEUE.with(|queue| {
            queue.borrow_mut().clear();
        });
    }

    #[cfg(not(test))]
    {
        if let Ok(mut queue) = editor_event_queue_slot().lock() {
            queue.clear();
        }
    }
}

/// Resets the thread-local test buffer pool to an empty state. Intended for
/// tests that need a clean pool between assertions.
#[cfg_attr(not(test), allow(dead_code))]
pub fn clear_buffer_pool_for_tests() {
    #[cfg(test)]
    {
        let _pool_guard = buffer_pool_test_lock();
        TEST_BUFFER_POOL.with(|pool| {
            *pool.borrow_mut() = BufferPool::new();
        });
    }
}

#[cfg(test)]
/// Returns a process-wide guard for synchronizing notification-related tests.
pub fn notification_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
/// Returns a process-wide guard for synchronizing buffer-pool tests.
pub fn buffer_pool_test_lock() -> std::sync::MutexGuard<'static, ()> {
    BUFFER_POOL_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::notification::NotificationLevel;
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::layer::SubscriberExt;
    use urvim_terminal::Color;
    use urvim_terminal::Style;
    use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

    struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut output = self.0.lock().expect("capture buffer lock");
            output.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn captured_subscriber(output: Arc<Mutex<Vec<u8>>>) -> impl tracing::Subscriber {
        tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .with_writer(move || CapturedWriter(output.clone()))
                .with_ansi(false)
                .without_time(),
        )
    }

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
            Tag::parse("ui.window.lines").expect("valid tag"),
            Style::new().fg(Color::ansi(15)).bg(Color::ansi(16)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines.resize").expect("valid tag"),
            Style::new().fg(Color::ansi(17)).bg(Color::ansi(18)),
        );
        highlights.insert(
            Tag::parse("ui.input.prompt").expect("valid tag"),
            Style::new().fg(Color::ansi(19)).bold(),
        );
        highlights.insert(
            Tag::parse("ui.input.prompt.exact").expect("valid tag"),
            Style::new().fg(Color::ansi(20)).bold(),
        );
        highlights.insert(
            Tag::parse("ui.input.prompt.fuzzy").expect("valid tag"),
            Style::new().fg(Color::ansi(21)).italic(),
        );
        highlights.insert(
            Tag::parse("ui.input.prompt.separator").expect("valid tag"),
            Style::new().fg(Color::ansi(22)).faint(),
        );

        Theme::new("demo", ThemeKind::Ansi256, default_style, highlights)
    }

    fn themed_config(theme: &str) -> Config {
        Config {
            theme: theme.to_string(),
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
        set_last_find(state);
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

    #[test]
    fn test_notification_queue_round_trip() {
        let _guard = notification_test_lock();
        clear_notifications();

        assert!(enqueue_notification(
            NotificationLevel::Info,
            "Saved".to_string()
        ));
        let message = active_notification(std::time::Instant::now()).expect("message");
        assert_eq!(message.text, "Saved");
        assert_eq!(message.level, NotificationLevel::Info);
    }

    #[test]
    fn test_notification_enqueue_logs_message() {
        let _guard = notification_test_lock();
        clear_notifications();

        let output = Arc::new(Mutex::new(Vec::new()));
        let subscriber = captured_subscriber(output.clone());
        let _subscriber_guard = tracing::subscriber::set_default(subscriber);

        assert!(enqueue_notification(
            NotificationLevel::Error,
            "Unknown command: foo".to_string()
        ));

        let output = String::from_utf8(output.lock().expect("capture buffer lock").clone())
            .expect("captured log should be valid utf-8");
        assert!(output.contains("ERROR"));
        assert!(output.contains("Unknown command: foo"));
    }

    #[test]
    fn test_notification_redraw_flag_round_trip() {
        let _guard = notification_test_lock();
        clear_notifications();

        assert!(!take_notification_redraw_requested());
        request_notification_redraw();
        assert!(take_notification_redraw_requested());
        assert!(!take_notification_redraw_requested());
    }

    #[test]
    fn test_notification_state_redraw_flag_round_trip() {
        let mut state = NotificationState::new();
        assert!(!state.take_redraw_requested());
        state.request_redraw();
        assert!(state.take_redraw_requested());
        assert!(!state.take_redraw_requested());
    }

    #[test]
    fn test_try_with_lsp_runtime_mut_returns_none_when_locked() {
        let _guard = lsp_runtime_test_lock();
        use std::sync::mpsc;
        use std::thread;
        use std::time::{Duration, Instant};

        let config = themed_config("demo");
        set_lsp_runtime(crate::lsp::runtime::LspRuntime::new(&config));

        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let _ = with_lsp_runtime_mut(|_| {
                ready_tx.send(()).expect("ready signal");
                let _ = release_rx.recv_timeout(Duration::from_secs(5));
            });
        });

        let deadline = Instant::now() + Duration::from_secs(5);
        while ready_rx.try_recv().is_err() {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for worker to lock runtime"
            );
            thread::sleep(Duration::from_millis(5));
        }
        assert!(try_with_lsp_runtime_mut(|_| ()).is_none());
        release_tx.send(()).expect("release worker");
        handle.join().expect("worker should finish");
        clear_lsp_runtime();
    }

    #[test]
    fn test_editor_event_queue_is_fifo_and_empty_after_drain() {
        clear_editor_events_for_tests();
        assert!(take_editor_event().is_none());

        enqueue_editor_event(EditorEvent::EditorStarted);
        enqueue_editor_event(EditorEvent::BufferLoaded {
            buffer_id: BufferId::new(1),
        });
        enqueue_editor_event(EditorEvent::BufferSaved {
            buffer_id: BufferId::new(1),
        });
        enqueue_editor_event(EditorEvent::CommandExecuted {
            command: "Write".to_string(),
        });

        let first = take_editor_event();
        let second = take_editor_event();
        let third = take_editor_event();
        let fourth = take_editor_event();
        let fifth = take_editor_event();

        assert!(matches!(first, Some(EditorEvent::EditorStarted)));
        assert!(matches!(
            second,
            Some(EditorEvent::BufferLoaded { buffer_id }) if buffer_id == BufferId::new(1)
        ));
        assert!(matches!(
            third,
            Some(EditorEvent::BufferSaved { buffer_id }) if buffer_id == BufferId::new(1)
        ));
        assert!(matches!(
            fourth,
            Some(EditorEvent::CommandExecuted { ref command }) if command == "Write"
        ));
        assert!(fifth.is_none());

        clear_editor_events_for_tests();
    }

    #[test]
    fn test_clear_editor_events_for_tests_empties_the_queue() {
        clear_editor_events_for_tests();
        enqueue_editor_event(EditorEvent::EditorStarted);
        enqueue_editor_event(EditorEvent::BufferLoaded {
            buffer_id: BufferId::new(2),
        });
        assert!(take_editor_event().is_some());

        clear_editor_events_for_tests();
        assert!(take_editor_event().is_none());
    }
}
