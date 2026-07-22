use super::*;

#[test]
fn test_generic_marker_store_shifts_point_payloads() {
    let mut store: MarkerStore<usize, ()> = MarkerStore::with_line_count(2);

    let id = store.insert_point(Cursor::new(0, 2), Gravity::Right, 7usize);
    store.shift_insert(InsertShape {
        at: Cursor::new(0, 1),
        line_delta: 1,
        tail_col: 0,
    });

    let marker = store.get(id).expect("marker should exist");
    let (point, payload) = marker.as_point().expect("point marker");
    assert_eq!(*payload, 7);
    assert_eq!(
        point,
        PointAnchor {
            pos: Cursor::new(1, 1),
            gravity: Gravity::Right,
        }
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

    let (point, payload) = store
        .get(point_id)
        .and_then(Marker::as_point)
        .expect("point marker");
    assert_eq!(
        point,
        PointAnchor {
            pos: Cursor::new(0, 3),
            gravity: Gravity::Left,
        }
    );
    assert_eq!(*payload, "7");

    let (range, payload) = store
        .get(range_id)
        .and_then(Marker::as_range)
        .expect("range marker");
    assert_eq!(
        range,
        RangeAnchor {
            start: Cursor::new(0, 1),
            end: Cursor::new(0, 4),
            start_gravity: Gravity::Left,
            end_gravity: Gravity::Right,
        }
    );
    assert_eq!(*payload, "range");
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

    let (point, payload) = store
        .get(point_id)
        .and_then(Marker::as_point)
        .expect("point marker");
    assert_eq!(
        point,
        PointAnchor {
            pos: Cursor::new(0, 2),
            gravity: Gravity::Left,
        }
    );
    assert_eq!(*payload, "11");

    let (range, payload) = store
        .get(range_id)
        .and_then(Marker::as_range)
        .expect("range marker");
    assert_eq!(
        range,
        RangeAnchor {
            start: Cursor::new(0, 1),
            end: Cursor::new(0, 3),
            start_gravity: Gravity::Left,
            end_gravity: Gravity::Right,
        }
    );
    assert_eq!(*payload, "22");
}

#[test]
fn test_marker_store_multiline_insert_shifts_later_lines() {
    let mut store: MarkerStore<&str, ()> = MarkerStore::with_line_count(2);

    let tail_id = store.insert_point(Cursor::new(0, 2), Gravity::Right, "1");
    let later_id = store.insert_point(Cursor::new(1, 1), Gravity::Left, "2");

    store.shift_insert(InsertShape {
        at: Cursor::new(0, 1),
        line_delta: 1,
        tail_col: 1,
    });

    let (tail, payload) = store
        .get(tail_id)
        .and_then(Marker::as_point)
        .expect("tail marker");
    assert_eq!(tail.pos, Cursor::new(1, 2));
    assert_eq!(*payload, "1");

    let (later, payload) = store
        .get(later_id)
        .and_then(Marker::as_point)
        .expect("later marker");
    assert_eq!(later.pos, Cursor::new(2, 1));
    assert_eq!(*payload, "2");
}

#[test]
fn test_namespaced_virtual_texts_are_indexed_and_isolated() {
    let mut buf = Buffer::from_str("abcd\nefgh");
    let style = urvim_theme::StyleOverlay {
        bold: Some(true),
        ..urvim_theme::StyleOverlay::default()
    };

    let first = buf.insert_namespaced_virtual_text(
        "first",
        Cursor::new(0, 2),
        Gravity::Right,
        "boo",
        Some(style),
    );
    let second = buf.insert_namespaced_virtual_text(
        "second",
        Cursor::new(1, 1),
        Gravity::Left,
        "other",
        None,
    );

    assert_eq!(buf.namespaced_virtual_texts("first").len(), 1);
    assert_eq!(buf.namespaced_virtual_texts("second").len(), 1);
    assert!(buf.namespaced_virtual_text("first", second).is_none());
    let (_, payload) = buf
        .namespaced_virtual_text("first", first)
        .and_then(Marker::as_point)
        .expect("virtual text");
    assert_eq!(payload.style, Some(style));

    assert!(buf.update_namespaced_virtual_text(
        "first",
        first,
        Cursor::new(1, 3),
        Gravity::Left,
        "moved",
        None,
    ));
    let (point, payload) = buf
        .namespaced_virtual_text("first", first)
        .and_then(Marker::as_point)
        .expect("virtual text");
    assert_eq!(payload.text, "moved");
    assert_eq!(point.pos, Cursor::new(1, 3));

    assert_eq!(buf.clear_namespaced_virtual_texts("first"), 1);
    assert!(buf.namespaced_virtual_text("first", first).is_none());
    assert!(buf.namespaced_virtual_text("second", second).is_some());
}

#[test]
fn test_virtual_texts_shift_and_clear() {
    let mut buf = Buffer::from_str("abcd\nefgh");

    let id = buf.insert_virtual_text(Cursor::new(0, 2), Gravity::Right, "virtual");

    let line = buf.virtual_texts_for_line(0).expect("line should exist");
    assert_eq!(line.len(), 1);
    assert_eq!(line[0].id(), id);

    buf.insert_text(Cursor::new(0, 1), "X");

    let (point, payload) = buf
        .virtual_text(id)
        .and_then(Marker::as_point)
        .expect("virtual text");
    assert_eq!(point.pos, Cursor::new(0, 3));
    assert_eq!(payload.text, "virtual");

    assert!(buf.remove_virtual_text(id).is_some());
    assert!(
        buf.virtual_texts_for_line(0)
            .expect("line should exist")
            .is_empty()
    );
}

#[test]
fn test_namespaced_highlights_are_range_typed_and_isolated() {
    let mut buf = Buffer::from_str("abcd\nefgh");
    let style = urvim_theme::StyleOverlay {
        bold: Some(true),
        ..urvim_theme::StyleOverlay::default()
    };
    let range = RangeAnchor {
        start: Cursor::new(0, 1),
        end: Cursor::new(1, 2),
        start_gravity: Gravity::Right,
        end_gravity: Gravity::Left,
    };
    let id = buf
        .insert_namespaced_highlight("first", range, style)
        .expect("valid highlight");

    assert!(buf.namespaced_highlight("second", id).is_none());
    let (stored_range, payload) = buf
        .namespaced_highlight("first", id)
        .and_then(Marker::as_range)
        .expect("range highlight");
    assert_eq!(stored_range, range);
    assert_eq!(payload.style, style);
    assert_eq!(buf.clear_namespaced_highlights("first"), 1);
}

#[test]
fn test_multiline_highlight_tracks_edits_after_its_anchor_line() {
    let mut buf = Buffer::from_str("abc\ndef");
    let id = buf
        .insert_namespaced_highlight(
            "test",
            RangeAnchor {
                start: Cursor::new(0, 1),
                end: Cursor::new(1, 2),
                start_gravity: Gravity::Right,
                end_gravity: Gravity::Left,
            },
            urvim_theme::StyleOverlay::default(),
        )
        .expect("highlight");

    buf.insert_text(Cursor::new(1, 1), "X");
    let (range, _) = buf
        .namespaced_highlight("test", id)
        .and_then(Marker::as_range)
        .expect("highlight");
    assert_eq!(range.start, Cursor::new(0, 1));
    assert_eq!(range.end, Cursor::new(1, 3));
}

#[test]
fn test_highlight_default_gravity_excludes_boundary_insertions() {
    let mut buf = Buffer::from_str("abcd");
    let id = buf
        .insert_namespaced_highlight(
            "test",
            RangeAnchor {
                start: Cursor::new(0, 1),
                end: Cursor::new(0, 3),
                start_gravity: Gravity::Right,
                end_gravity: Gravity::Left,
            },
            urvim_theme::StyleOverlay::default(),
        )
        .expect("highlight");

    buf.insert_text(Cursor::new(0, 1), "X");
    buf.insert_text(Cursor::new(0, 4), "Y");
    let (range, _) = buf
        .namespaced_highlight("test", id)
        .and_then(Marker::as_range)
        .expect("highlight");
    assert_eq!(range.start, Cursor::new(0, 2));
    assert_eq!(range.end, Cursor::new(0, 4));
}

#[test]
fn test_highlight_contracts_and_is_removed_when_fully_deleted() {
    let mut buf = Buffer::from_str("abcdef");
    let id = buf
        .insert_namespaced_highlight(
            "test",
            RangeAnchor {
                start: Cursor::new(0, 1),
                end: Cursor::new(0, 5),
                start_gravity: Gravity::Right,
                end_gravity: Gravity::Left,
            },
            urvim_theme::StyleOverlay::default(),
        )
        .expect("highlight");

    buf.delete_range(TextObjectRange {
        start: Cursor::new(0, 2),
        end: Cursor::new(0, 4),
    });
    let (range, _) = buf
        .namespaced_highlight("test", id)
        .and_then(Marker::as_range)
        .expect("contracted highlight");
    assert_eq!(range.start, Cursor::new(0, 1));
    assert_eq!(range.end, Cursor::new(0, 3));

    buf.delete_range(TextObjectRange {
        start: Cursor::new(0, 1),
        end: Cursor::new(0, 3),
    });
    assert!(buf.namespaced_highlight("test", id).is_none());
}
