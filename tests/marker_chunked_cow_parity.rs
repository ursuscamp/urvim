use std::sync::Arc;

use urvim::buffer::{
    Cursor, DeleteShape, Gravity, InsertShape, Marker, MarkerId, MarkerShape, MarkerStore,
    PointMarker, RangeMarker,
};

const CHUNK_SIZE: usize = 128;
const ID_CHUNK_SIZE: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadKind {
    Inlay,
    Ghost,
}

#[derive(Debug, Clone)]
struct LineBucket {
    markers: Arc<[Marker<PayloadKind>]>,
}

impl LineBucket {
    fn new() -> Self {
        Self {
            markers: Arc::from(Vec::<Marker<PayloadKind>>::new()),
        }
    }

    fn is_empty(&self) -> bool {
        self.markers.is_empty()
    }

    fn get(&self, id: MarkerId) -> Option<&Marker<PayloadKind>> {
        self.markers.iter().find(|marker| marker.id == id)
    }

    fn remove(&mut self, id: MarkerId) -> Option<Marker<PayloadKind>> {
        let mut markers = self.markers.as_ref().to_vec();
        let index = markers.iter().position(|marker| marker.id == id)?;
        let removed = markers.remove(index);
        self.markers = Arc::from(markers.into_boxed_slice());
        Some(removed)
    }

    fn iter(&self) -> impl Iterator<Item = &Marker<PayloadKind>> {
        self.markers.iter()
    }

    fn insert_sorted(&mut self, marker: Marker<PayloadKind>) {
        let anchor = marker_anchor(&marker);
        let mut markers = self.markers.as_ref().to_vec();
        let index = insertion_index(&markers, anchor);
        markers.insert(index, marker);
        self.markers = Arc::from(markers.into_boxed_slice());
    }
}

#[derive(Debug, Clone)]
struct ChunkedCowStore {
    chunks: Vec<Arc<Vec<LineBucket>>>,
    id_line_chunks: Vec<Arc<Vec<Option<usize>>>>,
    line_count: usize,
    next_id: MarkerId,
}

impl ChunkedCowStore {
    fn with_line_count(line_count: usize) -> Self {
        let line_count = line_count.max(1);
        Self {
            chunks: chunk_vec_lines(line_count),
            id_line_chunks: Vec::new(),
            line_count,
            next_id: 0,
        }
    }

    fn insert_point(&mut self, pos: Cursor, gravity: Gravity, payload: PayloadKind) -> MarkerId {
        let id = self.next_marker_id();
        self.insert_marker(Marker {
            id,
            kind: MarkerShape::Point(PointMarker { pos, gravity }),
            payload,
        });
        self.set_index(id, pos.line);
        id
    }

    fn markers_for_line(&self, line: usize) -> Option<&[Marker<PayloadKind>]> {
        if line >= self.line_count {
            return None;
        }
        self.bucket(line).map(|bucket| bucket.markers.as_ref())
    }

    fn get(&self, id: MarkerId) -> Option<&Marker<PayloadKind>> {
        let line = self.index_line(id)?;
        self.bucket(line)?.get(id)
    }

    fn remove(&mut self, id: MarkerId) -> Option<Marker<PayloadKind>> {
        let line = self.index_line(id)?;
        let removed = self.bucket_mut(line)?.remove(id);
        if removed.is_some() {
            self.clear_index(id);
        }
        removed
    }

