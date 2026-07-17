use super::*;
use crate::action::ActionResult;
use crate::background::{
    JobEvent, JobKind, JobPayload, JobToken, LspInlayHint, LspInlayHintsChunk,
};
use crate::buffer::Buffer;
use crate::buffer::Cursor;
use crate::config::{Config, KeymapsConfig};
use crate::editor::{EditorAction, EditorOperation, ModeKind};
use crate::globals;
use crate::path::AbsolutePath;
use crate::ui::{Command, Intent, UiEvent, UiEventResult};
use crate::window::{Position, Size};
use crate::window_group::WindowGroup;
use lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use smol_str::SmolStr;
use std::collections::BTreeSet;
use std::fs;
use std::thread;
use std::time::Duration;
use urvim_terminal::{Color, Key, KeyCode, Modifiers, Style};
use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

fn layout_with_buffers(buffers: Vec<Buffer>) -> Layout {
    Layout::new(WindowGroup::from_buffers(buffers))
}

fn abs_path(path: &std::path::Path) -> crate::AbsolutePath {
    crate::AbsolutePath::from_path(path).unwrap()
}

fn dispatch_layout_action<T>(layout: &mut Layout, intent: T) -> ActionResult
where
    T: Into<Intent>,
{
    let intent = intent.into();
    if layout.dispatch_intent(&intent) {
        ActionResult::Handled
    } else {
        ActionResult::NotHandled
    }
}

fn border_theme() -> Theme {
    let default_style = Style::new().fg(Color::ansi(15)).bg(Color::ansi(30));
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
        Style::new().fg(Color::ansi(33)),
    );
    highlights.insert(
        Tag::parse("ui.window.lines.resize").expect("valid tag"),
        Style::new().fg(Color::ansi(160)).bold(),
    );

    Theme::new("demo", ThemeKind::Ansi256, default_style, highlights)
}

fn key(code: KeyCode) -> Key {
    Key {
        code,
        modifiers: Modifiers::default(),
    }
}

fn border_config(unicode_borders: bool) -> Config {
    let advanced_glyphs = if unicode_borders {
        BTreeSet::from([crate::config::AdvancedGlyphCapability::UnicodeBorders])
    } else {
        BTreeSet::new()
    };

    Config {
        theme: "demo".to_string(),
        syntax: true,
        auto_close_pairs: true,
        active_line: false,
        advanced_glyphs,
        ..Default::default()
    }
}

fn buffer_line_count(view: &crate::window::BufferView) -> usize {
    view.with_buffer(|buffer| buffer.line_count()).unwrap_or(0)
}

fn pane_buffer_view(node: &LayoutNode) -> &crate::window::BufferView {
    match node {
        LayoutNode::Pane(pane) => pane
            .editor_window_group()
            .expect("expected editor pane")
            .active_buffer_view(),
        LayoutNode::Split(_) => panic!("expected buffer pane"),
    }
}

fn pane_window(node: &LayoutNode) -> &crate::window::Window {
    match node {
        LayoutNode::Pane(pane) => pane
            .editor_window_group()
            .expect("expected editor pane")
            .active_window(),
        LayoutNode::Split(_) => panic!("expected buffer pane"),
    }
}

fn pane_count(node: &LayoutNode) -> usize {
    match node {
        LayoutNode::Pane(_) => 1,
        LayoutNode::Split(split) => pane_count(&split.first) + pane_count(&split.second),
    }
}

#[test]
fn test_layout_session_round_trips_cursor_and_file_paths() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let _config_guard = globals::set_test_config(Config::default());
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-layout-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("session.txt");
    fs::write(&path, "alpha\nbeta").unwrap();

    let mut layout = layout_with_buffers(vec![Buffer::from_str_with_path(
        "alpha\nbeta",
        abs_path(&path),
    )]);
    layout
        .active_buffer_view_mut()
        .set_cursor_synced(Cursor::new(1, 2));

    let restored = Layout::from_session(layout.to_session());

    assert_eq!(restored.active_buffer_view().cursor(), Cursor::new(1, 2));
    assert_eq!(
        restored.active_buffer_view().with_buffer(|buffer| buffer
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())),
        Some(Some("session.txt".to_string()))
    );
}

#[test]
fn test_apply_completion_text_expands_basic_snippets() {
    assert_eq!(
        super::apply_completion_text(
            "new($0)",
            crate::ui::completion::CompletionInsertFormat::Snippet
        ),
        ("new()".to_string(), 4)
    );
    assert_eq!(
        super::apply_completion_text(
            "plain",
            crate::ui::completion::CompletionInsertFormat::PlainText
        ),
        ("plain".to_string(), 5)
    );
}

#[test]
fn test_apply_completion_text_expands_snippet_tabstops() {
    assert_eq!(
        super::apply_completion_text(
            "rfind(${1:pat})",
            crate::ui::completion::CompletionInsertFormat::Snippet
        ),
        ("rfind(pat)".to_string(), 6)
    );
    assert_eq!(
        super::apply_completion_text(
            "rfind($1)",
            crate::ui::completion::CompletionInsertFormat::Snippet
        ),
        ("rfind()".to_string(), 6)
    );
    assert_eq!(
        super::apply_completion_text(
            "${1:pat} + $0",
            crate::ui::completion::CompletionInsertFormat::Snippet
        ),
        ("pat + ".to_string(), 0)
    );
}

fn collect_pane_ids(node: &LayoutNode, ids: &mut Vec<PaneId>) {
    match node {
        LayoutNode::Pane(pane) => ids.push(pane.id),
        LayoutNode::Split(split) => {
            collect_pane_ids(&split.first, ids);
            collect_pane_ids(&split.second, ids);
        }
    }
}

fn assert_all_splits_even(node: &LayoutNode) {
    match node {
        LayoutNode::Pane(_) => {}
        LayoutNode::Split(split) => {
            assert_eq!(split.split_size.first_weight(), 1);
            assert_eq!(split.split_size.second_weight(), 1);
            assert_all_splits_even(&split.first);
            assert_all_splits_even(&split.second);
        }
    }
}

#[test]
fn test_layout_new_wraps_window_group() {
    let layout = Layout::new(WindowGroup::new(Vec::new()));

    assert_eq!(layout.origin(), Position::default());
    assert_eq!(layout.size(), Size::default());
    assert_eq!(layout.window_group().active_tab_index(), 0);
    assert_eq!(
        layout.window_group().active_window_mode_kind(),
        ModeKind::Normal
    );
    assert_eq!(layout.mode_label(), "NORMAL");
}

#[test]
fn test_layout_exposes_active_buffer_view() {
    let layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);

    assert_eq!(buffer_line_count(layout.active_buffer_view()), 1);
}

#[test]
fn test_layout_dispatch_intent_handles_command_notifications() {
    globals::clear_notifications();
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert!(
        layout.dispatch_intent(&Intent::Command(Command::EnqueueNotification {
            level: crate::notification::NotificationLevel::Info,
            message: "saved".to_string(),
        }))
    );

    let message = globals::active_notification(std::time::Instant::now()).expect("notification");
    assert_eq!(message.text, "saved");
}

#[test]
fn test_layout_file_picker_opens_and_closes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenFilePicker)));
    assert!(layout.file_picker_is_open());

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Esc)));
    assert!(result.handled());
    assert!(!layout.file_picker_is_open());
}

#[test]
fn test_layout_plugin_picker_selects_and_cancels() {
    use crate::ui::picker::plugin::{PluginPickerCancelled, PluginPickerItem};

    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let (sender, receiver) = std::sync::mpsc::channel();
    layout.open_plugin_picker(
        "demo".to_string(),
        4,
        "Branches".to_string(),
        vec![PluginPickerItem {
            id: 8,
            key: "main".to_string(),
            label: "main".to_string(),
            detail: Some("origin/main".to_string()),
        }],
        sender,
    );
    layout.route_ui_event(&UiEvent::Tick);

    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::Enter))),
        UiEventResult::Handled(vec![Intent::Command(Command::PluginPickerSelect {
            plugin: "demo".to_string(),
            picker_id: 4,
            item_id: 8,
        })])
    );
    assert!(!layout.plugin_picker_is_open());
    assert!(receiver.try_recv().is_err());

    let (sender, receiver) = std::sync::mpsc::channel();
    layout.open_plugin_picker(
        "demo".to_string(),
        5,
        "Empty".to_string(),
        Vec::new(),
        sender,
    );
    layout.route_ui_event(&UiEvent::Key(key(KeyCode::Esc)));
    assert_eq!(
        receiver.try_recv().expect("cancellation"),
        PluginPickerCancelled {
            plugin: "demo".to_string(),
            picker_id: 5,
        }
    );
}

#[test]
fn test_picker_keeps_local_text_input_before_inherited_application_mappings() {
    let _config_guard = globals::set_test_config(Config {
        keymaps: KeymapsConfig {
            normal: std::collections::BTreeMap::from([
                ("x".to_string(), "try-quit".to_string()),
                ("<F7>".to_string(), "try-quit".to_string()),
            ]),
            ..Default::default()
        },
        ..Default::default()
    });
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenFilePicker)));

    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::Char('x')))),
        UiEventResult::Handled(Vec::new())
    );
    assert_eq!(layout.file_picker_mut().unwrap().query(), "x");
    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::F7))),
        UiEventResult::Handled(vec![Intent::Command(Command::TryQuit)])
    );
}

#[test]
fn test_picker_consumes_failed_inherited_application_sequence() {
    let _config_guard = globals::set_test_config(Config {
        keymaps: KeymapsConfig {
            normal: std::collections::BTreeMap::from([(
                "<C-x>q".to_string(),
                "try-quit".to_string(),
            )]),
            ..Default::default()
        },
        ..Default::default()
    });
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenFilePicker)));

    let ctrl_x = Key::with_modifiers(KeyCode::Char('x'), Modifiers::CTRL);
    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(ctrl_x)),
        UiEventResult::Handled(Vec::new())
    );
    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::Char('z')))),
        UiEventResult::Handled(Vec::new())
    );
    assert_eq!(layout.file_picker_mut().unwrap().query(), "");
}

#[test]
fn test_rename_and_confirmation_inherit_application_mappings_after_local_controls() {
    let _config_guard = globals::set_test_config(Config {
        keymaps: KeymapsConfig {
            normal: std::collections::BTreeMap::from([
                ("<F7>".to_string(), "try-quit".to_string()),
                ("<Enter>".to_string(), "try-quit".to_string()),
            ]),
            ..Default::default()
        },
        ..Default::default()
    });
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    layout.dialogs.lsp_rename_prompt = Some(crate::ui::lsp_rename::LspRenamePrompt::new("name"));

    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::F7))),
        UiEventResult::Handled(vec![Intent::Command(Command::TryQuit)])
    );

    layout.close_all_dialogs();
    layout.open_confirmation_box("Confirm?", Command::Quit);
    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::F7))),
        UiEventResult::Handled(vec![Intent::Command(Command::TryQuit)])
    );
    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::Enter))),
        UiEventResult::Handled(vec![Intent::Command(Command::Quit)])
    );
}

