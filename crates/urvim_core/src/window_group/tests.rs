use super::*;
use crate::action::ActionResult;
use crate::buffer::Cursor;
use crate::cli::CliFileSpec;
use crate::config::{AdvancedGlyphCapability, Config};
use crate::editor::{EditorAction, EditorOperation, ModeKind};
use crate::globals;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use urvim_terminal::{Color, Style};
use urvim_theme::{HighlightStyles, Tag, Theme, ThemeKind};

fn abs_path(path: &Path) -> crate::AbsolutePath {
    crate::AbsolutePath::from_path(path).unwrap()
}

fn buffer_with_label(label: &str) -> Buffer {
    let path = PathBuf::from(format!("/tmp/{}", label));
    Buffer::from_str_with_path("line1\nline2", abs_path(&path))
}

fn buffer_with_lines(label: &str, line_count: usize) -> Buffer {
    let path = PathBuf::from(format!("/tmp/{}", label));
    let text = (0..line_count)
        .map(|line| format!("line{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    Buffer::from_str_with_path(&text, abs_path(&path))
}

fn window_group_with_labels(labels: &[&str]) -> WindowGroup {
    WindowGroup::from_buffers(
        labels
            .iter()
            .map(|label| buffer_with_label(label))
            .collect(),
    )
}

fn themed_group() -> Theme {
    let default_style = Style::new().fg(Color::ansi(10)).bg(Color::ansi(20));
    let mut highlights = HighlightStyles::default();
    highlights.insert(
        Tag::parse("ui.status_bar").expect("valid tag"),
        Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
    );
    highlights.insert(
        Tag::parse("ui.status_bar.modified_marker").expect("valid tag"),
        Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)).bold(),
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
    for tag_name in [
        "syntax.comment",
        "syntax.constant",
        "syntax.function",
        "syntax.keyword",
        "syntax.operator",
        "syntax.punctuation",
        "syntax.string",
        "syntax.type",
        "syntax.variable",
    ] {
        highlights.insert(Tag::parse(tag_name).expect("valid tag"), Style::new());
    }

    Theme::new("demo", ThemeKind::Ansi256, default_style, highlights)
}

fn buffer_line_count(view: &BufferView) -> usize {
    view.with_buffer(|buffer| buffer.line_count()).unwrap_or(0)
}

fn buffer_file_name(view: &BufferView) -> Option<std::ffi::OsString> {
    view.with_buffer(|buffer| buffer.file_name().map(|name| name.to_os_string()))
        .flatten()
}

fn buffer_cursor(view: &BufferView) -> Cursor {
    view.cursor()
}

#[test]
fn test_window_group_session_skips_missing_files_during_restore() {
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-window-group-session-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).unwrap();

    let first = temp_dir.join("first.txt");
    let second = temp_dir.join("second.txt");
    fs::write(&first, "alpha").unwrap();
    fs::write(&second, "beta").unwrap();

    let group = WindowGroup::from_paths(&[first.clone(), second.clone()]);
    let session = group.to_session();
    fs::remove_file(&second).unwrap();

    let restored = WindowGroup::from_session(session);
    assert_eq!(restored.tabs.len(), 1);
    assert_eq!(
        buffer_file_name(restored.active_window().buffer_view()).unwrap(),
        first.file_name().unwrap()
    );
}

#[test]
fn test_window_group_new_creates_empty_tab() {
    let group = WindowGroup::new(Vec::new());
    assert_eq!(group.active_tab_index(), 0);
    assert_eq!(buffer_line_count(group.active_buffer_view()), 1);
}

#[test]
fn test_window_group_from_paths_opens_missing_files_as_empty_buffers() {
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-tab-group-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let first = temp_dir.join("first.txt");
    let second = temp_dir.join("second.txt");
    let missing = temp_dir.join("missing.txt");
    std::fs::write(&first, "alpha").unwrap();
    std::fs::write(&second, "beta").unwrap();

    let group = WindowGroup::from_paths(&[first.clone(), missing, second.clone()]);

    assert_eq!(group.tabs.len(), 3);
    assert_eq!(
        buffer_file_name(group.active_window().buffer_view()).unwrap(),
        first.file_name().unwrap()
    );
    assert_eq!(
        buffer_file_name(group.tabs[1].buffer_view()).unwrap(),
        "missing.txt"
    );
    assert_eq!(
        group.tabs[1]
            .buffer_view()
            .with_buffer(|buffer| buffer.as_str())
            .unwrap(),
        ""
    );
}

