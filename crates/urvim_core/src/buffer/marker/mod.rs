use super::{Buffer, Cursor, MarkersStore};
use smol_str::SmolStr;
use std::collections::BTreeSet;
use std::sync::Arc;
use urvim_terminal::Style;
use urvim_theme::StyleOverlay;

const CHUNK_SIZE: usize = 128;
const ID_CHUNK_SIZE: usize = 512;
const NAMESPACE_CHUNK_SIZE: usize = 256;

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

/// Location of a marker inside the forward namespace index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NamespaceLocation {
    namespace_index: usize,
    slot_index: usize,
}

#[derive(Debug, Clone)]
struct NamespaceIndex {
    name: SmolStr,
    id_chunks: Vec<Arc<Vec<Option<MarkerId>>>>,
    next_slot: usize,
    len: usize,
}

impl NamespaceIndex {
    fn new(name: &str) -> Self {
        Self {
            name: SmolStr::new(name),
            id_chunks: Vec::new(),
            next_slot: 0,
            len: 0,
        }
    }

    fn insert(&mut self, id: MarkerId) -> usize {
        let slot = self.next_slot;
        let chunk_idx = slot / NAMESPACE_CHUNK_SIZE;
        let slot_idx = slot % NAMESPACE_CHUNK_SIZE;
        while self.id_chunks.len() <= chunk_idx {
            self.id_chunks
                .push(Arc::new(vec![None; NAMESPACE_CHUNK_SIZE]));
        }
        Arc::make_mut(&mut self.id_chunks[chunk_idx])[slot_idx] = Some(id);
        self.next_slot += 1;
        self.len += 1;
        slot
    }

    fn remove(&mut self, slot: usize) {
        let chunk_idx = slot / NAMESPACE_CHUNK_SIZE;
        let slot_idx = slot % NAMESPACE_CHUNK_SIZE;
        if let Some(chunk) = self.id_chunks.get_mut(chunk_idx)
            && Arc::make_mut(chunk)[slot_idx].take().is_some()
        {
            self.len = self.len.saturating_sub(1);
        }
        if self.len == 0 {
            self.id_chunks.clear();
            self.next_slot = 0;
        }
    }

    fn ids(&self) -> impl Iterator<Item = MarkerId> + '_ {
        self.id_chunks
            .iter()
            .flat_map(|chunk| chunk.iter().copied().flatten())
    }
}

