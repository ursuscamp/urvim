use std::collections::VecDeque;
use std::io::{Read, Write};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use crate::actions::*;
use crate::plugin::*;
use crate::render::*;
use crate::startup::*;
use urvim_core::buffer::{Buffer, BufferId};
use urvim_core::cli::CliFileSpec;
use urvim_core::editor::{EditorAction, EditorOperation, ModeKind, RepeatReplay};
use urvim_core::ui::{Intent, UiEvent};
use urvim_core::window::VisualSelectionKind;
use urvim_core::window_group::WindowGroup;
use urvim_core::{config::Config, globals, screen::Screen};
use urvim_terminal::{Event, Key, KeyCode, Terminal};

struct TestBackend {
    input: Arc<Mutex<VecDeque<u8>>>,
    output: Arc<Mutex<Vec<u8>>>,
}

fn shared_test_layout() -> SharedLayout {
    Rc::new(RefCell::new(urvim_core::Layout::new(
        WindowGroup::from_buffers(vec![Buffer::new()]),
    )))
}

fn repeat_state_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn notification_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn theme_registry_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn test_theme_source(name: &str) -> String {
    format!(
        r##"
name = "{name}"

[palette]
bg = "#101010"
fg = "#eeeeee"

[default]
fg = "fg"
bg = "bg"
"##
    )
}

fn resolve_test_plugin_editor_request(
    layout: &mut urvim_core::Layout,
    request: &urvim_plugin::PluginRequest,
) -> urvim_plugin::PluginResponse {
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        layout,
        request,
    )
}

fn resolve_test_plugin_editor_request_with_contributions(
    contributions: &mut urvim_plugin::PluginContributionRegistry,
    plugin: &str,
    layout: &mut urvim_core::Layout,
    request: &urvim_plugin::PluginRequest,
) -> urvim_plugin::PluginResponse {
    match resolve_plugin_editor_request(contributions, plugin, layout, request) {
        PluginEditorRequestOutcome::Respond(response) => response,
        PluginEditorRequestOutcome::Pending(_) => panic!("test request unexpectedly pending"),
    }
}

fn cwd_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn buffer_pool_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

impl TestBackend {
    fn new(data: Vec<u8>) -> Self {
        Self {
            input: Arc::new(Mutex::new(VecDeque::from(data))),
            output: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Read for TestBackend {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut input = self.input.lock().unwrap();
        if input.is_empty() {
            return Ok(0);
        }
        let mut i = 0;
        while i < buf.len() {
            match input.pop_front() {
                Some(b) => {
                    buf[i] = b;
                    i += 1;
                }
                None => break,
            }
        }
        Ok(i)
    }
}

impl Write for TestBackend {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut output = self.output.lock().unwrap();
        output.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl rustix::fd::AsFd for TestBackend {
    fn as_fd(&self) -> rustix::fd::BorrowedFd<'_> {
        panic!("TestBackend does not have a valid file descriptor")
    }
}

#[test]
fn test_handle_resize_clears_terminal() {
    let stdin = TestBackend::new(Vec::new());
    let stdout = TestBackend::new(Vec::new());
    let output = stdout.output.clone();
    let mut terminal = Terminal::new_for_testing(stdin, stdout);
    let mut screen = Screen::new(2, 2);

    handle_resize(&mut terminal, &mut screen, 3, 4).unwrap();

    assert_eq!(screen.size(), (3, 4));
    assert_eq!(output.lock().unwrap().as_slice(), b"\x1b[2J\x1b[H");
}

#[test]
fn render_frame_if_needed_skips_idle_noop_frames() {
    let stdin = TestBackend::new(Vec::new());
    let stdout = TestBackend::new(Vec::new());
    let output = stdout.output.clone();
    let mut terminal = Terminal::new_for_testing(stdin, stdout);
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let mut screen = Screen::new(1, 5);

    assert!(!render_frame_if_needed(false, &mut layout, &mut screen, &mut terminal, 1, 5).unwrap());
    assert!(output.lock().unwrap().is_empty());
}

#[test]
fn apply_undo_redo_requests_redraw_after_success() {
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));

    assert!(
        layout
            .active_buffer_view_mut()
            .with_buffer_mut(|buffer| {
                buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "hello");
                buffer.push_snapshot(urvim_core::buffer::Cursor::new(0, 5));
            })
            .is_some()
    );

    assert!(apply_undo_redo(&mut layout, false));
    assert_eq!(
        layout.active_buffer_view().cursor(),
        urvim_core::buffer::Cursor::new(0, 0)
    );
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some(String::new())
    );

    assert!(apply_undo_redo(&mut layout, true));
    assert_eq!(
        layout.active_buffer_view().cursor(),
        urvim_core::buffer::Cursor::new(0, 5)
    );
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some("hello".to_string())
    );
}

#[test]
fn plugin_open_file_opens_existing_file() {
    let _guard = buffer_pool_lock();
    let unique = format!(
        "urvim-plugin-open-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::write(&path, "hello world").unwrap();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        200,
        "editor/openFile",
        serde_json::json!({ "path": path.to_string_lossy() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 200);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["opened"], serde_json::json!(true));
    assert!(result["buffer"]["buffer_id"].as_u64().is_some());
    assert_eq!(result["buffer"]["active"], serde_json::json!(true));
    assert_eq!(result["buffer"]["visible"], serde_json::json!(true));

    std::fs::remove_file(path).ok();
}

#[test]
fn plugin_open_file_reports_false_when_file_already_open() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let unique = format!(
        "urvim-plugin-open-existing-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::write(&path, "hello world").unwrap();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_paths(std::slice::from_ref(&path)));
    let buffer_id = layout.active_buffer_view().buffer_id();
    drain_editor_events_serial();

    let request = urvim_plugin::PluginRequest::new(
        226,
        "editor/openFile",
        serde_json::json!({ "path": path.to_string_lossy() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 226);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["opened"], serde_json::json!(false));
    assert_eq!(
        result["buffer"]["buffer_id"],
        serde_json::json!(buffer_id.get())
    );
    assert_eq!(result["buffer"]["active"], serde_json::json!(true));

    let events = drain_editor_events_serial();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, urvim_core::event::EditorEvent::BufferOpened { buffer_id: id } if *id == buffer_id)),
        "did not expect BufferOpened for already-open file {buffer_id:?}, got {events:?}"
    );

    std::fs::remove_file(path).ok();
}

#[test]
fn plugin_open_file_opens_missing_file_backed_buffer() {
    let _guard = buffer_pool_lock();
    let path = std::env::temp_dir().join(format!(
        "urvim-plugin-missing-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ));

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        201,
        "editor/openFile",
        serde_json::json!({ "path": path.to_string_lossy() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 201);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["opened"], serde_json::json!(true));
    assert!(result["buffer"]["buffer_id"].as_u64().is_some());
}

#[test]
fn plugin_open_file_requires_path() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(202, "editor/openFile", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 202);
    assert!(response.error.unwrap().contains("requires path"));
}

#[test]
fn plugin_open_buffer_opens_hidden_loaded_buffer() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("visible")]));

    let hidden_id = globals::with_buffer_pool(|pool| {
        let id = pool.create_buffer();
        pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "hidden content");
        });
        id
    });

    let request = urvim_plugin::PluginRequest::new(
        203,
        "editor/openBuffer",
        serde_json::json!({ "buffer_id": hidden_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 203);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["opened"], serde_json::json!(true));
    assert_eq!(
        result["buffer"]["buffer_id"],
        serde_json::json!(hidden_id.get())
    );
    assert_eq!(result["buffer"]["visible"], serde_json::json!(true));
    assert_eq!(result["buffer"]["active"], serde_json::json!(true));
}

#[test]
fn plugin_open_buffer_rejects_unknown_id() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        204,
        "editor/openBuffer",
        serde_json::json!({ "buffer_id": 999999 }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 204);
    assert!(response.error.unwrap().contains("unknown buffer_id"));
}

#[test]
fn plugin_focus_buffer_focuses_visible_buffer() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::from_str("first"),
        Buffer::from_str("second"),
    ]));
    let first_id = layout.active_buffer_view().buffer_id();

    let request = urvim_plugin::PluginRequest::new(
        205,
        "editor/focusBuffer",
        serde_json::json!({ "buffer_id": first_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 205);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["focused"], serde_json::json!(true));
    assert_eq!(result["opened"], serde_json::json!(false));
    assert_eq!(
        result["buffer"]["buffer_id"],
        serde_json::json!(first_id.get())
    );
}

#[test]
fn plugin_focus_buffer_rejects_hidden_without_open_if_needed() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("visible")]));

    let hidden_id = globals::with_buffer_pool(|pool| {
        let id = pool.create_buffer();
        pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "hidden content");
        });
        id
    });

    let request = urvim_plugin::PluginRequest::new(
        206,
        "editor/focusBuffer",
        serde_json::json!({ "buffer_id": hidden_id.get(), "open_if_needed": false }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 206);
    assert!(response.error.unwrap().contains("is not visible"));
}

#[test]
fn plugin_focus_buffer_opens_hidden_with_open_if_needed() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("visible")]));

    let hidden_id = globals::with_buffer_pool(|pool| {
        let id = pool.create_buffer();
        pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "hidden content");
        });
        id
    });

    let request = urvim_plugin::PluginRequest::new(
        207,
        "editor/focusBuffer",
        serde_json::json!({ "buffer_id": hidden_id.get(), "open_if_needed": true }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 207);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["focused"], serde_json::json!(true));
    assert_eq!(result["opened"], serde_json::json!(true));
    assert_eq!(
        result["buffer"]["buffer_id"],
        serde_json::json!(hidden_id.get())
    );
}

#[test]
fn plugin_focus_buffer_rejects_unknown_id() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        208,
        "editor/focusBuffer",
        serde_json::json!({ "buffer_id": 999999 }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 208);
    assert!(response.error.unwrap().contains("unknown buffer_id"));
}

#[test]
fn plugin_execute_command_runs_safe_command() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let request = urvim_plugin::PluginRequest::new(
        209,
        "editor/executeCommand",
        serde_json::json!({ "command": "pane wrap-toggle" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 209);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["handled"], serde_json::json!(true));
    assert_eq!(result["command"], serde_json::json!("pane wrap-toggle"));
}

#[test]
fn plugin_execute_command_rejects_plugin_commands() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        210,
        "editor/executeCommand",
        serde_json::json!({ "command": "plugin demo-plugin echo text=hello" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 210);
    assert!(response.error.is_some());
    let error = response.error.unwrap();
    assert!(
        error.contains("does not allow plugin commands") || error.contains("Unknown plugin"),
        "expected plugin command rejection, got: {error}"
    );
}

#[test]
fn plugin_execute_command_rejects_quit_commands() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        211,
        "editor/executeCommand",
        serde_json::json!({ "command": "quit" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 211);
    assert!(
        response
            .error
            .unwrap()
            .contains("does not allow quit commands")
    );
}

#[test]
fn plugin_execute_command_rejects_unknown_command() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        212,
        "editor/executeCommand",
        serde_json::json!({ "command": "nonexistent_command" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 212);
    assert!(response.error.is_some());
}

#[test]
fn plugin_execute_command_requires_command() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request =
        urvim_plugin::PluginRequest::new(213, "editor/executeCommand", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 213);
    assert!(response.error.unwrap().contains("requires command"));
}

#[test]
fn plugin_close_buffer_closes_view() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::from_str("first"),
        Buffer::from_str("second"),
    ]));
    let active_id = layout.active_buffer_view().buffer_id();
    let request = urvim_plugin::PluginRequest::new(
        214,
        "editor/closeBuffer",
        serde_json::json!({ "buffer_id": active_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 214);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["closed"], serde_json::json!(true));
    assert_eq!(result["buffer_id"], serde_json::json!(active_id.get()));
}

#[test]
fn plugin_close_buffer_returns_closed_false_for_non_visible_buffer() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("visible")]));

    let hidden_id = globals::with_buffer_pool(|pool| {
        let id = pool.create_buffer();
        pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "hidden content");
        });
        id
    });

    let request = urvim_plugin::PluginRequest::new(
        215,
        "editor/closeBuffer",
        serde_json::json!({ "buffer_id": hidden_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 215);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["closed"], serde_json::json!(false));
}