#[test]
fn test_window_group_from_paths_deduplicates_duplicate_files() {
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-tab-group-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("duplicate.txt");
    std::fs::write(&path, "alpha").unwrap();

    let group = WindowGroup::from_paths(&[path.clone(), path.clone()]);

    assert_eq!(group.tabs.len(), 1);
    assert_eq!(
        buffer_file_name(group.active_window().buffer_view()).unwrap(),
        path.file_name().unwrap()
    );

    std::fs::remove_file(&path).ok();
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_window_group_from_cli_files_applies_and_syncs_cursor() {
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-cli-file-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("cursor.txt");
    std::fs::write(&path, "a😀b").unwrap();

    let group = WindowGroup::from_cli_files(&[CliFileSpec {
        path: path.clone(),
        cursor: Some(Cursor::new(0, 2)),
    }]);

    assert_eq!(
        buffer_cursor(group.active_window().buffer_view()),
        Cursor::new(0, 1)
    );

    std::fs::remove_file(&path).ok();
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_window_group_from_cli_files_clamps_cursor_to_file_bounds() {
    let temp_dir = std::env::temp_dir().join(format!(
        "urvim-cli-file-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("clamp.txt");
    std::fs::write(&path, "alpha\nbeta").unwrap();

    let group = WindowGroup::from_cli_files(&[CliFileSpec {
        path: path.clone(),
        cursor: Some(Cursor::new(98, 98)),
    }]);

    assert_eq!(
        buffer_cursor(group.active_window().buffer_view()),
        Cursor::new(1, 4)
    );

    std::fs::remove_file(&path).ok();
    std::fs::remove_dir_all(&temp_dir).ok();
}

#[test]
fn test_tab_navigation_wraps_and_supports_counts() {
    let mut group = window_group_with_labels(&["a", "b", "c", "d", "e"]);

    group.previous_tab(1);
    assert_eq!(group.active_tab_index(), 4);

    group.next_tab(2);
    assert_eq!(group.active_tab_index(), 1);

    group.previous_tab(3);
    assert_eq!(group.active_tab_index(), 3);
}

#[test]
fn test_each_window_restores_its_own_mode() {
    let mut group =
        WindowGroup::from_buffers(vec![Buffer::from_str("first"), Buffer::from_str("second")]);

    group.active_window_mut().switch_mode(ModeKind::Insert);

    group.next_tab(1);
    assert_eq!(group.active_tab_index(), 1);
    assert_eq!(group.active_window_mode_kind(), ModeKind::Normal);

    group.active_window_mut().switch_mode(ModeKind::VisualLine);

    group.previous_tab(1);
    assert_eq!(group.active_tab_index(), 0);
    assert_eq!(group.active_window_mode_kind(), ModeKind::Insert);

    group.next_tab(1);
    assert_eq!(group.active_tab_index(), 1);
    assert_eq!(group.active_window_mode_kind(), ModeKind::VisualLine);
}

#[test]
fn test_tab_bar_scrolls_only_when_active_tab_is_offscreen() {
    let mut group = window_group_with_labels(&["a", "b", "c", "d", "e"]);
    let mut screen = crate::screen::Screen::new(2, 12);

    group.active_tab = 4;
    group.tab_bar_start = 1;
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 12));

    assert_eq!(group.tab_bar_start, 2);

    group.active_tab = 3;
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 12));
    assert_eq!(group.tab_bar_start, 2);
}

