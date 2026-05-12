use super::{Buffer, Cursor, MarkersStore};
use crate::terminal::Style;
use crate::theme::StyleOverlay;
use imbl::Vector;
use smol_str::SmolStr;
use std::sync::Arc;

/// Marker payload kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    /// LSP inlay hint inserted inline.
    InlayHint,
}

/// A marker payload shared by ghost text and inlay hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerPayload {
    /// Marker label text.
    pub label: SmolStr,
    /// Marker kind, present only for inlay hints.
    pub kind: Option<MarkerKind>,
    /// Optional style override.
    pub style: Option<StyleOverlay>,
}

impl MarkerPayload {
    /// Creates a new marker payload.
    pub fn new(label: impl Into<SmolStr>) -> Self {
        Self {
            label: label.into(),
            kind: None,
            style: None,
        }
    }

    /// Creates an inlay-hint payload.
    pub fn inlay_hint(label: impl Into<SmolStr>) -> Self {
        Self {
            label: label.into(),
            kind: Some(MarkerKind::InlayHint),
            style: None,
        }
    }

    /// Resolves the display style for this marker.
    pub fn style(&self, default_ghost_style: Style, inlay_hint_style: Style) -> Style {
        let base_style = if self.kind.is_some() {
            inlay_hint_style
        } else {
            default_ghost_style
        };

        self.style
            .map_or(base_style, |style| style.apply_to(base_style))
    }
}

/// Stable identifier for a marker stored in a [`MarkerStore`].
pub type MarkerId = u64;

/// Gravity controls which side of an edit a marker prefers at an exact boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravity {
    /// Keep the marker on the left side of the edit boundary.
    Left,
    /// Move the marker to the right side of the edit boundary.
    Right,
}

/// Describes the geometry of an insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InsertShape {
    /// Insertion cursor.
    pub at: Cursor,
    /// Number of newline characters inserted.
    pub line_delta: usize,
    /// Column where the original suffix resumes on the final inserted line.
    pub tail_col: usize,
}

/// Describes the geometry of a deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteShape {
    /// Inclusive start cursor.
    pub start: Cursor,
    /// Exclusive end cursor.
    pub end: Cursor,
}

/// A marker anchored to a single cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointMarker {
    /// Marker position.
    pub pos: Cursor,
    /// Exact-boundary insertion behavior.
    pub gravity: Gravity,
}

/// A marker anchored to a half-open range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeMarker {
    /// Range start, inclusive.
    pub start: Cursor,
    /// Range end, exclusive.
    pub end: Cursor,
    /// Exact-boundary behavior at the start.
    pub start_gravity: Gravity,
    /// Exact-boundary behavior at the end.
    pub end_gravity: Gravity,
}

/// Marker geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerShape {
    /// A single anchored point.
    Point(PointMarker),
    /// A half-open range.
    Range(RangeMarker),
}

/// A marker with attached payload.
#[derive(Debug, Clone)]
pub struct Marker<T> {
    /// Stable marker identifier.
    pub id: MarkerId,
    /// Marker geometry.
    pub kind: MarkerShape,
    /// Attached external data.
    pub payload: T,
}

impl<T> Marker<T> {
    fn anchor(&self) -> Cursor {
        match self.kind {
            MarkerShape::Point(point) => point.pos,
            MarkerShape::Range(range) => range.start,
        }
    }
}

/// A line-local bucket of markers.
#[derive(Debug, Clone)]
pub struct LineBucket<T> {
    markers: Arc<[Marker<T>]>,
}

impl<T> LineBucket<T> {
    fn new() -> Self {
        Self {
            markers: Arc::from(Vec::<Marker<T>>::new()),
        }
    }

    fn is_empty(&self) -> bool {
        self.markers.is_empty()
    }

    fn len(&self) -> usize {
        self.markers.len()
    }

    fn get(&self, id: MarkerId) -> Option<&Marker<T>> {
        self.markers.iter().find(|marker| marker.id == id)
    }

    fn get_mut(&mut self, id: MarkerId) -> Option<&mut Marker<T>>
    where
        T: Clone,
    {
        let markers = Arc::make_mut(&mut self.markers);
        markers.iter_mut().find(|marker| marker.id == id)
    }

    fn remove(&mut self, id: MarkerId) -> Option<Marker<T>>
    where
        T: Clone,
    {
        let mut markers = self.markers.as_ref().to_vec();
        let index = markers.iter().position(|marker| marker.id == id)?;
        let removed = markers.remove(index);
        self.markers = Arc::from(markers.into_boxed_slice());
        Some(removed)
    }