#[test]
fn test_layout_filetype_picker_opens_and_closes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenFiletypePicker)));
    assert!(layout.filetype_picker_is_open());

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Esc)));
    assert!(result.handled());
    assert!(!layout.filetype_picker_is_open());
}

#[test]
fn test_layout_set_buffer_filetype_defaults_to_active_buffer() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());

    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let active_id = layout.active_buffer_view().buffer_id();

    assert!(
        layout.dispatch_intent(&Intent::Command(Command::SetBufferFiletype(
            None,
            "rust".to_string(),
        )))
    );

    let syntax = globals::with_buffer(active_id, |buffer| buffer.syntax_name().to_string())
        .expect("active buffer");
    assert_eq!(syntax, "rust");
}

#[test]
fn test_layout_set_buffer_filetype_accepts_plugin_filetype() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    globals::set_plugin_filetypes(vec!["simplelang".to_string()]);

    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let active_id = layout.active_buffer_view().buffer_id();

    assert!(
        layout.dispatch_intent(&Intent::Command(Command::SetBufferFiletype(
            None,
            "simplelang".to_string(),
        )))
    );

    let syntax = globals::with_buffer(active_id, |buffer| buffer.syntax_name().to_string())
        .expect("active buffer");
    assert_eq!(syntax, "simplelang");
    globals::set_plugin_filetypes(Vec::new());
}

#[test]
fn test_layout_set_buffer_filetype_targets_explicit_buffer() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());

    let explicit_id =
        globals::with_buffer_pool(|pool| pool.register_buffer(Buffer::from_str("two")));
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let active_id = layout.active_buffer_view().buffer_id();

    assert!(
        layout.dispatch_intent(&Intent::Command(Command::SetBufferFiletype(
            Some(explicit_id),
            "rust".to_string(),
        )))
    );

    let active_syntax = globals::with_buffer(active_id, |buffer| buffer.syntax_name().to_string())
        .expect("active buffer");
    let explicit_syntax =
        globals::with_buffer(explicit_id, |buffer| buffer.syntax_name().to_string())
            .expect("explicit buffer");
    assert_eq!(active_syntax, "plaintext");
    assert_eq!(explicit_syntax, "rust");
}

#[test]
fn test_layout_buffer_picker_opens_and_focuses_first_visible_pane() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());

    let alpha = Buffer::from_str("alpha");
    let second_buffer_id =
        globals::with_buffer_pool(|pool| pool.register_buffer(Buffer::from_str("beta")));
    let mut layout = layout_with_buffers(vec![alpha]);
    layout
        .active_window_group_mut()
        .activate_or_open_buffer(second_buffer_id);

    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenBufferPicker)));
    assert!(layout.buffer_picker_is_open());

    let result = layout.route_ui_event(&UiEvent::Tick);
    assert!(result.handled());

    let picker = layout
        .dialogs
        .buffer_picker
        .as_ref()
        .expect("buffer picker");
    assert!(
        picker
            .results()
            .iter()
            .any(|item| item.buffer_id == second_buffer_id)
    );

    assert!(layout.dispatch_intent(&Intent::Command(Command::FocusBuffer(second_buffer_id,))));
    assert_eq!(layout.active_buffer_view().buffer_id(), second_buffer_id);
}

#[test]
fn test_layout_git_picker_opens_and_closes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenGitPicker)));
    assert!(layout.git_picker_is_open());

    let mut screen = crate::screen::Screen::new(8, 40);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 40));
    assert!(
        layout.visual_cursor().is_some(),
        "git picker prompt cursor should be visible"
    );

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Esc)));
    assert!(result.handled());
    assert!(!layout.git_picker_is_open());
}

#[test]
fn test_layout_git_picker_stage_and_discard_commands() {
    let repo = temp_git_repo();
    let tracked = repo.join("tracked.txt");
    std::fs::write(&tracked, "one\ntwo\n").unwrap();
    git(&repo, ["add", "tracked.txt"]);
    git(
        &repo,
        [
            "-c",
            "user.name=urvim",
            "-c",
            "user.email=urvim@example.com",
            "commit",
            "-q",
            "-m",
            "init",
        ],
    );
    std::fs::write(&tracked, "one\nTWO\n").unwrap();

    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let action = crate::ui::picker::git::GitPickerAction {
        path: tracked.clone(),
        untracked: false,
        staged: false,
    };

    assert!(
        layout.dispatch_intent(&Intent::Command(Command::GitPickerToggleStage(
            action.clone(),
        )))
    );
    assert!(git_status(&repo).contains("M  tracked.txt"));

    assert!(layout.dispatch_intent(&Intent::Command(Command::GitPickerDiscard(action,))));
    assert!(layout.confirmation_box_is_open());

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Enter)));
    assert!(result.handled());
    for intent in result.into_intents() {
        assert!(layout.dispatch_intent(&intent));
    }
    assert_eq!(std::fs::read_to_string(&tracked).unwrap(), "one\ntwo\n");
}

#[test]
fn test_layout_confirmation_box_takes_precedence_over_git_picker() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenGitPicker)));
    layout.open_confirmation_box("Confirm?", Command::Quit);

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Enter)));
    assert!(result.handled());
    assert_eq!(result.into_intents(), vec![Intent::Command(Command::Quit)]);
}

#[test]
fn test_layout_confirmation_takes_input_and_render_precedence_over_plugin_window() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let id = layout.create_plugin_window(
        "demo".to_string(),
        crate::ui::plugin_window::PluginWindowOptions::default(),
    );
    layout
        .plugin_windows_mut()
        .focus("demo", id)
        .expect("plugin window should focus");
    layout.open_confirmation_box("Confirm?", Command::Quit);

    let mut screen = crate::screen::Screen::new(12, 60);
    layout.render(&mut screen, Position::new(0, 0), Size::new(12, 60));
    let rendered = (0..12)
        .flat_map(|row| (0..60).map(move |col| (row, col)))
        .map(|(row, col)| screen.get_cell_mut(row, col).unwrap().text.clone())
        .collect::<String>();
    assert!(rendered.contains("Confirm?"));

    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::Enter))),
        UiEventResult::Handled(vec![Intent::Command(Command::Quit)])
    );
    assert_eq!(layout.plugin_windows().focused(), Some(id));
}

#[test]
fn test_picker_cursor_takes_precedence_over_focused_plugin_window() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let id = layout.create_plugin_window(
        "demo".to_string(),
        crate::ui::plugin_window::PluginWindowOptions::default(),
    );
    layout
        .plugin_windows_mut()
        .focus("demo", id)
        .expect("plugin window should focus");
    assert_eq!(layout.visual_cursor(), None);

    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenFiletypePicker)));
    let mut screen = crate::screen::Screen::new(12, 60);
    layout.render(&mut screen, Position::new(0, 0), Size::new(12, 60));

    assert!(layout.visual_cursor().is_some());
    assert_eq!(layout.plugin_windows().focused(), Some(id));

    layout.close_filetype_picker();
    assert_eq!(layout.visual_cursor(), None);
    assert_eq!(layout.plugin_windows().focused(), Some(id));
}

#[test]
fn test_layout_completion_esc_closes_popup_and_exits_insert_mode() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout
        .active_window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::Insert);

    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenCompletion)));
    assert!(layout.dialogs.completion.is_some());

    let overlay = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Esc)));
    assert!(matches!(overlay, UiEventResult::NotHandled));
    assert!(layout.dialogs.completion.is_none());

    let result = layout
        .active_window_group_mut()
        .active_window_mut()
        .handle_key(&key(KeyCode::Esc));
    let intent = match result {
        crate::editor::HandleKeyResult::Complete(intent) => intent,
        other => panic!("expected insert mode to handle Esc, got {other:?}"),
    };

    match intent {
        Intent::Editor(action) => assert_eq!(action.to_mode, Some(ModeKind::Normal)),
        other => panic!("expected an action intent, got {other:?}"),
    }
}

fn temp_git_repo() -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let repo =
        std::env::temp_dir().join(format!("urvim-layout-git-{}-{}", std::process::id(), stamp));
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, ["init", "-q"]);
    repo
}

fn git<const N: usize>(dir: &std::path::Path, args: [&str; N]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .expect("run git command");
    assert!(status.success(), "git command failed");
}

fn git_status(repo: &std::path::Path) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain=v1"])
        .output()
        .expect("run git status");
    assert!(output.status.success(), "git status failed");
    String::from_utf8(output.stdout).expect("utf8 status")
}

#[test]
fn test_layout_grep_picker_opens_and_closes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenGrepPicker)));
    assert!(layout.grep_picker_is_open());

    let mut screen = crate::screen::Screen::new(8, 40);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 40));
    assert!(
        layout.visual_cursor().is_some(),
        "grep picker prompt cursor should be visible"
    );

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Tab)));
    assert!(result.handled());
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 40));
    assert!(layout.visual_cursor().is_some());

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Esc)));
    assert!(result.handled());
    assert!(!layout.grep_picker_is_open());
}

#[test]
fn test_layout_lsp_hover_binding_noops_without_runtime() {
    let _guard = globals::notification_test_lock();
    globals::clear_notifications();

    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::LspHover)));
    assert!(layout.dispatch_intent(&Intent::Command(Command::LspDefinition)));
    assert!(layout.dispatch_intent(&Intent::Command(Command::LspPreviousDiagnostic)));
    assert!(layout.dispatch_intent(&Intent::Command(Command::LspNextDiagnostic)));
    assert!(layout.dispatch_intent(&Intent::Command(Command::LspPreviousErrorDiagnostic)));
    assert!(layout.dispatch_intent(&Intent::Command(Command::LspNextErrorDiagnostic)));
    assert!(layout.dispatch_intent(&Intent::Command(Command::LspCodeActions)));
    assert!(layout.dispatch_intent(&Intent::Command(Command::LspRenamePrompt)));
    assert!(globals::active_notification(std::time::Instant::now()).is_none());
    assert!(!layout.command_line_is_open());
}

#[test]
fn test_layout_diagnostic_hover_opens_and_closes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    layout.open_diagnostic_hover(
        vec![Diagnostic {
            range: Range::default(),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("lsp".to_string()),
            message: "problem".to_string(),
            related_information: None,
            tags: None,
            data: None,
        }],
        Position::new(1, 1),
    );

    assert!(layout.diagnostic_hover_is_open());

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Esc)));
    assert!(result.handled());
    assert!(!layout.diagnostic_hover_is_open());
}

