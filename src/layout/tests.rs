use super::*;
use crate::action::ActionResult;
use crate::buffer::Buffer;
use crate::editor::Action;
use crate::tab_group::TabGroup;
use crate::window::{Position, Size};

fn layout_with_buffers(buffers: Vec<Buffer>) -> Layout {
    Layout::new(TabGroup::from_buffers(buffers))
}

#[test]
fn test_layout_new_wraps_tab_group() {
    let layout = Layout::new(TabGroup::new(Vec::new()));

    assert_eq!(layout.origin(), Position::default());
    assert_eq!(layout.size(), Size::default());
    assert_eq!(layout.tab_group().active_tab_index(), 0);
}

#[test]
fn test_layout_exposes_active_buffer_view() {
    let layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);

    assert_eq!(layout.active_buffer_view().buffer().line_count(), 1);
}

#[test]
fn test_layout_process_action_delegates_to_tab_group() {
    let mut layout = layout_with_buffers(vec![
        Buffer::from_str("one"),
        Buffer::from_str("two"),
        Buffer::from_str("three"),
    ]);

    assert_eq!(layout.process_action(&Action::NextTab), ActionResult::Handled);
    assert_eq!(layout.tab_group().active_tab_index(), 1);
}

#[test]
fn test_layout_render_stores_geometry_and_forwards_size() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    let mut screen = crate::screen::Screen::new(2, 12);
    let origin = Position::new(3, 4);
    let size = Size::new(2, 12);

    layout.render(&mut screen, origin, size);

    assert_eq!(layout.origin(), origin);
    assert_eq!(layout.size(), size);
    assert_eq!(layout.tab_group().active_window().size(), Size::new(1, 12));
}

#[test]
fn test_layout_visual_cursor_tracks_child() {
    let mut layout = layout_with_buffers(vec![Buffer::from_str("alpha")]);
    layout
        .active_buffer_view_mut()
        .set_cursor(crate::buffer::Cursor::new(0, 0));

    let mut screen = crate::screen::Screen::new(2, 12);
    layout.render(&mut screen, Position::new(0, 0), Size::new(2, 12));

    let cursor = layout.visual_cursor().unwrap();
    assert_eq!(cursor.row, 1);
}