    fn iter(&self) -> impl Iterator<Item = &Marker<T>> {
        self.markers.iter()
    }

    fn insert_sorted(&mut self, marker: Marker<T>)
    where
        T: Clone,
    {
        let anchor = marker.anchor();
        let mut markers = self.markers.as_ref().to_vec();
        let index = insertion_index(&markers, anchor);
        markers.insert(index, marker);
        self.markers = Arc::from(markers.into_boxed_slice());
    }
}

/// Generic marker store organized by line buckets.
#[derive(Debug, Clone)]
pub struct MarkerStore<T> {
    lines: Vector<LineBucket<T>>,
    next_id: MarkerId,
}

impl<T: Clone> Default for MarkerStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> MarkerStore<T> {
    /// Creates an empty marker store with one empty line bucket.
    pub fn new() -> Self {
        Self::with_line_count(1)
    }

    /// Creates an empty marker store with the requested line count.
    pub fn with_line_count(line_count: usize) -> Self {
        Self {
            lines: blank_lines(line_count.max(1)),
            next_id: 0,
        }
    }

    /// Returns the number of markers in the store.
    pub fn len(&self) -> usize {
        self.lines.iter().map(LineBucket::len).sum()
    }

    /// Returns true when the store contains no markers.
    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(LineBucket::is_empty)
    }

    /// Removes all markers and resets the line count to one.
    pub fn clear(&mut self) {
        self.lines = blank_lines(1);
        self.next_id = 0;
    }

    /// Removes all markers and resets the line count.
    pub fn clear_to_line_count(&mut self, line_count: usize) {
        self.lines = blank_lines(line_count.max(1));
        self.next_id = 0;
    }

    /// Returns an immutable marker by id.
    pub fn get(&self, id: MarkerId) -> Option<&Marker<T>> {
        self.lines.iter().find_map(|bucket| bucket.get(id))
    }

    /// Returns a mutable marker by id.
    pub fn get_mut(&mut self, id: MarkerId) -> Option<&mut Marker<T>> {
        self.lines.iter_mut().find_map(|bucket| bucket.get_mut(id))
    }

    /// Removes a marker by id.
    pub fn remove(&mut self, id: MarkerId) -> Option<Marker<T>> {
        self.lines.iter_mut().find_map(|bucket| bucket.remove(id))
    }

    /// Returns all markers in line and position order.
    pub fn iter(&self) -> impl Iterator<Item = &Marker<T>> {
        self.lines.iter().flat_map(LineBucket::iter)
    }

    /// Returns the markers stored on a specific line.
    pub fn markers_for_line(&self, line: usize) -> Option<&[Marker<T>]> {
        self.lines.get(line).map(|bucket| bucket.markers.as_ref())
    }

    /// Inserts a point marker.
    pub fn insert_point(&mut self, pos: Cursor, gravity: Gravity, payload: T) -> MarkerId {
        let id = self.next_marker_id();
        let marker = Marker {
            id,
            kind: MarkerShape::Point(PointMarker { pos, gravity }),
            payload,
        };
        self.insert_marker(marker);
        id
    }

    /// Inserts a range marker.
    pub fn insert_range(
        &mut self,
        start: Cursor,
        end: Cursor,
        start_gravity: Gravity,
        end_gravity: Gravity,
        payload: T,
    ) -> MarkerId {
        let (start, end, start_gravity, end_gravity) =
            normalize_range(start, end, start_gravity, end_gravity);
        let id = self.next_marker_id();
        let marker = Marker {
            id,
            kind: MarkerShape::Range(RangeMarker {
                start,
                end,
                start_gravity,
                end_gravity,
            }),
            payload,
        };
        self.insert_marker(marker);
        id
    }

    /// Shifts markers for an insertion.
    pub fn shift_insert(&mut self, edit: InsertShape) {
        if edit.line_delta == 0 {
            self.shift_insert_single_line(edit);
        } else {
            self.shift_insert_multi_line(edit);
        }
    }

    fn shift_insert_single_line(&mut self, edit: InsertShape) {
        let line = edit.at.line;
        let Some(bucket) = self.lines.get(line) else {
            return;
        };
        if bucket.is_empty() {
            return;
        }

        let mut new_markers: Vec<Marker<T>> = Vec::with_capacity(bucket.len());
        for marker in bucket.iter().cloned() {
            new_markers.push(shift_marker_insert(marker, edit));
        }
        new_markers.sort_by_key(|m| m.anchor());
        let new_bucket = LineBucket {
            markers: Arc::from(new_markers.into_boxed_slice()),
        };
        self.lines = self.lines.update(line, new_bucket);
    }

    fn shift_insert_multi_line(&mut self, edit: InsertShape) {
        let boundary = edit.at.line;
        let after = self.lines.skip(boundary);
        let before = self.lines.take(boundary);

        let new_line_count = after.len().saturating_add(edit.line_delta).max(1);
        let mut new_after = blank_lines(new_line_count);

        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            let marker = shift_marker_insert(marker, edit);
            insert_marker_into_lines_offset(&mut new_after, marker, boundary);
        }

        self.lines = before;
        self.lines.append(new_after);
    }

    /// Inserts whole lines at a given line index.
    pub fn insert_lines(&mut self, start_line: usize, count: usize) {
        if count == 0 {
            return;
        }

        let after = self.lines.skip(start_line);
        let before = self.lines.take(start_line);

        let new_line_count = after.len().saturating_add(count).max(1);
        let mut new_after = blank_lines(new_line_count);

        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            let marker = insert_marker_shift_lines(marker, start_line, count);
            insert_marker_into_lines_offset(&mut new_after, marker, start_line);
        }

        self.lines = before;
        self.lines.append(new_after);
    }

    /// Shifts markers for a deletion.
    pub fn shift_delete(&mut self, edit: DeleteShape) {
        if edit.start >= edit.end {
            return;
        }

        if edit.start.line == edit.end.line {
            self.shift_delete_single_line(edit);
        } else {
            self.shift_delete_multi_line(edit);
        }
    }

    fn shift_delete_single_line(&mut self, edit: DeleteShape) {
        let line = edit.start.line;
        let Some(bucket) = self.lines.get(line) else {
            return;
        };
        if bucket.is_empty() {
            return;
        }

        let mut new_markers: Vec<Marker<T>> = Vec::with_capacity(bucket.len());
        for marker in bucket.iter().cloned() {
            new_markers.push(shift_marker_delete(marker, edit));
        }
        new_markers.sort_by_key(|m| m.anchor());
        let new_bucket = LineBucket {
            markers: Arc::from(new_markers.into_boxed_slice()),
        };
        self.lines = self.lines.update(line, new_bucket);
    }

    fn shift_delete_multi_line(&mut self, edit: DeleteShape) {
        let boundary = edit.start.line;
        let after = self.lines.skip(boundary);
        let before = self.lines.take(boundary);

        let deleted_lines = edit.end.line.saturating_sub(edit.start.line);
        let new_line_count = after.len().saturating_sub(deleted_lines).max(1);
        let mut new_after = blank_lines(new_line_count);

        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            let marker = shift_marker_delete(marker, edit);
            insert_marker_into_lines_offset(&mut new_after, marker, boundary);
        }

        self.lines = before;
        self.lines.append(new_after);
    }

    /// Clears markers on the specified half-open line range.
    pub fn clear_line_range(&mut self, start_line: usize, end_line: usize) {
        if start_line >= end_line {
            return;
        }

        let num_lines = self.lines.len();
        if start_line >= num_lines {
            return;
        }
        let end_line = end_line.min(num_lines);

        let before = self.lines.take(start_line);
        let middle_len = end_line - start_line;
        let after = self.lines.skip(end_line);

        let mut result = before;
        for _ in 0..middle_len {
            result.push_back(LineBucket::new());
        }
        result.append(after);
        self.lines = result;
    }

    /// Deletes complete lines and removes markers anchored to the deleted range.
    pub fn delete_lines(&mut self, start_line: usize, count: usize) {
        let total_lines = self.lines.len();
        if total_lines == 0 || start_line >= total_lines || count == 0 {
            return;
        }

        let actual_count = (total_lines - start_line).min(count);
        let deleted_end = start_line + actual_count;

        if start_line == 0 && deleted_end >= total_lines {
            self.lines = blank_lines(1);
            return;
        }

        let after = self.lines.skip(start_line);
        let before = self.lines.take(start_line);

        let new_line_count = after.len().saturating_sub(actual_count).max(1);
        let mut new_after = blank_lines(new_line_count);

        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            let line = marker.anchor().line;
            if line >= deleted_end {
                let mut marker = marker;
                match &mut marker.kind {
                    MarkerShape::Point(point) => point.pos.line -= actual_count,
                    MarkerShape::Range(range) => {
                        range.start.line -= actual_count;
                        range.end.line -= actual_count;
                    }
                }
                insert_marker_into_lines_offset(&mut new_after, marker, start_line);
            }
        }

        self.lines = before;
        self.lines.append(new_after);
    }

    fn next_marker_id(&mut self) -> MarkerId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn insert_marker(&mut self, marker: Marker<T>) {
        insert_marker_into_lines(&mut self.lines, marker);
    }
}