#[test]
fn test_layout_diagnostic_navigation_scrolls_target_into_view() {
    let _theme_guard = globals::set_test_active_theme(border_theme());
    let _config_guard = globals::set_test_config(border_config(false));
    let _notification_guard = globals::notification_test_lock();
    globals::clear_diagnostics_store();

    let text = (0..20)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut layout = layout_with_buffers(vec![Buffer::from_str(text.as_str())]);
    let buffer_id = layout.active_buffer_view().buffer_id();

    globals::with_diagnostics_store(|store| {
        store.set(
            buffer_id,
            "lsp",
            vec![Diagnostic {
                range: Range::new(
                    lsp_types::Position::new(12, 0),
                    lsp_types::Position::new(12, 4),
                ),
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("lsp".to_string()),
                message: "problem".to_string(),
                related_information: None,
                tags: None,
                data: None,
            }],
        );
    });

    let mut screen = crate::screen::Screen::new(6, 40);
    layout.render(&mut screen, Position::new(0, 0), Size::new(6, 40));

    assert!(layout.dispatch_intent(&Intent::Command(Command::LspNextDiagnostic)));
    assert!(layout.diagnostic_hover_is_open());

    layout.render(&mut screen, Position::new(0, 0), Size::new(6, 40));
    let mut rendered = String::new();
    let (rows, cols) = screen.size();
    for row in 0..rows {
        if row > 0 {
            rendered.push('\n');
        }
        for col in 0..cols {
            rendered.push_str(screen.get_cell_mut(row, col).unwrap().text.as_str());
        }
    }

    assert!(rendered.contains("problem"));
    assert!(layout.visual_cursor().is_some());
}

#[test]
fn test_layout_doc_symbols_picker_binding_opens_for_file_backed_buffer() {
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-doc-symbols-layout-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let path = temp_dir.join("example.rs");
    std::fs::write(&path, "fn example() {}\n").unwrap();

    let mut layout = layout_with_buffers(vec![Buffer::from_str_with_path(
        "fn example() {}\n",
        abs_path(&path),
    )]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenDocumentSymbolsPicker)));
    assert!(layout.doc_symbols_picker_is_open());

    std::fs::remove_file(path).ok();
    std::fs::remove_dir_all(temp_dir).ok();
}

#[test]
fn test_layout_workspace_symbols_picker_binding_opens_without_a_file_path() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("fn example() {}")]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::OpenWorkspaceSymbolsPicker)));
    assert!(layout.workspace_symbols_picker_is_open());

    let mut screen = crate::screen::Screen::new(8, 40);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 40));
    assert!(
        layout.visual_cursor().is_some(),
        "workspace picker prompt cursor should be visible"
    );
}

#[test]
fn test_layout_lsp_hover_closes_on_action() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    layout.open_lsp_hover("hover text".to_string(), Position::new(1, 1));

    assert!(layout.hover_is_open());

    assert!(layout.dispatch_intent(&Intent::Editor(EditorAction::new(
        EditorOperation::MoveRight
    ))));
    assert!(!layout.hover_is_open());
}

#[test]
fn test_layout_lsp_rename_job_failure_surfaces_notification() {
    let _guard = globals::notification_test_lock();
    globals::clear_notifications();

    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let buffer_id = layout.active_buffer_view().buffer_id();

    layout.dispatch_lsp_job_event(JobEvent::Completed {
        kind: JobKind::LspRename(buffer_id),
        token: JobToken::new(7),
        payload: Some(JobPayload::LspRename(Err("boom".to_string()))),
    });

    let message = globals::active_notification(std::time::Instant::now()).expect("notification");
    assert!(message.text.contains("LSP rename failed: boom"));
}

#[test]
fn test_layout_lsp_inlay_hint_chunk_replaces_existing_hints() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("let value = foo();")]);
    let buffer_id = layout.active_buffer_view().buffer_id();
    let syntax_generation = layout
        .active_buffer_view()
        .with_buffer(|buffer| buffer.syntax_generation())
        .expect("buffer");

    layout.dispatch_lsp_job_event(JobEvent::Chunk {
        kind: JobKind::LspInlayHints(buffer_id),
        token: JobToken::new(7),
        payload: JobPayload::LspInlayHintsChunk(LspInlayHintsChunk {
            buffer_id,
            syntax_generation,
            start_line: 0,
            end_line: 1,
            hints: vec![LspInlayHint {
                position: Cursor::new(0, 4),
                label: SmolStr::new("first"),
            }],
        }),
    });

    layout.dispatch_lsp_job_event(JobEvent::Chunk {
        kind: JobKind::LspInlayHints(buffer_id),
        token: JobToken::new(7),
        payload: JobPayload::LspInlayHintsChunk(LspInlayHintsChunk {
            buffer_id,
            syntax_generation,
            start_line: 0,
            end_line: 1,
            hints: vec![LspInlayHint {
                position: Cursor::new(0, 4),
                label: SmolStr::new("second"),
            }],
        }),
    });

    let line = layout
        .active_buffer_view()
        .with_buffer(|buffer| buffer.inlay_hints_for_line(0))
        .flatten()
        .expect("inlay hints");

    assert_eq!(line.len(), 1);
    assert_eq!(line[0].payload.label, SmolStr::new("second"));
}

#[test]
fn test_layout_lsp_inlay_hint_chunk_marks_visible_visuals_stale() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("let value = foo();")]);
    let buffer_id = layout.active_buffer_view().buffer_id();
    let syntax_generation = layout
        .active_buffer_view()
        .with_buffer(|buffer| buffer.syntax_generation())
        .expect("buffer");
    let mut screen = crate::screen::Screen::new(2, 40);
    layout.render(&mut screen, Position::new(0, 0), Size::new(2, 40));

    assert!(!layout.has_stale_visible_visuals());

    layout.dispatch_lsp_job_event(JobEvent::Chunk {
        kind: JobKind::LspInlayHints(buffer_id),
        token: JobToken::new(7),
        payload: JobPayload::LspInlayHintsChunk(LspInlayHintsChunk {
            buffer_id,
            syntax_generation,
            start_line: 0,
            end_line: 1,
            hints: vec![LspInlayHint {
                position: Cursor::new(0, 4),
                label: SmolStr::new("hint"),
            }],
        }),
    });

    assert!(layout.has_stale_visible_visuals());

    layout.render(&mut screen, Position::new(0, 0), Size::new(2, 40));

    assert!(!layout.has_stale_visible_visuals());
}

#[test]
fn test_layout_lsp_inlay_hint_chunk_pads_labels_on_insert() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("abcd")]);
    let buffer_id = layout.active_buffer_view().buffer_id();
    let syntax_generation = layout
        .active_buffer_view()
        .with_buffer(|buffer| buffer.syntax_generation())
        .expect("buffer");

    layout.dispatch_lsp_job_event(JobEvent::Chunk {
        kind: JobKind::LspInlayHints(buffer_id),
        token: JobToken::new(7),
        payload: JobPayload::LspInlayHintsChunk(LspInlayHintsChunk {
            buffer_id,
            syntax_generation,
            start_line: 0,
            end_line: 1,
            hints: vec![
                LspInlayHint {
                    position: Cursor::new(0, 2),
                    label: SmolStr::new("name:"),
                },
                LspInlayHint {
                    position: Cursor::new(0, 4),
                    label: SmolStr::new("Type"),
                },
            ],
        }),
    });

    let line = layout
        .active_buffer_view()
        .with_buffer(|buffer| buffer.inlay_hints_for_line(0))
        .flatten()
        .expect("inlay hints");

    assert_eq!(line.len(), 2);
    assert_eq!(line[0].payload.label, SmolStr::new("name: "));
    assert_eq!(line[1].payload.label, SmolStr::new(" Type"));
}

#[test]
fn test_layout_lsp_inlay_hint_chunk_ignores_stale_buffer_generation() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("abcd")]);
    let buffer_id = layout.active_buffer_view().buffer_id();
    let stale_generation = layout
        .active_buffer_view()
        .with_buffer(|buffer| buffer.syntax_generation())
        .expect("buffer")
        .wrapping_add(1);

    layout.dispatch_lsp_job_event(JobEvent::Chunk {
        kind: JobKind::LspInlayHints(buffer_id),
        token: JobToken::new(7),
        payload: JobPayload::LspInlayHintsChunk(LspInlayHintsChunk {
            buffer_id,
            syntax_generation: stale_generation,
            start_line: 0,
            end_line: 1,
            hints: vec![LspInlayHint {
                position: Cursor::new(0, 2),
                label: SmolStr::new("stale"),
            }],
        }),
    });

    assert!(
        layout
            .active_buffer_view()
            .with_buffer(|buffer| {
                buffer
                    .inlay_hints_for_line(0)
                    .is_none_or(|hints| hints.is_empty())
            })
            .expect("buffer")
    );
}

#[test]
fn test_layout_open_file_at_cursor_sets_cursor_position() {
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-open-file-at-cursor-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let path = temp_dir.join("match.txt");
    std::fs::write(&path, "hello world\n").unwrap();

    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    assert!(
        layout.dispatch_intent(&Intent::Command(Command::OpenFileAtCursor(
            path.clone(),
            crate::buffer::Cursor::new(0, 6),
        )))
    );
    assert_eq!(
        layout.active_buffer_view().cursor(),
        crate::buffer::Cursor::new(0, 6)
    );

    std::fs::remove_file(path).ok();
    std::fs::remove_dir_all(temp_dir).ok();
}

#[test]
fn test_layout_dispatch_intent_quit_exits_immediately() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::Quit)));
    assert!(layout.should_exit());
}

#[test]
fn test_layout_try_quit_without_modified_buffers_exits_immediately() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::TryQuit)));
    assert!(layout.should_exit());

    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
}

#[test]
fn test_layout_try_quit_with_modified_buffers_opens_confirmation_prompt() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let cursor = crate::buffer::Cursor::new(0, 1);
    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(cursor, "x"));

    assert!(layout.dispatch_intent(&Intent::Command(Command::TryQuit)));
    assert!(!layout.should_exit());
    assert!(layout.confirmation_box_is_open());
}