#[test]
fn plugin_close_buffer_requires_buffer_id() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request =
        urvim_plugin::PluginRequest::new(216, "editor/closeBuffer", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 216);
    assert!(response.error.unwrap().contains("requires buffer_id"));
}

#[test]
fn plugin_unload_buffer_refuses_modified_without_force() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "modified",
    )]));
    let buffer_id = layout.active_buffer_view().buffer_id();

    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "x"))
        .unwrap();

    let request = urvim_plugin::PluginRequest::new(
        217,
        "editor/unloadBuffer",
        serde_json::json!({ "buffer_id": buffer_id.get(), "force": false }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 217);
    assert!(response.error.unwrap().contains("unsaved changes"));
}

#[test]
fn plugin_unload_buffer_force_unloads_modified_buffer() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "modified",
    )]));
    drain_editor_events_serial();

    let buffer_id = layout.active_buffer_view().buffer_id();

    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "x"))
        .unwrap();

    let request = urvim_plugin::PluginRequest::new(
        218,
        "editor/unloadBuffer",
        serde_json::json!({ "buffer_id": buffer_id.get(), "force": true }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 218);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["unloaded"], serde_json::json!(true));
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id.get()));
}

#[test]
fn plugin_unload_buffer_requires_buffer_id() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request =
        urvim_plugin::PluginRequest::new(219, "editor/unloadBuffer", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 219);
    assert!(response.error.unwrap().contains("requires buffer_id"));
}

#[test]
fn plugin_get_editor_state_returns_snapshot() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::from_str("first"),
        Buffer::from_str("second"),
    ]));
    let active_id = layout.active_buffer_view().buffer_id().get();
    let request =
        urvim_plugin::PluginRequest::new(220, "editor/getEditorState", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 220);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["active_buffer_id"], serde_json::json!(active_id));
    assert!(result["mode"].as_str().is_some());
    assert!(result["cwd"].as_str().is_some());
    assert!(result["theme"].as_str().is_some());
    let buffers = result["buffers"]
        .as_array()
        .expect("buffers should be an array");
    assert!(buffers.len() >= 2);
    assert!(
        buffers
            .iter()
            .any(|b| b["buffer_id"] == serde_json::json!(active_id)
                && b["active"] == serde_json::json!(true))
    );
}

#[test]
fn plugin_open_buffer_enqueues_buffer_opened_for_hidden_loaded_buffer() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("visible")]));
    drain_editor_events_serial();

    let hidden_id = globals::with_buffer_pool(|pool| {
        let id = pool.create_buffer();
        pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "hidden content");
        });
        id
    });

    let request = urvim_plugin::PluginRequest::new(
        221,
        "editor/openBuffer",
        serde_json::json!({ "buffer_id": hidden_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 221);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["opened"], serde_json::json!(true));

    let events = drain_editor_events_serial();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, urvim_core::event::EditorEvent::BufferOpened { buffer_id } if *buffer_id == hidden_id)),
        "expected BufferOpened event for hidden buffer {hidden_id:?}, got {events:?}"
    );
}

#[test]
fn plugin_open_buffer_does_not_enqueue_buffer_opened_when_already_visible() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::from_str("first"),
        Buffer::from_str("second"),
    ]));
    let active_id = layout.active_buffer_view().buffer_id();
    drain_editor_events_serial();

    let request = urvim_plugin::PluginRequest::new(
        222,
        "editor/openBuffer",
        serde_json::json!({ "buffer_id": active_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 222);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["opened"], serde_json::json!(false));

    let events = drain_editor_events_serial();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, urvim_core::event::EditorEvent::BufferOpened { buffer_id } if *buffer_id == active_id)),
        "did not expect BufferOpened for already-visible buffer {active_id:?}, got {events:?}"
    );
}

#[test]
fn plugin_focus_buffer_open_if_needed_enqueues_buffer_opened() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("visible")]));
    drain_editor_events_serial();

    let hidden_id = globals::with_buffer_pool(|pool| {
        let id = pool.create_buffer();
        pool.with_buffer_mut(id, |buffer| {
            buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "hidden content");
        });
        id
    });

    let request = urvim_plugin::PluginRequest::new(
        223,
        "editor/focusBuffer",
        serde_json::json!({ "buffer_id": hidden_id.get(), "open_if_needed": true }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 223);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["focused"], serde_json::json!(true));
    assert_eq!(result["opened"], serde_json::json!(true));

    let events = drain_editor_events_serial();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, urvim_core::event::EditorEvent::BufferOpened { buffer_id } if *buffer_id == hidden_id)),
        "expected BufferOpened event for hidden buffer {hidden_id:?}, got {events:?}"
    );
}

#[test]
fn plugin_close_buffer_runs_orphan_cleanup() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let unique = format!(
        "urvim-plugin-orphan-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::write(&path, "orphan").unwrap();

    let visible_buffer = Buffer::from_str("visible");
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![visible_buffer]));
    let visible_id = layout.active_buffer_view().buffer_id();

    let orphan_id = globals::with_buffer_pool(|pool| {
        pool.open_buffer(&path)
            .expect("should load orphan file into pool")
    });
    drain_editor_events_serial();

    let request = urvim_plugin::PluginRequest::new(
        224,
        "editor/closeBuffer",
        serde_json::json!({ "buffer_id": visible_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 224);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["closed"], serde_json::json!(true));

    let events = drain_editor_events_serial();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, urvim_core::event::EditorEvent::BufferUnloaded { buffer_id, .. } if *buffer_id == orphan_id)),
        "expected orphan BufferUnloaded event for {orphan_id:?}, got {events:?}"
    );
    assert!(
        globals::with_buffer(orphan_id, |_| ()).is_none(),
        "orphan buffer {orphan_id:?} should have been removed from pool"
    );

    std::fs::remove_file(path).ok();
}

#[test]
fn plugin_execute_command_reports_unhandled() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));

    let request = urvim_plugin::PluginRequest::new(
        225,
        "editor/executeCommand",
        serde_json::json!({ "command": "pane focus-left" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 225);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["handled"], serde_json::json!(false));
    assert_eq!(result["command"], serde_json::json!("pane focus-left"));
}

#[test]
fn plugin_active_buffer_request_returns_metadata() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.set_syntax_name("rust"));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request =
        urvim_plugin::PluginRequest::new(7, "editor/getActiveBuffer", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 7);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id));
    assert_eq!(result["filetype"], serde_json::json!("rust"));
    assert_eq!(result["line_count"], serde_json::json!(1));
}

#[test]
fn plugin_buffer_text_request_returns_content() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "one\ntwo",
    )]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        8,
        "editor/getBufferText",
        serde_json::json!({ "buffer_id": buffer_id }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 8);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id));
    assert_eq!(result["text"], serde_json::json!("one\ntwo"));
}

#[test]
fn plugin_config_request_returns_safe_subset() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let config = Config {
        theme: "Demo Night".to_string(),
        syntax: false,
        tab_width: 2,
        ..Config::default()
    };
    globals::set_config(config);
    let request = urvim_plugin::PluginRequest::new(9, "editor/getConfig", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 9);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["theme"], serde_json::json!("Demo Night"));
    assert_eq!(result["syntax"], serde_json::json!(false));
    assert_eq!(result["tab_width"], serde_json::json!(2));
    assert!(result.get("keymaps").is_none());
}

#[test]
fn plugin_buffer_text_request_errors_for_unknown_buffer() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        10,
        "editor/getBufferText",
        serde_json::json!({ "buffer_id": 999999 }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 10);
    assert!(response.error.unwrap().contains("unknown buffer_id"));
}

#[test]
fn plugin_editor_request_errors_for_unsupported_method() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(11, "editor/mutate", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 11);
    assert!(
        response
            .error
            .unwrap()
            .contains("unsupported editor request")
    );
}