impl Buffer {
    /// Returns the current marker store.
    pub fn markers(&self) -> &MarkersStore {
        &self.markers
    }

    /// Returns a marker entry by id.
    pub fn marker(&self, id: MarkerId) -> Option<&Marker<MarkerPayload>> {
        self.markers.get(id)
    }

    /// Returns the markers stored on a line.
    pub fn markers_for_line(&self, line: usize) -> Option<&[Marker<MarkerPayload>]> {
        self.markers.markers_for_line(line)
    }

    /// Returns the ghost-text markers stored on a line.
    pub fn ghost_texts_for_line(&self, line: usize) -> Option<Vec<Marker<MarkerPayload>>> {
        self.markers_for_line(line).map(|markers| {
            markers
                .iter()
                .filter(|marker| marker.payload.kind.is_none())
                .cloned()
                .collect()
        })
    }

    /// Returns the inlay-hint markers stored on a line.
    pub fn inlay_hints_for_line(&self, line: usize) -> Option<Vec<Marker<MarkerPayload>>> {
        self.markers_for_line(line).map(|markers| {
            markers
                .iter()
                .filter(|marker| marker.payload.kind.is_some())
                .cloned()
                .collect()
        })
    }

    /// Returns the ghost-text marker by id.
    pub fn ghost_text(&self, id: MarkerId) -> Option<&Marker<MarkerPayload>> {
        self.marker(id)
            .filter(|marker| marker.payload.kind.is_none())
    }