#[test]
fn test_layout_try_quit_counts_hidden_modified_buffers() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let visible = Buffer::from_str("one");
    let hidden_path = std::env::temp_dir().join(format!(
        "urvim-hidden-quit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::write(&hidden_path, "alpha").unwrap();

    let mut layout = layout_with_buffers(vec![visible]);
    let hidden_buffer_id =
        globals::with_buffer_pool(|pool| pool.open_buffer(&hidden_path)).unwrap();
    globals::with_buffer_mut(hidden_buffer_id, |buffer| {
        buffer.insert_text(crate::buffer::Cursor::new(0, 5), "-dirty");
    });

    assert!(layout.dispatch_intent(&Intent::Command(Command::TryQuit)));
    let prompt = layout
        .confirmation_box_mut()
        .expect("confirmation prompt should be open");
    assert_eq!(prompt.query(), "Quit without saving 1 buffer?");

    globals::with_buffer_pool(|pool| pool.save_buffer(hidden_buffer_id)).unwrap();

    std::fs::remove_file(hidden_path).ok();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
}

#[test]
fn test_layout_write_all_saves_hidden_modified_buffers() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let visible_path = std::env::temp_dir().join(format!(
        "urvim-visible-write-all-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let hidden_path = std::env::temp_dir().join(format!(
        "urvim-hidden-write-all-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::write(&visible_path, "alpha").unwrap();
    fs::write(&hidden_path, "gamma").unwrap();

    let visible = Buffer::from_str_with_path("alpha", abs_path(&visible_path));
    let mut layout = layout_with_buffers(vec![visible]);
    let hidden_buffer_id =
        globals::with_buffer_pool(|pool| pool.open_buffer(&hidden_path)).unwrap();
    globals::with_buffer_mut(hidden_buffer_id, |buffer| {
        buffer.insert_text(Cursor::new(0, 5), "-dirty");
    });
    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(Cursor::new(0, 5), "-dirty"))
        .unwrap();

    assert!(layout.dispatch_intent(&Intent::Command(Command::WriteAll)));
    assert_eq!(fs::read_to_string(&visible_path).unwrap(), "alpha-dirty");
    assert_eq!(fs::read_to_string(&hidden_path).unwrap(), "gamma-dirty");

    fs::remove_file(visible_path).ok();
    fs::remove_file(hidden_path).ok();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
}

#[test]
fn test_layout_write_all_prompts_when_disk_changed() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let path = std::env::temp_dir().join(format!(
        "urvim-write-all-confirm-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::write(&path, "alpha").unwrap();

    let buffer = Buffer::load_from_file(&path).unwrap();
    let mut layout = layout_with_buffers(vec![buffer]);
    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(Cursor::new(0, 5), "-dirty"))
        .unwrap();
    fs::write(&path, "alpha-external").unwrap();

    assert!(layout.dispatch_intent(&Intent::Command(Command::WriteAll)));
    assert!(layout.confirmation_box_is_open());
    assert_eq!(fs::read_to_string(&path).unwrap(), "alpha-external");

    fs::remove_file(path).ok();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
}

#[test]
fn test_layout_confirmation_prompt_returns_quit_intent_on_enter() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let cursor = crate::buffer::Cursor::new(0, 1);
    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(cursor, "x"));
    assert!(layout.dispatch_intent(&Intent::Command(Command::TryQuit)));

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Enter)));
    assert!(result.handled());
    assert_eq!(result.into_intents(), vec![Intent::Command(Command::Quit)]);
}

#[test]
fn test_layout_confirmation_prompt_cancels_on_n() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    let cursor = crate::buffer::Cursor::new(0, 1);
    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(cursor, "x"));
    assert!(layout.dispatch_intent(&Intent::Command(Command::TryQuit)));

    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Char('n'))));
    assert!(result.handled());
    assert!(result.into_intents().is_empty());
    assert!(!layout.should_exit());
}

#[test]
fn test_layout_routes_tick_to_overlay_before_base_layer() {
    let _guard = globals::notification_test_lock();
    globals::clear_notifications();
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert!(globals::enqueue_notification(
        crate::notification::NotificationLevel::Info,
        "saved".to_string(),
    ));

    let result = layout.route_ui_event(&UiEvent::Tick);
    assert!(matches!(result, UiEventResult::NotHandled));
    assert!(globals::active_notification(std::time::Instant::now()).is_some());
}

#[test]
fn test_layout_autocomplete_fires_after_debounce_on_insert_edit() {
    let _guard = globals::set_test_config(Config {
        completion_trigger: crate::config::CompletionTrigger::Auto,
        ..Config::default()
    });
    let mut layout = layout_with_buffers(vec![Buffer::from_str("al")]);
    layout
        .active_buffer_view_mut()
        .set_cursor(crate::buffer::Cursor::new(0, 2));
    layout.handle_insert_completion_change();
    layout.autocomplete.pending_since =
        Some(std::time::Instant::now() - Duration::from_millis(200));

    let result = layout.route_ui_event(&UiEvent::Tick);
    assert!(result.handled());
    let completion = layout
        .dialogs
        .completion
        .as_ref()
        .expect("autocomplete should open completion");
    assert!(completion.is_pending());
}

#[test]
fn test_layout_autocomplete_does_not_fire_on_whitespace() {
    let _guard = globals::set_test_config(Config {
        completion_trigger: crate::config::CompletionTrigger::Auto,
        ..Config::default()
    });
    let mut layout = layout_with_buffers(vec![Buffer::from_str("  ")]);
    layout
        .active_buffer_view_mut()
        .set_cursor(crate::buffer::Cursor::new(0, 2));
    layout.dispatch_intent(&Intent::Command(Command::OpenCompletion));
    layout.handle_insert_completion_change();
    layout.autocomplete.pending_since =
        Some(std::time::Instant::now() - Duration::from_millis(200));

    let result = layout.route_ui_event(&UiEvent::Tick);
    assert!(matches!(result, UiEventResult::NotHandled));
    assert!(layout.dialogs.completion.is_none());
}

#[test]
fn test_layout_layered_render_preserves_focus_and_cursor_in_split_layout() {
    let _guard = globals::notification_test_lock();
    globals::clear_notifications();
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical)),
        ActionResult::Handled
    );
    dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft));

    layout
        .active_buffer_view_mut()
        .set_cursor(crate::buffer::Cursor::new(0, 2));

    let mut screen = crate::screen::Screen::new(8, 40);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 40));
    let cursor_before = layout
        .visual_cursor()
        .expect("cursor should exist before layered render");

    assert!(globals::enqueue_notification(
        crate::notification::NotificationLevel::Warn,
        "queued".to_string(),
    ));

    let focused_before = layout.focused_pane;
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 40));

    assert_eq!(layout.focused_pane, focused_before);
    assert_eq!(layout.visual_cursor(), Some(cursor_before));
}

#[test]
fn test_layout_process_action_delegates_to_window_group() {
    let mut layout = layout_with_buffers(vec![
        Buffer::from_str("one"),
        Buffer::from_str("two"),
        Buffer::from_str("three"),
    ]);

    assert!(layout.dispatch_intent(&Intent::Command(Command::NextTab(1))));
    assert_eq!(layout.window_group().active_tab_index(), 1);
}

#[test]
fn test_layout_vertical_split_creates_second_pane_with_even_weights() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical)),
        ActionResult::Handled
    );

    let root = layout.root.as_ref().expect("layout should keep a root");
    assert_eq!(pane_count(root), 2);
    match root {
        LayoutNode::Split(split) => {
            assert_eq!(split.axis, SplitAxis::Vertical);
            assert_eq!(split.split_size.first_weight(), 1);
            assert_eq!(split.split_size.second_weight(), 1);
        }
        LayoutNode::Pane(_) => {
            panic!("split action should replace the root pane")
        }
    }
}

#[test]
fn test_layout_horizontal_split_creates_second_pane_with_even_weights() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal)),
        ActionResult::Handled
    );

    let root = layout.root.as_ref().expect("layout should keep a root");
    assert_eq!(pane_count(root), 2);
    match root {
        LayoutNode::Split(split) => {
            assert_eq!(split.axis, SplitAxis::Horizontal);
            assert_eq!(split.split_size.first_weight(), 1);
            assert_eq!(split.split_size.second_weight(), 1);
        }
        LayoutNode::Pane(_) => {
            panic!("split action should replace the root pane")
        }
    }
}

#[test]
fn test_layout_split_copies_active_buffer_view_state() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one\ntwo\nthree\nfour")]);
    let source_cursor = crate::buffer::Cursor::new(2, 3);
    let source_scroll = Position::new(1, 4);
    let source_wrapped_row = 3;

    layout.active_buffer_view_mut().set_cursor(source_cursor);
    layout
        .active_buffer_view_mut()
        .set_scroll_offset(source_scroll);
    layout
        .active_buffer_view_mut()
        .set_wrapped_row_offset(source_wrapped_row);
    layout
        .active_window_group_mut()
        .active_window_mut()
        .set_wrap_enabled(true);

    let source_buffer_id = layout.active_buffer_view().buffer_id();

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical)),
        ActionResult::Handled
    );

    assert_eq!(layout.focused_pane, PaneId(1));
    assert_eq!(layout.active_buffer_view().buffer_id(), source_buffer_id);
    assert_eq!(layout.active_buffer_view().cursor(), source_cursor);
    assert_eq!(layout.active_buffer_view().scroll_offset(), source_scroll);
    assert_eq!(
        layout.active_buffer_view().wrapped_row_offset(),
        source_wrapped_row
    );
    assert!(layout.active_window_group().active_window().wrap_enabled());

    let root = layout.root.as_ref().expect("layout should keep a root");
    match root {
        LayoutNode::Split(split) => {
            let original = pane_buffer_view(&split.first);
            let copied = pane_buffer_view(&split.second);

            assert_eq!(original.buffer_id(), source_buffer_id);
            assert_eq!(original.cursor(), source_cursor);
            assert_eq!(original.scroll_offset(), source_scroll);
            assert_eq!(original.wrapped_row_offset(), source_wrapped_row);
            assert_eq!(copied.buffer_id(), source_buffer_id);
            assert_eq!(copied.cursor(), source_cursor);
            assert_eq!(copied.scroll_offset(), source_scroll);
            assert_eq!(copied.wrapped_row_offset(), source_wrapped_row);
            assert!(pane_window(&split.first).wrap_enabled());
            assert!(pane_window(&split.second).wrap_enabled());
        }
        LayoutNode::Pane(_) => {
            panic!("split action should replace the root pane")
        }
    }
}

#[test]
fn test_layout_exposes_stable_window_ids_for_visible_panes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one\ntwo")]);

    assert_eq!(layout.active_window_id(), Some(PaneId(0)));
    assert_eq!(layout.window_ids(), vec![PaneId(0)]);

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical)),
        ActionResult::Handled
    );

    assert_eq!(layout.active_window_id(), Some(PaneId(1)));
    assert_eq!(layout.window_ids(), vec![PaneId(0), PaneId(1)]);
    assert!(layout.buffer_view_for_window(PaneId(0)).is_some());
    assert!(layout.buffer_view_for_window(PaneId(1)).is_some());

    assert!(layout.focus_pane(PaneId(0)));
    assert_eq!(layout.active_window_id(), Some(PaneId(0)));
    assert_eq!(layout.window_ids(), vec![PaneId(0), PaneId(1)]);

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ClosePane)),
        ActionResult::Handled
    );

    assert_eq!(layout.window_ids(), vec![PaneId(1)]);
    assert!(layout.buffer_view_for_window(PaneId(0)).is_none());
}

