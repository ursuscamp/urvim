use super::{Buffer, Cursor, MarkersStore};
use smol_str::SmolStr;
use std::collections::BTreeSet;
use std::sync::Arc;
use urvim_terminal::Style;
use urvim_theme::StyleOverlay;

const CHUNK_SIZE: usize = 128;
const ID_CHUNK_SIZE: usize = 512;
const NAMESPACE_CHUNK_SIZE: usize = 256;

/// Determines the theme style used by virtual text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualTextKind {
    /// Generic virtual text created by the editor or a plugin.
    Generic,
    /// An LSP inlay hint.
    InlayHint,
}

/// Text displayed at a buffer position without changing the buffer contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualText {
    /// Text displayed by the marker.
    pub text: SmolStr,
    /// Semantic virtual-text kind.
    pub kind: VirtualTextKind,
    /// Optional style layered over the kind's theme style.
    pub style: Option<StyleOverlay>,
}

impl VirtualText {
    /// Creates generic virtual text.
    pub fn new(text: impl Into<SmolStr>) -> Self {
        Self {
            text: text.into(),
            kind: VirtualTextKind::Generic,
            style: None,
        }
    }

    /// Creates virtual text for an LSP inlay hint.
    pub fn inlay_hint(text: impl Into<SmolStr>) -> Self {
        Self {
            text: text.into(),
            kind: VirtualTextKind::InlayHint,
            style: None,
        }
    }

    /// Resolves this virtual text's display style.
    pub fn resolved_style(&self, virtual_text_style: Style, inlay_hint_style: Style) -> Style {
        let base_style = match self.kind {
            VirtualTextKind::Generic => virtual_text_style,
            VirtualTextKind::InlayHint => inlay_hint_style,
        };
        self.style
            .map_or(base_style, |style| style.apply_to(base_style))
    }
}

/// A style applied to a half-open range of real buffer text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Highlight {
    /// Partial style composed with the text's existing style.
    pub style: StyleOverlay,
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
pub struct PointAnchor {
    /// Marker position.
    pub pos: Cursor,
    /// Exact-boundary insertion behavior.
    pub gravity: Gravity,
}

/// A marker anchored to a half-open range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeAnchor {
    /// Range start, inclusive.
    pub start: Cursor,
    /// Range end, exclusive.
    pub end: Cursor,
    /// Exact-boundary behavior at the start.
    pub start_gravity: Gravity,
    /// Exact-boundary behavior at the end.
    pub end_gravity: Gravity,
}

/// An edit-tracking marker whose geometry determines its payload type.
#[derive(Debug, Clone)]
pub enum Marker<P, R> {
    /// A point marker.
    Point {
        /// Stable marker identifier.
        id: MarkerId,
        /// Point geometry.
        anchor: PointAnchor,
        /// Point-specific payload.
        payload: P,
    },
    /// A half-open range marker.
    Range {
        /// Stable marker identifier.
        id: MarkerId,
        /// Range geometry.
        anchor: RangeAnchor,
        /// Range-specific payload.
        payload: R,
    },
}

impl<P, R> Marker<P, R> {
    /// Returns the stable marker identifier.
    pub fn id(&self) -> MarkerId {
        match self {
            Self::Point { id, .. } | Self::Range { id, .. } => *id,
        }
    }

    /// Returns point marker components.
    pub fn as_point(&self) -> Option<(PointAnchor, &P)> {
        match self {
            Self::Point {
                anchor, payload, ..
            } => Some((*anchor, payload)),
            Self::Range { .. } => None,
        }
    }

    /// Returns range marker components.
    pub fn as_range(&self) -> Option<(RangeAnchor, &R)> {
        match self {
            Self::Range {
                anchor, payload, ..
            } => Some((*anchor, payload)),
            Self::Point { .. } => None,
        }
    }

    fn anchor(&self) -> Cursor {
        match self {
            Self::Point { anchor, .. } => anchor.pos,
            Self::Range { anchor, .. } => anchor.start,
        }
    }

    fn is_empty_range(&self) -> bool {
        matches!(self, Self::Range { anchor, .. } if anchor.start >= anchor.end)
    }
}

/// A line-local bucket of markers.
#[derive(Debug, Clone)]
pub struct LineBucket<P, R> {
    markers: Arc<[Marker<P, R>]>,
}