#[test]
fn plugin_register_command_request_updates_contributions() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    let request = urvim_plugin::PluginRequest::new(
        16,
        "editor/registerCommand",
        serde_json::json!({ "name": "echo", "request": "demo/echo", "description": "Echo text" }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    assert_eq!(
        contributions
            .command("demo-plugin", "echo")
            .map(|command| command.request.as_str()),
        Some("demo/echo")
    );
}

#[test]
fn plugin_unregister_command_request_removes_contribution() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    contributions
        .register_command(
            "demo-plugin",
            urvim_plugin::DynamicPluginCommand {
                name: "echo".to_string(),
                description: None,
                request: "demo/echo".to_string(),
            },
        )
        .expect("command should register");
    let request = urvim_plugin::PluginRequest::new(
        17,
        "editor/unregisterCommand",
        serde_json::json!({ "name": "echo" }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    assert!(contributions.command("demo-plugin", "echo").is_none());
}

#[test]
fn plugin_register_script_request_updates_contributions() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    let request = urvim_plugin::PluginRequest::new(
        18,
        "editor/registerScript",
        serde_json::json!({ "name": "notify_write", "commands": ["plugin demo-plugin notify message=writing", "write"], "description": "Notify before writing" }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    assert_eq!(
        contributions
            .script("demo-plugin", "notify_write")
            .map(|script| script.commands.as_slice()),
        Some(
            &[
                "plugin demo-plugin notify message=writing".to_string(),
                "write".to_string()
            ][..]
        )
    );
}

#[test]
fn plugin_unregister_script_request_removes_dynamic_script() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    contributions
        .register_script(
            "demo-plugin",
            urvim_plugin::DynamicPluginScript {
                name: "notify_write".to_string(),
                description: None,
                commands: vec!["write".to_string()],
            },
        )
        .expect("script should register");
    let request = urvim_plugin::PluginRequest::new(
        19,
        "editor/unregisterScript",
        serde_json::json!({ "name": "notify_write" }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    assert!(
        contributions
            .script("demo-plugin", "notify_write")
            .is_none()
    );
}

#[test]
fn plugin_register_script_rejects_manifest_script_conflict() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    contributions.register_static_script(
        "demo-plugin",
        "wq",
        vec!["write".to_string(), "quit".to_string()],
    );
    let request = urvim_plugin::PluginRequest::new(
        20,
        "editor/registerScript",
        serde_json::json!({ "name": "wq", "commands": ["write"] }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.unwrap().contains("manifest script"));
    assert!(contributions.script("demo-plugin", "wq").is_none());
}

#[test]
fn plugin_register_theme_request_loads_theme_and_records_owner() {
    let _pool_guard = buffer_pool_lock();
    let _lock = theme_registry_test_lock();
    let dir = std::env::temp_dir().join(format!("urvim-dynamic-theme-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("theme dir should be created");
    let path = dir.join("dynamic.toml");
    std::fs::write(&path, test_theme_source("Dynamic Plugin Theme"))
        .expect("theme should be written");
    globals::set_theme_registry(
        urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load"),
    );
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    let request = urvim_plugin::PluginRequest::new(
        21,
        "editor/registerTheme",
        serde_json::json!({ "path": path.to_string_lossy() }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    assert!(
        contributions
            .theme("demo-plugin", "Dynamic Plugin Theme")
            .is_some()
    );
    globals::with_theme_registry(|registry| {
        assert!(
            registry
                .expect("registry should be set")
                .get("Dynamic Plugin Theme")
                .is_some()
        );
    });

    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn plugin_unregister_theme_request_removes_owned_theme() {
    let _pool_guard = buffer_pool_lock();
    let _lock = theme_registry_test_lock();
    let registry = urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load");
    let mut registry = registry;
    let theme = urvim_theme::resolve_theme_from_str(
        "dynamic.toml",
        &test_theme_source("Dynamic Plugin Theme"),
    )
    .expect("theme should resolve");
    registry.insert(theme).expect("theme should insert");
    globals::set_theme_registry(registry);
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    contributions
        .register_theme(
            "demo-plugin",
            urvim_plugin::DynamicPluginTheme {
                name: "Dynamic Plugin Theme".to_string(),
                source: urvim_plugin::DynamicPluginThemeSource::File(std::path::PathBuf::from(
                    "dynamic.toml",
                )),
            },
        )
        .expect("theme owner should register");
    let request = urvim_plugin::PluginRequest::new(
        22,
        "editor/unregisterTheme",
        serde_json::json!({ "name": "Dynamic Plugin Theme" }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    assert!(
        contributions
            .theme("demo-plugin", "Dynamic Plugin Theme")
            .is_none()
    );
    globals::with_theme_registry(|registry| {
        assert!(
            registry
                .expect("registry should be set")
                .get("Dynamic Plugin Theme")
                .is_none()
        );
    });
}

#[test]
fn plugin_register_event_hook_request_updates_contributions() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    let request = urvim_plugin::PluginRequest::new(
        23,
        "editor/registerEventHook",
        serde_json::json!({ "event": "BufferSaved", "method": "demo/onBufferSaved" }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    assert_eq!(
        contributions
            .event_hooks("demo-plugin", urvim_plugin::PluginEventKind::BufferSaved)
            .collect::<Vec<_>>(),
        vec![0]
    );
}

#[test]
fn plugin_register_event_hook_rejects_unknown_event() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    let request = urvim_plugin::PluginRequest::new(
        24,
        "editor/registerEventHook",
        serde_json::json!({ "event": "buffer/changed", "method": "demo/onBufferChanged" }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.unwrap().contains("unknown plugin event"));
    assert_eq!(contributions.event_hook_count("demo-plugin"), 0);
}

#[test]
fn plugin_unregister_event_hook_removes_only_calling_plugin_hook() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    contributions
        .register_event_hook(
            "demo-plugin",
            urvim_plugin::PluginEventKind::BufferSaved,
            1,
        )
        .expect("hook should register");
    contributions
        .register_event_hook(
            "other-plugin",
            urvim_plugin::PluginEventKind::BufferSaved,
            1,
        )
        .expect("hook should register");
    let request = urvim_plugin::PluginRequest::new(
        25,
        "editor/unregisterEventHook",
        serde_json::json!({ "hook": 1 }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    assert_eq!(contributions.event_hook_count("demo-plugin"), 0);
    assert_eq!(contributions.event_hook_count("other-plugin"), 1);
}

#[test]
fn plugin_command_response_with_message_requests_notification() {
    let command = urvim_plugin::PluginCommandRequestMetadata {
        command: "echo".to_string(),
        method: "demo/echo".to_string(),
    };
    let response =
        urvim_plugin::PluginResponse::success(2, serde_json::json!({ "message": "echo: hello" }));

    assert!(handle_plugin_command_response(
        "demo-plugin",
        &command,
        &response
    ));
}

#[test]
fn plugin_command_error_response_requests_notification() {
    let command = urvim_plugin::PluginCommandRequestMetadata {
        command: "echo".to_string(),
        method: "demo/echo".to_string(),
    };
    let response = urvim_plugin::PluginResponse::error(2, "boom");

    assert!(handle_plugin_command_response(
        "demo-plugin",
        &command,
        &response
    ));
}

#[test]
fn plugin_command_empty_response_does_not_notify() {
    let command = urvim_plugin::PluginCommandRequestMetadata {
        command: "echo".to_string(),
        method: "demo/echo".to_string(),
    };
    let response = urvim_plugin::PluginResponse::success(2, serde_json::json!({}));

    assert!(!handle_plugin_command_response(
        "demo-plugin",
        &command,
        &response
    ));
}

#[test]
fn plugin_apply_edit_inserts_text() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("world")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        12,
        "editor/applyEdit",
        serde_json::json!({ "buffer_id": buffer_id, "kind": "insert", "start": { "line": 0, "col": 0 }, "text": "hello " }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.is_none());
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some("hello world".to_string())
    );
}

#[test]
fn plugin_save_buffer_saves_file_backed_buffer_and_emits_event() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let unique = format!(
        "urvim-plugin-save-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    let absolute_path = urvim_core::AbsolutePath::from_path(path.as_path())
        .expect("temp path should resolve absolutely");

    let buffer = Buffer::with_path(absolute_path);
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![buffer]));
    let buffer_id = layout.active_buffer_view().buffer_id();
    drain_editor_events_serial();

    let request = urvim_plugin::PluginRequest::new(
        300,
        "editor/saveBuffer",
        serde_json::json!({ "buffer_id": buffer_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 300);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["saved"], serde_json::json!(true));
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id.get()));

    let events = drain_editor_events_serial();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, urvim_core::event::EditorEvent::BufferSaved { buffer_id: id } if *id == buffer_id)),
        "expected BufferSaved event for buffer {buffer_id:?}, got {events:?}"
    );

    std::fs::remove_file(path).ok();
}

#[test]
fn plugin_save_buffer_save_as_writes_path_and_emits_event() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let unique = format!(
        "urvim-plugin-save-as-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);

    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("save me")]));
    let buffer_id = layout.active_buffer_view().buffer_id();
    drain_editor_events_serial();

    let request = urvim_plugin::PluginRequest::new(
        301,
        "editor/saveBuffer",
        serde_json::json!({ "buffer_id": buffer_id.get(), "path": path.to_string_lossy() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 301);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["saved"], serde_json::json!(true));
    assert!(result["path"].as_str().is_some());

    let saved_text = std::fs::read_to_string(&path).expect("saved file should be readable");
    assert_eq!(saved_text, "save me");

    let events = drain_editor_events_serial();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, urvim_core::event::EditorEvent::BufferSaved { buffer_id: id } if *id == buffer_id)),
        "expected BufferSaved event, got {events:?}"
    );

    std::fs::remove_file(path).ok();
}

#[test]
fn plugin_save_buffer_rejects_unknown_buffer() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        302,
        "editor/saveBuffer",
        serde_json::json!({ "buffer_id": 999999 }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 302);
    assert!(response.error.unwrap().contains("unknown buffer_id"));
}

#[test]
fn plugin_save_buffer_rejects_unnamed_without_path() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let buffer_id = layout.active_buffer_view().buffer_id();
    let request = urvim_plugin::PluginRequest::new(
        303,
        "editor/saveBuffer",
        serde_json::json!({ "buffer_id": buffer_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 303);
    assert!(response.error.unwrap().contains("requires path"));
}

#[test]
fn plugin_set_buffer_filetype_sets_filetype() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let buffer_id = layout.active_buffer_view().buffer_id();
    let request = urvim_plugin::PluginRequest::new(
        304,
        "editor/setBufferFiletype",
        serde_json::json!({ "buffer_id": buffer_id.get(), "filetype": "rust" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 304);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["changed"], serde_json::json!(true));
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id.get()));
}

#[test]
fn plugin_set_buffer_filetype_enqueues_event() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let buffer_id = layout.active_buffer_view().buffer_id();
    drain_editor_events_serial();

    let request = urvim_plugin::PluginRequest::new(
        305,
        "editor/setBufferFiletype",
        serde_json::json!({ "buffer_id": buffer_id.get(), "filetype": "rust" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);
    assert!(response.error.is_none());

    let events = drain_editor_events_serial();
    let matching_events = events
        .iter()
        .filter(|event| matches!(
            event,
            urvim_core::event::EditorEvent::BufferFiletypeChanged { buffer_id: id } if *id == buffer_id
        ))
        .count();
    assert_eq!(
        matching_events, 1,
        "expected exactly one BufferFiletypeChanged event for {buffer_id:?}, got {events:?}"
    );
}

#[test]
fn plugin_set_buffer_filetype_requires_filetype() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let buffer_id = layout.active_buffer_view().buffer_id();
    let request = urvim_plugin::PluginRequest::new(
        306,
        "editor/setBufferFiletype",
        serde_json::json!({ "buffer_id": buffer_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 306);
    assert!(response.error.unwrap().contains("requires filetype"));
}

#[test]
fn plugin_set_buffer_filetype_rejects_unknown_buffer() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        307,
        "editor/setBufferFiletype",
        serde_json::json!({ "buffer_id": 999999, "filetype": "rust" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 307);
    assert!(response.error.unwrap().contains("unknown buffer_id"));
}

#[test]
fn plugin_set_buffer_filetype_rejects_unknown_filetype_without_event() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let buffer_id = layout.active_buffer_view().buffer_id();
    drain_editor_events_serial();
    let request = urvim_plugin::PluginRequest::new(
        324,
        "editor/setBufferFiletype",
        serde_json::json!({ "buffer_id": buffer_id.get(), "filetype": "not-a-real-filetype" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 324);
    assert!(response.error.unwrap().contains("unknown filetype"));
    let events = drain_editor_events_serial();
    assert!(events.iter().all(|event| !matches!(
        event,
        urvim_core::event::EditorEvent::BufferFiletypeChanged { buffer_id: id } if *id == buffer_id
    )));
}

#[test]
fn plugin_list_themes_returns_registered_themes_and_active() {
    let _guard = theme_registry_test_lock();
    globals::set_theme_registry(
        urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load"),
    );
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(308, "editor/listThemes", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 308);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let themes = result["themes"]
        .as_array()
        .expect("themes should be an array");
    assert!(!themes.is_empty());
    assert!(
        themes
            .iter()
            .any(|t| t["name"] == serde_json::json!("Friday Night"))
    );
}

#[test]
fn plugin_set_theme_updates_active_theme() {
    let _guard = theme_registry_test_lock();
    globals::set_theme_registry(
        urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load"),
    );
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        309,
        "editor/setTheme",
        serde_json::json!({ "name": "Nord" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 309);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["name"], serde_json::json!("Nord"));
    assert_eq!(result["active"], serde_json::json!(true));

    let list_request =
        urvim_plugin::PluginRequest::new(310, "editor/listThemes", serde_json::json!({}));
    let list_response = resolve_test_plugin_editor_request(&mut layout, &list_request);
    let list_result = list_response
        .result
        .expect("response should include result");
    assert_eq!(list_result["active"], serde_json::json!("Nord"));
}

#[test]
fn plugin_set_theme_rejects_unknown_theme() {
    let _guard = theme_registry_test_lock();
    globals::set_theme_registry(
        urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load"),
    );
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        311,
        "editor/setTheme",
        serde_json::json!({ "name": "NonexistentTheme" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 311);
    assert!(response.error.unwrap().contains("unknown theme"));
}

#[test]
fn plugin_list_commands_includes_builtin_roots() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request =
        urvim_plugin::PluginRequest::new(312, "editor/listCommands", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 312);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let commands = result["commands"]
        .as_array()
        .expect("commands should be an array");
    let names: Vec<&str> = commands.iter().filter_map(|c| c["name"].as_str()).collect();
    assert!(
        names.contains(&"buffer"),
        "expected builtin 'buffer', got {names:?}"
    );
    assert!(
        names.contains(&"action"),
        "expected builtin 'action', got {names:?}"
    );
    assert!(
        names.contains(&"pick"),
        "expected builtin 'pick', got {names:?}"
    );
    assert!(
        names.contains(&"app"),
        "expected builtin 'app', got {names:?}"
    );
    assert!(
        names.contains(&"plugin"),
        "expected builtin 'plugin', got {names:?}"
    );
}

#[test]
fn plugin_list_commands_includes_configured_alias_and_script() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request =
        urvim_plugin::PluginRequest::new(313, "editor/listCommands", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 313);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let commands = result["commands"]
        .as_array()
        .expect("commands should be an array");
    let builtin_roots: Vec<&str> = ["buffer", "action", "pick", "lsp", "pane", "app", "plugin"]
        .into_iter()
        .collect();
    for root in &builtin_roots {
        let found = commands.iter().any(|c| {
            c["name"] == serde_json::json!(root) && c["kind"] == serde_json::json!("builtin")
        });
        assert!(found, "expected builtin root '{root}'");
    }
    let write_alias = commands
        .iter()
        .find(|c| c["name"] == serde_json::json!("write"));
    assert!(write_alias.is_some(), "expected builtin alias 'write'");
    assert_eq!(write_alias.unwrap()["kind"], serde_json::json!("alias"));
}

#[test]
fn plugin_list_commands_includes_dynamic_plugin_command_and_script() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    contributions
        .register_command(
            "demo-plugin",
            urvim_plugin::DynamicPluginCommand {
                name: "echo".to_string(),
                description: Some("Echo text".to_string()),
                request: "demo/echo".to_string(),
            },
        )
        .expect("command should register");
    contributions
        .register_script(
            "demo-plugin",
            urvim_plugin::DynamicPluginScript {
                name: "notify_write".to_string(),
                description: None,
                commands: vec!["write".to_string()],
            },
        )
        .expect("script should register");
    let request =
        urvim_plugin::PluginRequest::new(314, "editor/listCommands", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let commands = result["commands"]
        .as_array()
        .expect("commands should be an array");

    let echo_cmd = commands
        .iter()
        .find(|c| c["name"] == serde_json::json!("plugin demo-plugin echo"));
    assert!(echo_cmd.is_some(), "expected plugin command 'echo'");
    assert_eq!(
        echo_cmd.unwrap()["kind"],
        serde_json::json!("pluginCommand")
    );

    let script_cmd = commands
        .iter()
        .find(|c| c["name"] == serde_json::json!("plugin demo-plugin notify_write"));
    assert!(
        script_cmd.is_some(),
        "expected plugin script 'notify_write'"
    );
    assert_eq!(
        script_cmd.unwrap()["kind"],
        serde_json::json!("pluginScript")
    );
}

#[test]
fn plugin_list_plugin_contributions_returns_dynamic_contributions() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    contributions
        .register_command(
            "demo-plugin",
            urvim_plugin::DynamicPluginCommand {
                name: "echo".to_string(),
                description: Some("Echo text".to_string()),
                request: "demo/echo".to_string(),
            },
        )
        .expect("command should register");
    contributions
        .register_event_hook(
            "demo-plugin",
            urvim_plugin::PluginEventKind::BufferSaved,
            1,
        )
        .expect("hook should register");
    let request = urvim_plugin::PluginRequest::new(
        315,
        "editor/listPluginContributions",
        serde_json::json!({}),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let plugins = result["plugins"]
        .as_array()
        .expect("plugins should be an array");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0]["plugin"], serde_json::json!("demo-plugin"));
    let commands = plugins[0]["commands"].as_array().unwrap();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0]["name"], serde_json::json!("echo"));
    let hooks = plugins[0]["event_hooks"].as_array().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0]["event"], serde_json::json!("BufferSaved"));
}

