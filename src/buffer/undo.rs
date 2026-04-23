use super::*;

impl UndoState {
    pub(super) fn new(lines: Vector<Arc<str>>, cursor: Cursor, buffer_cache: BufferCache) -> Self {
        Self {
            history: Vector::unit(Snapshot {
                lines,
                cursor,
                buffer_cache,
            }),
            position: 0,
        }
    }

    fn push_snapshot(
        &mut self,
        lines: Vector<Arc<str>>,
        cursor: Cursor,
        buffer_cache: BufferCache,
    ) {
        if let Some(active) = self.history.get(self.position)
            && active.lines == lines
        {
            if let Some(active_snapshot) = self.history.get_mut(self.position) {
                *active_snapshot = Snapshot {
                    lines,
                    cursor,
                    buffer_cache,
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

    fn undo(&mut self) -> Option<(Vector<Arc<str>>, BufferCache, Cursor)> {
        if self.position == 0 {
            return None;
        }

        self.position -= 1;
        let snapshot = self.history.get(self.position)?;
        Some((
            snapshot.lines.clone(),
            snapshot.buffer_cache.clone(),
            snapshot.cursor,
        ))
    }

    fn redo(&mut self) -> Option<(Vector<Arc<str>>, BufferCache, Cursor)> {
        if self.position >= self.history.len() - 1 {
            return None;
        }

        self.position += 1;
        let snapshot = self.history.get(self.position)?;
        Some((
            snapshot.lines.clone(),
            snapshot.buffer_cache.clone(),
            snapshot.cursor,
        ))
    }

    fn can_undo(&self) -> bool {
        self.position > 0
    }

    fn can_redo(&self) -> bool {
        self.position < self.history.len() - 1
    }

    fn current_snapshot_matches(&self, lines: &Vector<Arc<str>>) -> bool {
        self.history
            .get(self.position)
            .is_some_and(|active| active.lines == *lines)
    }
}

impl Buffer {
    /// Records the current text and syntax state as an undo snapshot.
    pub fn push_snapshot(&mut self, cursor: Cursor) {
        self.undo_state
            .push_snapshot(self.lines.clone(), cursor, self.buffer_cache.clone());
    }

    /// Updates the cursor stored in the active undo snapshot.
    pub fn update_cursor(&mut self, cursor: Cursor) {
        self.undo_state.update_cursor(cursor);
    }

    pub fn undo(&mut self) -> Option<Cursor> {
        match self.undo_state.undo() {
            Some((lines, buffer_cache, cursor)) => {
                self.lines = lines;
                self.buffer_cache = buffer_cache;
                self.syntax_generation = self.syntax_generation.wrapping_add(1);
                self.syntax_background_generation = None;
                Some(cursor)
            }
            None => None,
        }
    }

    pub fn redo(&mut self) -> Option<Cursor> {
        match self.undo_state.redo() {
            Some((lines, buffer_cache, cursor)) => {
                self.lines = lines;
                self.buffer_cache = buffer_cache;
                self.syntax_generation = self.syntax_generation.wrapping_add(1);
                self.syntax_background_generation = None;
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