    fn shift_insert(&mut self, edit: InsertShape) {
        if edit.line_delta == 0 {
            self.replace_shifted_bucket(edit.at.line, |marker| shift_marker_insert(marker, edit));
            return;
        }

        let boundary = edit.at.line.min(self.line_count);
        let after = self.suffix_from(boundary);
        let mut new_after = blank_vec_lines(after.len().saturating_add(edit.line_delta));
        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            insert_marker_into_vec_lines_offset(
                &mut new_after,
                shift_marker_insert(marker, edit),
                boundary,
            );
        }
        self.replace_suffix(boundary, new_after);
        self.rebuild_index();
    }

    fn shift_delete(&mut self, edit: DeleteShape) {
        if edit.start >= edit.end {
            return;
        }
        if edit.start.line == edit.end.line {
            self.replace_shifted_bucket(edit.start.line, |marker| {
                shift_marker_delete(marker, edit)
            });
            return;
        }

        let boundary = edit.start.line.min(self.line_count);
        let after = self.suffix_from(boundary);
        let deleted_lines = edit.end.line.saturating_sub(edit.start.line);
        let mut new_after = blank_vec_lines(after.len().saturating_sub(deleted_lines));
        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            insert_marker_into_vec_lines_offset(
                &mut new_after,
                shift_marker_delete(marker, edit),
                boundary,
            );
        }
        self.replace_suffix(boundary, new_after);
        self.rebuild_index();
    }

    fn insert_lines(&mut self, start_line: usize, count: usize) {
        if count == 0 {
            return;
        }
        let boundary = start_line.min(self.line_count);
        let after = self.suffix_from(boundary);
        let mut new_after = blank_vec_lines(after.len().saturating_add(count));
        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            insert_marker_into_vec_lines_offset(
                &mut new_after,
                insert_marker_shift_lines(marker, start_line, count),
                start_line,
            );
        }
        self.replace_suffix(boundary, new_after);
        self.rebuild_index();
    }

    fn delete_lines(&mut self, start_line: usize, count: usize) {
        if self.line_count == 0 || start_line >= self.line_count || count == 0 {
            return;
        }
        let actual_count = (self.line_count - start_line).min(count);
        let deleted_end = start_line + actual_count;
        if start_line == 0 && deleted_end >= self.line_count {
            self.line_count = 1;
            self.chunks = chunk_vec_lines(1);
            self.id_line_chunks.clear();
            return;
        }

        let after = self.suffix_from(start_line);
        let mut new_after = blank_vec_lines(after.len().saturating_sub(actual_count));
        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            if marker_anchor(&marker).line >= deleted_end {
                insert_marker_into_vec_lines_offset(
                    &mut new_after,
                    shift_marker_delete_lines(marker, actual_count),
                    start_line,
                );
            }
        }
        self.replace_suffix(start_line, new_after);
        self.rebuild_index();
    }

    fn clear_inlay_hints_for_lines(&mut self, start_line: usize, end_line: usize) {
        if start_line >= self.line_count || start_line >= end_line {
            return;
        }
        let end_line = end_line.min(self.line_count);
        for line in start_line..end_line {
            let Some(bucket) = self.bucket(line) else {
                continue;
            };
            let removed: Vec<_> = bucket
                .iter()
                .filter(|marker| matches!(marker.payload, PayloadKind::Inlay))
                .map(|marker| marker.id)
                .collect();
            if removed.is_empty() {
                continue;
            }
            if let Some(bucket) = self.bucket_mut(line) {
                let retained: Vec<_> = bucket
                    .iter()
                    .filter(|marker| matches!(marker.payload, PayloadKind::Ghost))
                    .cloned()
                    .collect();
                bucket.markers = Arc::from(retained.into_boxed_slice());
            }
            for id in removed {
                self.clear_index(id);
            }
        }
    }

    fn next_marker_id(&mut self) -> MarkerId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn set_index(&mut self, id: MarkerId, line: usize) {
        let idx = id as usize;
        let chunk_idx = idx / ID_CHUNK_SIZE;
        let slot_idx = idx % ID_CHUNK_SIZE;
        while self.id_line_chunks.len() <= chunk_idx {
            self.id_line_chunks
                .push(Arc::new(vec![None; ID_CHUNK_SIZE]));
        }
        Arc::make_mut(&mut self.id_line_chunks[chunk_idx])[slot_idx] = Some(line);
    }

    fn clear_index(&mut self, id: MarkerId) {
        let idx = id as usize;
        let chunk_idx = idx / ID_CHUNK_SIZE;
        let slot_idx = idx % ID_CHUNK_SIZE;
        if let Some(chunk) = self.id_line_chunks.get_mut(chunk_idx) {
            Arc::make_mut(chunk)[slot_idx] = None;
        }
    }

    fn index_line(&self, id: MarkerId) -> Option<usize> {
        let idx = id as usize;
        self.id_line_chunks
            .get(idx / ID_CHUNK_SIZE)?
            .get(idx % ID_CHUNK_SIZE)
            .copied()
            .flatten()
    }

    fn rebuild_index(&mut self) {
        self.id_line_chunks.clear();
        let mut entries = Vec::new();
        for line in 0..self.line_count {
            if let Some(bucket) = self.bucket(line) {
                entries.extend(bucket.iter().map(|marker| (marker.id, line)));
            }
        }
        for (id, line) in entries {
            self.set_index(id, line);
        }
    }

    fn bucket(&self, line: usize) -> Option<&LineBucket> {
        let chunk_idx = line / CHUNK_SIZE;
        let line_idx = line % CHUNK_SIZE;
        self.chunks.get(chunk_idx)?.get(line_idx)
    }

    fn bucket_mut(&mut self, line: usize) -> Option<&mut LineBucket> {
        let chunk_idx = line / CHUNK_SIZE;
        let line_idx = line % CHUNK_SIZE;
        Arc::make_mut(self.chunks.get_mut(chunk_idx)?).get_mut(line_idx)
    }

    fn insert_marker(&mut self, marker: Marker<PayloadKind>) {
        let line_idx = marker_anchor(&marker).line;
        self.ensure_line(line_idx);
        if let Some(bucket) = self.bucket_mut(line_idx) {
            bucket.insert_sorted(marker);
        }
    }

    fn ensure_line(&mut self, line: usize) {
        if line < self.line_count {
            return;
        }
        self.line_count = line + 1;
        while self.chunks.len() * CHUNK_SIZE < self.line_count {
            self.chunks.push(Arc::new(blank_vec_lines(CHUNK_SIZE)));
        }
    }

    fn replace_shifted_bucket(
        &mut self,
        line: usize,
        shift: impl Fn(Marker<PayloadKind>) -> Marker<PayloadKind>,
    ) {
        let Some(bucket) = self.bucket(line) else {
            return;
        };
        if bucket.is_empty() {
            return;
        }
        let mut new_markers: Vec<_> = bucket.iter().cloned().map(shift).collect();
        new_markers.sort_by_key(marker_anchor);
        let index_updates: Vec<_> = new_markers
            .iter()
            .map(|marker| (marker.id, marker_anchor(marker).line))
            .collect();
        if let Some(bucket) = self.bucket_mut(line) {
            bucket.markers = Arc::from(new_markers.into_boxed_slice());
        }
        for (id, line) in index_updates {
            self.set_index(id, line);
        }
    }

    fn suffix_from(&self, start_line: usize) -> Vec<LineBucket> {
        let start_line = start_line.min(self.line_count);
        let chunk_idx = start_line / CHUNK_SIZE;
        let line_idx = start_line % CHUNK_SIZE;
        let mut suffix = Vec::with_capacity(self.line_count.saturating_sub(start_line));

        if let Some(chunk) = self.chunks.get(chunk_idx) {
            let chunk_end = chunk
                .len()
                .min(self.line_count.saturating_sub(chunk_idx * CHUNK_SIZE));
            suffix.extend(chunk[line_idx..chunk_end].iter().cloned());
        }

        for chunk in self.chunks.iter().skip(chunk_idx + 1) {
            suffix.extend(chunk.iter().cloned());
        }

        suffix.truncate(self.line_count.saturating_sub(start_line));
        suffix
    }

    fn replace_suffix(&mut self, start_line: usize, suffix: Vec<LineBucket>) {
        let start_line = start_line.min(self.line_count);
        let full_prefix_chunks = start_line / CHUNK_SIZE;
        let prefix_remainder = start_line % CHUNK_SIZE;
        let mut chunks: Vec<_> = self
            .chunks
            .iter()
            .take(full_prefix_chunks)
            .cloned()
            .collect();
        let mut rebuilt_tail = Vec::with_capacity(prefix_remainder + suffix.len());

        if prefix_remainder > 0
            && let Some(chunk) = self.chunks.get(full_prefix_chunks)
        {
            rebuilt_tail.extend(chunk[..prefix_remainder].iter().cloned());
        }
        rebuilt_tail.extend(suffix);

        let suffix_len = rebuilt_tail.len().saturating_sub(prefix_remainder);
        chunks.extend(chunk_buckets(rebuilt_tail));
        self.line_count = start_line.saturating_add(suffix_len).max(1);
        self.chunks = chunks;
    }
}