#[test]
fn plugin_list_plugin_contributions_filters_by_plugin() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    contributions
        .register_command(
            "plugin-a",
            urvim_plugin::DynamicPluginCommand {
                name: "cmd-a".to_string(),
                description: None,
                request: "a/cmd".to_string(),
            },
        )
        .expect("command should register");
    contributions
        .register_command(
            "plugin-b",
            urvim_plugin::DynamicPluginCommand {
                name: "cmd-b".to_string(),
                description: None,
                request: "b/cmd".to_string(),
            },
        )
        .expect("command should register");
    let request = urvim_plugin::PluginRequest::new(
        316,
        "editor/listPluginContributions",
        serde_json::json!({ "plugin": "plugin-a" }),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "plugin-a",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let plugins = result["plugins"]
        .as_array()
        .expect("plugins should be an array");
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0]["plugin"], serde_json::json!("plugin-a"));
}

#[test]
fn plugin_list_plugin_contributions_returns_empty_for_no_contributions() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    let request = urvim_plugin::PluginRequest::new(
        317,
        "editor/listPluginContributions",
        serde_json::json!({}),
    );

    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let plugins = result["plugins"]
        .as_array()
        .expect("plugins should be an array");
    assert!(plugins.is_empty());
}

#[test]
fn plugin_get_visible_ranges_returns_active_pane_range() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let request =
        urvim_plugin::PluginRequest::new(318, "editor/getVisibleRanges", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 318);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let ranges = result["ranges"]
        .as_array()
        .expect("ranges should be an array");
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0]["active"], serde_json::json!(true));
    assert!(ranges[0]["pane_id"].as_u64().is_some());
    assert!(ranges[0]["buffer_id"].as_u64().is_some());
}

#[test]
fn plugin_get_visible_ranges_filters_by_buffer() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::from_str("first"),
        Buffer::from_str("second"),
    ]));
    let first_id = layout.active_buffer_view().buffer_id();

    let request = urvim_plugin::PluginRequest::new(
        319,
        "editor/getVisibleRanges",
        serde_json::json!({ "buffer_id": first_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 319);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let ranges = result["ranges"]
        .as_array()
        .expect("ranges should be an array");
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0]["buffer_id"], serde_json::json!(first_id.get()));
}

#[test]
fn plugin_get_visible_ranges_rejects_unknown_buffer() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let request = urvim_plugin::PluginRequest::new(
        320,
        "editor/getVisibleRanges",
        serde_json::json!({ "buffer_id": 999999 }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 320);
    assert!(response.error.unwrap().contains("unknown buffer_id"));
}

#[test]
fn plugin_get_pane_state_returns_single_pane() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id();
    let request =
        urvim_plugin::PluginRequest::new(321, "editor/getPaneState", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 321);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["focused_pane_id"], serde_json::json!(0));
    let panes = result["panes"]
        .as_array()
        .expect("panes should be an array");
    assert_eq!(panes.len(), 1);
    assert_eq!(panes[0]["pane_id"], serde_json::json!(0));
    assert_eq!(panes[0]["focused"], serde_json::json!(true));
    assert_eq!(
        panes[0]["active_buffer_id"],
        serde_json::json!(buffer_id.get())
    );
}

#[test]
fn plugin_get_pane_state_returns_split_panes() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    process_intent_queue(
        &mut layout,
        vec![Intent::Command(urvim_core::ui::Command::SplitVertical)],
    );

    let request =
        urvim_plugin::PluginRequest::new(322, "editor/getPaneState", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 322);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let panes = result["panes"]
        .as_array()
        .expect("panes should be an array");
    assert_eq!(panes.len(), 2);
    let focused_count = panes
        .iter()
        .filter(|p| p["focused"] == serde_json::json!(true))
        .count();
    assert_eq!(focused_count, 1);
}

#[test]
fn plugin_get_pane_state_includes_tabs_and_active_tab() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::from_str("first"),
        Buffer::from_str("second"),
    ]));
    let first_id = layout.active_buffer_view().buffer_id();
    process_intent_queue(
        &mut layout,
        vec![Intent::Command(Command::NextTab(1))],
    );
    let second_id = layout.active_buffer_view().buffer_id();

    let request =
        urvim_plugin::PluginRequest::new(323, "editor/getPaneState", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 323);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let panes = result["panes"]
        .as_array()
        .expect("panes should be an array");
    assert_eq!(panes.len(), 1);
    let tabs = panes[0]["tabs"]
        .as_array()
        .expect("tabs should be an array");
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0]["buffer_id"], serde_json::json!(first_id.get()));
    assert_eq!(tabs[0]["active"], serde_json::json!(false));
    assert_eq!(tabs[1]["buffer_id"], serde_json::json!(second_id.get()));
    assert_eq!(tabs[1]["active"], serde_json::json!(true));
    assert_eq!(panes[0]["active_tab_index"], serde_json::json!(1));
}

fn diagnostics_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn clear_diagnostics_store_for_test() {
    globals::with_diagnostics_store(|store| {
        store.clear_all();
    });
}

fn lsp_diagnostic(
    line: u32,
    start: u32,
    end: u32,
    severity: lsp_types::DiagnosticSeverity,
    source: &str,
    message: &str,
) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range: lsp_types::Range::new(
            lsp_types::Position::new(line, start),
            lsp_types::Position::new(line, end),
        ),
        severity: Some(severity),
        code: None,
        code_description: None,
        source: Some(source.to_string()),
        message: message.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}

#[test]
fn plugin_list_diagnostics_returns_sorted_payloads() {
    let _guard = diagnostics_test_lock();
    let _pool_guard = buffer_pool_lock();
    clear_diagnostics_store_for_test();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "hello world",
    )]));
    let buffer_id = layout.active_buffer_view().buffer_id();

    globals::with_diagnostics_store(|store| {
        store.set(
            buffer_id,
            "test-server",
            vec![
                lsp_diagnostic(
                    0,
                    6,
                    11,
                    lsp_types::DiagnosticSeverity::WARNING,
                    "rust-analyzer",
                    "unused variable",
                ),
                lsp_diagnostic(
                    0,
                    0,
                    5,
                    lsp_types::DiagnosticSeverity::ERROR,
                    "rust-analyzer",
                    "mismatched types",
                ),
            ],
        );
    });

    let request = urvim_plugin::PluginRequest::new(
        100,
        "editor/listDiagnostics",
        serde_json::json!({ "buffer_id": buffer_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.is_none());
    let result = response.result.unwrap();
    assert_eq!(result["buffer_id"], buffer_id.get());
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0]["severity"], "error");
    assert_eq!(diagnostics[0]["source"], "rust-analyzer");
    assert_eq!(diagnostics[0]["message"], "mismatched types");
    assert_eq!(diagnostics[1]["severity"], "warning");
    assert_eq!(diagnostics[1]["source"], "rust-analyzer");
    assert_eq!(diagnostics[1]["message"], "unused variable");
}

#[test]
fn plugin_list_diagnostics_filters_by_severity() {
    let _guard = diagnostics_test_lock();
    let _pool_guard = buffer_pool_lock();
    clear_diagnostics_store_for_test();

    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id();

    globals::with_diagnostics_store(|store| {
        store.set(
            buffer_id,
            "test-server",
            vec![
                lsp_diagnostic(
                    0,
                    0,
                    5,
                    lsp_types::DiagnosticSeverity::ERROR,
                    "lsp",
                    "error msg",
                ),
                lsp_diagnostic(
                    0,
                    0,
                    5,
                    lsp_types::DiagnosticSeverity::WARNING,
                    "lsp",
                    "warning msg",
                ),
            ],
        );
    });

    let request = urvim_plugin::PluginRequest::new(
        101,
        "editor/listDiagnostics",
        serde_json::json!({ "buffer_id": buffer_id.get(), "severity": "error" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.is_none());
    let result = response.result.unwrap();
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["severity"], "error");
}

#[test]
fn plugin_list_diagnostics_rejects_unknown_severity() {
    let _guard = diagnostics_test_lock();
    let _pool_guard = buffer_pool_lock();
    clear_diagnostics_store_for_test();

    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id();

    let request = urvim_plugin::PluginRequest::new(
        102,
        "editor/listDiagnostics",
        serde_json::json!({ "buffer_id": buffer_id.get(), "severity": "fatal" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.unwrap().contains("severity must be one of"));
}

#[test]
fn plugin_get_diagnostic_counts_returns_counts() {
    let _guard = diagnostics_test_lock();
    let _pool_guard = buffer_pool_lock();
    clear_diagnostics_store_for_test();

    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id();

    globals::with_diagnostics_store(|store| {
        store.set(
            buffer_id,
            "test-server",
            vec![
                lsp_diagnostic(0, 0, 1, lsp_types::DiagnosticSeverity::ERROR, "lsp", "e1"),
                lsp_diagnostic(0, 1, 2, lsp_types::DiagnosticSeverity::ERROR, "lsp", "e2"),
                lsp_diagnostic(0, 2, 3, lsp_types::DiagnosticSeverity::WARNING, "lsp", "w1"),
                lsp_diagnostic(
                    0,
                    3,
                    4,
                    lsp_types::DiagnosticSeverity::INFORMATION,
                    "lsp",
                    "i1",
                ),
                lsp_diagnostic(0, 4, 5, lsp_types::DiagnosticSeverity::HINT, "lsp", "h1"),
                lsp_diagnostic(0, 5, 6, lsp_types::DiagnosticSeverity::HINT, "lsp", "h2"),
            ],
        );
    });

    let request = urvim_plugin::PluginRequest::new(
        103,
        "editor/getDiagnosticCounts",
        serde_json::json!({ "buffer_id": buffer_id.get() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.is_none());
    let result = response.result.unwrap();
    assert_eq!(result["buffer_id"], buffer_id.get());
    assert_eq!(result["error"], 2);
    assert_eq!(result["warning"], 1);
    assert_eq!(result["info"], 1);
    assert_eq!(result["hint"], 2);
}

#[test]
fn plugin_get_diagnostics_at_cursor_returns_matching_diagnostics() {
    let _guard = diagnostics_test_lock();
    let _pool_guard = buffer_pool_lock();
    clear_diagnostics_store_for_test();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "hello world",
    )]));
    let buffer_id = layout.active_buffer_view().buffer_id();

    globals::with_diagnostics_store(|store| {
        store.set(
            buffer_id,
            "test-server",
            vec![
                lsp_diagnostic(
                    0,
                    0,
                    5,
                    lsp_types::DiagnosticSeverity::ERROR,
                    "lsp",
                    "covers 0..5",
                ),
                lsp_diagnostic(
                    0,
                    6,
                    11,
                    lsp_types::DiagnosticSeverity::WARNING,
                    "lsp",
                    "covers 6..11",
                ),
            ],
        );
    });

    let request = urvim_plugin::PluginRequest::new(
        104,
        "editor/getDiagnosticsAtCursor",
        serde_json::json!({ "buffer_id": buffer_id.get(), "cursor": { "line": 0, "col": 2 } }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.is_none());
    let result = response.result.unwrap();
    assert_eq!(result["buffer_id"], buffer_id.get());
    let diagnostics = result["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["message"], "covers 0..5");
}

#[test]
fn plugin_get_diagnostics_at_cursor_rejects_invalid_cursor() {
    let _guard = diagnostics_test_lock();
    let _pool_guard = buffer_pool_lock();
    clear_diagnostics_store_for_test();

    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id();

    let request = urvim_plugin::PluginRequest::new(
        105,
        "editor/getDiagnosticsAtCursor",
        serde_json::json!({ "buffer_id": buffer_id.get(), "cursor": { "line": 99, "col": 0 } }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.unwrap().contains("invalid cursor"));
}

#[test]
fn diagnostics_changed_event_dispatches_counts() {
    let _guard = diagnostics_test_lock();
    let _pool_guard = buffer_pool_lock();
    clear_diagnostics_store_for_test();

    let buffer_id = BufferId::new(42);

    globals::with_diagnostics_store(|store| {
        store.set(
            buffer_id,
            "test-server",
            vec![
                lsp_diagnostic(
                    0,
                    0,
                    1,
                    lsp_types::DiagnosticSeverity::ERROR,
                    "lsp",
                    "error",
                ),
                lsp_diagnostic(
                    0,
                    1,
                    2,
                    lsp_types::DiagnosticSeverity::WARNING,
                    "lsp",
                    "warning",
                ),
            ],
        );
    });

    let payload = diagnostics_changed_payload(buffer_id);

    assert_eq!(payload["event"], "DiagnosticsChanged");
    assert_eq!(payload["buffer_id"], buffer_id.get());
    assert_eq!(payload["counts"]["error"], 1);
    assert_eq!(payload["counts"]["warning"], 1);
    assert_eq!(payload["counts"]["info"], 0);
    assert_eq!(payload["counts"]["hint"], 0);
}

#[test]
fn plugin_apply_edit_replaces_text() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "hello old",
    )]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        13,
        "editor/applyEdit",
        serde_json::json!({ "buffer_id": buffer_id, "kind": "replace", "start": { "line": 0, "col": 6 }, "end": { "line": 0, "col": 9 }, "text": "new" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.is_none());
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some("hello new".to_string())
    );
}

#[test]
fn plugin_apply_edit_rejects_invalid_range_without_mutating() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        14,
        "editor/applyEdit",
        serde_json::json!({ "buffer_id": buffer_id, "kind": "delete", "start": { "line": 0, "col": 99 }, "end": { "line": 0, "col": 100 } }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.unwrap().contains("invalid start cursor"));
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some("hello".to_string())
    );
}