#[test]
fn test_layout_cycles_focus_across_panes_and_plugin_windows() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));
    let first_plugin = layout.create_plugin_window(
        "demo".to_string(),
        crate::ui::plugin_window::PluginWindowOptions::default(),
    );
    let second_plugin = layout.create_plugin_window(
        "demo".to_string(),
        crate::ui::plugin_window::PluginWindowOptions::default(),
    );
    layout.focus_pane(PaneId(0));

    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    assert!(layout.focus_next_window());
    assert_eq!(layout.active_window_id(), Some(PaneId(1)));
    assert_eq!(layout.plugin_windows().focused(), None);

    assert!(layout.focus_next_window());
    assert_eq!(layout.plugin_windows().focused(), Some(first_plugin));
    assert_eq!(layout.active_window_id(), Some(PaneId(1)));

    assert!(layout.focus_next_window());
    assert_eq!(layout.plugin_windows().focused(), Some(second_plugin));

    assert!(layout.focus_next_window());
    assert_eq!(layout.active_window_id(), Some(PaneId(0)));
    assert_eq!(layout.plugin_windows().focused(), None);

    assert!(layout.focus_previous_window());
    assert_eq!(layout.plugin_windows().focused(), Some(second_plugin));
}

#[test]
fn test_layout_creates_and_closes_targeted_plugin_pane() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("editor")]);
    let id = layout
        .create_plugin_pane(
            "demo".to_string(),
            None,
            SplitAxis::Vertical,
            SplitSize::new(2, 1),
            crate::ui::plugin_pane::PluginPaneOptions::default(),
        )
        .expect("plugin pane should be created beside the focused pane");

    assert_eq!(layout.focused_plugin_pane(), Some(id));
    assert_eq!(layout.pane_regions().len(), 2);
    let root = layout.root.as_ref().expect("layout should keep a root");
    match root {
        LayoutNode::Split(split) => {
            assert_eq!(split.axis, SplitAxis::Vertical);
            assert_eq!(split.split_size.first_weight(), 2);
            assert_eq!(split.split_size.second_weight(), 1);
            assert!(matches!(split.first.as_ref(), LayoutNode::Pane(_)));
            assert!(
                matches!(split.second.as_ref(), LayoutNode::Pane(pane) if pane.id == id && pane.is_plugin())
            );
        }
        _ => panic!("plugin pane creation should create a split"),
    }

    layout
        .close_plugin_pane("demo", id)
        .expect("plugin pane should close through its owner");
    assert_eq!(layout.focused_plugin_pane(), None);
    assert_eq!(layout.pane_regions().len(), 1);
}

#[test]
fn test_plugin_pane_render_updates_header_style_with_focus() {
    let theme = border_theme();
    let active_style = theme.resolve_name_with_default("ui.tab.active");
    let inactive_style = theme.resolve_name_with_default("ui.tab.inactive");
    let _theme_guard = globals::set_test_active_theme(theme);
    let mut layout = layout_with_buffers(vec![Buffer::from_str("editor")]);
    let id = layout
        .create_plugin_pane(
            "demo".to_string(),
            None,
            SplitAxis::Vertical,
            SplitSize::even(),
            crate::ui::plugin_pane::PluginPaneOptions {
                title: Some("Plugin".to_string()),
                ..Default::default()
            },
        )
        .expect("plugin pane should be created");
    layout
        .set_plugin_pane_content(
            "demo",
            id,
            vec![vec![crate::ui::plugin_window::PluginWindowSegment {
                text: "content".to_string(),
                style: None,
            }]],
        )
        .unwrap();
    let mut screen = crate::screen::Screen::new(4, 21);
    let rect = crate::ui::UiRect::new(Position::new(0, 0), Size::new(4, 21));

    assert_eq!(layout.focused_plugin_pane(), Some(id));
    let focused = layout.focused_plugin_pane() == Some(id);
    layout
        .plugin_pane("demo", id)
        .unwrap()
        .render(&mut screen, rect, focused);
    let title_col = (rect.size.cols - "Plugin".len() as u16) / 2;
    assert_eq!(
        screen
            .get_cell_mut(rect.origin.row, title_col)
            .unwrap()
            .style,
        active_style
    );

    assert!(layout.focus_layout_pane(PaneId(0)));
    assert_eq!(layout.focused_plugin_pane(), None);
    let focused = layout.focused_plugin_pane() == Some(id);
    layout
        .plugin_pane("demo", id)
        .unwrap()
        .render(&mut screen, rect, focused);
    assert_eq!(
        screen
            .get_cell_mut(rect.origin.row, title_col)
            .unwrap()
            .style,
        inactive_style
    );
}

#[test]
fn test_command_line_cursor_takes_precedence_over_focused_plugin_pane() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("editor")]);
    let id = layout
        .create_plugin_pane(
            "demo".to_string(),
            None,
            SplitAxis::Vertical,
            SplitSize::even(),
            crate::ui::plugin_pane::PluginPaneOptions::default(),
        )
        .expect("plugin pane should be created");
    assert_eq!(layout.visual_cursor(), None);

    layout.open_command_line();
    let mut screen = crate::screen::Screen::new(8, 40);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 40));

    assert!(layout.visual_cursor().is_some());
    assert_eq!(layout.focused_plugin_pane(), Some(id));

    layout.close_command_line();
    assert_eq!(layout.visual_cursor(), None);
    assert_eq!(layout.focused_plugin_pane(), Some(id));
}

#[test]
fn test_plugin_pane_routes_standard_focus_sequences() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("editor")]);
    let plugin_id = layout
        .create_plugin_pane(
            "demo".to_string(),
            None,
            SplitAxis::Vertical,
            SplitSize::even(),
            crate::ui::plugin_pane::PluginPaneOptions::default(),
        )
        .expect("plugin pane should be created");
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    let ctrl_w = Key {
        code: KeyCode::Char('w'),
        modifiers: Modifiers::CTRL,
    };
    assert!(layout.route_ui_event(&UiEvent::Key(ctrl_w)).handled());
    let result = layout.route_ui_event(&UiEvent::Key(key(KeyCode::Char('h'))));
    assert_eq!(
        result,
        UiEventResult::Handled(vec![Intent::Command(Command::FocusPaneLeft)])
    );
    assert!(layout.dispatch_intent(&Intent::Command(Command::FocusPaneLeft)));
    assert_eq!(layout.focused_plugin_pane(), None);
    assert_eq!(layout.active_window_id(), Some(PaneId(0)));

    layout.focus_plugin_pane("demo", plugin_id).unwrap();
    assert!(layout.route_ui_event(&UiEvent::Key(ctrl_w)).handled());
    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::Char('x')))),
        UiEventResult::Handled(Vec::new())
    );
}

#[test]
fn test_plugin_pane_consumes_editor_motions_without_mutating_hidden_editor() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one\ntwo\nthree")]);
    let plugin_id = layout
        .create_plugin_pane(
            "demo".to_string(),
            None,
            SplitAxis::Vertical,
            SplitSize::even(),
            crate::ui::plugin_pane::PluginPaneOptions::default(),
        )
        .expect("plugin pane should be created");
    let before = layout.active_buffer_view().cursor();

    for code in [
        KeyCode::Char('h'),
        KeyCode::Char('j'),
        KeyCode::Char('k'),
        KeyCode::Char('l'),
    ] {
        assert_eq!(
            layout.route_ui_event(&UiEvent::Key(key(code))),
            UiEventResult::Handled(Vec::new())
        );
    }

    assert_eq!(layout.focused_plugin_pane(), Some(plugin_id));
    assert_eq!(layout.active_buffer_view().cursor(), before);
}

#[test]
fn test_plugin_pane_inherits_rebound_focus_mapping_without_hardcoded_default() {
    let _config_guard = globals::set_test_config(Config {
        keymaps: KeymapsConfig {
            normal: std::collections::BTreeMap::from([
                ("<C-h>".to_string(), "pane focus-left".to_string()),
                ("<C-w>h".to_string(), "pane wrap-toggle".to_string()),
            ]),
            ..Default::default()
        },
        ..Default::default()
    });
    let mut layout = layout_with_buffers(vec![Buffer::from_str("editor")]);
    layout
        .create_plugin_pane(
            "demo".to_string(),
            None,
            SplitAxis::Vertical,
            SplitSize::even(),
            crate::ui::plugin_pane::PluginPaneOptions::default(),
        )
        .expect("plugin pane should be created");
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    let ctrl_w = Key::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL);
    assert!(layout.route_ui_event(&UiEvent::Key(ctrl_w)).handled());
    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::Char('h')))),
        UiEventResult::Handled(Vec::new())
    );
    assert!(layout.focused_plugin_pane().is_some());

    let ctrl_h = Key::with_modifiers(KeyCode::Char('h'), Modifiers::CTRL);
    let result = layout.route_ui_event(&UiEvent::Key(ctrl_h));
    assert_eq!(
        result,
        UiEventResult::Handled(vec![Intent::Command(Command::FocusPaneLeft)])
    );
    assert!(layout.dispatch_intent(&Intent::Command(Command::FocusPaneLeft)));
    assert_eq!(layout.focused_plugin_pane(), None);
}

#[test]
fn test_plugin_pane_local_mapping_wins_over_inherited_mapping() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("editor")]);
    let plugin_id = layout
        .create_plugin_pane(
            "demo".to_string(),
            None,
            SplitAxis::Vertical,
            SplitSize::even(),
            crate::ui::plugin_pane::PluginPaneOptions::default(),
        )
        .expect("plugin pane should be created");
    layout
        .set_plugin_pane_keymap(
            "demo",
            plugin_id,
            vec!["<C-w>".to_string(), "h".to_string()],
            "pane wrap-toggle".to_string(),
            Intent::Command(Command::ToggleWrap),
        )
        .expect("local mapping should be installed");

    let ctrl_w = Key::with_modifiers(KeyCode::Char('w'), Modifiers::CTRL);
    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(ctrl_w)),
        UiEventResult::Handled(Vec::new())
    );
    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::Char('h')))),
        UiEventResult::Handled(vec![Intent::Command(Command::ToggleWrap)])
    );
    assert_eq!(layout.focused_plugin_pane(), Some(plugin_id));
}

#[test]
fn test_plugin_pane_inherits_application_mapping() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("editor")]);
    layout
        .create_plugin_pane(
            "demo".to_string(),
            None,
            SplitAxis::Vertical,
            SplitSize::even(),
            crate::ui::plugin_pane::PluginPaneOptions::default(),
        )
        .expect("plugin pane should be created");

    assert_eq!(
        layout.route_ui_event(&UiEvent::Key(key(KeyCode::F1))),
        UiEventResult::Handled(vec![Intent::Command(Command::OpenFilePicker)])
    );
}

