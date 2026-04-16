use super::*;
use crate::action::ActionResult;
use crate::buffer::Buffer;
use crate::config::Config;
use crate::editor::{Action, ActionKind, ModeKind};
use crate::globals;
use crate::path::AbsolutePath;
use crate::tab_group::TabGroup;
use crate::window::{Position, Size};
use std::collections::BTreeSet;

fn layout_with_buffers(buffers: Vec<Buffer>) -> Layout {
    Layout::new(TabGroup::from_buffers(buffers))
}

fn buffer_line_count(view: &crate::window::BufferView) -> usize {
    view.with_buffer(|buffer| buffer.line_count()).unwrap_or(0)
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

#[test]
fn test_layout_new_wraps_tab_group() {
    let layout = Layout::new(TabGroup::new(Vec::new()));

    assert_eq!(layout.origin(), Position::default());
    assert_eq!(layout.size(), Size::default());
    assert_eq!(layout.tab_group().active_tab_index(), 0);
    assert_eq!(
        layout.tab_group().active_window_mode_kind(),
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
fn test_layout_process_action_delegates_to_tab_group() {
    let mut layout = layout_with_buffers(vec![
        Buffer::from_str("one"),
        Buffer::from_str("two"),
        Buffer::from_str("three"),
    ]);

    assert_eq!(
        layout.process_action(&Action::new(ActionKind::NextTab)),
        ActionResult::Handled
    );
    assert_eq!(layout.tab_group().active_tab_index(), 1);
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
    assert_eq!(layout.tab_group().active_window().size(), Size::new(1, 12));
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
    assert_eq!(regions[1].size.cols, 7);
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
    assert_eq!(regions[1].size.rows, 4);
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
        .tab_group_mut()
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
fn test_layout_prunes_empty_tab_group_during_render() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("one")]);
    layout.process_action(&Action::new(ActionKind::SplitVertical));

    assert!(layout.active_tab_group_mut().close_active_tab());

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
        .active_tab_group_mut()
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
