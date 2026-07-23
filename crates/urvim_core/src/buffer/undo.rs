use super::*;

impl UndoState {
    pub(super) fn new(
        lines: PieceTable,
        cursor: Cursor,
        buffer_cache: BufferCache,
        markers: MarkersStore,
    ) -> Self {
        Self {
            history: vec![Snapshot {
                lines,
                cursor,
                buffer_cache,
                markers,
            }],
            position: 0,
            revision: 0,
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
            self.history.pop();
        }

        self.history.push(Snapshot {
            lines,
            cursor,
            buffer_cache,
            markers,
        });
        self.position = self.history.len() - 1;
        self.revision = self.revision.wrapping_add(1);
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
        self.revision = self.revision.wrapping_add(1);
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
        self.revision = self.revision.wrapping_add(1);
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

    fn checkpoint(&self) -> UndoCheckpoint {
        let active = &self.history[self.position];
        UndoCheckpoint {
            position: self.position,
            revision: self.revision,
            lines: active.lines.clone(),
        }
    }

    fn squash_since(&mut self, checkpoint: UndoCheckpoint) -> Option<PieceTable> {
        let Some(base) = self.history.get(checkpoint.position) else {
            return None;
        };
        if base.lines != checkpoint.lines || self.position < checkpoint.position {
            return None;
        }
        if self.revision == checkpoint.revision {
            return Some(base.lines.clone());
        }

        let base_lines = base.lines.clone();
        let mut final_snapshot = self.history[self.position].clone();
        let net_no_op = piece_tables_have_equal_text(&base_lines, &final_snapshot.lines);
        self.history.truncate(checkpoint.position + 1);
        self.position = checkpoint.position;
        if net_no_op {
            final_snapshot.lines = base_lines;
            self.history[self.position] = final_snapshot;
        } else {
            self.push_snapshot(
                final_snapshot.lines,
                final_snapshot.cursor,
                final_snapshot.buffer_cache,
                final_snapshot.markers,
            );
        }
        self.revision = self.revision.wrapping_add(1);
        self.history
            .get(self.position)
            .map(|snapshot| snapshot.lines.clone())
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

    /// Captures the current undo head for a future grouped edit.
    pub fn undo_checkpoint(&self) -> UndoCheckpoint {
        self.undo_state.checkpoint()
    }

    /// Combines snapshots created after `checkpoint` into one undo entry.
    ///
    /// Returns `false` when the checkpoint no longer belongs to the active
    /// history branch.
    pub fn squash_undo_history(&mut self, checkpoint: UndoCheckpoint) -> bool {
        let Some(lines) = self.undo_state.squash_since(checkpoint) else {
            return false;
        };
        self.lines = lines;
        true
    }

    /// Restores the previous undo snapshot and returns its stored cursor.
    pub fn undo(&mut self) -> Option<Cursor> {
        match self.undo_state.undo() {
            Some((lines, buffer_cache, markers, cursor)) => {
                self.lines = lines;
                self.buffer_cache = buffer_cache;
                self.markers = markers;
                self.generations.syntax = self.generations.syntax.wrapping_add(1);
                self.generations.syntax_background = None;
                self.generations.indent_background = None;
                self.generations.diff = self.generations.diff.wrapping_add(1);
                self.generations.diff_background = None;
                Some(cursor)
            }
            None => None,
        }
    }

    /// Restores the next redo snapshot and returns its stored cursor.
    pub fn redo(&mut self) -> Option<Cursor> {
        match self.undo_state.redo() {
            Some((lines, buffer_cache, markers, cursor)) => {
                self.lines = lines;
                self.buffer_cache = buffer_cache;
                self.markers = markers;
                self.generations.syntax = self.generations.syntax.wrapping_add(1);
                self.generations.syntax_background = None;
                self.generations.indent_background = None;
                self.generations.diff = self.generations.diff.wrapping_add(1);
                self.generations.diff_background = None;
                Some(cursor)
            }
            None => None,
        }
    }

    /// Returns whether an older undo snapshot is available.
    pub fn can_undo(&self) -> bool {
        self.undo_state.can_undo()
    }

    /// Returns whether a newer redo snapshot is available.
    pub fn can_redo(&self) -> bool {
        self.undo_state.can_redo()
    }

    /// Returns true when the current buffer text matches the active undo snapshot.
    pub fn current_text_matches_undo_head(&self) -> bool {
        self.undo_state.current_snapshot_matches(&self.lines)
    }
}

fn piece_tables_have_equal_text(left: &PieceTable, right: &PieceTable) -> bool {
    if left == right {
        return true;
    }
    if left.len() != right.len() {
        return false;
    }
    left.text()
        .chunks()
        .flat_map(str::bytes)
        .eq(right.text().chunks().flat_map(str::bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squash_undo_history_groups_multiple_snapshots() {
        let mut buffer = Buffer::from_str("one");
        let checkpoint = buffer.undo_checkpoint();

        buffer.insert_text(Cursor::new(0, 3), " two");
        buffer.push_snapshot(Cursor::new(0, 7));
        buffer.insert_text(Cursor::new(0, 7), " three");
        buffer.push_snapshot(Cursor::new(0, 13));

        assert!(buffer.squash_undo_history(checkpoint));
        assert_eq!(buffer.undo(), Some(Cursor::new(0, 0)));
        assert_eq!(buffer.as_str(), "one");
        assert!(!buffer.can_undo());
        assert_eq!(buffer.redo(), Some(Cursor::new(0, 13)));
        assert_eq!(buffer.as_str(), "one two three");
    }

    #[test]
    fn squash_undo_history_omits_net_no_op() {
        let mut buffer = Buffer::from_str("one");
        let checkpoint = buffer.undo_checkpoint();

        buffer.insert_text(Cursor::new(0, 3), " two");
        buffer.push_snapshot(Cursor::new(0, 7));
        buffer.remove(Cursor::new(0, 3), Cursor::new(0, 7));
        buffer.push_snapshot(Cursor::new(0, 3));

        assert!(buffer.squash_undo_history(checkpoint));
        assert_eq!(buffer.as_str(), "one");
        assert!(!buffer.can_undo());
    }

    #[test]
    fn squash_undo_history_without_edits_preserves_redo() {
        let mut buffer = Buffer::from_str("one");
        buffer.insert_text(Cursor::new(0, 3), " two");
        buffer.push_snapshot(Cursor::new(0, 7));
        buffer.undo().expect("first edit should be undoable");
        let checkpoint = buffer.undo_checkpoint();

        assert!(buffer.squash_undo_history(checkpoint));
        assert!(buffer.can_redo());
        buffer.redo().expect("redo branch should remain available");
        assert_eq!(buffer.as_str(), "one two");
    }

    #[test]
    fn squash_undo_history_replaces_the_redo_branch() {
        let mut buffer = Buffer::from_str("one");
        buffer.insert_text(Cursor::new(0, 3), " two");
        buffer.push_snapshot(Cursor::new(0, 7));
        buffer.undo().expect("first edit should be undoable");
        let checkpoint = buffer.undo_checkpoint();

        buffer.insert_text(Cursor::new(0, 3), " changed");
        buffer.push_snapshot(Cursor::new(0, 11));

        assert!(buffer.squash_undo_history(checkpoint));
        assert!(!buffer.can_redo());
        buffer.undo().expect("grouped edit should be undoable");
        assert_eq!(buffer.as_str(), "one");
        buffer.redo().expect("grouped edit should be redoable");
        assert_eq!(buffer.as_str(), "one changed");
    }
}