#[test]
fn test_layout_wrap_toggle_is_window_local() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one\ntwo")]);
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical)),
        ActionResult::Handled
    );
    assert!(!layout.active_window_group().active_window().wrap_enabled());

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ToggleWrap)),
        ActionResult::Handled
    );
    assert!(layout.active_window_group().active_window().wrap_enabled());
    let root = layout.root.as_ref().expect("layout should keep a root");
    match root {
        LayoutNode::Split(split) => {
            assert!(!pane_window(&split.first).wrap_enabled());
            assert!(pane_window(&split.second).wrap_enabled());
        }
        LayoutNode::Pane(_) => {
            panic!("split action should replace the root pane")
        }
    }
}

#[test]
fn test_layout_close_pane_exits_when_last_pane_is_removed() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ClosePane)),
        ActionResult::Handled
    );
    assert!(layout.should_exit());
}

#[test]
fn test_layout_render_stores_geometry_and_forwards_size() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let _notification_guard = globals::notification_test_lock();
    globals::clear_notifications();
    let _config_guard = globals::set_test_config(Config::default());
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    let mut screen = crate::screen::Screen::new(6, 20);
    let origin = Position::new(3, 4);
    let size = Size::new(3, 12);

    layout.render(&mut screen, origin, size);

    assert_eq!(layout.origin(), origin);
    assert_eq!(layout.size(), size);
    assert_eq!(
        layout.window_group().active_window().size(),
        Size::new(1, 12)
    );
    assert_eq!(screen.get_cell_mut(5, 4).unwrap().text, "N");
}

#[test]
fn test_layout_render_uses_a_fixed_width_command_line_frame() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout.open_command_line();
    layout
        .dialogs
        .command_line
        .input_widget_mut()
        .set_text("1234567890123456789012345678901234567890123456789012345678901234");

    let mut screen = crate::screen::Screen::new(4, 60);
    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 60));

    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "+");
    assert_eq!(screen.get_cell_mut(0, 56).unwrap().text, "+");
    assert_eq!(screen.get_cell_mut(1, 3).unwrap().text, ":");
    assert_eq!(layout.visual_cursor(), Some(Position::new(1, 55)));
    assert_eq!(screen.get_cell_mut(1, 54).unwrap().text, "4");
    assert_eq!(screen.get_cell_mut(1, 55).unwrap().text, " ");
}

#[test]
fn test_layout_render_divides_vertical_split_width_by_weights() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 13);
    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 13));

    let root = layout.root.as_ref().expect("layout should keep a root");
    let mut regions = Vec::new();
    Layout::collect_pane_regions(root, Position::new(0, 0), Size::new(4, 13), &mut regions);
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].size.cols, 6);
    assert_eq!(regions[1].size.cols, 6);
}

#[test]
fn test_layout_render_divides_horizontal_split_rows_by_weights() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));

    let mut screen = crate::screen::Screen::new(8, 10);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 10));

    let root = layout.root.as_ref().expect("layout should keep a root");
    let mut regions = Vec::new();
    Layout::collect_pane_regions(root, Position::new(0, 0), Size::new(7, 10), &mut regions);
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].size.rows, 3);
    assert_eq!(regions[1].size.rows, 3);
}

#[test]
fn test_layout_resize_left_moves_vertical_split_for_the_left_pane() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 13);
    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 13));

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft)),
        ActionResult::Handled
    );

    let regions_before = layout.pane_regions();
    let left_before = regions_before
        .iter()
        .find(|region| region.id == PaneId(0))
        .expect("left pane should be visible before resize");
    let right_before = regions_before
        .iter()
        .find(|region| region.id == PaneId(1))
        .expect("right pane should be visible before resize");

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ResizePaneLeft(1))),
        ActionResult::Handled
    );

    let regions_after = layout.pane_regions();
    let left_after = regions_after
        .iter()
        .find(|region| region.id == PaneId(0))
        .expect("left pane should be visible after resize");
    let right_after = regions_after
        .iter()
        .find(|region| region.id == PaneId(1))
        .expect("right pane should be visible after resize");

    assert!(left_after.size.cols < left_before.size.cols);
    assert!(right_after.size.cols > right_before.size.cols);
}

#[test]
fn test_layout_resize_right_moves_vertical_split_for_the_right_pane() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 13);
    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 13));

    let regions_before = layout.pane_regions();
    let left_before = regions_before
        .iter()
        .find(|region| region.id == PaneId(0))
        .expect("left pane should be visible before resize");
    let right_before = regions_before
        .iter()
        .find(|region| region.id == PaneId(1))
        .expect("right pane should be visible before resize");

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ResizePaneRight(1))),
        ActionResult::Handled
    );

    let regions_after = layout.pane_regions();
    let left_after = regions_after
        .iter()
        .find(|region| region.id == PaneId(0))
        .expect("left pane should be visible after resize");
    let right_after = regions_after
        .iter()
        .find(|region| region.id == PaneId(1))
        .expect("right pane should be visible after resize");

    assert!(left_after.size.cols > left_before.size.cols);
    assert!(right_after.size.cols < right_before.size.cols);
}

#[test]
fn test_layout_resize_counted_steps_apply_larger_changes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 21);
    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 21));

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft)),
        ActionResult::Handled
    );

    let regions_before = layout.pane_regions();
    let left_before = regions_before
        .iter()
        .find(|region| region.id == PaneId(0))
        .expect("left pane should be visible before counted resize");
    let right_before = regions_before
        .iter()
        .find(|region| region.id == PaneId(1))
        .expect("right pane should be visible before counted resize");

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ResizePaneLeft(5))),
        ActionResult::Handled
    );

    let regions_after = layout.pane_regions();
    let left_after = regions_after
        .iter()
        .find(|region| region.id == PaneId(0))
        .expect("left pane should be visible after counted resize");
    let right_after = regions_after
        .iter()
        .find(|region| region.id == PaneId(1))
        .expect("right pane should be visible after counted resize");

    assert_eq!(left_before.size.cols - left_after.size.cols, 5);
    assert_eq!(right_after.size.cols - right_before.size.cols, 5);
}

#[test]
fn test_layout_equalize_splits_recursively_resets_weights() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));
    dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft));
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ResizePaneDown(1))),
        ActionResult::Handled
    );
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneRight)),
        ActionResult::Handled
    );
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ResizePaneLeft(1))),
        ActionResult::Handled
    );

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::EqualizeSplits)),
        ActionResult::Handled
    );

    let root = layout.root.as_ref().expect("layout should keep a root");
    assert_all_splits_even(root);
}

#[test]
fn test_layout_resize_clamps_and_stays_local_to_the_matching_split() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft)),
        ActionResult::Handled
    );
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    let regions_before = layout.pane_regions();
    let focused_before = regions_before
        .iter()
        .find(|region| region.id == layout.focused_pane)
        .expect("focused pane should be visible before resize");
    let inner_sibling_before = regions_before
        .iter()
        .find(|region| region.id == PaneId(0))
        .expect("inner sibling should be visible before resize");
    let outer_sibling_before = regions_before
        .iter()
        .find(|region| region.id == PaneId(1))
        .expect("outer sibling should be visible before resize");

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ResizePaneDown(1))),
        ActionResult::Handled
    );

    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    let regions_after = layout.pane_regions();
    let focused_after = regions_after
        .iter()
        .find(|region| region.id == layout.focused_pane)
        .expect("focused pane should be visible after resize");
    let inner_sibling_after = regions_after
        .iter()
        .find(|region| region.id == PaneId(0))
        .expect("inner sibling should be visible after resize");
    let outer_sibling_after = regions_after
        .iter()
        .find(|region| region.id == PaneId(1))
        .expect("outer sibling should be visible after resize");

    assert_eq!(outer_sibling_after.size, outer_sibling_before.size);
    assert!(focused_after.size.rows < focused_before.size.rows);
    assert!(inner_sibling_after.size.rows > inner_sibling_before.size.rows);
    assert_eq!(
        focused_after.size.rows + inner_sibling_after.size.rows,
        focused_before.size.rows + inner_sibling_before.size.rows
    );

    for _ in 0..20 {
        assert_eq!(
            dispatch_layout_action(&mut layout, Intent::Command(Command::ResizePaneUp(1))),
            ActionResult::Handled
        );
    }

    let root_after_clamp = layout.root.as_ref().expect("layout should keep a root");
    match root_after_clamp {
        LayoutNode::Split(outer) => match outer.first.as_ref() {
            LayoutNode::Split(inner) => {
                assert_eq!(inner.split_size.first_weight(), 1);
                assert_eq!(
                    inner.split_size.first_weight() + inner.split_size.second_weight(),
                    focused_after.size.rows + inner_sibling_after.size.rows
                );
            }
            LayoutNode::Pane(_) => {
                panic!("expected nested split on the left side")
            }
        },
        LayoutNode::Pane(_) => {
            panic!("resize test should keep the root split")
        }
    }
}

#[test]
fn test_layout_visual_cursor_tracks_child() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout
        .active_buffer_view_mut()
        .set_cursor(crate::buffer::Cursor::new(0, 0));

    let mut screen = crate::screen::Screen::new(3, 12);
    layout.render(&mut screen, Position::new(0, 0), Size::new(3, 12));

    let cursor = layout.visual_cursor().unwrap();
    assert_eq!(cursor.row, 1);
}

#[test]
fn test_layout_mode_kind_updates_footer() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let _notification_guard = globals::notification_test_lock();
    globals::clear_notifications();
    let _config_guard = globals::set_test_config(Config::default());
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout
        .window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::Insert);

    let mut screen = crate::screen::Screen::new(3, 12);
    layout.render(&mut screen, Position::new(0, 0), Size::new(3, 12));

    assert_eq!(screen.get_cell_mut(2, 0).unwrap().text, "I");
}

#[test]
fn test_layout_nested_mixed_axis_split_creates_three_panes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical)),
        ActionResult::Handled
    );
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal)),
        ActionResult::Handled
    );

    let root = layout.root.as_ref().expect("layout should keep a root");
    assert_eq!(pane_count(root), 3);
    let mut ids = Vec::new();
    collect_pane_ids(root, &mut ids);
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&layout.focused_pane));
}

#[test]
fn test_layout_render_includes_filetype_label() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.rs")).unwrap();
    let buffer = Buffer::from_str_with_path("fn main() {}", path);
    let mut layout = layout_with_buffers(vec![buffer]);
    let mut screen = crate::screen::Screen::new(3, 40);

    layout.render(&mut screen, Position::new(0, 0), Size::new(3, 40));

    assert_eq!(screen.get_cell_mut(2, 9).unwrap().text, "R");
}