#[test]
fn chunked_cow_matches_marker_store_for_edit_sequence() {
    let mut reference = MarkerStore::with_line_count(8);
    let mut candidate = ChunkedCowStore::with_line_count(8);
    assert_store_eq(&reference, &candidate, 8);

    for idx in 0..24 {
        let line = idx % 8;
        let col = (idx * 3) % 17;
        let payload = if idx % 3 == 0 {
            PayloadKind::Inlay
        } else {
            PayloadKind::Ghost
        };
        let gravity = if idx % 2 == 0 {
            Gravity::Right
        } else {
            Gravity::Left
        };
        let reference_id = reference.insert_point(Cursor::new(line, col), gravity, payload);
        let candidate_id = candidate.insert_point(Cursor::new(line, col), gravity, payload);
        assert_eq!(reference_id, candidate_id);
    }
    assert_store_eq(&reference, &candidate, 8);

    let edit = InsertShape {
        at: Cursor::new(3, 4),
        line_delta: 0,
        tail_col: 7,
    };
    reference.shift_insert(edit);
    candidate.shift_insert(edit);
    assert_store_eq(&reference, &candidate, 8);

    let edit = InsertShape {
        at: Cursor::new(2, 2),
        line_delta: 2,
        tail_col: 1,
    };
    reference.shift_insert(edit);
    candidate.shift_insert(edit);
    assert_store_eq(&reference, &candidate, 10);

    reference.insert_lines(4, 3);
    candidate.insert_lines(4, 3);
    assert_store_eq(&reference, &candidate, 13);

    let edit = DeleteShape {
        start: Cursor::new(1, 2),
        end: Cursor::new(1, 5),
    };
    reference.shift_delete(edit);
    candidate.shift_delete(edit);
    assert_store_eq(&reference, &candidate, 13);

    let edit = DeleteShape {
        start: Cursor::new(2, 1),
        end: Cursor::new(5, 3),
    };
    reference.shift_delete(edit);
    candidate.shift_delete(edit);
    assert_store_eq(&reference, &candidate, 10);

    clear_reference_inlays(&mut reference, 0, 10);
    candidate.clear_inlay_hints_for_lines(0, 10);
    assert_store_eq(&reference, &candidate, 10);

    reference.delete_lines(2, 2);
    candidate.delete_lines(2, 2);
    assert_store_eq(&reference, &candidate, 8);
}