#[test]
fn plugin_apply_edit_participates_in_undo() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        15,
        "editor/applyEdit",
        serde_json::json!({ "buffer_id": buffer_id, "kind": "insert", "start": { "line": 0, "col": 5 }, "text": "!" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert!(response.error.is_none());
    assert_eq!(
        layout
            .active_buffer_view_mut()
            .with_buffer_mut(|buffer| buffer.undo())
            .flatten(),
        Some(urvim_core::buffer::Cursor::new(0, 0))
    );
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some("hello".to_string())
    );
}

#[test]
fn insert_exit_commits_single_undo_snapshot_when_text_changes() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("world")]));

    assert!(
        layout
            .active_buffer_view_mut()
            .with_buffer_mut(
                |buffer| buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "hello")
            )
            .is_some()
    );
    layout
        .active_buffer_view_mut()
        .set_cursor(urvim_core::buffer::Cursor::new(0, 5));

    commit_insert_exit_snapshot(&mut layout);

    assert!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.can_undo())
            .unwrap_or(false)
    );
    assert_eq!(
        layout
            .active_buffer_view_mut()
            .with_buffer_mut(|buffer| buffer.undo())
            .flatten(),
        Some(urvim_core::buffer::Cursor::new(0, 0))
    );
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str().to_string()),
        Some("world".to_string())
    );
}

#[test]
fn insert_exit_does_not_commit_snapshot_without_text_changes() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("world")]));

    commit_insert_exit_snapshot(&mut layout);

    assert!(
        !layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.can_undo())
            .unwrap_or(true)
    );
}

#[test]
fn select_active_theme_defaults_to_friday_night() {
    let registry = urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load");

    let theme = select_active_theme(&registry, None).expect("default theme should exist");

    assert_eq!(theme.name(), "Friday Night");
}

#[test]
fn select_active_theme_can_select_nord() {
    let registry = urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load");

    let theme = select_active_theme(&registry, Some("Nord")).expect("Nord theme should exist");

    assert_eq!(theme.name(), "Nord");
}

#[test]
fn select_active_theme_reports_unknown_theme() {
    let registry = urvim_theme::ThemeRegistry::load_builtin().expect("builtins should load");

    let error =
        select_active_theme(&registry, Some("missing")).expect_err("unknown theme should fail");

    assert!(error.contains("missing"));
    assert!(error.contains("Friday Night"));
}

#[test]
fn load_startup_plugins_and_themes_loads_plugin_theme_and_scripts() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "urvim-startup-plugin-test-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("themes")).expect("plugin dirs should be created");
    std::fs::write(
        root.join("urvim-plugin.toml"),
        r#"
name = "test-plugin"
version = "0.1.0"

[scripts]
wq = ["write", "quit"]
"#,
    )
    .expect("manifest should be written");
    std::fs::write(
        root.join("themes/test-theme.toml"),
        r##"
name = "Test Theme"

[palette]
bg = "#101010"
fg = "#eeeeee"

[default]
fg = "fg"
bg = "bg"
"##,
    )
    .expect("theme should be written");

    let config = Config {
        theme: "Test Theme".to_string(),
        plugins: std::collections::BTreeMap::from([(
            "test-plugin".to_string(),
            urvim_core::config::PluginConfig {
                enabled: true,
                path: root.clone(),
            },
        )]),
        ..Config::default()
    };

    let startup = load_startup_plugins_and_themes(&config, shared_test_layout())
        .expect("startup plugins should load");

    assert_eq!(startup.active_theme.name(), "Test Theme");
    assert!(startup.theme_registry.get("Test Theme").is_some());
    assert!(startup.plugin_registry.get("test-plugin").is_some());
    assert_eq!(
        startup
            .plugin_registry
            .script("test-plugin", "wq")
            .map(|script| script.len()),
        Some(2)
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn load_startup_plugins_and_themes_skips_disabled_missing_plugins() {
    let config = Config {
        plugins: std::collections::BTreeMap::from([(
            "missing-demo".to_string(),
            urvim_core::config::PluginConfig {
                enabled: false,
                path: std::path::PathBuf::from("/tmp/urvim-missing-disabled-plugin"),
            },
        )]),
        ..Config::default()
    };

    let startup = load_startup_plugins_and_themes(&config, shared_test_layout())
        .expect("disabled missing plugin should not fail startup");

    assert_eq!(startup.active_theme.name(), "Friday Night");
    assert!(startup.plugin_registry.is_empty());
}

#[test]
fn bearscript_plugin_command_uses_function_reference_callback() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "urvim-command-callback-test-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("plugin dir should be created");
    std::fs::write(
        root.join("urvim-plugin.toml"),
        r#"
name = "callback-plugin"
version = "0.1.0"
entry = "plugin.bear"
"#,
    )
    .expect("manifest should be written");
    std::fs::write(
        root.join("plugin.bear"),
        r#"
fn init() {
    urvim.commands.register("hello", hello, "Say hello")
}

fn hello(args) {
    urvim.ui.show_message("hello " + args[0], { "level": "info" })
}
"#,
    )
    .expect("plugin should be written");

    let registry = urvim_plugin::PluginRegistry::load_from_config(
        &std::collections::BTreeMap::from([(
            "callback-plugin".to_string(),
            urvim_core::config::PluginConfig {
                enabled: true,
                path: root.clone(),
            },
        )]),
    )
    .expect("plugin registry should load");
    let layout = shared_test_layout();
    let mut runtime = BearscriptPluginRuntime::load_from_registry(&registry, layout);

    runtime
        .run_command("callback-plugin", "hello", &["bear".to_string()])
        .expect("command should run");

    let notifications = globals::take_notifications();
    assert!(notifications
        .iter()
        .any(|notification| notification.message == "hello bear"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn bearscript_plugin_event_hook_uses_function_reference_callback() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "urvim-event-callback-test-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("plugin dir should be created");
    std::fs::write(
        root.join("urvim-plugin.toml"),
        r#"
name = "event-plugin"
version = "0.1.0"
entry = "plugin.bear"
"#,
    )
    .expect("manifest should be written");
    std::fs::write(
        root.join("plugin.bear"),
        r#"
fn init() {
    urvim.register_event_hook(urvim.events.BufferSaved, on_saved)
}

fn on_saved(event) {
    urvim.ui.show_message("saved " + event["buffer_id"], { "level": "info" })
}
"#,
    )
    .expect("plugin should be written");

    let registry = urvim_plugin::PluginRegistry::load_from_config(
        &std::collections::BTreeMap::from([(
            "event-plugin".to_string(),
            urvim_core::config::PluginConfig {
                enabled: true,
                path: root.clone(),
            },
        )]),
    )
    .expect("plugin registry should load");
    let mut runtime = BearscriptPluginRuntime::load_from_registry(&registry, shared_test_layout());

    assert!(runtime.dispatch_editor_event(urvim_core::event::EditorEvent::BufferSaved {
        buffer_id: BufferId::new(9),
    }));

    let notifications = globals::take_notifications();
    assert!(notifications
        .iter()
        .any(|notification| notification.message == "saved 9"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn bearscript_plugin_event_hook_supports_anonymous_callback_and_unregister_id() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "urvim-event-unregister-test-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("plugin dir should be created");
    std::fs::write(
        root.join("urvim-plugin.toml"),
        r#"
name = "unregister-plugin"
version = "0.1.0"
entry = "plugin.bear"
"#,
    )
    .expect("manifest should be written");
    std::fs::write(
        root.join("plugin.bear"),
        r#"
fn init() {
    let hook = urvim.register_event_hook(urvim.events.BufferSaved, fn(event) {
        urvim.ui.show_message("should not run", { "level": "info" })
    })
    urvim.unregister_event_hook(hook)
}
"#,
    )
    .expect("plugin should be written");

    let registry = urvim_plugin::PluginRegistry::load_from_config(
        &std::collections::BTreeMap::from([(
            "unregister-plugin".to_string(),
            urvim_core::config::PluginConfig {
                enabled: true,
                path: root.clone(),
            },
        )]),
    )
    .expect("plugin registry should load");
    let mut runtime = BearscriptPluginRuntime::load_from_registry(&registry, shared_test_layout());

    assert!(!runtime.dispatch_editor_event(urvim_core::event::EditorEvent::BufferSaved {
        buffer_id: BufferId::new(9),
    }));

    let notifications = globals::take_notifications();
    assert!(!notifications
        .iter()
        .any(|notification| notification.message == "should not run"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn terminal_event_adapter_converts_event_variants() {
    let key_event = Event::Key(urvim_terminal::Key::new(urvim_terminal::KeyCode::Char('x')));
    assert!(
        matches!(UiEvent::from(key_event), UiEvent::Key(key) if key.code == urvim_terminal::KeyCode::Char('x'))
    );

    let resize_event = Event::Resize(40, 120);
    assert_eq!(UiEvent::from(resize_event), UiEvent::Resize(40, 120));

    let paste_event = Event::Paste("abc".to_string());
    assert_eq!(
        UiEvent::from(paste_event),
        UiEvent::Paste("abc".to_string())
    );

    assert_eq!(UiEvent::from(Event::Tick), UiEvent::Tick);
}

#[test]
fn process_intent_queue_dispatches_actions() {
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::new(),
        Buffer::new(),
    ]));

    assert!(process_intent_queue(
        &mut layout,
        vec![Intent::Command(Command::NextTab(1))]
    ));
    assert_eq!(layout.window_group().active_tab_index(), 1);
}

#[test]
fn process_intent_queue_records_repeat_state_for_command_actions() {
    let _pool_guard = buffer_pool_lock();
    let _guard = repeat_state_lock();
    globals::set_last_repeat(globals::RepeatState {
        action: EditorAction::new(EditorOperation::MoveDown),
        count: 99,
        insert_text: Some("stale".to_string()),
    });
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "alpha\nbeta",
    )]));

    assert!(process_intent_queue(
        &mut layout,
        vec![Intent::Editor(EditorAction::new(EditorOperation::DeleteLine))]
    ));

    let repeat = globals::get_last_repeat().expect("repeat state should be recorded");
    assert!(matches!(
        repeat.action.kind.as_ref(),
        Some(EditorOperation::DeleteLine)
    ));
    assert_eq!(repeat.count, 1);
    assert!(repeat.insert_text.is_none());

    let cursor = layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.undo())
        .flatten()
        .expect("undo should restore the deleted line");
    layout.active_buffer_view_mut().set_cursor_synced(cursor);
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str())
            .expect("buffer should be available"),
        "alpha\nbeta"
    );
}

