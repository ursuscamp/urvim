use super::*;
use crate::action::ActionResult;
use crate::buffer::Buffer;
use crate::config::Config;
use crate::editor::{Action, ActionKind, ModeKind};
use crate::globals;
use crate::path::AbsolutePath;
use crate::terminal::{Color, Style};
use crate::theme::{SyntaxTagStyles, Theme, ThemeKind, UiStyles};
use crate::window::{Position, Size};
use crate::window_group::WindowGroup;
use std::collections::{BTreeMap, BTreeSet};

fn layout_with_buffers(buffers: Vec<Buffer>) -> Layout {
    Layout::new(WindowGroup::from_buffers(buffers))
}

fn border_theme() -> Theme {
    let default_style = Style::new().fg(Color::ansi(15)).bg(Color::ansi(30));
    let ui_styles = UiStyles::new(
        Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
        Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
        Style::new().reverse(),
        Style::new().bg(Color::ansi(21)),
        Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
        Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
        Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
        Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
        Style::new().fg(Color::ansi(13)).bg(Color::ansi(14)),
        Style::new().fg(Color::ansi(33)),
        Style::new().fg(Color::ansi(160)).bold(),
    );
    let syntax_styles = SyntaxTagStyles::new(BTreeMap::new());

    Theme::new(
        "demo",
        ThemeKind::Ansi256,
        default_style,
        ui_styles,
        syntax_styles,
    )
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

fn pane_count(node: &LayoutNode) -> usize {
    match node {
        LayoutNode::Pane(_) => 1,
        LayoutNode::Split(split) => pane_count(&split.first) + pane_count(&split.second),
    }
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
fn test_layout_process_action_delegates_to_window_group() {
    let mut layout = layout_with_buffers(vec![
        Buffer::from_str("one"),
        Buffer::from_str("two"),
        Buffer::from_str("three"),
    ]);

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::NextTab)),
        ActionResult::Handled
    );
    assert_eq!(layout.window_group().active_tab_index(), 1);
}

#[test]
fn test_layout_vertical_split_creates_second_pane_with_even_weights() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::SplitVertical)),
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
        layout.process_action(&Action::new(ActionKind::SplitHorizontal)),
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

    layout.active_buffer_view_mut().set_cursor(source_cursor);
    layout
        .active_buffer_view_mut()
        .set_scroll_offset(source_scroll);

    let source_buffer_id = layout.active_buffer_view().buffer_id();

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::SplitVertical)),
        ActionResult::Handled
    );

    assert_eq!(layout.focused_pane, PaneId(1));
    assert_eq!(layout.active_buffer_view().buffer_id(), source_buffer_id);
    assert_eq!(layout.active_buffer_view().cursor(), source_cursor);
    assert_eq!(layout.active_buffer_view().scroll_offset(), source_scroll);

    let root = layout.root.as_ref().expect("layout should keep a root");
    match root {
        LayoutNode::Split(split) => {
            let original = pane_buffer_view(&split.first);
            let copied = pane_buffer_view(&split.second);

            assert_eq!(original.buffer_id(), source_buffer_id);
            assert_eq!(original.cursor(), source_cursor);
            assert_eq!(original.scroll_offset(), source_scroll);
            assert_eq!(copied.buffer_id(), source_buffer_id);
            assert_eq!(copied.cursor(), source_cursor);
            assert_eq!(copied.scroll_offset(), source_scroll);
        }
        LayoutNode::Pane(_) => panic!("split action should replace the root pane"),
    }
}

#[test]
fn test_layout_close_pane_exits_when_last_pane_is_removed() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::ClosePane)),
        ActionResult::Handled
    );
    assert!(layout.should_exit());
}

#[test]
fn test_layout_render_stores_geometry_and_forwards_size() {
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
fn test_layout_render_divides_vertical_split_width_by_weights() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));

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
    layout.process_action(&Action::new(ActionKind::SplitHorizontal));

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
    layout.process_action(&Action::new(ActionKind::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 13);
    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 13));

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneLeft)),
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
        layout.process_action(&Action::new(ActionKind::ResizePaneLeft)),
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
    layout.process_action(&Action::new(ActionKind::SplitVertical));

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
        layout.process_action(&Action::new(ActionKind::ResizePaneRight)),
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
    layout.process_action(&Action::new(ActionKind::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 21);
    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 21));

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneLeft)),
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
        layout.process_action(&Action::count(
            5,
            Box::new(Action::new(ActionKind::ResizePaneLeft))
        )),
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
    layout.process_action(&Action::new(ActionKind::SplitVertical));
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));
    layout.process_action(&Action::new(ActionKind::FocusPaneLeft));
    layout.process_action(&Action::new(ActionKind::SplitHorizontal));
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::ResizePaneDown)),
        ActionResult::Handled
    );
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneRight)),
        ActionResult::Handled
    );
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::ResizePaneLeft)),
        ActionResult::Handled
    );

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::EqualizeSplits)),
        ActionResult::Handled
    );

    let root = layout.root.as_ref().expect("layout should keep a root");
    assert_all_splits_even(root);
}

#[test]
fn test_layout_resize_clamps_and_stays_local_to_the_matching_split() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneLeft)),
        ActionResult::Handled
    );
    layout.process_action(&Action::new(ActionKind::SplitHorizontal));
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
        layout.process_action(&Action::new(ActionKind::ResizePaneDown)),
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
            layout.process_action(&Action::new(ActionKind::ResizePaneUp)),
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
        layout.process_action(&Action::new(ActionKind::SplitVertical)),
        ActionResult::Handled
    );
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::SplitHorizontal)),
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
    assert_eq!(screen.get_cell_mut(1, 3).unwrap().style, Default::default());
    assert_eq!(screen.get_cell_mut(2, 9).unwrap().text, "R");
}