#[test]
fn chunked_cow_matches_marker_store_for_removes_after_edits() {
    let mut reference = MarkerStore::with_line_count(8);
    let mut candidate = ChunkedCowStore::with_line_count(8);
    let mut ids = Vec::new();

    for idx in 0..24 {
        let line = idx % 8;
        let payload = if idx % 3 == 0 {
            PayloadKind::Inlay
        } else {
            PayloadKind::Ghost
        };
        ids.push(reference.insert_point(Cursor::new(line, idx % 11), Gravity::Right, payload));
        candidate.insert_point(Cursor::new(line, idx % 11), Gravity::Right, payload);
    }

    let edit = InsertShape {
        at: Cursor::new(2, 2),
        line_delta: 2,
        tail_col: 1,
    };
    reference.shift_insert(edit);
    candidate.shift_insert(edit);
    reference.delete_lines(2, 2);
    candidate.delete_lines(2, 2);

    for id in ids {
        let reference_removed = reference.remove(id);
        let candidate_removed = candidate.remove(id);
        assert_marker_eq(reference_removed.as_ref(), candidate_removed.as_ref());
        assert_store_eq(&reference, &candidate, 8);
    }
}

fn assert_store_eq(
    reference: &MarkerStore<PayloadKind>,
    candidate: &ChunkedCowStore,
    line_count: usize,
) {
    for line in 0..line_count {
        let mut reference_markers = reference
            .markers_for_line(line)
            .unwrap_or_default()
            .to_vec();
        let mut candidate_markers = candidate
            .markers_for_line(line)
            .unwrap_or_default()
            .to_vec();
        reference_markers.sort_by_key(|marker| marker.id);
        candidate_markers.sort_by_key(|marker| marker.id);
        assert_eq!(
            reference_markers.len(),
            candidate_markers.len(),
            "line {line}"
        );
        for (reference_marker, candidate_marker) in
            reference_markers.iter().zip(candidate_markers.iter())
        {
            assert_marker_eq(Some(reference_marker), Some(candidate_marker));
        }
    }

    for reference_marker in reference.iter() {
        assert_marker_eq(Some(reference_marker), candidate.get(reference_marker.id));
    }
}