#[test]
fn test_tab_bar_indicators_appear_when_tabs_are_offscreen() {
    let mut group = window_group_with_labels(&["a", "b", "c", "d", "e"]);
    let mut screen = crate::screen::Screen::new(2, 12);

    group.active_tab = 3;
    group.tab_bar_start = 1;
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 12));

    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "<");
    assert_eq!(screen.get_cell_mut(0, 11).unwrap().text, ">");
}

#[test]
fn test_tab_bar_right_indicator_appears_when_only_right_tabs_are_offscreen() {
    let mut group = window_group_with_labels(&["aaaa", "bbbb", "cccc", "dddd"]);
    let mut screen = crate::screen::Screen::new(2, 10);

    group.active_tab = 0;
    group.tab_bar_start = 0;
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 10));

    assert_eq!(screen.get_cell_mut(0, 9).unwrap().text, ">");
}

#[test]
fn test_tab_bar_active_style_and_unicode_width() {
    let mut group = window_group_with_labels(&["あ", "b"]);
    let mut screen = crate::screen::Screen::new(2, 12);

    group.render(&mut screen, Position::new(0, 0), Size::new(2, 12));

    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "あ");
    assert_eq!(screen.get_cell_mut(0, 5).unwrap().text, "b");
    let active_style = screen.get_cell_mut(0, 1).unwrap().style;
    let inactive_style = screen.get_cell_mut(0, 5).unwrap().style;
    assert_ne!(active_style, inactive_style);
}

#[test]
fn test_tab_bar_uses_theme_modified_marker_style() {
    let path = PathBuf::from("/tmp/a.txt");
    let mut buffer = Buffer::from_str_with_path("line1", abs_path(&path));
    buffer.insert_char(Cursor::new(0, 5), '!');

    let mut group = WindowGroup::from_buffers(vec![buffer]);
    let theme = themed_group();
    let expected_style = theme.resolve_name_with_default("ui.tab.active");
    let expected_marker_style =
        expected_style.accent(theme.highlight_style_for_name("ui.status_bar.modified_marker"));
    let _theme_guard = globals::set_test_active_theme(theme);

    let mut screen = crate::screen::Screen::new(2, 20);
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 20));

    assert_eq!(screen.get_cell_mut(0, 1).unwrap().style, expected_style);
    assert_eq!(screen.get_cell_mut(0, 6).unwrap().text, "*");
    assert_eq!(
        screen.get_cell_mut(0, 6).unwrap().style,
        expected_marker_style
    );
}

#[test]
fn test_tab_bar_uses_theme_styles() {
    let mut group = window_group_with_labels(&["demo"]);
    let theme = themed_group();
    let expected_style = theme.resolve_name_with_default("ui.tab.active");
    let _theme_guard = globals::set_test_active_theme(theme);

    let mut screen = crate::screen::Screen::new(2, 16);
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 16));

    assert_eq!(screen.get_cell_mut(0, 0).unwrap().style, expected_style);
}

#[test]
fn test_tab_bar_renders_glyph_when_enabled() {
    let path = PathBuf::from("/tmp/rust-icon.rs");
    let buffer = Buffer::from_str_with_path("fn main() {}", abs_path(&path));
    let mut group = WindowGroup::from_buffers(vec![buffer]);
    let theme = themed_group();
    let expected_tab_style = theme.resolve_name_with_default("ui.tab.active");
    let expected_glyph_style =
        expected_tab_style.accent(Style::default().fg(Color::rgb(222, 165, 132)));
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(Config {
        theme: "demo".to_string(),
        syntax: true,
        auto_close_pairs: true,
        advanced_glyphs: BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
        ..Default::default()
    });

    let mut screen = crate::screen::Screen::new(2, 24);
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 24));

    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "");
    assert_eq!(
        screen.get_cell_mut(0, 1).unwrap().style,
        expected_glyph_style
    );
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "r");
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().style, expected_tab_style);
}

#[test]
fn test_visual_cursor_is_offset_by_tab_bar_row() {
    let mut group = window_group_with_labels(&["a"]);
    group
        .active_buffer_view_mut()
        .set_cursor(crate::buffer::Cursor::new(0, 0));

    let mut screen = crate::screen::Screen::new(2, 12);
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 12));

    let cursor = group.visual_cursor().unwrap();
    assert_eq!(cursor.row, 1);
}

