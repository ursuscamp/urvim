use super::*;

#[test]
fn test_generic_marker_store_shifts_payloads() {
    let mut store = MarkerStore::with_line_count(2);

    let id = store.insert_point(Cursor::new(0, 2), Gravity::Right, 7usize);
    store.shift_insert(InsertShape {
        at: Cursor::new(0, 1),
        line_delta: 1,
        tail_col: 0,
    });

    let marker = store.get(id).expect("marker should exist");
    assert_eq!(marker.payload, 7);
    assert_eq!(
        marker.kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(1, 1),
            gravity: Gravity::Right
        })
    );
}

#[test]
fn test_marker_store_insert_shifts_points_and_ranges() {
    let mut store = MarkerStore::with_line_count(1);

    let point_id = store.insert_point(Cursor::new(0, 2), Gravity::Left, "7");
    let range_id = store.insert_range(
        Cursor::new(0, 1),
        Cursor::new(0, 3),
        Gravity::Left,
        Gravity::Right,
        "range",
    );

    store.shift_insert(InsertShape {
        at: Cursor::new(0, 1),
        line_delta: 0,
        tail_col: 2,
    });

    let point = store.get(point_id).expect("point marker should exist");
    assert_eq!(
        point.kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(0, 3),
            gravity: Gravity::Left
        })
    );
    assert_eq!(point.payload, "7");

    let range = store.get(range_id).expect("range marker should exist");
    assert_eq!(
        range.kind,
        MarkerShape::Range(RangeMarker {
            start: Cursor::new(0, 1),
            end: Cursor::new(0, 4),
            start_gravity: Gravity::Left,
            end_gravity: Gravity::Right,
        })
    );
    assert_eq!(range.payload, "range");
}

#[test]
fn test_marker_store_delete_shifts_points_and_ranges() {
    let mut store = MarkerStore::with_line_count(2);

    let point_id = store.insert_point(Cursor::new(1, 1), Gravity::Left, "11");
    let range_id = store.insert_range(
        Cursor::new(0, 1),
        Cursor::new(1, 2),
        Gravity::Left,
        Gravity::Right,
        "22",
    );

    store.shift_delete(DeleteShape {
        start: Cursor::new(0, 2),
        end: Cursor::new(1, 1),
    });

    let point = store.get(point_id).expect("point marker should exist");
    assert_eq!(
        point.kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(0, 2),
            gravity: Gravity::Left
        })
    );
    assert_eq!(point.payload, "11");

    let range = store.get(range_id).expect("range marker should exist");
    assert_eq!(
        range.kind,
        MarkerShape::Range(RangeMarker {
            start: Cursor::new(0, 1),
            end: Cursor::new(0, 3),
            start_gravity: Gravity::Left,
            end_gravity: Gravity::Right,
        })
    );
    assert_eq!(range.payload, "22");
}

#[test]
fn test_marker_store_multiline_insert_shifts_later_lines() {
    let mut store = MarkerStore::with_line_count(2);

    let tail_id = store.insert_point(Cursor::new(0, 2), Gravity::Right, "1");
    let later_id = store.insert_point(Cursor::new(1, 1), Gravity::Left, "2");

    store.shift_insert(InsertShape {
        at: Cursor::new(0, 1),
        line_delta: 1,
        tail_col: 1,
    });

    let tail = store.get(tail_id).expect("tail marker should exist");
    assert_eq!(
        tail.kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(1, 2),
            gravity: Gravity::Right,
        })
    );
    assert_eq!(tail.payload, "1");

    let later = store.get(later_id).expect("later marker should exist");
    assert_eq!(
        later.kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(2, 1),
            gravity: Gravity::Left,
        })
    );
    assert_eq!(later.payload, "2");
}

#[test]
fn test_namespaced_ghost_texts_are_indexed_and_isolated() {
    let mut buf = Buffer::from_str("abcd\nefgh");
    let style = urvim_theme::StyleOverlay {
        bold: Some(true),
        ..urvim_theme::StyleOverlay::default()
    };

    let first = buf.insert_namespaced_ghost_text(
        "first",
        Cursor::new(0, 2),
        Gravity::Right,
        "boo",
        Some(style),
    );
    let second =
        buf.insert_namespaced_ghost_text("second", Cursor::new(1, 1), Gravity::Left, "other", None);

    assert_eq!(buf.namespaced_ghost_texts("first").len(), 1);
    assert_eq!(buf.namespaced_ghost_texts("second").len(), 1);
    assert!(buf.namespaced_ghost_text("first", second).is_none());
    assert_eq!(
        buf.namespaced_ghost_text("first", first)
            .unwrap()
            .payload
            .style,
        Some(style)
    );

    assert!(buf.update_namespaced_ghost_text(
        "first",
        first,
        Cursor::new(1, 3),
        Gravity::Left,
        "moved",
        None,
    ));
    let marker = buf.namespaced_ghost_text("first", first).unwrap();
    assert_eq!(marker.payload.label, "moved");
    assert_eq!(
        marker.kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(1, 3),
            gravity: Gravity::Left,
        })
    );

    assert_eq!(buf.clear_namespaced_ghost_texts("first"), 1);
    assert!(buf.namespaced_ghost_text("first", first).is_none());
    assert!(buf.namespaced_ghost_text("second", second).is_some());
}

#[test]
fn test_ghost_texts_shift_and_clear() {
    let mut buf = Buffer::from_str("abcd\nefgh");

    let id = buf.insert_ghost_text(Cursor::new(0, 2), Gravity::Right, "ghost");

    let line = buf.ghost_texts_for_line(0).expect("line should exist");
    assert_eq!(line.len(), 1);
    assert_eq!(line[0].id, id);

    buf.insert_text(Cursor::new(0, 1), "X");

    let moved = buf.ghost_text(id).expect("ghost text should exist");
    assert_eq!(
        moved.kind,
        MarkerShape::Point(PointMarker {
            pos: Cursor::new(0, 3),
            gravity: Gravity::Right,
        })
    );
    assert_eq!(moved.payload.label, "ghost");

    let line = buf.ghost_texts_for_line(0).expect("line should exist");
    assert_eq!(line.len(), 1);
    assert_eq!(line[0].id, id);

    assert_eq!(buf.remove_ghost_text(id).unwrap().payload.label, "ghost");
    assert!(
        buf.ghost_texts_for_line(0)
            .expect("line should exist")
            .is_empty()
    );
}
