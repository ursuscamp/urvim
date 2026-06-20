//! Session-local jumplist state.
//!
//! This module provides the shared jumplist history used by the active tab
//! group and the windows it manages.

use crate::buffer::{BufferId, Cursor};

const MAX_JUMPLIST_ENTRIES: usize = 100;
const JUMPLIST_DISTANCE_THRESHOLD: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
struct JumpEntry {
    buffer_id: BufferId,
    cursor: Cursor,
}

/// Session-local navigation history for jump playback.
#[derive(Debug, Clone)]
pub struct JumpList {
    entries: Vec<JumpEntry>,
    current: Option<usize>,
}

impl JumpList {
    /// Creates an empty jumplist.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            current: None,
        }
    }

    fn current_entry(&self) -> Option<&JumpEntry> {
        self.current.and_then(|index| self.entries.get(index))
    }

    /// Records a cursor position in the jumplist.
    pub fn record_cursor(&mut self, buffer_id: BufferId, cursor: Cursor) {
        let entry = JumpEntry { buffer_id, cursor };

        match self.current {
            None => {
                self.entries.push(entry);
                self.current = Some(0);
                self.enforce_limit();
            }
            Some(current_index) => {
                let branch = self.current_entry().is_none_or(|current| {
                    current.buffer_id != buffer_id
                        || Self::cursor_distance(current.cursor, cursor)
                            > JUMPLIST_DISTANCE_THRESHOLD
                });

                if branch {
                    self.entries.truncate(current_index + 1);
                    self.remove_duplicates_except(&entry, current_index);
                    self.entries.push(entry);
                    self.current = Some(self.entries.len().saturating_sub(1));
                    self.enforce_limit();
                } else {
                    if let Some(current) = self.entries.get_mut(current_index) {
                        *current = entry.clone();
                    }
                    self.remove_duplicates_except(&entry, current_index);
                }
            }
        }
    }

    /// Returns the previous jumplist entry without advancing the current index.
    pub fn peek_back(&self) -> Option<(BufferId, Cursor)> {
        let current_index = self.current?;
        if current_index == 0 {
            return None;
        }

        let next_index = current_index - 1;
        self.entries
            .get(next_index)
            .map(|entry| (entry.buffer_id, entry.cursor))
    }

    /// Returns the next jumplist entry without advancing the current index.
    pub fn peek_forward(&self) -> Option<(BufferId, Cursor)> {
        let current_index = self.current?;
        let next_index = current_index + 1;
        if next_index >= self.entries.len() {
            return None;
        }

        self.entries
            .get(next_index)
            .map(|entry| (entry.buffer_id, entry.cursor))
    }

    /// Moves backward in the jumplist and returns the target entry.
    pub fn jump_back(&mut self) -> Option<(BufferId, Cursor)> {
        let current_index = self.current?;
        if current_index == 0 {
            return None;
        }

        let next_index = current_index - 1;
        self.current = Some(next_index);
        self.entries
            .get(next_index)
            .map(|entry| (entry.buffer_id, entry.cursor))
    }

    /// Moves forward in the jumplist and returns the target entry.
    pub fn jump_forward(&mut self) -> Option<(BufferId, Cursor)> {
        let current_index = self.current?;
        let next_index = current_index + 1;
        if next_index >= self.entries.len() {
            return None;
        }

        self.current = Some(next_index);
        self.entries
            .get(next_index)
            .map(|entry| (entry.buffer_id, entry.cursor))
    }

    /// Updates the current jumplist entry after restoring a cursor.
    pub fn sync_current_cursor(&mut self, cursor: Cursor) {
        let Some(current_index) = self.current else {
            return;
        };

        let Some(current_entry) = self.entries.get_mut(current_index) else {
            return;
        };

        let entry = JumpEntry {
            buffer_id: current_entry.buffer_id,
            cursor,
        };
        *current_entry = entry.clone();
        self.remove_duplicates_except(&entry, current_index);
    }

    fn remove_duplicates_except(&mut self, entry: &JumpEntry, mut keep_index: usize) {
        let mut index = 0;
        while index < self.entries.len() {
            if index == keep_index {
                index += 1;
                continue;
            }

            if self.entries[index] == *entry {
                self.entries.remove(index);
                if index < keep_index {
                    keep_index = keep_index.saturating_sub(1);
                }
                continue;
            }

            index += 1;
        }

        self.current = Some(keep_index);
    }

    fn enforce_limit(&mut self) {
        while self.entries.len() > MAX_JUMPLIST_ENTRIES {
            self.entries.remove(0);
            if let Some(current_index) = self.current {
                self.current = Some(current_index.saturating_sub(1));
            }
        }

        if self.entries.is_empty() {
            self.current = None;
        }
    }

    fn cursor_distance(a: Cursor, b: Cursor) -> usize {
        a.line.abs_diff(b.line) + a.col.abs_diff(b.col)
    }
}

impl Default for JumpList {
    fn default() -> Self {
        Self::new()
    }
}