    /// Returns the inlay-hint marker by id.
    pub fn inlay_hint(&self, id: MarkerId) -> Option<&Marker<MarkerPayload>> {
        self.marker(id)
            .filter(|marker| marker.payload.kind.is_some())
    }

    /// Inserts ghost text anchored to a point.
    pub fn insert_ghost_text(
        &mut self,
        pos: Cursor,
        gravity: Gravity,
        label: impl Into<SmolStr>,
    ) -> MarkerId {
        self.insert_marker(pos, gravity, MarkerPayload::new(label))
    }

    /// Inserts an inlay hint anchored to a point.
    pub fn insert_inlay_hint(
        &mut self,
        pos: Cursor,
        gravity: Gravity,
        label: impl Into<SmolStr>,
    ) -> MarkerId {
        self.insert_marker(pos, gravity, MarkerPayload::inlay_hint(label))
    }

    /// Removes a marker by id.
    pub fn remove_marker(&mut self, id: MarkerId) -> Option<Marker<MarkerPayload>> {
        let removed = self.markers.remove(id);
        if removed.is_some() {
            self.bump_visual_generation();
            self.update_markers();
        }
        removed
    }

    /// Removes ghost text by id.
    pub fn remove_ghost_text(&mut self, id: MarkerId) -> Option<Marker<MarkerPayload>> {
        if self
            .marker(id)
            .is_some_and(|marker| marker.payload.kind.is_none())
        {
            self.remove_marker(id)
        } else {
            None
        }
    }

    /// Clears all markers.
    pub fn clear_markers(&mut self) {
        self.markers.clear_to_line_count(self.lines.len());
        self.bump_visual_generation();
        self.update_markers();
    }

    /// Clears markers on a line range.
    pub fn clear_markers_for_lines(&mut self, start_line: usize, end_line: usize) {
        self.markers.clear_line_range(start_line, end_line);
        self.bump_visual_generation();
        self.update_markers();
    }

    /// Clears all ghost text.
    pub fn clear_ghost_texts(&mut self) {
        self.retain_markers(|payload| payload.kind.is_some());
    }

    /// Clears all inlay hints.
    pub fn clear_inlay_hints(&mut self) {
        self.retain_markers(|payload| payload.kind.is_none());
    }

    /// Clears inlay hints on a line range.
    pub fn clear_inlay_hints_for_lines(&mut self, start_line: usize, end_line: usize) {
        self.retain_markers_in_line_range(start_line, end_line, |payload| payload.kind.is_none());
    }

