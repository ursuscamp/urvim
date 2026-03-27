use super::*;
use crate::action::ActionResult;
use crate::editor::Action;
use crate::globals;
use crate::terminal::{Color, Style};
use crate::theme::{SyntaxStyles, Theme, ThemeKind, UiStyles};
use std::path::{Path, PathBuf};

fn abs_path(path: &Path) -> crate::AbsolutePath {
    crate::AbsolutePath::from_path(path).unwrap()
}

fn buffer_with_label(label: &str) -> Buffer {
    let path = PathBuf::from(format!("/tmp/{}", label));
    Buffer::from_str_with_path("line1\nline2", abs_path(&path))
}

fn tab_group_with_labels(labels: &[&str]) -> TabGroup {
    TabGroup::from_buffers(
        labels
            .iter()
            .map(|label| buffer_with_label(label))
            .collect(),
    )
}

fn themed_group() -> Theme {
    let default_style = Style::new().fg(Color::ansi(10)).bg(Color::ansi(20));
    let ui_styles = UiStyles::new(
        Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
        Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
        Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
        Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
        Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
        Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
    );
    let syntax_styles = SyntaxStyles::new(
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
    );

    Theme::new(
        "demo",
        ThemeKind::Ansi256,
        default_style,
        ui_styles,
        syntax_styles,
    )
}

fn buffer_line_count(view: &BufferView) -> usize {
    view.with_buffer(|buffer| buffer.line_count()).unwrap_or(0)
}

fn buffer_file_name(view: &BufferView) -> Option<std::ffi::OsString> {
    view.with_buffer(|buffer| buffer.file_name().map(|name| name.to_os_string()))
        .flatten()
}

#[test]
fn test_tab_group_new_creates_empty_tab() {
    let group = TabGroup::new(Vec::new());
    assert_eq!(group.active_tab_index(), 0);
    assert_eq!(buffer_line_count(group.active_buffer_view()), 1);
}

#[test]
fn test_tab_group_from_paths_loads_success_and_skips_failures() {
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

    let group = TabGroup::from_paths(&[first.clone(), missing, second.clone()]);

    assert_eq!(group.tabs.len(), 2);
    assert_eq!(
        buffer_file_name(group.active_window().buffer_view()).unwrap(),
        first.file_name().unwrap()
    );
}

#[test]
fn test_tab_navigation_wraps_and_supports_counts() {
    let mut group = tab_group_with_labels(&["a", "b", "c", "d", "e"]);

    assert_eq!(
        group.process_action(&Action::PreviousTab),
        ActionResult::Handled
    );
    assert_eq!(group.active_tab_index(), 4);

    assert_eq!(
        group.process_action(&Action::Count(2, Box::new(Action::NextTab))),
        ActionResult::Handled
    );
    assert_eq!(group.active_tab_index(), 1);

    assert_eq!(
        group.process_action(&Action::Count(3, Box::new(Action::PreviousTab))),
        ActionResult::Handled
    );
    assert_eq!(group.active_tab_index(), 3);
}

#[test]
fn test_tab_bar_scrolls_only_when_active_tab_is_offscreen() {
    let mut group = tab_group_with_labels(&["a", "b", "c", "d", "e"]);
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
    let mut group = tab_group_with_labels(&["a", "b", "c", "d", "e"]);
    let mut screen = crate::screen::Screen::new(2, 12);

    group.active_tab = 3;
    group.tab_bar_start = 1;
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 12));

    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "<");
    assert_eq!(screen.get_cell_mut(0, 11).unwrap().text, ">");
}

#[test]
fn test_tab_bar_right_indicator_appears_when_only_right_tabs_are_offscreen() {
    let mut group = tab_group_with_labels(&["aaaa", "bbbb", "cccc", "dddd"]);
    let mut screen = crate::screen::Screen::new(2, 10);

    group.active_tab = 0;
    group.tab_bar_start = 0;
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 10));

    assert_eq!(screen.get_cell_mut(0, 9).unwrap().text, ">");
}

#[test]
fn test_tab_bar_active_style_and_unicode_width() {
    let mut group = tab_group_with_labels(&["あ", "b"]);
    let mut screen = crate::screen::Screen::new(2, 12);

    group.render(&mut screen, Position::new(0, 0), Size::new(2, 12));

    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "あ");
    assert_eq!(screen.get_cell_mut(0, 5).unwrap().text, "b");
    let active_style = screen.get_cell_mut(0, 1).unwrap().style;
    let inactive_style = screen.get_cell_mut(0, 5).unwrap().style;
    assert_ne!(active_style, inactive_style);
}

#[test]
fn test_tab_bar_uses_theme_styles() {
    let mut group = tab_group_with_labels(&["demo"]);
    let theme = themed_group();
    let expected_style = theme.ui.tab_active;
    let _theme_guard = globals::set_test_active_theme(theme);

    let mut screen = crate::screen::Screen::new(2, 16);
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 16));

    assert_eq!(screen.get_cell_mut(0, 0).unwrap().style, expected_style);
}

#[test]
fn test_visual_cursor_is_offset_by_tab_bar_row() {
    let mut group = tab_group_with_labels(&["a"]);
    group
        .active_buffer_view_mut()
        .set_cursor(crate::buffer::Cursor::new(0, 0));

    let mut screen = crate::screen::Screen::new(2, 12);
    group.render(&mut screen, Position::new(0, 0), Size::new(2, 12));

    let cursor = group.visual_cursor().unwrap();
    assert_eq!(cursor.row, 1);
}