#[test]
fn test_jump_navigation_selects_existing_tab() {
    let mut group = window_group_with_labels(&["a", "b"]);

    group.active_buffer_view_mut().set_cursor(Cursor::new(0, 1));
    let first_buffer = group.active_buffer_view().buffer_id();
    group.record_cursor_position();

    group.active_tab = 1;
    group.active_buffer_view_mut().set_cursor(Cursor::new(0, 3));
    let second_buffer = group.active_buffer_view().buffer_id();
    group.record_cursor_position();

    assert_eq!(
        group.dispatch_action(&EditorAction::jump_backward()),
        ActionResult::Handled
    );
    assert_eq!(group.active_tab_index(), 0);
    assert_eq!(
        group.active_window().buffer_view().buffer_id(),
        first_buffer
    );
    assert_eq!(buffer_cursor(group.active_buffer_view()), Cursor::new(0, 1));
    assert_eq!(
        group.dispatch_action(&EditorAction::jump_forward()),
        ActionResult::Handled
    );
    assert_eq!(group.active_tab_index(), 1);
    assert_eq!(
        group.active_window().buffer_view().buffer_id(),
        second_buffer
    );
    assert_eq!(buffer_cursor(group.active_buffer_view()), Cursor::new(0, 3));
}

#[test]
fn test_jump_navigation_reopens_missing_tab() {
    let mut group = window_group_with_labels(&["a", "b"]);

    group.active_buffer_view_mut().set_cursor(Cursor::new(0, 1));
    let first_buffer = group.active_buffer_view().buffer_id();
    group.record_cursor_position();

    group.active_tab = 1;
    group.active_buffer_view_mut().set_cursor(Cursor::new(0, 4));
    group.record_cursor_position();

    group.tabs.remove(0);
    group.active_tab = 0;

    assert_eq!(
        group.dispatch_action(&EditorAction::jump_backward()),
        ActionResult::Handled
    );
    assert_eq!(group.tabs.len(), 2);
    assert_eq!(group.active_tab_index(), 1);
    assert_eq!(
        group.active_window().buffer_view().buffer_id(),
        first_buffer
    );
    assert_eq!(buffer_cursor(group.active_buffer_view()), Cursor::new(0, 1));
}

#[test]
fn test_counted_jump_down_creates_a_new_entry() {
    let mut group = WindowGroup::from_buffers(vec![buffer_with_lines("jumplist-50j", 80)]);

    group.active_buffer_view_mut().set_cursor(Cursor::new(0, 0));
    group.record_cursor_position();

    assert_eq!(
        group.dispatch_action(&EditorAction::count(
            50,
            Box::new(EditorAction::new(EditorOperation::MoveDown))
        )),
        ActionResult::Handled
    );
    assert_eq!(
        buffer_cursor(group.active_buffer_view()),
        Cursor::new(50, 0)
    );

    assert_eq!(
        group.dispatch_action(&EditorAction::jump_backward()),
        ActionResult::Handled
    );
    assert_eq!(buffer_cursor(group.active_buffer_view()), Cursor::new(0, 0));
}

#[test]
fn test_smaller_counted_jump_down_updates_current_entry() {
    let mut group = WindowGroup::from_buffers(vec![buffer_with_lines("jumplist-5j", 20)]);

    group.active_buffer_view_mut().set_cursor(Cursor::new(0, 0));
    group.record_cursor_position();

    assert_eq!(
        group.dispatch_action(&EditorAction::count(
            5,
            Box::new(EditorAction::new(EditorOperation::MoveDown))
        )),
        ActionResult::Handled
    );
    assert_eq!(buffer_cursor(group.active_buffer_view()), Cursor::new(5, 0));

    assert_eq!(
        group.dispatch_action(&EditorAction::jump_backward()),
        ActionResult::Handled
    );
    assert_eq!(buffer_cursor(group.active_buffer_view()), Cursor::new(5, 0));
}
