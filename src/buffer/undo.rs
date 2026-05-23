use super::*;

impl UndoState {
    pub(super) fn new(
        lines: PieceTable,
        cursor: Cursor,
        buffer_cache: BufferCache,
        markers: MarkersStore,
    ) -> Self {
        Self {
            history: Vector::unit(Snapshot {
                lines,
                cursor,
                buffer_cache,
                markers,
            }),
            position: 0,
        }
    }

    fn push_snapshot(
        &mut self,
        lines: PieceTable,
        cursor: Cursor,
        buffer_cache: BufferCache,
        markers: MarkersStore,
    ) {
        if let Some(active) = self.history.get(self.position)
            && active.lines == lines
        {
            if let Some(active_snapshot) = self.history.get_mut(self.position) {
                *active_snapshot = Snapshot {
                    lines,
                    cursor,
                    buffer_cache,
                    markers,
                };
            }
            return;
        }

        while self.history.len() > self.position + 1 {
            self.history.pop_back();
        }

        self.history.push_back(Snapshot {
            lines,
            cursor,
            buffer_cache,
            markers,
        });
        self.position = self.history.len() - 1;
    }

    fn update_cursor(&mut self, cursor: Cursor) {
        if let Some(active) = self.history.get_mut(self.position) {
            active.cursor = cursor;
        }
    }

    pub(super) fn update_buffer_cache(&mut self, buffer_cache: BufferCache) {
        if let Some(active) = self.history.get_mut(self.position) {
            active.buffer_cache = buffer_cache;
        }
    }

    pub(super) fn update_markers(&mut self, markers: MarkersStore) {
        if let Some(active) = self.history.get_mut(self.position) {
            active.markers = markers;
        }
    }

    fn undo(&mut self) -> Option<(PieceTable, BufferCache, MarkersStore, Cursor)> {
        if self.position == 0 {
            return None;
        }

        self.position -= 1;
        let snapshot = self.history.get(self.position)?;
        Some((
            snapshot.lines.clone(),
            snapshot.buffer_cache.clone(),
            snapshot.markers.clone(),
            snapshot.cursor,
        ))
    }

    fn redo(&mut self) -> Option<(PieceTable, BufferCache, MarkersStore, Cursor)> {
        if self.position >= self.history.len() - 1 {
            return None;
        }

        self.position += 1;
        let snapshot = self.history.get(self.position)?;
        Some((
            snapshot.lines.clone(),
            snapshot.buffer_cache.clone(),
            snapshot.markers.clone(),
            snapshot.cursor,
        ))
    }

    fn can_undo(&self) -> bool {
        self.position > 0
    }

    fn can_redo(&self) -> bool {
        self.position < self.history.len() - 1
    }

    fn current_snapshot_matches(&self, lines: &PieceTable) -> bool {
        self.history
            .get(self.position)
            .is_some_and(|active| active.lines == *lines)
    }
}

impl Buffer {
    /// Records the current text and syntax state as an undo snapshot.
    pub fn push_snapshot(&mut self, cursor: Cursor) {
        self.undo_state.push_snapshot(
            self.lines.clone(),
            cursor,
            self.buffer_cache.clone(),
            self.markers.clone(),
        );
    }

    /// Updates the cursor stored in the active undo snapshot.
    pub fn update_cursor(&mut self, cursor: Cursor) {
        self.undo_state.update_cursor(cursor);
    }

    /// Updates the marker state stored in the active undo snapshot.
    pub fn update_markers(&mut self) {
        self.undo_state.update_markers(self.markers.clone());
    }

    /// Updates the inlay hint state stored in the active undo snapshot.
    pub fn update_inlay_hints(&mut self) {
        self.update_markers();
    }

    /// Returns the cursor stored in the active undo snapshot.
    pub fn current_cursor(&self) -> Cursor {
        self.undo_state
            .history
            .get(self.undo_state.position)
            .map(|snapshot| snapshot.cursor)
            .unwrap_or_default()
    }

    pub fn undo(&mut self) -> Option<Cursor> {
        match self.undo_state.undo() {
            Some((lines, buffer_cache, markers, cursor)) => {
                self.lines = lines;
                self.buffer_cache = buffer_cache;
                self.markers = markers;
                self.syntax_generation = self.syntax_generation.wrapping_add(1);
                self.syntax_background_generation = None;
                self.indent_background_generation = None;
                Some(cursor)
            }
            None => None,
        }
    }

    pub fn redo(&mut self) -> Option<Cursor> {
        match self.undo_state.redo() {
            Some((lines, buffer_cache, markers, cursor)) => {
                self.lines = lines;
                self.buffer_cache = buffer_cache;
                self.markers = markers;
                self.syntax_generation = self.syntax_generation.wrapping_add(1);
                self.syntax_background_generation = None;
                self.indent_background_generation = None;
                Some(cursor)
            }
            None => None,
        }
    }

    pub fn can_undo(&self) -> bool {
        self.undo_state.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo_state.can_redo()
    }

    /// Returns true when the current buffer text matches the active undo snapshot.
    pub fn current_text_matches_undo_head(&self) -> bool {
        self.undo_state.current_snapshot_matches(&self.lines)
    }
}