#[test]
fn test_layout_render_omits_split_borders_for_single_pane_layouts() {
    let path = AbsolutePath::from_path(std::path::Path::new("/tmp/example.rs")).unwrap();
    let buffer = Buffer::from_str_with_path("hi", path);
    let mut layout = layout_with_buffers(vec![buffer]);
    let mut screen = crate::screen::Screen::new(4, 20);
    let theme = border_theme();
    let border_style = theme.ui.split_border;
    let _theme_guard = globals::set_test_active_theme(theme);
    let _config_guard = globals::set_test_config(border_config(true));

    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    assert_ne!(screen.get_cell_mut(0, 9).unwrap().style, border_style);
}

#[test]
fn test_layout_render_draws_flattened_split_borders() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));

    let mut screen = crate::screen::Screen::new(5, 20);
    let _theme_guard = globals::set_test_active_theme(border_theme());
    let _config_guard = globals::set_test_config(border_config(true));

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));
    layout.process_action(&Action::new(ActionKind::FocusPaneLeft));
    layout.process_action(&Action::new(ActionKind::SplitHorizontal));

    layout.render(&mut screen, Position::new(0, 0), Size::new(5, 20));

    assert_eq!(screen.get_cell_mut(0, 9).unwrap().text, "│");
    assert_eq!(screen.get_cell_mut(1, 8).unwrap().text, "─");
    assert_eq!(screen.get_cell_mut(1, 9).unwrap().text, "│");
}

#[test]
fn test_layout_render_uses_resize_border_style_in_resize_mode() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));
    layout
        .window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::Resizing);

    let mut screen = crate::screen::Screen::new(4, 20);
    let _theme_guard = globals::set_test_active_theme(border_theme());
    let _config_guard = globals::set_test_config(border_config(true));

    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    assert_eq!(
        screen.get_cell_mut(0, 9).unwrap().style,
        Style::new().fg(Color::ansi(160)).bold()
    );
}

#[test]
fn test_layout_focus_moves_across_rendered_vertical_split() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));

    let mut screen = crate::screen::Screen::new(4, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 20));

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneLeft)),
        ActionResult::Handled
    );
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneRight)),
        ActionResult::Handled
    );
}

#[test]
fn test_layout_focus_moves_across_nested_mixed_axis_splits() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));
    layout.process_action(&Action::new(ActionKind::SplitHorizontal));

    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneUp)),
        ActionResult::Handled
    );
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneLeft)),
        ActionResult::Handled
    );
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneRight)),
        ActionResult::Handled
    );
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneDown)),
        ActionResult::Handled
    );
}

#[test]
fn test_layout_restores_last_focused_pane_when_reentering_split_subtree() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));
    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneLeft)),
        ActionResult::Handled
    );
    layout.process_action(&Action::new(ActionKind::SplitHorizontal));

    let mut screen = crate::screen::Screen::new(8, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 20));

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneRight)),
        ActionResult::Handled
    );
    assert_eq!(layout.focused_pane, PaneId(1));
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneLeft)),
        ActionResult::Handled
    );
    assert_eq!(layout.focused_pane, PaneId(2));
}

#[test]
fn test_layout_falls_back_to_surviving_pane_when_remembered_pane_closes() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("left")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));
    let mut screen = crate::screen::Screen::new(8, 24);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 24));
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneLeft)),
        ActionResult::Handled
    );
    layout.process_action(&Action::new(ActionKind::SplitHorizontal));
    layout.process_action(&Action::new(ActionKind::SplitVertical));

    let mut screen = crate::screen::Screen::new(8, 24);
    layout.render(&mut screen, Position::new(0, 0), Size::new(8, 24));

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::ClosePane)),
        ActionResult::Handled
    );

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneRight)),
        ActionResult::Handled
    );
    assert_eq!(
        layout.process_action(&Action::new(ActionKind::FocusPaneLeft)),
        ActionResult::Handled
    );
    assert_eq!(layout.focused_pane, PaneId(0));
}

#[test]
fn test_layout_close_pane_collapses_parent_split_to_surviving_child() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::ClosePane)),
        ActionResult::Handled
    );

    let root = layout.root.as_ref().expect("layout should keep one pane");
    assert_eq!(pane_count(root), 1);
    assert!(matches!(root, LayoutNode::Pane(_)));
}

#[test]
fn test_layout_prunes_empty_window_group_during_render() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));

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
fn test_layout_preserves_unrelated_pane_cursor_and_mode_state() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout
        .active_buffer_view_mut()
        .set_cursor(crate::buffer::Cursor::new(0, 2));
    layout
        .active_window_group_mut()
        .active_window_mut()
        .switch_mode(ModeKind::Insert);

    layout.process_action(&Action::new(ActionKind::SplitVertical));
    let mut screen = crate::screen::Screen::new(4, 20);
    layout.render(&mut screen, Position::new(0, 0), Size::new(4, 20));
    layout.process_action(&Action::new(ActionKind::FocusPaneLeft));

    assert_eq!(
        layout.active_buffer_view().cursor(),
        crate::buffer::Cursor::new(0, 2)
    );
    assert_eq!(layout.active_window_mode_kind(), ModeKind::Insert);
}