#[test]
fn process_intent_queue_returns_false_when_any_intent_is_unhandled() {
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));

    let handled = process_intent_queue(
        &mut layout,
        vec![
            Intent::Command(urvim_core::ui::Command::EnqueueNotification {
                level: urvim_core::notification::NotificationLevel::Info,
                message: "queued".to_string(),
            }),
            Intent::Editor(EditorAction::new(EditorOperation::VisualTextObject(
                urvim_core::editor::TextObject::InnerWord,
            ))),
        ],
    );

    assert!(!handled);
}

#[test]
fn confirmed_try_quit_flows_through_ui_result_handling_and_exits() {
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
    let cursor = urvim_core::buffer::Cursor::new(0, 1);
    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(cursor, "x"));

    assert!(layout.dispatch_intent(&Intent::Command(urvim_core::ui::Command::TryQuit)));
    assert!(!layout.should_exit());

    let ui_result = layout.route_ui_event(&UiEvent::Key(Key::new(KeyCode::Char('y'))));
    assert!(handle_ui_result(&mut layout, ui_result));
    assert!(layout.should_exit());
}

#[test]
fn handle_save_buffer_action_emits_success_notification() {
    let _guard = notification_test_lock();
    globals::clear_notifications();

    let unique = format!(
        "urvim-save-success-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    let absolute_path = urvim_core::AbsolutePath::from_path(path.as_path())
        .expect("temp path should resolve absolutely");

    let mut buffer = Buffer::with_path(absolute_path);
    buffer.insert_text(urvim_core::buffer::Cursor::new(0, 0), "hello");

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![buffer]));
    assert!(handle_save_buffer_action(&mut layout, None, false));

    let saved_text = std::fs::read_to_string(path).expect("saved file should be readable");
    assert_eq!(saved_text, "hello");
}

#[test]
fn handle_save_buffer_action_prompts_when_disk_changed() {
    let _pool_guard = buffer_pool_lock();
    let _notification_guard = notification_test_lock();
    globals::clear_notifications();

    let unique = format!(
        "urvim-save-confirm-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::write(&path, "alpha").unwrap();
    let buffer = Buffer::load_from_file(&path).unwrap();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![buffer]));
    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| {
            buffer.insert_text(urvim_core::buffer::Cursor::new(0, 5), "-dirty")
        })
        .unwrap();
    std::fs::write(&path, "alpha-external").unwrap();

    let buffer_id = layout.active_buffer_view().buffer_id();
    assert!(handle_save_buffer_action(
        &mut layout,
        Some(buffer_id),
        false
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha-external");

    let ui_result = layout.route_ui_event(&UiEvent::Key(Key::new(KeyCode::Enter)));
    assert!(handle_ui_result(&mut layout, ui_result));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha-dirty");

    std::fs::remove_file(path).ok();
}

#[test]
fn handle_save_buffer_action_emits_error_notification_for_missing_buffer() {
    let _guard = notification_test_lock();
    globals::clear_notifications();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));

    assert!(handle_save_buffer_action(
        &mut layout,
        Some(BufferId::new(usize::MAX)),
        false
    ));
}

#[test]
fn try_quit_saves_session_before_layout_is_cleared() {
    let _guard = cwd_lock();
    let _buffer_guard = buffer_pool_lock();
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-try-quit-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();

    (|| {
        urvim_core::session::set_enabled(true);

        let mut layout =
            urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
        assert!(process_intent_queue(
            &mut layout,
            vec![Intent::Command(urvim_core::ui::Command::SplitVertical)]
        ));

        urvim_core::session::save_now(&layout);
        let session_file = urvim_core::session::load_current_cwd()
            .expect("session should load")
            .expect("session should exist");
        match session_file.root {
            urvim_core::session::SessionNode::Split(_) => {}
            other => panic!("expected split session root, got {other:?}"),
        }

        assert!(process_intent_queue(
            &mut layout,
            vec![Intent::Command(urvim_core::ui::Command::TryQuit)]
        ));

        let saved = urvim_core::session::load_current_cwd()
            .expect("session should still load")
            .expect("session should still exist");
        match saved.root {
            urvim_core::session::SessionNode::Split(_) => {}
            other => panic!("expected split session root after try-quit, got {other:?}"),
        }
    })();

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn quit_saves_session_before_layout_is_cleared() {
    let _guard = cwd_lock();
    let _buffer_guard = buffer_pool_lock();
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-quit-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();

    (|| {
        urvim_core::session::set_enabled(true);

        let mut layout =
            urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
        assert!(process_intent_queue(
            &mut layout,
            vec![Intent::Command(urvim_core::ui::Command::SplitVertical)]
        ));

        urvim_core::session::save_now(&layout);
        let session_file = urvim_core::session::load_current_cwd()
            .expect("session should load")
            .expect("session should exist");
        assert!(matches!(
            session_file.root,
            urvim_core::session::SessionNode::Split(_)
        ));

        assert!(process_intent_queue(
            &mut layout,
            vec![Intent::Command(urvim_core::ui::Command::Quit)]
        ));

        let saved = urvim_core::session::load_current_cwd()
            .expect("session should still load")
            .expect("session should still exist");
        assert!(matches!(
            saved.root,
            urvim_core::session::SessionNode::Split(_)
        ));
    })();

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn startup_layout_restores_existing_session_when_no_files_are_passed() {
    let _guard = cwd_lock();
    let _buffer_guard = buffer_pool_lock();
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-startup-restore-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("restore.txt");
    std::fs::write(&path, "saved session file\nsecond line").unwrap();
    let session = urvim_core::session::SessionFile {
        version: 1,
        cwd: temp_dir.display().to_string(),
        label: temp_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("cwd")
            .to_string(),
        focused_pane: 0,
        root: urvim_core::session::SessionNode::Pane(urvim_core::session::SessionPane {
            pane_id: 0,
            window_group: urvim_core::session::SessionWindowGroup {
                active_tab: 0,
                tabs: vec![urvim_core::session::SessionWindow {
                    path: path.display().to_string(),
                    cursor: urvim_core::session::SessionCursor { row: 1, col: 0 },
                    scroll_offset: urvim_core::session::SessionPosition { row: 0, col: 0 },
                    wrapped_row_offset: 0,
                    wrap_enabled: false,
                }],
            },
        }),
    };
    urvim_core::session::save_session_for_cwd(&temp_dir, &session).unwrap();

    let loaded = urvim_core::session::load_session_for_cwd(&temp_dir).unwrap();
    assert!(loaded.is_some());

    let restored = startup_layout_for_cwd(&temp_dir, &[]);
    assert_eq!(
        restored
            .active_buffer_view()
            .with_buffer(|buffer| buffer
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()))
            .unwrap(),
        Some("restore.txt".to_string())
    );
    assert_eq!(
        restored.active_buffer_view().cursor(),
        urvim_core::buffer::Cursor::new(1, 0)
    );
}

#[test]
fn startup_layout_uses_blank_buffer_when_no_session_exists() {
    let _guard = cwd_lock();
    let _buffer_guard = buffer_pool_lock();
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-startup-blank-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let restored = startup_layout_for_cwd(&temp_dir, &[]);
    assert_eq!(
        restored
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str())
            .unwrap(),
        ""
    );
}

#[test]
fn startup_layout_with_files_does_not_restore_session() {
    let _guard = cwd_lock();
    let _buffer_guard = buffer_pool_lock();
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-startup-files-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let session_path = temp_dir.join("session.txt");
    std::fs::write(&session_path, "session state").unwrap();
    let saved_layout = urvim_core::Layout::new(WindowGroup::from_paths(&[session_path.clone()]));
    urvim_core::session::save_session_for_cwd(&temp_dir, &saved_layout.to_session()).unwrap();

    let cli_path = temp_dir.join("cli.txt");
    std::fs::write(&cli_path, "cli file").unwrap();
    let restored = startup_layout_for_cwd(
        &temp_dir,
        &[CliFileSpec {
            path: cli_path.clone(),
            cursor: None,
        }],
    );

    assert_eq!(
        restored
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str())
            .unwrap(),
        "cli file"
    );
    assert_eq!(
        restored
            .active_buffer_view()
            .with_buffer(|buffer| buffer
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()))
            .unwrap(),
        Some("cli.txt".to_string())
    );
}

#[test]
fn raw_paste_action_for_insert_and_normal_modes_inserts_text() {
    let insert = raw_paste_action_for_mode(ModeKind::Insert, "hello".to_string())
        .expect("insert mode paste should be handled");
    let normal = raw_paste_action_for_mode(ModeKind::Normal, "hello".to_string())
        .expect("normal mode paste should be handled");

    assert!(
        matches!(insert.kind.as_ref(), Some(EditorOperation::InsertRawPaste(text)) if text == "hello")
    );
    assert_eq!(insert.from_mode, Some(ModeKind::Insert));
    assert_eq!(insert.to_mode, None);

    assert!(
        matches!(normal.kind.as_ref(), Some(EditorOperation::InsertRawPaste(text)) if text == "hello")
    );
    assert_eq!(normal.from_mode, Some(ModeKind::Normal));
    assert_eq!(normal.to_mode, None);
}

#[test]
fn raw_paste_action_for_visual_modes_replaces_selection_then_exits() {
    let visual = raw_paste_action_for_mode(ModeKind::Visual, "hello".to_string())
        .expect("visual mode paste should be handled");
    let visual_line = raw_paste_action_for_mode(ModeKind::VisualLine, "hello".to_string())
        .expect("visual line mode paste should be handled");

    assert!(
        matches!(visual.kind.as_ref(), Some(EditorOperation::ReplaceSelectionRawPaste(text)) if text == "hello")
    );
    assert_eq!(visual.from_mode, Some(ModeKind::Visual));
    assert_eq!(visual.to_mode, Some(ModeKind::Normal));

    assert!(
        matches!(visual_line.kind.as_ref(), Some(EditorOperation::ReplaceSelectionRawPaste(text)) if text == "hello")
    );
    assert_eq!(visual_line.from_mode, Some(ModeKind::VisualLine));
    assert_eq!(visual_line.to_mode, Some(ModeKind::Normal));
}

#[test]
fn resolve_repeat_action_uses_stored_repeat_state() {
    let _guard = repeat_state_lock();
    globals::set_last_repeat(globals::RepeatState {
        action: EditorAction::new(EditorOperation::DeleteLine),
        count: 3,
        insert_text: Some("hello".to_string()),
    });

    let replay = EditorAction::new(EditorOperation::RepeatLastChange)
        .resolve_dot_repeat()
        .expect("repeat should resolve");
    assert!(matches!(
        replay.action.kind.as_ref(),
        Some(EditorOperation::DeleteLine)
    ));
    assert_eq!(replay.structural_count, 3);
    assert_eq!(replay.repeat_count, 1);
    assert_eq!(replay.insert_text.as_deref(), Some("hello"));
}