#[test]
fn test_layout_render_keeps_syntax_label_when_syntax_disabled() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.rs")).unwrap();
    let buffer = Buffer::from_str_with_path("fn main() {}", path);
    let mut layout = layout_with_buffers(vec![buffer]);
    let mut screen = crate::screen::Screen::new(3, 40);
    let _theme_guard = globals::set_test_active_theme(border_theme());
    let _config_guard = globals::set_test_config(Config {
        theme: "Friday Night".to_string(),
        syntax: false,
        auto_close_pairs: true,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    layout.render(&mut screen, Position::new(0, 0), Size::new(3, 40));

    let content_col = (0..40)
        .find(|col| screen.get_cell_mut(1, *col).unwrap().text == "f")
        .expect("buffer content should render");
    assert_eq!(screen.get_cell_mut(1, content_col).unwrap().text, "f");
    assert_eq!(
        screen.get_cell_mut(1, content_col).unwrap().style,
        border_theme().default_style()
    );
    assert_eq!(screen.get_cell_mut(2, 9).unwrap().text, "R");
}

#[test]
fn test_layout_render_omits_split_borders_for_single_pane_layouts() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.rs")).unwrap();
    let buffer = Buffer::from_str_with_path("hi", path);
    let mut layout = layout_with_buffers(vec![buffer]);
    let mut screen = crate::screen::Screen::new(4, 20);
    let theme = border_theme();
    let border_style = theme.resolve_name_with_default("ui.window.lines.border");
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(border_config(true));

    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    assert_ne!(screen.get_cell_mut(0, 9).unwrap().style, border_style);
}

#[test]
fn test_layout_render_draws_split_border_junction_in_unicode_mode() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let _notification_guard = globals::notification_test_lock();
    globals::clear_notifications();
    let _theme_guard = globals::set_test_active_theme(border_theme());
    let _config_guard = globals::set_test_config(border_config(true));
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 20);

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));
    dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft));
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));

    assert_eq!(screen.get_cell_mut(0, 9).unwrap().text, "│");
    assert_eq!(screen.get_cell_mut(1, 8).unwrap().text, "─");
    assert_eq!(screen.get_cell_mut(1, 9).unwrap().text, "┤");
}

#[test]
fn test_layout_render_draws_split_border_junction_in_ascii_mode() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let _notification_guard = globals::notification_test_lock();
    globals::clear_notifications();
    let _theme_guard = globals::set_test_active_theme(border_theme());
    let _config_guard = globals::set_test_config(border_config(false));
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 20);

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));
    dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft));
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));

    assert_eq!(screen.get_cell_mut(0, 9).unwrap().text, "|");
    assert_eq!(screen.get_cell_mut(1, 8).unwrap().text, "-");
    assert_eq!(screen.get_cell_mut(1, 9).unwrap().text, "+");
}

#[test]
fn test_layout_render_draws_four_way_split_junction_in_unicode_mode() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let _notification_guard = globals::notification_test_lock();
    globals::clear_notifications();
    let _theme_guard = globals::set_test_active_theme(border_theme());
    let _config_guard = globals::set_test_config(border_config(true));
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 20);

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));
    dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft));
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));
    dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneRight));
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));

    assert_eq!(screen.get_cell_mut(1, 9).unwrap().text, "┼");
}

#[test]
fn test_layout_render_draws_four_way_split_junction_in_ascii_mode() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let _notification_guard = globals::notification_test_lock();
    globals::clear_notifications();
    let _theme_guard = globals::set_test_active_theme(border_theme());
    let _config_guard = globals::set_test_config(border_config(false));
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 20);

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));
    dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft));
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));
    dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneRight));
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));

    assert_eq!(screen.get_cell_mut(1, 9).unwrap().text, "+");
}

#[test]
fn test_layout_render_uses_resize_border_style_in_resize_mode() {
    let _pool_guard = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let _notification_guard = globals::notification_test_lock();
    globals::clear_notifications();
    let _theme_guard = globals::set_test_active_theme(border_theme());
    let _config_guard = globals::set_test_config(border_config(true));
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));
    layout
        .window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::Resizing);

    let mut screen = crate::screen::Screen::new(4, 20);

    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    assert_eq!(
        screen.get_cell_mut(0, 9).unwrap().style,
        Style::new().fg(Color::ansi(160)).bold().bg(Color::ansi(30))
    );
}

#[test]
fn test_layout_focus_moves_across_rendered_vertical_split() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(4, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft)),
        ActionResult::Handled
    );
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneRight)),
        ActionResult::Handled
    );
}

#[test]
fn test_layout_focus_moves_across_nested_mixed_axis_splits() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));

    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneUp)),
        ActionResult::Handled
    );
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft)),
        ActionResult::Handled
    );
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneRight)),
        ActionResult::Handled
    );
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneDown)),
        ActionResult::Handled
    );
}

#[test]
fn test_layout_restores_last_focused_pane_when_reentering_split_subtree() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft)),
        ActionResult::Handled
    );
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));

    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneRight)),
        ActionResult::Handled
    );
    assert_eq!(layout.focused_pane, PaneId(1));
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft)),
        ActionResult::Handled
    );
    assert_eq!(layout.focused_pane, PaneId(2));
}

#[test]
fn test_layout_falls_back_to_surviving_pane_when_remembered_pane_closes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));
    let mut screen = crate::screen::Screen::new(8, 24);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 24));
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft)),
        ActionResult::Handled
    );
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitHorizontal));
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    let mut screen = crate::screen::Screen::new(8, 24);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 24));

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ClosePane)),
        ActionResult::Handled
    );

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneRight)),
        ActionResult::Handled
    );
    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft)),
        ActionResult::Handled
    );
    assert_eq!(layout.focused_pane, PaneId(0));
}

#[test]
fn test_layout_close_pane_collapses_parent_split_to_surviving_child() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    assert_eq!(
        dispatch_layout_action(&mut layout, Intent::Command(Command::ClosePane)),
        ActionResult::Handled
    );

    let root = layout.root.as_ref().expect("layout should keep one pane");
    assert_eq!(pane_count(root), 1);
    assert!(matches!(root, LayoutNode::Pane(_)));
}

#[test]
fn test_layout_prunes_empty_window_group_during_render() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));

    assert!(layout.active_window_group_mut().close_active_tab());

    let mut screen = crate::screen::Screen::new(4, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    let root = layout
        .root
        .as_ref()
        .expect("layout should keep surviving pane");
    assert_eq!(pane_count(root), 1);
}

#[test]
fn test_layout_prunes_expired_yank_flash_during_tick() {
    let theme = border_theme();
    let expected_style = theme.highlight_style_for_name("ui.selection");
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo".to_string(),
        syntax: true,
        auto_close_pairs: true,
        auto_indent: crate::config::AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    let buffer = Buffer::from_str("alpha");
    let mut layout = layout_with_buffers(vec![buffer]);
    assert_eq!(
        layout
            .active_window_group_mut()
            .active_window_mut()
            .dispatch_action(&EditorAction::new(EditorOperation::YankLine)),
        ActionResult::Handled
    );

    thread::sleep(Duration::from_millis(220));

    assert!(layout.prune_expired_yank_flashes());

    let mut screen = crate::screen::Screen::new(3, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(3, 20));

    let line = layout
        .active_window_group()
        .active_window()
        .render_data()
        .get_line(0)
        .expect("rendered line should exist");
    assert!(!line.iter().any(|chunk| chunk.style == expected_style));
}

#[test]
fn test_layout_preserves_unrelated_pane_cursor_and_mode_state() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout
        .active_buffer_view_mut()
        .set_cursor(crate::buffer::Cursor::new(0, 2));
    layout
        .active_window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::Insert);

    dispatch_layout_action(&mut layout, Intent::Command(Command::SplitVertical));
    let mut screen = crate::screen::Screen::new(4, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 20));
    dispatch_layout_action(&mut layout, Intent::Command(Command::FocusPaneLeft));

    assert_eq!(
        layout.active_buffer_view().cursor(),
        crate::buffer::Cursor::new(0, 2)
    );
    assert_eq!(layout.active_window_mode_kind(), ModeKind::Insert);
}

#[test]
fn test_layout_process_workspace_file_operations_closes_deleted_tabs() {
    let _lock = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    globals::clear_workspace_file_operation_notifications();

    let layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    let buffer_id = layout.active_buffer_view().buffer_id();

    globals::enqueue_workspace_file_operation_notification(
        globals::WorkspaceFileOperationNotification::Delete {
            path: std::path::PathBuf::from("alpha.txt"),
            buffer_id: Some(buffer_id),
        },
    );

    let mut layout = layout;
    assert!(layout.process_workspace_file_operations());
    assert!(layout.should_exit());
}

fn drain_editor_events() -> Vec<EditorEvent> {
    std::iter::from_fn(globals::take_editor_event).collect()
}

fn initial_lifecycle_event_names(events: &[EditorEvent]) -> Vec<&'static str> {
    events
        .iter()
        .filter_map(|event| match event {
            EditorEvent::WindowCreated { .. } => Some("WindowCreated"),
            EditorEvent::TabOpened { .. } => Some("TabOpened"),
            EditorEvent::BufferOpened { .. } => Some("BufferOpened"),
            EditorEvent::WindowFocused { .. } => Some("WindowFocused"),
            EditorEvent::TabActivated { .. } => Some("TabActivated"),
            EditorEvent::ActiveBufferChanged { .. } => Some("ActiveBufferChanged"),
            EditorEvent::EditorStarted => Some("EditorStarted"),
            _ => None,
        })
        .collect()
}

#[test]
fn fresh_layout_emits_initial_lifecycle_before_editor_started() {
    let _lock = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let window_group =
        WindowGroup::from_buffers(vec![Buffer::from_str("first"), Buffer::from_str("second")]);
    drain_editor_events();

    let layout = Layout::new(window_group);
    globals::enqueue_editor_event(EditorEvent::EditorStarted);

    let events = drain_editor_events();
    assert_eq!(
        initial_lifecycle_event_names(&events),
        vec![
            "WindowCreated",
            "TabOpened",
            "TabOpened",
            "BufferOpened",
            "BufferOpened",
            "WindowFocused",
            "TabActivated",
            "ActiveBufferChanged",
            "EditorStarted",
        ]
    );
    let active_buffer_id = layout.active_buffer_view().buffer_id();
    assert_eq!(
        globals::with_active_buffer_id(|id| id),
        Some(active_buffer_id)
    );
    assert!(matches!(
        events.as_slice(),
        [
            EditorEvent::WindowCreated {
                window_id: PaneId(0),
                tab_id: created_tab_id,
                buffer_id: created_buffer_id,
            },
            EditorEvent::TabOpened {
                window_id: PaneId(0),
                tab_id: first_tab_id,
                snapshot: first_snapshot,
            },
            EditorEvent::TabOpened { .. },
            EditorEvent::BufferOpened { .. },
            EditorEvent::BufferOpened { .. },
            EditorEvent::WindowFocused {
                previous_window_id: None,
                window_id: PaneId(0),
                tab_id: focused_tab_id,
                buffer_id: focused_buffer_id,
            },
            EditorEvent::TabActivated {
                previous_tab_id: None,
                window_id: PaneId(0),
                tab_id: activated_tab_id,
                buffer_id: activated_buffer_id,
            },
            EditorEvent::ActiveBufferChanged {
                previous_buffer_id: None,
                window_id: PaneId(0),
                tab_id: active_tab_id,
                buffer_id,
            },
            EditorEvent::EditorStarted,
        ] if *created_tab_id == *first_tab_id
            && *first_tab_id == *focused_tab_id
            && *focused_tab_id == *activated_tab_id
            && *activated_tab_id == *active_tab_id
            && *created_buffer_id == first_snapshot.buffer_id
            && *created_buffer_id == *focused_buffer_id
            && *focused_buffer_id == *activated_buffer_id
            && *activated_buffer_id == *buffer_id
    ));
}