/// Generic marker store organized by line buckets.
#[derive(Debug, Clone)]
pub struct MarkerStore<T> {
    chunks: Vec<Arc<Vec<LineBucket<T>>>>,
    id_line_chunks: Vec<Arc<Vec<Option<usize>>>>,
    namespace_locations_by_id: Vec<Arc<Vec<Option<NamespaceLocation>>>>,
    namespaces: Vec<Arc<NamespaceIndex>>,
    line_count: usize,
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
        let line_count = line_count.max(1);
        Self {
            chunks: chunk_vec_lines(line_count),
            id_line_chunks: Vec::new(),
            namespace_locations_by_id: Vec::new(),
            namespaces: Vec::new(),
            line_count,
            next_id: 0,
        }
    }

    /// Returns the number of markers in the store.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Returns true when the store contains no markers.
    pub fn is_empty(&self) -> bool {
        self.chunks
            .iter()
            .flat_map(|chunk| chunk.iter())
            .all(LineBucket::is_empty)
    }

    /// Removes all markers and resets the line count to one.
    pub fn clear(&mut self) {
        self.chunks = chunk_vec_lines(1);
        self.id_line_chunks.clear();
        self.namespace_locations_by_id.clear();
        self.namespaces.clear();
        self.line_count = 1;
        self.next_id = 0;
    }

    /// Removes all markers and resets the line count.
    pub fn clear_to_line_count(&mut self, line_count: usize) {
        self.line_count = line_count.max(1);
        self.chunks = chunk_vec_lines(self.line_count);
        self.id_line_chunks.clear();
        self.namespace_locations_by_id.clear();
        self.namespaces.clear();
        self.next_id = 0;
    }

    /// Returns an immutable marker by id.
    pub fn get(&self, id: MarkerId) -> Option<&Marker<T>> {
        let line = self.index_line(id)?;
        self.bucket(line)?.get(id)
    }

    /// Returns a mutable marker by id.
    pub fn get_mut(&mut self, id: MarkerId) -> Option<&mut Marker<T>> {
        let line = self.index_line(id)?;
        self.bucket_mut(line)?.get_mut(id)
    }

    /// Removes a marker by id.
    pub fn remove(&mut self, id: MarkerId) -> Option<Marker<T>> {
        let line = self.index_line(id)?;
        let removed = self.bucket_mut(line)?.remove(id);
        if removed.is_some() {
            self.clear_index(id);
        }
        removed
    }

    /// Returns whether a marker belongs to `namespace`.
    pub fn marker_is_in_namespace(&self, id: MarkerId, namespace: &str) -> bool {
        self.namespace_for_id(id)
            .is_some_and(|stored| stored == namespace)
    }

    /// Returns the live marker ids belonging to `namespace`.
    pub fn marker_ids_in_namespace(&self, namespace: &str) -> Vec<MarkerId> {
        self.namespaces
            .iter()
            .find(|index| index.name == namespace)
            .map(|index| index.ids().collect())
            .unwrap_or_default()
    }

    /// Returns all markers in line and position order.
    pub fn iter(&self) -> impl Iterator<Item = &Marker<T>> {
        (0..self.line_count)
            .flat_map(|line| self.bucket(line).into_iter().flat_map(LineBucket::iter))
    }

    /// Returns the markers stored on a specific line.
    pub fn markers_for_line(&self, line: usize) -> Option<&[Marker<T>]> {
        if line >= self.line_count {
            return None;
        }
        self.bucket(line).map(|bucket| bucket.markers.as_ref())
    }

    /// Inserts a point marker.
    pub fn insert_point(&mut self, pos: Cursor, gravity: Gravity, payload: T) -> MarkerId {
        self.insert_point_in_namespace(pos, gravity, payload, None)
    }

    /// Inserts a point marker belonging to an optional namespace.
    pub fn insert_point_in_namespace(
        &mut self,
        pos: Cursor,
        gravity: Gravity,
        payload: T,
        namespace: Option<&str>,
    ) -> MarkerId {
        let id = self.next_marker_id();
        let marker = Marker {
            id,
            kind: MarkerShape::Point(PointMarker { pos, gravity }),
            payload,
        };
        self.insert_marker(marker);
        self.set_index(id, pos.line);
        if let Some(namespace) = namespace {
            self.set_namespace_index(id, namespace);
        }
        id
    }

    /// Replaces a point marker while preserving its id and namespace.
    pub fn update_point(
        &mut self,
        id: MarkerId,
        pos: Cursor,
        gravity: Gravity,
        payload: T,
    ) -> bool {
        if !self
            .get(id)
            .is_some_and(|marker| matches!(marker.kind, MarkerShape::Point(_)))
        {
            return false;
        }
        let Some(line) = self.index_line(id) else {
            return false;
        };
        let Some(mut marker) = self.bucket_mut(line).and_then(|bucket| bucket.remove(id)) else {
            return false;
        };
        self.clear_line_index(id);
        marker.kind = MarkerShape::Point(PointMarker { pos, gravity });
        marker.payload = payload;
        self.insert_marker(marker);
        self.set_index(id, pos.line);
        true
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
        self.set_index(id, start.line);
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
        let Some(bucket) = self.bucket(line) else {
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
        let index_updates: Vec<_> = new_markers
            .iter()
            .map(|marker| (marker.id, marker.anchor().line))
            .collect();
        if let Some(bucket) = self.bucket_mut(line) {
            bucket.markers = Arc::from(new_markers.into_boxed_slice());
        }
        for (id, line) in index_updates {
            self.set_index(id, line);
        }
    }

    fn shift_insert_multi_line(&mut self, edit: InsertShape) {
        let boundary = edit.at.line.min(self.line_count);
        let after = self.suffix_from(boundary);
        let mut new_after = blank_vec_lines(after.len().saturating_add(edit.line_delta));

        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            let marker = shift_marker_insert(marker, edit);
            insert_marker_into_vec_lines_offset(&mut new_after, marker, boundary);
        }

        self.replace_suffix(boundary, new_after);
        self.rebuild_index();
    }

    /// Inserts whole lines at a given line index.
    pub fn insert_lines(&mut self, start_line: usize, count: usize) {
        if count == 0 {
            return;
        }

        let boundary = start_line.min(self.line_count);
        let after = self.suffix_from(boundary);
        let mut new_after = blank_vec_lines(after.len().saturating_add(count));

        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            let marker = insert_marker_shift_lines(marker, start_line, count);
            insert_marker_into_vec_lines_offset(&mut new_after, marker, start_line);
        }

        self.replace_suffix(boundary, new_after);
        self.rebuild_index();
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
        let Some(bucket) = self.bucket(line) else {
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
        let index_updates: Vec<_> = new_markers
            .iter()
            .map(|marker| (marker.id, marker.anchor().line))
            .collect();
        if let Some(bucket) = self.bucket_mut(line) {
            bucket.markers = Arc::from(new_markers.into_boxed_slice());
        }
        for (id, line) in index_updates {
            self.set_index(id, line);
        }
    }

    fn shift_delete_multi_line(&mut self, edit: DeleteShape) {
        let boundary = edit.start.line.min(self.line_count);
        let after = self.suffix_from(boundary);

        let deleted_lines = edit.end.line.saturating_sub(edit.start.line);
        let mut new_after = blank_vec_lines(after.len().saturating_sub(deleted_lines));

        for marker in after.iter().flat_map(LineBucket::iter).cloned() {
            let marker = shift_marker_delete(marker, edit);
            insert_marker_into_vec_lines_offset(&mut new_after, marker, boundary);
        }

        self.replace_suffix(boundary, new_after);
        self.rebuild_index();
    }

    /// Clears markers on the specified half-open line range.
    pub fn clear_line_range(&mut self, start_line: usize, end_line: usize) {
        if start_line >= end_line {
            return;
        }

        let num_lines = self.line_count;
        if start_line >= num_lines {
            return;
        }
        let end_line = end_line.min(num_lines);

        for line in start_line..end_line {
            let ids: Vec<_> = self
                .bucket(line)
                .into_iter()
                .flat_map(LineBucket::iter)
                .map(|marker| marker.id)
                .collect();
            if let Some(bucket) = self.bucket_mut(line) {
                bucket.markers = Arc::from(Vec::<Marker<T>>::new());
            }
            for id in ids {
                self.clear_index(id);
            }
        }
    }

    /// Retains markers in the specified half-open line range matching `keep`.
    pub fn retain_in_line_range(
        &mut self,
        start_line: usize,
        end_line: usize,
        keep: impl Fn(&Marker<T>) -> bool,
    ) {
        let num_lines = self.line_count;
        if start_line >= num_lines || start_line >= end_line {
            return;
        }
        let end_line = end_line.min(num_lines);

        for line in start_line..end_line {
            let Some(bucket) = self.bucket(line) else {
                continue;
            };
            let removed: Vec<_> = bucket
                .iter()
                .filter(|marker| !keep(marker))
                .map(|marker| marker.id)
                .collect();
            if removed.is_empty() {
                continue;
            }
            if let Some(bucket) = self.bucket_mut(line) {
                let retained: Vec<_> = bucket
                    .iter()
                    .filter(|marker| keep(marker))
                    .cloned()
                    .collect();
                bucket.markers = Arc::from(retained.into_boxed_slice());
            }
            for id in removed {
                self.clear_index(id);
            }
        }
    }

    /// Retains markers anchored in the cursor range matching `keep`.
    pub fn retain_in_cursor_range(
        &mut self,
        start: Cursor,
        end: Cursor,
        keep: impl Fn(&Marker<T>) -> bool,
    ) {
        if start >= end {
            return;
        }
        let end_line = end.line.min(self.line_count.saturating_sub(1));
        if start.line > end_line {
            return;
        }
        for line in start.line..=end_line {
            let Some(bucket) = self.bucket(line) else {
                continue;
            };
            let removed: Vec<_> = bucket
                .iter()
                .filter(|marker| !keep(marker))
                .map(|marker| marker.id)
                .collect();
            if removed.is_empty() {
                continue;
            }
            if let Some(bucket) = self.bucket_mut(line) {
                let retained: Vec<_> = bucket
                    .iter()
                    .filter(|marker| keep(marker))
                    .cloned()
                    .collect();
                bucket.markers = Arc::from(retained.into_boxed_slice());
            }
            for id in removed {
                self.clear_index(id);
            }
        }
    }

    /// Deletes complete lines and removes markers anchored to the deleted range.
    pub fn delete_lines(&mut self, start_line: usize, count: usize) {
        let total_lines = self.line_count;
        if total_lines == 0 || start_line >= total_lines || count == 0 {
            return;
        }

        let actual_count = (total_lines - start_line).min(count);
        let deleted_end = start_line + actual_count;

        if start_line == 0 && deleted_end >= total_lines {
            self.chunks = chunk_vec_lines(1);
            self.id_line_chunks.clear();
            self.namespace_locations_by_id.clear();
            self.namespaces.clear();
            self.line_count = 1;
            return;
        }

        let after = self.suffix_from(start_line);
        let mut new_after = blank_vec_lines(after.len().saturating_sub(actual_count));

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
                insert_marker_into_vec_lines_offset(&mut new_after, marker, start_line);
            }
        }

        self.replace_suffix(start_line, new_after);
        self.rebuild_index();
        self.prune_namespace_index();
    }

    fn next_marker_id(&mut self) -> MarkerId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn insert_marker(&mut self, marker: Marker<T>) {
        let line_idx = marker.anchor().line;
        self.ensure_line(line_idx);
        if let Some(bucket) = self.bucket_mut(line_idx) {
            bucket.insert_sorted(marker);
        }
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
        self.clear_line_index(id);
        self.clear_namespace_index(id);
    }

    fn clear_line_index(&mut self, id: MarkerId) {
        let idx = id as usize;
        let chunk_idx = idx / ID_CHUNK_SIZE;
        let slot_idx = idx % ID_CHUNK_SIZE;
        if let Some(chunk) = self.id_line_chunks.get_mut(chunk_idx) {
            Arc::make_mut(chunk)[slot_idx] = None;
        }
    }

    fn set_namespace_index(&mut self, id: MarkerId, namespace: &str) {
        let namespace_idx = self
            .namespaces
            .iter()
            .position(|index| index.name == namespace)
            .unwrap_or_else(|| {
                self.namespaces
                    .push(Arc::new(NamespaceIndex::new(namespace)));
                self.namespaces.len() - 1
            });
        let slot = Arc::make_mut(&mut self.namespaces[namespace_idx]).insert(id);
        let idx = id as usize;
        let chunk_idx = idx / ID_CHUNK_SIZE;
        let slot_idx = idx % ID_CHUNK_SIZE;
        while self.namespace_locations_by_id.len() <= chunk_idx {
            self.namespace_locations_by_id
                .push(Arc::new(vec![None; ID_CHUNK_SIZE]));
        }
        Arc::make_mut(&mut self.namespace_locations_by_id[chunk_idx])[slot_idx] =
            Some(NamespaceLocation {
                namespace_index: namespace_idx,
                slot_index: slot,
            });
    }

    fn clear_namespace_index(&mut self, id: MarkerId) {
        let idx = id as usize;
        let chunk_idx = idx / ID_CHUNK_SIZE;
        let slot_idx = idx % ID_CHUNK_SIZE;
        let Some(chunk) = self.namespace_locations_by_id.get_mut(chunk_idx) else {
            return;
        };
        let Some(location) = Arc::make_mut(chunk)[slot_idx].take() else {
            return;
        };
        if let Some(index) = self.namespaces.get_mut(location.namespace_index) {
            Arc::make_mut(index).remove(location.slot_index);
        }
    }

    fn namespace_for_id(&self, id: MarkerId) -> Option<&str> {
        let idx = id as usize;
        let location = self
            .namespace_locations_by_id
            .get(idx / ID_CHUNK_SIZE)?
            .get(idx % ID_CHUNK_SIZE)
            .copied()
            .flatten()?;
        Some(self.namespaces.get(location.namespace_index)?.name.as_str())
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

    fn prune_namespace_index(&mut self) {
        let live_ids: BTreeSet<_> = self.iter().map(|marker| marker.id).collect();
        let stale_ids: Vec<_> = self
            .namespaces
            .iter()
            .flat_map(|index| index.ids())
            .filter(|id| !live_ids.contains(id))
            .collect();
        for id in stale_ids {
            self.clear_namespace_index(id);
        }
    }

    fn bucket(&self, line: usize) -> Option<&LineBucket<T>> {
        let chunk_idx = line / CHUNK_SIZE;
        let line_idx = line % CHUNK_SIZE;
        self.chunks.get(chunk_idx)?.get(line_idx)
    }

    fn bucket_mut(&mut self, line: usize) -> Option<&mut LineBucket<T>> {
        let chunk_idx = line / CHUNK_SIZE;
        let line_idx = line % CHUNK_SIZE;
        Arc::make_mut(self.chunks.get_mut(chunk_idx)?).get_mut(line_idx)
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

    fn suffix_from(&self, start_line: usize) -> Vec<LineBucket<T>> {
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

    fn replace_suffix(&mut self, start_line: usize, suffix: Vec<LineBucket<T>>) {
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

    /// Inserts plugin-owned ghost text anchored to a point.
    pub fn insert_namespaced_ghost_text(
        &mut self,
        namespace: &str,
        pos: Cursor,
        gravity: Gravity,
        label: impl Into<SmolStr>,
        style: Option<StyleOverlay>,
    ) -> MarkerId {
        let mut payload = MarkerPayload::new(label);
        payload.style = style;
        let id = self
            .markers
            .insert_point_in_namespace(pos, gravity, payload, Some(namespace));
        self.bump_visual_generation();
        self.update_markers();
        id
    }

    /// Returns plugin-owned ghost text by id.
    pub fn namespaced_ghost_text(
        &self,
        namespace: &str,
        id: MarkerId,
    ) -> Option<&Marker<MarkerPayload>> {
        self.markers
            .marker_is_in_namespace(id, namespace)
            .then(|| self.ghost_text(id))
            .flatten()
    }

    /// Returns plugin-owned ghost text in line and position order.
    pub fn namespaced_ghost_texts(&self, namespace: &str) -> Vec<Marker<MarkerPayload>> {
        let mut markers: Vec<_> = self
            .markers
            .marker_ids_in_namespace(namespace)
            .into_iter()
            .filter_map(|id| self.ghost_text(id).cloned())
            .collect();
        markers.sort_by_key(Marker::anchor);
        markers
    }

    /// Updates plugin-owned ghost text while preserving its id.
    pub fn update_namespaced_ghost_text(
        &mut self,
        namespace: &str,
        id: MarkerId,
        pos: Cursor,
        gravity: Gravity,
        label: impl Into<SmolStr>,
        style: Option<StyleOverlay>,
    ) -> bool {
        if self.namespaced_ghost_text(namespace, id).is_none() {
            return false;
        }
        let mut payload = MarkerPayload::new(label);
        payload.style = style;
        if !self.markers.update_point(id, pos, gravity, payload) {
            return false;
        }
        self.bump_visual_generation();
        self.update_markers();
        true
    }

    /// Removes plugin-owned ghost text by id.
    pub fn remove_namespaced_ghost_text(
        &mut self,
        namespace: &str,
        id: MarkerId,
    ) -> Option<Marker<MarkerPayload>> {
        self.namespaced_ghost_text(namespace, id)?;
        self.remove_marker(id)
    }

    /// Clears and returns the number of plugin-owned ghost-text markers.
    pub fn clear_namespaced_ghost_texts(&mut self, namespace: &str) -> usize {
        let ids = self.markers.marker_ids_in_namespace(namespace);
        let mut removed = 0;
        for id in ids {
            if self
                .markers
                .get(id)
                .is_some_and(|marker| marker.payload.kind.is_none())
                && self.markers.remove(id).is_some()
            {
                removed += 1;
            }
        }
        if removed > 0 {
            self.bump_visual_generation();
            self.update_markers();
        }
        removed
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
        self.markers.clear_to_line_count(self.lines.line_count());
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
        self.markers.retain_in_cursor_range(start, end, |marker| {
            marker.payload.kind.is_none() || marker.anchor() < start || marker.anchor() >= end
        });
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
        self.retain_markers_in_line_range(0, self.lines.line_count(), keep);
    }

    fn retain_markers_in_line_range(
        &mut self,
        start_line: usize,
        end_line: usize,
        keep: impl Fn(&MarkerPayload) -> bool,
    ) {
        self.markers
            .retain_in_line_range(start_line, end_line, |marker| keep(&marker.payload));
        self.bump_visual_generation();
        self.update_markers();
    }
}

fn blank_vec_lines<T: Clone>(line_count: usize) -> Vec<LineBucket<T>> {
    (0..line_count).map(|_| LineBucket::new()).collect()
}

fn chunk_vec_lines<T: Clone>(line_count: usize) -> Vec<Arc<Vec<LineBucket<T>>>> {
    chunk_buckets(blank_vec_lines(line_count))
}

fn chunk_buckets<T: Clone>(mut lines: Vec<LineBucket<T>>) -> Vec<Arc<Vec<LineBucket<T>>>> {
    if lines.is_empty() {
        lines.push(LineBucket::new());
    }
    let mut chunks = Vec::with_capacity(lines.len().div_ceil(CHUNK_SIZE));
    for chunk in lines.chunks(CHUNK_SIZE) {
        chunks.push(Arc::new(chunk.to_vec()));
    }
    chunks
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

fn insert_marker_into_vec_lines_offset<T: Clone>(
    lines: &mut [LineBucket<T>],
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