impl<P, R> LineBucket<P, R> {
    fn new() -> Self {
        Self {
            markers: Arc::from(Vec::<Marker<P, R>>::new()),
        }
    }

    fn is_empty(&self) -> bool {
        self.markers.is_empty()
    }

    fn get(&self, id: MarkerId) -> Option<&Marker<P, R>> {
        self.markers.iter().find(|marker| marker.id() == id)
    }

    fn get_mut(&mut self, id: MarkerId) -> Option<&mut Marker<P, R>>
    where
        P: Clone,
        R: Clone,
    {
        let markers = Arc::make_mut(&mut self.markers);
        markers.iter_mut().find(|marker| marker.id() == id)
    }

    fn remove(&mut self, id: MarkerId) -> Option<Marker<P, R>>
    where
        P: Clone,
        R: Clone,
    {
        let mut markers = self.markers.as_ref().to_vec();
        let index = markers.iter().position(|marker| marker.id() == id)?;
        let removed = markers.remove(index);
        self.markers = Arc::from(markers.into_boxed_slice());
        Some(removed)
    }

    fn iter(&self) -> impl Iterator<Item = &Marker<P, R>> {
        self.markers.iter()
    }

    fn insert_sorted(&mut self, marker: Marker<P, R>)
    where
        P: Clone,
        R: Clone,
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
pub struct MarkerStore<P, R> {
    chunks: Vec<Arc<Vec<LineBucket<P, R>>>>,
    id_line_chunks: Vec<Arc<Vec<Option<usize>>>>,
    namespace_locations_by_id: Vec<Arc<Vec<Option<NamespaceLocation>>>>,
    namespaces: Vec<Arc<NamespaceIndex>>,
    line_count: usize,
    next_id: MarkerId,
}

impl<P: Clone, R: Clone> Default for MarkerStore<P, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: Clone, R: Clone> MarkerStore<P, R> {
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
    pub fn get(&self, id: MarkerId) -> Option<&Marker<P, R>> {
        let line = self.index_line(id)?;
        self.bucket(line)?.get(id)
    }

    /// Returns a mutable marker by id.
    pub fn get_mut(&mut self, id: MarkerId) -> Option<&mut Marker<P, R>> {
        let line = self.index_line(id)?;
        self.bucket_mut(line)?.get_mut(id)
    }

