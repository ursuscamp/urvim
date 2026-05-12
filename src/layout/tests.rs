use super::*;
use crate::action::ActionResult;
use crate::background::{
    JobEvent, JobKind, JobPayload, JobToken, LspInlayHint, LspInlayHintsChunk,
};
use crate::buffer::Buffer;
use crate::buffer::Cursor;
use crate::config::Config;
use crate::editor::{Action, ActionKind, ModeKind};
use crate::globals;
use crate::path::AbsolutePath;
use crate::terminal::{Color, Key, KeyCode, Modifiers, Style};
use crate::theme::{HighlightStyles, Tag, Theme, ThemeKind};
use crate::ui::{Command, Intent, UiEvent, UiEventResult};
use crate::window::{Position, Size};
use crate::window_group::WindowGroup;
use lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use smol_str::SmolStr;
use std::collections::BTreeSet;
use std::fs;
use std::thread;
use std::time::Duration;

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
        insert_escape: None,
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
        LayoutNode::Pane(pane) => pane.window_group.active_buffer_view(),
        LayoutNode::Split(_) => panic!("expected pane"),
    }
}

fn pane_window(node: &LayoutNode) -> &crate::window::Window {
    match node {
        LayoutNode::Pane(pane) => pane.window_group.active_window(),
        LayoutNode::Split(_) => panic!("expected pane"),
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

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir_all(temp_dir);
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

    assert!(layout.dispatch_intent(&Intent::Action(Action::new(ActionKind::MoveRight))));
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

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_dir_all(temp_dir);
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

    let _ = std::fs::remove_file(hidden_path);
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

    let _ = fs::remove_file(visible_path);
    let _ = fs::remove_file(hidden_path);
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

    assert_eq!(
        dispatch_layout_action(&mut layout, Action::new(ActionKind::NextTab)),
        ActionResult::Handled
    );
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
        LayoutNode::Pane(_) => panic!("split action should replace the root pane"),
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
        LayoutNode::Pane(_) => panic!("split action should replace the root pane"),
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
        LayoutNode::Pane(_) => panic!("split action should replace the root pane"),
    }
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
        LayoutNode::Pane(_) => panic!("split action should replace the root pane"),
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
            LayoutNode::Pane(_) => panic!("expected nested split on the left side"),
        },
        LayoutNode::Pane(_) => panic!("resize test should keep the root split"),
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
        insert_escape: None,
        syntax: false,
        auto_close_pairs: true,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    });

    layout.render(&mut screen, Position::new(0, 0), Size::new(3, 40));

    assert_eq!(screen.get_cell_mut(1, 3).unwrap().text, "f");
    assert_eq!(
        screen.get_cell_mut(1, 3).unwrap().style,
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
        insert_escape: None,
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
            .dispatch_action(&Action::new(ActionKind::YankLine)),
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