fn assert_marker_eq(
    reference: Option<&Marker<PayloadKind>>,
    candidate: Option<&Marker<PayloadKind>>,
) {
    match (reference, candidate) {
        (Some(reference), Some(candidate)) => {
            assert_eq!(reference.id, candidate.id);
            assert_eq!(reference.kind, candidate.kind);
            assert_eq!(reference.payload, candidate.payload);
        }
        (None, None) => {}
        (reference, candidate) => panic!("marker mismatch: ref={reference:?} cand={candidate:?}"),
    }
}

fn clear_reference_inlays(
    reference: &mut MarkerStore<PayloadKind>,
    start_line: usize,
    end_line: usize,
) {
    let ids: Vec<_> = (start_line..end_line)
        .flat_map(|line| reference.markers_for_line(line).unwrap_or_default())
        .filter(|marker| matches!(marker.payload, PayloadKind::Inlay))
        .map(|marker| marker.id)
        .collect();
    for id in ids {
        reference.remove(id);
    }
}

fn blank_vec_lines(line_count: usize) -> Vec<LineBucket> {
    (0..line_count).map(|_| LineBucket::new()).collect()
}

fn chunk_vec_lines(line_count: usize) -> Vec<Arc<Vec<LineBucket>>> {
    chunk_buckets(blank_vec_lines(line_count))
}

fn chunk_buckets(mut lines: Vec<LineBucket>) -> Vec<Arc<Vec<LineBucket>>> {
    if lines.is_empty() {
        lines.push(LineBucket::new());
    }
    let mut chunks = Vec::with_capacity(lines.len().div_ceil(CHUNK_SIZE));
    for chunk in lines.chunks(CHUNK_SIZE) {
        chunks.push(Arc::new(chunk.to_vec()));
    }
    chunks
}

fn insert_marker_into_vec_lines_offset(
    lines: &mut [LineBucket],
    marker: Marker<PayloadKind>,
    offset_line: usize,
) {
    let line_idx = marker_anchor(&marker).line.saturating_sub(offset_line);
    if let Some(bucket) = lines.get_mut(line_idx) {
        bucket.insert_sorted(marker);
    }
}

fn marker_anchor<T>(marker: &Marker<T>) -> Cursor {
    match marker.kind {
        MarkerShape::Point(point) => point.pos,
        MarkerShape::Range(range) => range.start,
    }
}