    /// Clears inlay hints whose anchors are inside a half-open cursor range.
    pub fn clear_inlay_hints_in_range(&mut self, start: Cursor, end: Cursor) {
        if start >= end {
            return;
        }

        let num_lines = self.markers.lines.len();
        let start_line = start.line;
        let end_line = end.line.min(num_lines.saturating_sub(1));

        // Lines before start_line: keep all markers as-is
        let before = self.markers.lines.take(start_line);

        // Lines after end_line: keep all markers as-is (anchor >= end)
        let after = if end_line.saturating_add(1) < num_lines {
            self.markers.lines.skip(end_line + 1)
        } else {
            Vector::new()
        };

        // Lines start_line..=end_line: scan and filter
        let affected_start = self.markers.lines.skip(start_line);
        let affected_len = end_line
            .saturating_sub(start_line)
            .saturating_add(1)
            .min(affected_start.len());
        let affected = affected_start.take(affected_len);

        let mut new_affected = blank_lines(affected.len());
        for marker in affected.iter().flat_map(LineBucket::iter).cloned() {
            let anchor = marker.anchor();
            let stale_inlay_hint = marker.payload.kind.is_some() && anchor >= start && anchor < end;
            if !stale_inlay_hint {
                insert_marker_into_lines_offset(&mut new_affected, marker, start_line);
            }
        }

        let mut result = before;
        result.append(new_affected);
        result.append(after);
        self.markers.lines = result;
        self.bump_visual_generation();
        self.update_markers();
    }

    fn insert_marker(&mut self, pos: Cursor, gravity: Gravity, payload: MarkerPayload) -> MarkerId {
        let id = self.markers.insert_point(pos, gravity, payload);
        self.bump_visual_generation();
        self.update_markers();
        id
    }

    fn retain_markers(&mut self, keep: impl Fn(&MarkerPayload) -> bool) {
        self.retain_markers_in_line_range(0, self.lines.len(), keep);
    }

    fn retain_markers_in_line_range(
        &mut self,
        start_line: usize,
        end_line: usize,
        keep: impl Fn(&MarkerPayload) -> bool,
    ) {
        let num_lines = self.markers.lines.len();
        if start_line >= num_lines || start_line >= end_line {
            return;
        }
        let end_line = end_line.min(num_lines);

        // Three-way split: keep markers in [0, start_line) and [end_line, num_lines) as-is
        // Only process markers in [start_line, end_line)
        let before = self.markers.lines.take(start_line);
        let middle = self
            .markers
            .lines
            .skip(start_line)
            .take(end_line - start_line);
        let after = self.markers.lines.skip(end_line);

        let mut new_middle = blank_lines(middle.len());
        for marker in middle.iter().flat_map(LineBucket::iter).cloned() {
            if !keep(&marker.payload) {
                // Within the range and doesn't satisfy keep — drop it
                continue;
            }
            insert_marker_into_lines_offset(&mut new_middle, marker, start_line);
        }

        let mut result = before;
        result.append(new_middle);
        result.append(after);
        self.markers.lines = result;
        self.bump_visual_generation();
        self.update_markers();
    }
}

fn blank_lines<T: Clone>(line_count: usize) -> Vector<LineBucket<T>> {
    let mut lines: Vector<LineBucket<T>> = Vector::new();
    for _ in 0..line_count.max(1) {
        lines.push_back(LineBucket::new());
    }
    lines
}

fn insertion_index<T>(markers: &[Marker<T>], anchor: Cursor) -> usize {
    let mut low = 0usize;
    let mut high = markers.len();

    while low < high {
        let mid = low + (high - low) / 2;
        if markers[mid].anchor() <= anchor {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    low
}

fn insert_marker_into_lines<T: Clone>(lines: &mut Vector<LineBucket<T>>, marker: Marker<T>) {
    let line_idx = marker.anchor().line;
    while lines.len() <= line_idx {
        lines.push_back(LineBucket::new());
    }

    if let Some(bucket) = lines.get_mut(line_idx) {
        bucket.insert_sorted(marker);
    }
}

fn insert_marker_into_lines_offset<T: Clone>(
    lines: &mut Vector<LineBucket<T>>,
    marker: Marker<T>,
    offset_line: usize,
) {
    let line_idx = marker.anchor().line.saturating_sub(offset_line);
    if let Some(bucket) = lines.get_mut(line_idx) {
        bucket.insert_sorted(marker);
    }
}

fn normalize_range(
    start: Cursor,
    end: Cursor,
    start_gravity: Gravity,
    end_gravity: Gravity,
) -> (Cursor, Cursor, Gravity, Gravity) {
    if start <= end {
        (start, end, start_gravity, end_gravity)
    } else {
        (end, start, end_gravity, start_gravity)
    }
}

fn shift_marker_insert<T: Clone>(marker: Marker<T>, edit: InsertShape) -> Marker<T> {
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

fn shift_marker_delete<T: Clone>(marker: Marker<T>, edit: DeleteShape) -> Marker<T> {
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

fn insert_marker_shift_lines<T: Clone>(
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