    /// Removes a marker by id.
    pub fn remove(&mut self, id: MarkerId) -> Option<Marker<P, R>> {
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
    pub fn iter(&self) -> impl Iterator<Item = &Marker<P, R>> {
        (0..self.line_count)
            .flat_map(|line| self.bucket(line).into_iter().flat_map(LineBucket::iter))
    }

    /// Returns the markers stored on a specific line.
    pub fn markers_for_line(&self, line: usize) -> Option<&[Marker<P, R>]> {
        if line >= self.line_count {
            return None;
        }
        self.bucket(line).map(|bucket| bucket.markers.as_ref())
    }

    /// Inserts a point marker.
    pub fn insert_point(&mut self, pos: Cursor, gravity: Gravity, payload: P) -> MarkerId {
        self.insert_point_in_namespace(pos, gravity, payload, None)
    }

    /// Inserts a point marker belonging to an optional namespace.
    pub fn insert_point_in_namespace(
        &mut self,
        pos: Cursor,
        gravity: Gravity,
        payload: P,
        namespace: Option<&str>,
    ) -> MarkerId {
        let id = self.next_marker_id();
        let marker = Marker::Point {
            id,
            anchor: PointAnchor { pos, gravity },
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
        payload: P,
    ) -> bool {
        if self
            .get(id)
            .is_none_or(|marker| marker.as_point().is_none())
        {
            return false;
        }
        let Some(line) = self.index_line(id) else {
            return false;
        };
        if self
            .bucket_mut(line)
            .and_then(|bucket| bucket.remove(id))
            .is_none()
        {
            return false;
        }
        self.clear_line_index(id);
        let marker = Marker::Point {
            id,
            anchor: PointAnchor { pos, gravity },
            payload,
        };
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
        payload: R,
    ) -> MarkerId {
        self.insert_range_in_namespace(start, end, start_gravity, end_gravity, payload, None)
    }

    /// Inserts a range marker belonging to an optional namespace.
    pub fn insert_range_in_namespace(
        &mut self,
        start: Cursor,
        end: Cursor,
        start_gravity: Gravity,
        end_gravity: Gravity,
        payload: R,
        namespace: Option<&str>,
    ) -> MarkerId {
        let (start, end, start_gravity, end_gravity) =
            normalize_range(start, end, start_gravity, end_gravity);
        let id = self.next_marker_id();
        let marker = Marker::Range {
            id,
            anchor: RangeAnchor {
                start,
                end,
                start_gravity,
                end_gravity,
            },
            payload,
        };
        self.insert_marker(marker);
        self.set_index(id, start.line);
        if let Some(namespace) = namespace {
            self.set_namespace_index(id, namespace);
        }
        id
    }

    /// Replaces a range marker while preserving its id and namespace.
    pub fn update_range(
        &mut self,
        id: MarkerId,
        start: Cursor,
        end: Cursor,
        start_gravity: Gravity,
        end_gravity: Gravity,
        payload: R,
    ) -> bool {
        if self
            .get(id)
            .is_none_or(|marker| marker.as_range().is_none())
        {
            return false;
        }
        let Some(line) = self.index_line(id) else {
            return false;
        };
        if self
            .bucket_mut(line)
            .and_then(|bucket| bucket.remove(id))
            .is_none()
        {
            return false;
        }
        self.clear_line_index(id);
        let marker = Marker::Range {
            id,
            anchor: RangeAnchor {
                start,
                end,
                start_gravity,
                end_gravity,
            },
            payload,
        };
        self.insert_marker(marker);
        self.set_index(id, start.line);
        true
    }

    /// Shifts markers for an insertion.
    pub fn shift_insert(&mut self, edit: InsertShape) {
        let line_count = self.line_count.saturating_add(edit.line_delta);
        self.rebuild_markers(line_count, |marker| Some(shift_marker_insert(marker, edit)));
    }

    /// Inserts whole lines at a given line index.
    pub fn insert_lines(&mut self, start_line: usize, count: usize) {
        if count == 0 {
            return;
        }
        let line_count = self.line_count.saturating_add(count);
        self.rebuild_markers(line_count, |marker| {
            Some(insert_marker_shift_lines(marker, start_line, count))
        });
    }

    /// Shifts markers for a deletion.
    pub fn shift_delete(&mut self, edit: DeleteShape) {
        if edit.start >= edit.end {
            return;
        }
        let deleted_lines = edit.end.line.saturating_sub(edit.start.line);
        let line_count = self.line_count.saturating_sub(deleted_lines).max(1);
        self.rebuild_markers(line_count, |marker| {
            let marker = shift_marker_delete(marker, edit);
            (!marker.is_empty_range()).then_some(marker)
        });
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
                .map(Marker::id)
                .collect();
            if let Some(bucket) = self.bucket_mut(line) {
                bucket.markers = Arc::from(Vec::<Marker<P, R>>::new());
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
        keep: impl Fn(&Marker<P, R>) -> bool,
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
                .map(Marker::id)
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
        keep: impl Fn(&Marker<P, R>) -> bool,
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
                .map(Marker::id)
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

    /// Deletes complete lines, contracting ranges around the removed text.
    pub fn delete_lines(&mut self, start_line: usize, count: usize) {
        let total_lines = self.line_count;
        if start_line >= total_lines || count == 0 {
            return;
        }
        let actual_count = (total_lines - start_line).min(count);
        let deleted_end = start_line + actual_count;
        let line_count = total_lines.saturating_sub(actual_count).max(1);
        self.rebuild_markers(line_count, |marker| {
            delete_marker_lines(marker, start_line, deleted_end, actual_count)
        });
    }

    fn next_marker_id(&mut self) -> MarkerId {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn insert_marker(&mut self, marker: Marker<P, R>) {
        let line_idx = marker.anchor().line;
        self.ensure_line(line_idx);
        if let Some(bucket) = self.bucket_mut(line_idx) {
            bucket.insert_sorted(marker);
        }
    }

    /// Rebuilds line buckets after an edit. Ranges may cross the edited line even
    /// when their start anchor lives in an earlier bucket, so edit transforms
    /// must inspect every marker rather than only the edited line's bucket.
    fn rebuild_markers(
        &mut self,
        line_count: usize,
        transform: impl Fn(Marker<P, R>) -> Option<Marker<P, R>>,
    ) {
        let markers: Vec<_> = self.iter().cloned().collect();
        self.line_count = line_count.max(1);
        self.chunks = chunk_vec_lines(self.line_count);
        self.id_line_chunks.clear();
        for marker in markers.into_iter().filter_map(transform) {
            let id = marker.id();
            let line = marker.anchor().line;
            self.insert_marker(marker);
            self.set_index(id, line);
        }
        self.prune_namespace_index();
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

    fn prune_namespace_index(&mut self) {
        let live_ids: BTreeSet<_> = self.iter().map(Marker::id).collect();
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

    fn bucket(&self, line: usize) -> Option<&LineBucket<P, R>> {
        let chunk_idx = line / CHUNK_SIZE;
        let line_idx = line % CHUNK_SIZE;
        self.chunks.get(chunk_idx)?.get(line_idx)
    }

    fn bucket_mut(&mut self, line: usize) -> Option<&mut LineBucket<P, R>> {
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
}

impl Buffer {
    /// Returns the current decoration store.
    pub fn markers(&self) -> &MarkersStore {
        &self.markers
    }

    /// Returns a decoration marker by id.
    pub fn marker(&self, id: MarkerId) -> Option<&Marker<VirtualText, Highlight>> {
        self.markers.get(id)
    }

    /// Returns markers anchored on a line.
    pub fn markers_for_line(&self, line: usize) -> Option<&[Marker<VirtualText, Highlight>]> {
        self.markers.markers_for_line(line)
    }

    /// Returns generic virtual text anchored on a line.
    pub fn virtual_texts_for_line(
        &self,
        line: usize,
    ) -> Option<Vec<Marker<VirtualText, Highlight>>> {
        self.markers_for_line(line).map(|markers| {
            markers
                .iter()
                .filter(|marker| {
                    marker
                        .as_point()
                        .is_some_and(|(_, text)| text.kind == VirtualTextKind::Generic)
                })
                .cloned()
                .collect()
        })
    }

    /// Returns inlay hints anchored on a line.
    pub fn inlay_hints_for_line(&self, line: usize) -> Option<Vec<Marker<VirtualText, Highlight>>> {
        self.markers_for_line(line).map(|markers| {
            markers
                .iter()
                .filter(|marker| {
                    marker
                        .as_point()
                        .is_some_and(|(_, text)| text.kind == VirtualTextKind::InlayHint)
                })
                .cloned()
                .collect()
        })
    }

    /// Returns generic virtual text by id.
    pub fn virtual_text(&self, id: MarkerId) -> Option<&Marker<VirtualText, Highlight>> {
        self.marker(id).filter(|marker| {
            marker
                .as_point()
                .is_some_and(|(_, text)| text.kind == VirtualTextKind::Generic)
        })
    }

    /// Returns an inlay hint by id.
    pub fn inlay_hint(&self, id: MarkerId) -> Option<&Marker<VirtualText, Highlight>> {
        self.marker(id).filter(|marker| {
            marker
                .as_point()
                .is_some_and(|(_, text)| text.kind == VirtualTextKind::InlayHint)
        })
    }

    /// Returns all range highlights in creation order.
    pub fn highlights(&self) -> Vec<Marker<VirtualText, Highlight>> {
        let mut highlights: Vec<_> = self
            .markers
            .iter()
            .filter(|marker| marker.as_range().is_some())
            .cloned()
            .collect();
        highlights.sort_by_key(Marker::id);
        highlights
    }

    /// Inserts generic virtual text anchored to a point.
    pub fn insert_virtual_text(
        &mut self,
        pos: Cursor,
        gravity: Gravity,
        text: impl Into<SmolStr>,
    ) -> MarkerId {
        self.insert_virtual_text_payload(pos, gravity, VirtualText::new(text))
    }

    /// Inserts plugin-owned virtual text anchored to a point.
    pub fn insert_namespaced_virtual_text(
        &mut self,
        namespace: &str,
        pos: Cursor,
        gravity: Gravity,
        text: impl Into<SmolStr>,
        style: Option<StyleOverlay>,
    ) -> MarkerId {
        let mut payload = VirtualText::new(text);
        payload.style = style;
        let id = self
            .markers
            .insert_point_in_namespace(pos, gravity, payload, Some(namespace));
        self.decorations_changed();
        id
    }

    /// Returns plugin-owned virtual text by id.
    pub fn namespaced_virtual_text(
        &self,
        namespace: &str,
        id: MarkerId,
    ) -> Option<&Marker<VirtualText, Highlight>> {
        self.markers
            .marker_is_in_namespace(id, namespace)
            .then(|| self.virtual_text(id))
            .flatten()
    }

    /// Returns plugin-owned virtual text in buffer order.
    pub fn namespaced_virtual_texts(&self, namespace: &str) -> Vec<Marker<VirtualText, Highlight>> {
        let mut markers: Vec<_> = self
            .markers
            .marker_ids_in_namespace(namespace)
            .into_iter()
            .filter_map(|id| self.virtual_text(id).cloned())
            .collect();
        markers.sort_by_key(Marker::anchor);
        markers
    }

    /// Updates plugin-owned virtual text while preserving its id.
    pub fn update_namespaced_virtual_text(
        &mut self,
        namespace: &str,
        id: MarkerId,
        pos: Cursor,
        gravity: Gravity,
        text: impl Into<SmolStr>,
        style: Option<StyleOverlay>,
    ) -> bool {
        if self.namespaced_virtual_text(namespace, id).is_none() {
            return false;
        }
        let mut payload = VirtualText::new(text);
        payload.style = style;
        if !self.markers.update_point(id, pos, gravity, payload) {
            return false;
        }
        self.decorations_changed();
        true
    }

    /// Removes plugin-owned virtual text by id.
    pub fn remove_namespaced_virtual_text(
        &mut self,
        namespace: &str,
        id: MarkerId,
    ) -> Option<Marker<VirtualText, Highlight>> {
        self.namespaced_virtual_text(namespace, id)?;
        self.remove_marker(id)
    }

    /// Clears and returns the number of plugin-owned virtual-text markers.
    pub fn clear_namespaced_virtual_texts(&mut self, namespace: &str) -> usize {
        let ids = self.markers.marker_ids_in_namespace(namespace);
        let removed = self.remove_matching_ids(ids, |marker| {
            marker
                .as_point()
                .is_some_and(|(_, text)| text.kind == VirtualTextKind::Generic)
        });
        if removed > 0 {
            self.decorations_changed();
        }
        removed
    }

    /// Inserts an LSP inlay hint anchored to a point.
    pub fn insert_inlay_hint(
        &mut self,
        pos: Cursor,
        gravity: Gravity,
        text: impl Into<SmolStr>,
    ) -> MarkerId {
        self.insert_virtual_text_payload(pos, gravity, VirtualText::inlay_hint(text))
    }

    /// Inserts a plugin-owned range highlight.
    pub fn insert_namespaced_highlight(
        &mut self,
        namespace: &str,
        range: RangeAnchor,
        style: StyleOverlay,
    ) -> Option<MarkerId> {
        if range.start >= range.end {
            return None;
        }
        let id = self.markers.insert_range_in_namespace(
            range.start,
            range.end,
            range.start_gravity,
            range.end_gravity,
            Highlight { style },
            Some(namespace),
        );
        self.decorations_changed();
        Some(id)
    }

    /// Returns a plugin-owned range highlight by id.
    pub fn namespaced_highlight(
        &self,
        namespace: &str,
        id: MarkerId,
    ) -> Option<&Marker<VirtualText, Highlight>> {
        self.markers
            .marker_is_in_namespace(id, namespace)
            .then(|| self.marker(id).filter(|marker| marker.as_range().is_some()))
            .flatten()
    }

    /// Returns plugin-owned range highlights in buffer order.
    pub fn namespaced_highlights(&self, namespace: &str) -> Vec<Marker<VirtualText, Highlight>> {
        let mut markers: Vec<_> = self
            .markers
            .marker_ids_in_namespace(namespace)
            .into_iter()
            .filter_map(|id| self.namespaced_highlight(namespace, id).cloned())
            .collect();
        markers.sort_by_key(Marker::anchor);
        markers
    }

    /// Updates a plugin-owned range highlight while preserving its id.
    pub fn update_namespaced_highlight(
        &mut self,
        namespace: &str,
        id: MarkerId,
        range: RangeAnchor,
        style: StyleOverlay,
    ) -> bool {
        if range.start >= range.end || self.namespaced_highlight(namespace, id).is_none() {
            return false;
        }
        if !self.markers.update_range(
            id,
            range.start,
            range.end,
            range.start_gravity,
            range.end_gravity,
            Highlight { style },
        ) {
            return false;
        }
        self.decorations_changed();
        true
    }

    /// Removes a plugin-owned range highlight by id.
    pub fn remove_namespaced_highlight(
        &mut self,
        namespace: &str,
        id: MarkerId,
    ) -> Option<Marker<VirtualText, Highlight>> {
        self.namespaced_highlight(namespace, id)?;
        self.remove_marker(id)
    }

    /// Clears and returns the number of plugin-owned range highlights.
    pub fn clear_namespaced_highlights(&mut self, namespace: &str) -> usize {
        let ids = self.markers.marker_ids_in_namespace(namespace);
        let removed = self.remove_matching_ids(ids, |marker| marker.as_range().is_some());
        if removed > 0 {
            self.decorations_changed();
        }
        removed
    }

    /// Removes a decoration marker by id.
    pub fn remove_marker(&mut self, id: MarkerId) -> Option<Marker<VirtualText, Highlight>> {
        let removed = self.markers.remove(id);
        if removed.is_some() {
            self.decorations_changed();
        }
        removed
    }

    /// Removes generic virtual text by id.
    pub fn remove_virtual_text(&mut self, id: MarkerId) -> Option<Marker<VirtualText, Highlight>> {
        self.virtual_text(id)?;
        self.remove_marker(id)
    }

    /// Clears all decoration markers.
    pub fn clear_markers(&mut self) {
        self.markers.clear_to_line_count(self.lines.line_count());
        self.decorations_changed();
    }

    /// Clears markers anchored on a line range.
    pub fn clear_markers_for_lines(&mut self, start_line: usize, end_line: usize) {
        self.markers.clear_line_range(start_line, end_line);
        self.decorations_changed();
    }

    /// Clears all generic virtual text.
    pub fn clear_virtual_texts(&mut self) {
        self.retain_point_markers_in_line_range(0, self.lines.line_count(), |text| {
            text.kind != VirtualTextKind::Generic
        });
    }

    /// Clears all inlay hints.
    pub fn clear_inlay_hints(&mut self) {
        self.retain_point_markers_in_line_range(0, self.lines.line_count(), |text| {
            text.kind != VirtualTextKind::InlayHint
        });
    }

    /// Clears inlay hints anchored on a line range.
    pub fn clear_inlay_hints_for_lines(&mut self, start_line: usize, end_line: usize) {
        self.retain_point_markers_in_line_range(start_line, end_line, |text| {
            text.kind != VirtualTextKind::InlayHint
        });
    }

    /// Clears inlay hints whose anchors are inside a half-open cursor range.
    pub fn clear_inlay_hints_in_range(&mut self, start: Cursor, end: Cursor) {
        if start >= end {
            return;
        }
        self.markers.retain_in_cursor_range(start, end, |marker| {
            !matches!(
                marker,
                Marker::Point { anchor, payload, .. }
                    if payload.kind == VirtualTextKind::InlayHint
                        && anchor.pos >= start
                        && anchor.pos < end
            )
        });
        self.decorations_changed();
    }

    fn insert_virtual_text_payload(
        &mut self,
        pos: Cursor,
        gravity: Gravity,
        payload: VirtualText,
    ) -> MarkerId {
        let id = self.markers.insert_point(pos, gravity, payload);
        self.decorations_changed();
        id
    }

    fn retain_point_markers_in_line_range(
        &mut self,
        start_line: usize,
        end_line: usize,
        keep: impl Fn(&VirtualText) -> bool,
    ) {
        self.markers
            .retain_in_line_range(start_line, end_line, |marker| {
                marker.as_point().is_none_or(|(_, payload)| keep(payload))
            });
        self.decorations_changed();
    }

    fn remove_matching_ids(
        &mut self,
        ids: Vec<MarkerId>,
        matches: impl Fn(&Marker<VirtualText, Highlight>) -> bool,
    ) -> usize {
        let mut removed = 0;
        for id in ids {
            if self.markers.get(id).is_some_and(&matches) && self.markers.remove(id).is_some() {
                removed += 1;
            }
        }
        removed
    }

    fn decorations_changed(&mut self) {
        self.bump_visual_generation();
        self.update_markers();
    }
}

fn blank_vec_lines<P: Clone, R: Clone>(line_count: usize) -> Vec<LineBucket<P, R>> {
    (0..line_count).map(|_| LineBucket::new()).collect()
}

fn chunk_vec_lines<P: Clone, R: Clone>(line_count: usize) -> Vec<Arc<Vec<LineBucket<P, R>>>> {
    chunk_buckets(blank_vec_lines(line_count))
}

fn chunk_buckets<P: Clone, R: Clone>(
    mut lines: Vec<LineBucket<P, R>>,
) -> Vec<Arc<Vec<LineBucket<P, R>>>> {
    if lines.is_empty() {
        lines.push(LineBucket::new());
    }
    let mut chunks = Vec::with_capacity(lines.len().div_ceil(CHUNK_SIZE));
    for chunk in lines.chunks(CHUNK_SIZE) {
        chunks.push(Arc::new(chunk.to_vec()));
    }
    chunks
}

fn insertion_index<P, R>(markers: &[Marker<P, R>], anchor: Cursor) -> usize {
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

fn shift_marker_insert<P, R>(marker: Marker<P, R>, edit: InsertShape) -> Marker<P, R> {
    match marker {
        Marker::Point {
            id,
            anchor,
            payload,
        } => Marker::Point {
            id,
            anchor: PointAnchor {
                pos: shift_cursor_insert(anchor.pos, edit, anchor.gravity),
                gravity: anchor.gravity,
            },
            payload,
        },
        Marker::Range {
            id,
            anchor,
            payload,
        } => Marker::Range {
            id,
            anchor: RangeAnchor {
                start: shift_cursor_insert(anchor.start, edit, anchor.start_gravity),
                end: shift_cursor_insert(anchor.end, edit, anchor.end_gravity),
                start_gravity: anchor.start_gravity,
                end_gravity: anchor.end_gravity,
            },
            payload,
        },
    }
}

fn shift_marker_delete<P, R>(marker: Marker<P, R>, edit: DeleteShape) -> Marker<P, R> {
    match marker {
        Marker::Point {
            id,
            anchor,
            payload,
        } => Marker::Point {
            id,
            anchor: PointAnchor {
                pos: shift_cursor_delete(anchor.pos, edit),
                gravity: anchor.gravity,
            },
            payload,
        },
        Marker::Range {
            id,
            anchor,
            payload,
        } => Marker::Range {
            id,
            anchor: RangeAnchor {
                start: shift_cursor_delete(anchor.start, edit),
                end: shift_cursor_delete(anchor.end, edit),
                start_gravity: anchor.start_gravity,
                end_gravity: anchor.end_gravity,
            },
            payload,
        },
    }
}

fn insert_marker_shift_lines<P, R>(
    mut marker: Marker<P, R>,
    start_line: usize,
    count: usize,
) -> Marker<P, R> {
    match &mut marker {
        Marker::Point { anchor, .. } => {
            if anchor.pos.line >= start_line {
                anchor.pos.line += count;
            }
        }
        Marker::Range { anchor, .. } => {
            if anchor.start.line >= start_line {
                anchor.start.line += count;
            }
            if anchor.end.line >= start_line {
                anchor.end.line += count;
            }
        }
    }
    marker
}

fn delete_marker_lines<P, R>(
    marker: Marker<P, R>,
    start_line: usize,
    deleted_end: usize,
    count: usize,
) -> Option<Marker<P, R>> {
    let shift = |cursor: Cursor| {
        if cursor.line < start_line {
            cursor
        } else if cursor.line >= deleted_end {
            Cursor::new(cursor.line - count, cursor.col)
        } else {
            Cursor::new(start_line, 0)
        }
    };

    match marker {
        Marker::Point {
            id,
            anchor,
            payload,
        } => {
            if anchor.pos.line >= start_line && anchor.pos.line < deleted_end {
                None
            } else {
                Some(Marker::Point {
                    id,
                    anchor: PointAnchor {
                        pos: shift(anchor.pos),
                        gravity: anchor.gravity,
                    },
                    payload,
                })
            }
        }
        Marker::Range {
            id,
            anchor,
            payload,
        } => {
            let anchor = RangeAnchor {
                start: shift(anchor.start),
                end: shift(anchor.end),
                start_gravity: anchor.start_gravity,
                end_gravity: anchor.end_gravity,
            };
            (anchor.start < anchor.end).then_some(Marker::Range {
                id,
                anchor,
                payload,
            })
        }
    }
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