#[test]
fn resolve_repeat_action_overrides_the_stored_count() {
    let _guard = repeat_state_lock();
    globals::set_last_repeat(globals::RepeatState {
        action: EditorAction::new(EditorOperation::DeleteLine),
        count: 3,
        insert_text: None,
    });

    let replay = EditorAction::count(2, Box::new(EditorAction::new(EditorOperation::RepeatLastChange)))
        .resolve_dot_repeat()
        .expect("repeat should resolve");
    assert!(matches!(
        replay.action.kind.as_ref(),
        Some(EditorOperation::DeleteLine)
    ));
    assert_eq!(replay.structural_count, 3);
    assert_eq!(replay.repeat_count, 2);
    assert_eq!(replay.insert_text, None);
}

#[test]
fn replay_repeat_action_applies_structural_count_once_before_insert_text() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "line1\nline2\nline3",
    )]));
    let replay = RepeatReplay {
        action: EditorAction::new(EditorOperation::ChangeLine),
        structural_count: 2,
        repeat_count: 1,
        insert_text: Some("hello".to_string()),
    };

    assert!(replay_repeat_action(&mut layout, &replay));
    let text = layout
        .active_buffer_view()
        .with_buffer(|buffer| buffer.as_str())
        .expect("buffer should exist");
    assert_eq!(text, "hello\nline3");
    assert_eq!(
        layout.active_buffer_view().cursor(),
        urvim_core::buffer::Cursor::new(0, 5)
    );
}

#[test]
fn replay_repeat_action_replays_direct_insert_text() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("world")]));
    let replay = RepeatReplay {
        action: EditorAction::mode_transition(ModeKind::Insert),
        structural_count: 1,
        repeat_count: 1,
        insert_text: Some("hello ".to_string()),
    };

    assert!(replay_repeat_action(&mut layout, &replay));
    let text = layout
        .active_buffer_view()
        .with_buffer(|buffer| buffer.as_str())
        .expect("buffer should exist");
    assert_eq!(text, "hello world");
    assert_eq!(
        layout.active_buffer_view().cursor(),
        urvim_core::buffer::Cursor::new(0, 6)
    );
}

#[test]
fn switch_mode_clears_visual_selection_when_leaving_visual() {
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let enter_repeat_text = layout
        .window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::Visual);
    let repeat_text = layout
        .window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::Normal);

    assert_eq!(
        layout.window_group().active_window_mode_kind(),
        ModeKind::Normal
    );
    assert!(enter_repeat_text.is_none());
    assert!(repeat_text.is_none());
    assert!(layout.active_buffer_view().visual_selection().is_none());
}

#[test]
fn switch_mode_restarts_visual_selection_when_entering_visual() {
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));

    let repeat_text = layout
        .window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::Visual);

    assert_eq!(
        layout.window_group().active_window_mode_kind(),
        ModeKind::Visual
    );
    assert!(repeat_text.is_none());
    assert!(layout.active_buffer_view().visual_selection().is_some());
}

#[test]
fn switch_mode_starts_linewise_visual_selection_when_entering_visual_line() {
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));

    let repeat_text = layout
        .window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::VisualLine);

    assert_eq!(
        layout.window_group().active_window_mode_kind(),
        ModeKind::VisualLine
    );
    assert!(repeat_text.is_none());
    let selection = layout
        .active_buffer_view()
        .visual_selection()
        .expect("linewise selection should exist");
    assert_eq!(selection.kind, VisualSelectionKind::Line);
}

fn drain_editor_events_serial() -> Vec<urvim_core::event::EditorEvent> {
    let mut events = Vec::new();
    while let Some(event) = globals::take_editor_event() {
        events.push(event);
    }
    events
}

fn clear_editor_events_for_test() {
    globals::clear_editor_events_for_tests();
}

#[test]
fn save_buffer_action_enqueues_buffer_saved_event() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let unique = format!(
        "urvim-event-save-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    let absolute_path = urvim_core::AbsolutePath::from_path(path.as_path())
        .expect("temp path should resolve absolutely");

    let buffer = Buffer::with_path(absolute_path);
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![buffer]));
    drain_editor_events_serial();

    let buffer_id = layout.active_buffer_view().buffer_id();
    assert!(handle_save_buffer_action(
        &mut layout,
        Some(buffer_id),
        false
    ));

    let events = drain_editor_events_serial();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, urvim_core::event::EditorEvent::BufferSaved { buffer_id: id } if *id == buffer_id)),
        "expected BufferSaved event for buffer {buffer_id:?}, got {events:?}"
    );

    std::fs::remove_file(path).ok();
}

#[test]
fn close_buffer_enqueues_buffer_closed_event() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::from_str("first"),
        Buffer::from_str("second"),
    ]));
    drain_editor_events_serial();

    let active = layout.active_buffer_view().buffer_id();
    let handled = process_intent_queue(
        &mut layout,
        vec![Intent::Command(urvim_core::ui::Command::CloseBuffer(Some(
            active,
        )))],
    );
    assert!(handled);

    let events = drain_editor_events_serial();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, urvim_core::event::EditorEvent::BufferClosed { buffer_id: id } if *id == active)),
        "expected BufferClosed event for buffer {active:?}, got {events:?}"
    );
}

#[test]
fn unload_buffer_with_force_enqueues_buffer_closed_and_unloaded() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("alpha")]));
    drain_editor_events_serial();

    let active = layout.active_buffer_view().buffer_id();
    let handled = process_intent_queue(
        &mut layout,
        vec![Intent::Command(urvim_core::ui::Command::UnloadBuffer {
            buffer_id: Some(active),
            force: true,
        })],
    );
    assert!(handled);

    let events = drain_editor_events_serial();
    let mut saw_closed = false;
    let mut saw_unloaded = false;
    for event in &events {
        match event {
            urvim_core::event::EditorEvent::BufferClosed { buffer_id } if *buffer_id == active => {
                saw_closed = true;
            }
            urvim_core::event::EditorEvent::BufferUnloaded { buffer_id, .. }
                if *buffer_id == active =>
            {
                saw_unloaded = true;
            }
            _ => {}
        }
    }
    assert!(saw_closed, "expected BufferClosed for {active:?}");
    assert!(saw_unloaded, "expected BufferUnloaded for {active:?}");
}

#[test]
fn unload_modified_buffer_without_force_emits_no_lifecycle_events() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let mut buffer = Buffer::from_str("hello");
    buffer.insert_text(urvim_core::buffer::Cursor::new(0, 5), " world");
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![buffer]));
    drain_editor_events_serial();

    let active = layout.active_buffer_view().buffer_id();
    let handled = process_intent_queue(
        &mut layout,
        vec![Intent::Command(urvim_core::ui::Command::UnloadBuffer {
            buffer_id: Some(active),
            force: false,
        })],
    );
    assert!(handled);

    let events = drain_editor_events_serial();
    assert!(events.iter().all(|event| !matches!(
        event,
        urvim_core::event::EditorEvent::BufferClosed { buffer_id } if *buffer_id == active
    ) && !matches!(
        event,
        urvim_core::event::EditorEvent::BufferUnloaded { buffer_id, .. } if *buffer_id == active
    )));
}

#[test]
fn set_buffer_filetype_enqueues_filetype_changed_event() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    drain_editor_events_serial();

    let active = layout.active_buffer_view().buffer_id();
    let handled = process_intent_queue(
        &mut layout,
        vec![Intent::Command(urvim_core::ui::Command::SetBufferFiletype(
            Some(active),
            "rust".to_string(),
        ))],
    );
    assert!(handled);

    let events = drain_editor_events_serial();
    assert!(events.iter().any(|event| matches!(
        event,
        urvim_core::event::EditorEvent::BufferFiletypeChanged { buffer_id } if *buffer_id == active
    )));
}

#[test]
fn non_plugin_command_enqueues_command_executed_event() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("alpha")]));
    drain_editor_events_serial();

    let handled = process_intent_queue(
        &mut layout,
        vec![Intent::Command(urvim_core::ui::Command::ToggleWrap)],
    );
    assert!(handled);

    let events = drain_editor_events_serial();
    assert!(events.iter().any(|event| matches!(
        event,
        urvim_core::event::EditorEvent::CommandExecuted { command } if command.contains("ToggleWrap")
    )));
}

#[test]
fn open_file_emits_buffer_loaded_before_buffer_opened() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let unique = format!(
        "urvim-event-open-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::write(&path, "open me\n").unwrap();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    drain_editor_events_serial();

    let handled = process_intent_queue(
        &mut layout,
        vec![Intent::Command(urvim_core::ui::Command::OpenFile(
            path.clone(),
        ))],
    );
    assert!(handled);

    let events = drain_editor_events_serial();
    let loaded_index = events
        .iter()
        .position(|event| matches!(event, urvim_core::event::EditorEvent::BufferLoaded { .. }));
    let opened_index = events
        .iter()
        .position(|event| matches!(event, urvim_core::event::EditorEvent::BufferOpened { .. }));
    let executed_index = events.iter().position(|event| {
        matches!(
            event,
            urvim_core::event::EditorEvent::CommandExecuted { .. }
        )
    });
    assert!(
        loaded_index.is_some() && opened_index.is_some() && executed_index.is_some(),
        "expected loaded, opened, and command events, got {events:?}"
    );
    assert!(loaded_index.unwrap() < opened_index.unwrap());
    assert!(opened_index.unwrap() < executed_index.unwrap());

    std::fs::remove_file(path).ok();
}

#[test]
fn reopen_loaded_file_in_pane_emits_buffer_opened_only() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let unique = format!(
        "urvim-event-reopen-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::write(&path, "alpha").unwrap();

    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let loaded_buffer_id = globals::with_buffer_pool(|pool| {
        pool.open_buffer(&path)
            .expect("target file should load into buffer pool")
    });
    drain_editor_events_serial();

    // The file is already loaded in the buffer pool but is not open in the
    // active pane. Opening it here should add a UI tab/view without loading a
    // new buffer.
    let before = layout
        .active_window_group()
        .buffer_ids()
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let handled = process_intent_queue(
        &mut layout,
        vec![Intent::Command(urvim_core::ui::Command::OpenFile(
            path.clone(),
        ))],
    );
    assert!(handled);
    let after = layout
        .active_window_group()
        .buffer_ids()
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();

    let events = drain_editor_events_serial();
    let loaded_events: Vec<_> = events
        .iter()
        .filter(|event| matches!(event, urvim_core::event::EditorEvent::BufferLoaded { buffer_id } if *buffer_id == loaded_buffer_id))
        .collect();
    assert!(
        loaded_events.is_empty(),
        "did not expect a new BufferLoaded event for {loaded_buffer_id:?}; events: {events:?}"
    );
    let new_opened: Vec<_> = after.difference(&before).copied().collect();
    assert_eq!(new_opened.len(), 1, "expected one buffer to open in pane");
    assert_eq!(new_opened[0], loaded_buffer_id);
    assert!(
        events.iter().any(|event| matches!(
            event,
            urvim_core::event::EditorEvent::BufferOpened { buffer_id }
                if *buffer_id == new_opened[0]
        )),
        "expected BufferOpened event for reopened buffer; events: {events:?}"
    );

    std::fs::remove_file(path).ok();
}

#[test]
fn orphan_cleanup_emits_buffer_unloaded_after_close_buffer() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let unique = format!(
        "urvim-event-orphan-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::write(&path, "alpha").unwrap();
    let cli_path = path.clone();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_paths(&[cli_path.clone()]));
    let visible_buffer_id = layout.active_buffer_view().buffer_id();
    drain_editor_events_serial();

    // Add a second, non-visible buffer to the pool so cleanup will unload it.
    let hidden_buffer_id = urvim_core::globals::with_buffer_pool(|pool| {
        pool.create_buffer_with_path(std::env::temp_dir().join(format!(
                "urvim-event-hidden-{}-{}.txt",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be after epoch")
                    .as_nanos()
            )))
        .expect("hidden buffer should be created")
    });
    drain_editor_events_serial();

    // Trigger a command that runs orphan cleanup so the hidden buffer is removed.
    let handled = process_intent_queue(
        &mut layout,
        vec![Intent::Command(urvim_core::ui::Command::ToggleWrap)],
    );
    assert!(handled);

    let events = drain_editor_events_serial();
    assert!(events.iter().any(|event| matches!(
        event,
        urvim_core::event::EditorEvent::BufferUnloaded { buffer_id, .. } if *buffer_id == hidden_buffer_id
    )));
    assert!(!events.iter().any(|event| matches!(
        event,
        urvim_core::event::EditorEvent::BufferUnloaded { buffer_id, .. } if *buffer_id == visible_buffer_id
    )));

    std::fs::remove_file(path).ok();
}

