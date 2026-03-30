use super::*;

impl UndoState {
    pub(super) fn new(lines: Vector<Arc<str>>, cursor: Cursor) -> Self {
        Self {
            history: Vector::unit(Snapshot { lines, cursor }),
            position: 0,
        }
    }

    fn push_snapshot(&mut self, lines: Vector<Arc<str>>, cursor: Cursor) {
        if let Some(active) = self.history.get(self.position)
            && active.lines == lines
        {
            if let Some(active_snapshot) = self.history.get_mut(self.position) {
                *active_snapshot = Snapshot { lines, cursor };
            }
            return;
        }

        while self.history.len() > self.position + 1 {
            self.history.pop_back();
        }

        self.history.push_back(Snapshot { lines, cursor });
        self.position = self.history.len() - 1;
    }

    fn update_cursor(&mut self, cursor: Cursor) {
        if let Some(active) = self.history.get_mut(self.position) {
            active.cursor = cursor;
        }
    }

    fn undo(&mut self) -> Option<(Vector<Arc<str>>, Cursor)> {
        if self.position == 0 {
            return None;
        }

        self.position -= 1;
        let snapshot = self.history.get(self.position)?;
        Some((snapshot.lines.clone(), snapshot.cursor))
    }

    fn redo(&mut self) -> Option<(Vector<Arc<str>>, Cursor)> {
        if self.position >= self.history.len() - 1 {
            return None;
        }

        self.position += 1;
        let snapshot = self.history.get(self.position)?;
        Some((snapshot.lines.clone(), snapshot.cursor))
    }

    fn can_undo(&self) -> bool {
        self.position > 0
    }

    fn can_redo(&self) -> bool {
        self.position < self.history.len() - 1
    }
}

impl Buffer {
    pub fn push_snapshot(&mut self, cursor: Cursor) {
        self.undo_state.push_snapshot(self.lines.clone(), cursor);
    }

    pub fn update_cursor(&mut self, cursor: Cursor) {
        self.undo_state.update_cursor(cursor);
    }

    pub fn undo(&mut self) -> Option<Cursor> {
        match self.undo_state.undo() {
            Some((lines, cursor)) => {
                self.lines = lines;
                self.invalidate_syntax_from(0);
                Some(cursor)
            }
            None => None,
        }
    }

    pub fn redo(&mut self) -> Option<Cursor> {
        match self.undo_state.redo() {
            Some((lines, cursor)) => {
                self.lines = lines;
                self.invalidate_syntax_from(0);
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
}