fn insertion_index<T>(markers: &[Marker<T>], anchor: Cursor) -> usize {
    let mut low = 0usize;
    let mut high = markers.len();
    while low < high {
        let mid = low + (high - low) / 2;
        if marker_anchor(&markers[mid]) <= anchor {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

fn shift_marker_insert<T>(marker: Marker<T>, edit: InsertShape) -> Marker<T> {
    match marker.kind {
        MarkerShape::Point(point) => Marker {
            id: marker.id,
            kind: MarkerShape::Point(PointMarker {
                pos: shift_cursor_insert(point.pos, edit, point.gravity),
                gravity: point.gravity,
            }),
            payload: marker.payload,
        },
        MarkerShape::Range(range) => Marker {
            id: marker.id,
            kind: MarkerShape::Range(RangeMarker {
                start: shift_cursor_insert(range.start, edit, range.start_gravity),
                end: shift_cursor_insert(range.end, edit, range.end_gravity),
                start_gravity: range.start_gravity,
                end_gravity: range.end_gravity,
            }),
            payload: marker.payload,
        },
    }
}

fn shift_marker_delete<T>(marker: Marker<T>, edit: DeleteShape) -> Marker<T> {
    match marker.kind {
        MarkerShape::Point(point) => Marker {
            id: marker.id,
            kind: MarkerShape::Point(PointMarker {
                pos: shift_cursor_delete(point.pos, edit),
                gravity: point.gravity,
            }),
            payload: marker.payload,
        },
        MarkerShape::Range(range) => Marker {
            id: marker.id,
            kind: MarkerShape::Range(RangeMarker {
                start: shift_cursor_delete(range.start, edit),
                end: shift_cursor_delete(range.end, edit),
                start_gravity: range.start_gravity,
                end_gravity: range.end_gravity,
            }),
            payload: marker.payload,
        },
    }
}

fn insert_marker_shift_lines<T>(
    mut marker: Marker<T>,
    start_line: usize,
    count: usize,
) -> Marker<T> {
    match &mut marker.kind {
        MarkerShape::Point(point) => {
            if point.pos.line >= start_line {
                point.pos.line += count;
            }
        }
        MarkerShape::Range(range) => {
            if range.start.line >= start_line {
                range.start.line += count;
            }
            if range.end.line >= start_line {
                range.end.line += count;
            }
        }
    }
    marker
}

fn shift_marker_delete_lines<T>(mut marker: Marker<T>, count: usize) -> Marker<T> {
    match &mut marker.kind {
        MarkerShape::Point(point) => point.pos.line -= count,
        MarkerShape::Range(range) => {
            range.start.line -= count;
            range.end.line -= count;
        }
    }
    marker
}

fn shift_cursor_insert(cursor: Cursor, edit: InsertShape, gravity: Gravity) -> Cursor {
    if cursor.line < edit.at.line {
        return cursor;
    }
    if cursor.line > edit.at.line {
        return Cursor::new(cursor.line + edit.line_delta, cursor.col);
    }
    let after_insertion = cursor.col > edit.at.col
        || (cursor.col == edit.at.col && matches!(gravity, Gravity::Right));
    if !after_insertion {
        return cursor;
    }
    if edit.line_delta == 0 {
        Cursor::new(
            cursor.line,
            cursor.col + edit.tail_col.saturating_sub(edit.at.col),
        )
    } else {
        Cursor::new(
            cursor.line + edit.line_delta,
            edit.tail_col + cursor.col.saturating_sub(edit.at.col),
        )
    }
}

fn shift_cursor_delete(cursor: Cursor, edit: DeleteShape) -> Cursor {
    if cursor < edit.start {
        return cursor;
    }
    if cursor >= edit.end {
        return cursor_after_delete(cursor, edit);
    }
    edit.start
}

fn cursor_after_delete(cursor: Cursor, edit: DeleteShape) -> Cursor {
    if edit.start.line == edit.end.line {
        if cursor.line == edit.end.line {
            return Cursor::new(
                cursor.line,
                cursor
                    .col
                    .saturating_sub(edit.end.col.saturating_sub(edit.start.col)),
            );
        }
        return Cursor::new(cursor.line, cursor.col);
    }
    let deleted_line_count = edit.end.line.saturating_sub(edit.start.line);
    if cursor.line == edit.end.line {
        return Cursor::new(
            edit.start.line,
            edit.start.col + cursor.col.saturating_sub(edit.end.col),
        );
    }
    Cursor::new(cursor.line.saturating_sub(deleted_line_count), cursor.col)
}