#[test]
fn startup_loaded_buffers_emit_buffer_loaded_before_editor_started() {
    let _guard = buffer_pool_lock();
    clear_editor_events_for_test();

    let unique = format!(
        "urvim-event-startup-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::write(&path, "startup content").unwrap();
    let cli_path = path.clone();

    let layout = urvim_core::Layout::new(WindowGroup::from_paths(&[cli_path.clone()]));
    let startup_buffer_id = layout.active_buffer_view().buffer_id();

    let mut events = drain_editor_events_serial();
    let startup_loaded_index = events.iter().position(|event| {
        matches!(event, urvim_core::event::EditorEvent::BufferLoaded { buffer_id } if *buffer_id == startup_buffer_id)
    });
    assert!(
        startup_loaded_index.is_some(),
        "expected startup buffer to enqueue BufferLoaded; events: {events:?}"
    );

    // The app loop is responsible for emitting EditorStarted only after the
    // initial plugin poll settles. Simulate that ordering here.
    globals::enqueue_editor_event(urvim_core::event::EditorEvent::EditorStarted);
    while let Some(event) = globals::take_editor_event() {
        events.push(event);
    }

    let editor_started_index = events
        .iter()
        .position(|event| matches!(event, urvim_core::event::EditorEvent::EditorStarted));
    assert!(
        editor_started_index.is_some(),
        "expected EditorStarted event; events: {events:?}"
    );
    assert!(
        startup_loaded_index.unwrap() < editor_started_index.unwrap(),
        "startup BufferLoaded must be delivered before EditorStarted; events: {events:?}"
    );

    std::fs::remove_file(path).ok();
}

#[test]
fn plugin_list_buffers_returns_loaded_buffer_metadata() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::from_str("first"),
        Buffer::from_str("second"),
    ]));
    let active_id = layout.active_buffer_view().buffer_id().get();
    let request =
        urvim_plugin::PluginRequest::new(100, "editor/listBuffers", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 100);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    let buffers = result["buffers"]
        .as_array()
        .expect("buffers should be an array");
    assert!(buffers.len() >= 2);
    assert!(
        buffers
            .iter()
            .any(|b| b["buffer_id"] == serde_json::json!(active_id)
                && b["active"] == serde_json::json!(true))
    );
}

#[test]
fn plugin_get_buffer_returns_metadata() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        101,
        "editor/getBuffer",
        serde_json::json!({ "buffer_id": buffer_id }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 101);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id));
    assert_eq!(result["line_count"], serde_json::json!(1));
    assert_eq!(result["active"], serde_json::json!(true));
    assert_eq!(result["visible"], serde_json::json!(true));
}

#[test]
fn plugin_get_buffer_rejects_unknown_id() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        102,
        "editor/getBuffer",
        serde_json::json!({ "buffer_id": 999999 }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 102);
    assert!(response.error.unwrap().contains("unknown buffer_id"));
}

#[test]
fn plugin_get_buffer_rejects_generic_id_alias() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        242,
        "editor/getBuffer",
        serde_json::json!({ "id": buffer_id }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 242);
    assert!(response.error.unwrap().contains("requires buffer_id"));
}

#[test]
fn plugin_request_hover_rejects_camel_case_buffer_id() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        243,
        "editor/requestHover",
        serde_json::json!({ "bufferId": buffer_id, "line": 0, "col": 0 }),
    );

    let mut contributions = urvim_plugin::PluginContributionRegistry::default();
    let response = resolve_test_plugin_editor_request_with_contributions(
        &mut contributions,
        "demo-plugin",
        &mut layout,
        &request,
    );

    assert_eq!(response.id, 243);
    assert!(response.error.unwrap().contains("requires buffer_id"));
}

#[test]
fn plugin_find_buffer_by_path_returns_found_buffer() {
    let _guard = buffer_pool_lock();
    let unique = format!(
        "urvim-find-buffer-{}-{}.txt",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::write(&path, "content").unwrap();

    let buffer = Buffer::load_from_file(&path).unwrap();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![buffer]));
    let request = urvim_plugin::PluginRequest::new(
        103,
        "editor/findBufferByPath",
        serde_json::json!({ "path": path.to_string_lossy() }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 103);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["found"], serde_json::json!(true));
    assert!(result["buffer"]["buffer_id"].as_u64().is_some());

    std::fs::remove_file(path).ok();
}

#[test]
fn plugin_find_buffer_by_path_returns_not_found_for_unloaded_path() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
    let request = urvim_plugin::PluginRequest::new(
        104,
        "editor/findBufferByPath",
        serde_json::json!({ "path": "/tmp/urvim-nonexistent-file.txt" }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 104);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["found"], serde_json::json!(false));
}

#[test]
fn plugin_get_buffer_lines_returns_requested_range() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "line0\nline1\nline2\nline3",
    )]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        105,
        "editor/getBufferLines",
        serde_json::json!({ "buffer_id": buffer_id, "start": 1, "end": 3 }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 105);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id));
    assert_eq!(result["start"], serde_json::json!(1));
    assert_eq!(result["end"], serde_json::json!(3));
    let lines = result["lines"]
        .as_array()
        .expect("lines should be an array");
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], serde_json::json!("line1"));
    assert_eq!(lines[1], serde_json::json!("line2"));
}

#[test]
fn plugin_get_buffer_lines_rejects_invalid_range() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "line0\nline1",
    )]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        106,
        "editor/getBufferLines",
        serde_json::json!({ "buffer_id": buffer_id, "start": 3, "end": 1 }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 106);
    assert!(response.error.unwrap().contains("start must be <= end"));
}

#[test]
fn plugin_get_cursor_returns_active_cursor() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    layout
        .active_buffer_view_mut()
        .set_cursor(urvim_core::buffer::Cursor::new(0, 3));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(107, "editor/getCursor", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 107);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id));
    assert_eq!(result["cursor"]["line"], serde_json::json!(0));
    assert_eq!(result["cursor"]["col"], serde_json::json!(3));
    assert_eq!(result["active"], serde_json::json!(true));
}

#[test]
fn plugin_set_cursor_updates_active_cursor() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        108,
        "editor/setCursor",
        serde_json::json!({ "cursor": { "line": 0, "col": 4 } }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 108);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id));
    assert_eq!(result["cursor"]["line"], serde_json::json!(0));
    assert_eq!(result["cursor"]["col"], serde_json::json!(4));
    assert_eq!(
        layout.active_buffer_view().cursor(),
        urvim_core::buffer::Cursor::new(0, 4)
    );
}

#[test]
fn plugin_set_cursor_rejects_invalid_cursor() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    let request = urvim_plugin::PluginRequest::new(
        109,
        "editor/setCursor",
        serde_json::json!({ "cursor": { "line": 0, "col": 99 } }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 109);
    assert!(response.error.unwrap().contains("invalid cursor"));
}

#[test]
fn plugin_set_cursor_rejects_non_active_buffer_id() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![
        Buffer::from_str("first"),
        Buffer::from_str("second"),
    ]));
    let active_id = layout.active_buffer_view().buffer_id();
    let other_id = layout
        .window_group()
        .buffer_ids()
        .into_iter()
        .find(|id| *id != active_id)
        .unwrap();
    let request = urvim_plugin::PluginRequest::new(
        110,
        "editor/setCursor",
        serde_json::json!({ "buffer_id": other_id.get(), "cursor": { "line": 0, "col": 0 } }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 110);
    assert!(response.error.unwrap().contains("is not the active buffer"));
}

#[test]
fn plugin_get_selection_returns_character_selection() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    layout
        .active_buffer_view_mut()
        .set_cursor(urvim_core::buffer::Cursor::new(0, 1));
    layout
        .active_buffer_view_mut()
        .begin_visual_selection(urvim_core::window::VisualSelectionKind::Character);
    layout
        .active_buffer_view_mut()
        .set_cursor(urvim_core::buffer::Cursor::new(0, 4));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request =
        urvim_plugin::PluginRequest::new(111, "editor/getSelection", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 111);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id));
    assert_eq!(result["active"], serde_json::json!(true));
    assert_eq!(result["kind"], serde_json::json!("character"));
    assert_eq!(result["anchor"]["line"], serde_json::json!(0));
    assert_eq!(result["anchor"]["col"], serde_json::json!(1));
    assert_eq!(result["cursor"]["line"], serde_json::json!(0));
    assert_eq!(result["cursor"]["col"], serde_json::json!(4));
}

#[test]
fn plugin_clear_selection_clears_active_selection() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
    layout
        .active_buffer_view_mut()
        .begin_visual_selection(urvim_core::window::VisualSelectionKind::Line);
    let request =
        urvim_plugin::PluginRequest::new(112, "editor/clearSelection", serde_json::json!({}));

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 112);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["cleared"], serde_json::json!(true));
    assert!(layout.active_buffer_view().visual_selection().is_none());
}

#[test]
fn plugin_get_buffer_range_reads_unicode_safely() {
    let _guard = buffer_pool_lock();
    let mut layout = urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
        "aébc\nsecond",
    )]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        113,
        "editor/getBufferRange",
        serde_json::json!({
            "buffer_id": buffer_id,
            "start": { "line": 0, "col": 1 },
            "end": { "line": 0, "col": 3 }
        }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 113);
    assert!(response.error.is_none());
    let result = response.result.expect("response should include result");
    assert_eq!(result["buffer_id"], serde_json::json!(buffer_id));
    assert_eq!(result["text"], serde_json::json!("é"));
}

#[test]
fn plugin_apply_buffer_edits_applies_original_coordinate_order() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("abc")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        114,
        "editor/applyBufferEdits",
        serde_json::json!({
            "buffer_id": buffer_id,
            "edits": [
                {
                    "kind": "replace",
                    "start": { "line": 0, "col": 0 },
                    "end": { "line": 0, "col": 1 },
                    "text": "A"
                },
                {
                    "kind": "insert",
                    "start": { "line": 0, "col": 2 },
                    "text": "X"
                }
            ]
        }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 114);
    assert!(response.error.is_none());
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some("AbXc".to_string())
    );
}

#[test]
fn plugin_apply_buffer_edits_rejects_insert_inside_replace_range_without_mutating() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("abcdef")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        115,
        "editor/applyBufferEdits",
        serde_json::json!({
            "buffer_id": buffer_id,
            "edits": [
                {
                    "kind": "replace",
                    "start": { "line": 0, "col": 1 },
                    "end": { "line": 0, "col": 4 },
                    "text": "X"
                },
                {
                    "kind": "insert",
                    "start": { "line": 0, "col": 2 },
                    "text": "Y"
                }
            ]
        }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 115);
    assert!(
        response
            .error
            .unwrap()
            .contains("insertions inside delete or replace ranges")
    );
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some("abcdef".to_string())
    );
}

#[test]
fn plugin_apply_buffer_edits_groups_undo_snapshot() {
    let _guard = buffer_pool_lock();
    let mut layout =
        urvim_core::Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("abcdef")]));
    let buffer_id = layout.active_buffer_view().buffer_id().get();
    let request = urvim_plugin::PluginRequest::new(
        116,
        "editor/applyBufferEdits",
        serde_json::json!({
            "buffer_id": buffer_id,
            "edits": [
                {
                    "kind": "replace",
                    "start": { "line": 0, "col": 0 },
                    "end": { "line": 0, "col": 1 },
                    "text": "A"
                },
                {
                    "kind": "replace",
                    "start": { "line": 0, "col": 5 },
                    "end": { "line": 0, "col": 6 },
                    "text": "F"
                }
            ]
        }),
    );

    let response = resolve_test_plugin_editor_request(&mut layout, &request);

    assert_eq!(response.id, 116);
    assert!(response.error.is_none());
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some("AbcdeF".to_string())
    );
    let cursor = layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.undo())
        .flatten()
        .expect("batch edit should create one undo snapshot");
    layout.active_buffer_view_mut().set_cursor_synced(cursor);
    assert_eq!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str()),
        Some("abcdef".to_string())
    );
}