#[test]
fn restored_multi_pane_layout_emits_initial_lifecycle_once_in_order() {
    let _lock = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-initial-layout-events-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).unwrap();
    let first_path = temp_dir.join("first.txt");
    let second_path = temp_dir.join("second.txt");
    fs::write(&first_path, "first").unwrap();
    fs::write(&second_path, "second").unwrap();

    let mut source = Layout::new(WindowGroup::from_paths(std::slice::from_ref(&first_path)));
    assert!(source.dispatch_intent(&Intent::Command(Command::SplitVertical)));
    let second_buffer_id =
        globals::with_buffer_pool(|pool| pool.open_buffer(&second_path)).unwrap();
    source.activate_or_open_buffer(second_buffer_id);
    let session = source.to_session();
    drain_editor_events();

    let restored = Layout::from_session(session);
    globals::enqueue_editor_event(EditorEvent::EditorStarted);

    let events = drain_editor_events();
    assert_eq!(
        initial_lifecycle_event_names(&events),
        vec![
            "WindowCreated",
            "WindowCreated",
            "TabOpened",
            "TabOpened",
            "TabOpened",
            "BufferOpened",
            "BufferOpened",
            "WindowFocused",
            "TabActivated",
            "TabActivated",
            "ActiveBufferChanged",
            "EditorStarted",
        ]
    );
    assert_eq!(restored.active_window_id(), Some(PaneId(1)));
    assert_eq!(
        globals::with_active_buffer_id(|id| id),
        Some(restored.active_buffer_view().buffer_id())
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, EditorEvent::WindowFocused { .. }))
            .count(),
        1
    );

    fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
fn layout_open_buffer_emits_ordered_tab_buffer_and_active_events() {
    let _lock = globals::buffer_pool_test_lock();
    globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
    let mut layout = layout_with_buffers(vec![Buffer::from_str("visible")]);
    let previous_buffer_id = layout.active_buffer_view().buffer_id();
    let previous_tab_id = layout.active_window_group().active_tab_id().unwrap();
    let buffer_id = globals::with_buffer_pool(|pool| pool.create_buffer());
    drain_editor_events();

    layout.activate_or_open_buffer(buffer_id);

    let events = drain_editor_events();
    assert!(matches!(
        events.as_slice(),
        [
            EditorEvent::TabOpened {
                window_id: PaneId(0),
                tab_id: opened_tab_id,
                snapshot: opened_snapshot,
            },
            EditorEvent::BufferOpened { snapshot },
            EditorEvent::TabActivated {
                window_id: PaneId(0),
                previous_tab_id: Some(previous_tab),
                tab_id: activated_tab_id,
                buffer_id: activated_buffer_id,
            },
            EditorEvent::ActiveBufferChanged {
                previous_buffer_id: Some(previous),
                buffer_id: active_buffer_id,
                window_id: PaneId(0),
                tab_id: active_tab_id,
            }
        ] if opened_snapshot.buffer_id == buffer_id
            && snapshot.buffer_id == buffer_id
            && *previous_tab == previous_tab_id
            && *opened_tab_id == *activated_tab_id
            && *activated_tab_id == *active_tab_id
            && *activated_buffer_id == buffer_id
            && *previous == previous_buffer_id
            && *active_buffer_id == buffer_id
    ));
}

#[test]
fn layout_split_emits_window_before_tab_then_focus_without_buffer_open() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("shared")]);
    let buffer_id = layout.active_buffer_view().buffer_id();
    let previous_tab_id = layout.active_window_group().active_tab_id().unwrap();
    drain_editor_events();

    assert!(layout.dispatch_intent(&Intent::Command(Command::SplitVertical)));

    let events = drain_editor_events();
    assert!(matches!(
        events.as_slice(),
        [
            EditorEvent::WindowCreated {
                window_id: PaneId(1),
                buffer_id: created_buffer_id,
                tab_id: created_tab_id,
            },
            EditorEvent::TabOpened {
                window_id: PaneId(1),
                tab_id: opened_tab_id,
                snapshot: opened_snapshot,
            },
            EditorEvent::WindowFocused {
                previous_window_id: Some(PaneId(0)),
                window_id: PaneId(1),
                buffer_id: focused_buffer_id,
                tab_id: focused_tab_id,
            },
            EditorEvent::TabActivated {
                previous_tab_id: None,
                window_id: PaneId(1),
                tab_id: activated_tab_id,
                buffer_id: activated_buffer_id,
            }
        ] if *created_buffer_id == buffer_id
            && opened_snapshot.buffer_id == buffer_id
            && *focused_buffer_id == buffer_id
            && *activated_buffer_id == buffer_id
            && *created_tab_id == *opened_tab_id
            && *opened_tab_id == *focused_tab_id
            && *focused_tab_id == *activated_tab_id
            && *created_tab_id != previous_tab_id
    ));
}

#[test]
fn closing_duplicate_split_tab_does_not_close_globally_visible_buffer() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("shared")]);
    assert!(layout.dispatch_intent(&Intent::Command(Command::SplitVertical)));
    let closed_tab_id = layout.active_window_group().active_tab_id().unwrap();
    let buffer_id = layout.active_buffer_view().buffer_id();
    assert!(layout.focus_pane(PaneId(0)));
    let focused_tab_id = layout.active_window_group().active_tab_id().unwrap();
    assert!(layout.focus_pane(PaneId(1)));
    drain_editor_events();

    assert!(layout.dispatch_intent(&Intent::Command(Command::ClosePane)));

    let events = drain_editor_events();
    assert!(matches!(
        events.as_slice(),
        [
            EditorEvent::TabClosed {
                window_id: PaneId(1),
                tab_id,
                snapshot,
            },
            EditorEvent::WindowClosed {
                window_id: PaneId(1),
                buffer_id: closed_buffer_id,
                tab_id: final_tab_id,
            },
            EditorEvent::WindowFocused {
                previous_window_id: Some(PaneId(1)),
                window_id: PaneId(0),
                buffer_id: focused_buffer_id,
                tab_id: new_focused_tab_id,
            }
        ] if *tab_id == closed_tab_id
            && snapshot.buffer_id == buffer_id
            && *closed_buffer_id == buffer_id
            && *final_tab_id == closed_tab_id
            && *focused_buffer_id == buffer_id
            && *new_focused_tab_id == focused_tab_id
    ));
}

#[test]
fn closing_whole_pane_closes_buffers_before_window() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("shared")]);
    let shared_buffer_id = layout.active_buffer_view().buffer_id();
    assert!(layout.dispatch_intent(&Intent::Command(Command::SplitVertical)));
    let unique_buffer_id = globals::with_buffer_pool(|pool| pool.create_buffer());
    layout.activate_or_open_buffer(unique_buffer_id);
    let final_tab_id = layout.active_window_group().active_tab_id().unwrap();
    drain_editor_events();

    assert!(layout.dispatch_intent(&Intent::Command(Command::ClosePane)));

    let events = drain_editor_events();
    assert!(matches!(
        events.as_slice(),
        [
            EditorEvent::TabClosed {
                snapshot: shared_snapshot,
                ..
            },
            EditorEvent::TabClosed {
                tab_id: closed_final_tab_id,
                snapshot: unique_snapshot,
                ..
            },
            EditorEvent::BufferClosed { snapshot },
            EditorEvent::WindowClosed {
                window_id: PaneId(1),
                buffer_id: closed_buffer_id,
                tab_id: window_final_tab_id,
            },
            EditorEvent::WindowFocused {
                previous_window_id: Some(PaneId(1)),
                window_id: PaneId(0),
                buffer_id: focused_buffer_id,
                ..
            },
            EditorEvent::ActiveBufferChanged {
                previous_buffer_id: Some(previous_buffer_id),
                buffer_id: active_buffer_id,
                window_id: PaneId(0),
                ..
            }
        ] if shared_snapshot.buffer_id == shared_buffer_id
            && unique_snapshot.buffer_id == unique_buffer_id
            && snapshot.buffer_id == unique_buffer_id
            && *closed_final_tab_id == final_tab_id
            && *closed_buffer_id == unique_buffer_id
            && *window_final_tab_id == final_tab_id
            && *focused_buffer_id == shared_buffer_id
            && *previous_buffer_id == unique_buffer_id
            && *active_buffer_id == shared_buffer_id
    ));
}

#[test]
fn closing_active_tab_snapshots_buffer_before_activation_changes() {
    let mut layout =
        layout_with_buffers(vec![Buffer::from_str("first"), Buffer::from_str("second")]);
    let closed_buffer_id = layout.active_buffer_view().buffer_id();
    let closed_tab_id = layout.active_window_group().active_tab_id().unwrap();
    let next_buffer_id = layout.active_window_group().buffer_ids()[1];
    drain_editor_events();

    assert!(layout.close_active_buffer_tab());

    let events = drain_editor_events();
    assert!(matches!(
        events.as_slice(),
        [
            EditorEvent::TabClosed {
                tab_id,
                snapshot: closed_snapshot,
                ..
            },
            EditorEvent::BufferClosed { snapshot },
            EditorEvent::TabActivated {
                previous_tab_id: Some(previous_tab),
                tab_id: activated_tab_id,
                buffer_id: activated_buffer_id,
                ..
            },
            EditorEvent::ActiveBufferChanged {
                previous_buffer_id: Some(previous),
                buffer_id: active_buffer_id,
                tab_id: active_tab_id,
                ..
            }
        ] if *tab_id == closed_tab_id
            && closed_snapshot.buffer_id == closed_buffer_id
            && snapshot.buffer_id == closed_buffer_id
            && *previous_tab == closed_tab_id
            && *activated_tab_id == *active_tab_id
            && *activated_buffer_id == next_buffer_id
            && *previous == closed_buffer_id
            && *active_buffer_id == next_buffer_id
    ));
}
