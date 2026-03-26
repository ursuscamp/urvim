//! Global state for the editor.
//!
//! This module stores persistent state that needs to survive across mode switches
//! and future multi-window support.

use crate::buffer::{Buffer, BufferId, BufferPool};
use std::sync::{Mutex, OnceLock, RwLock};

/// Direction of character search
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// Kind of character search motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindKind {
    /// f or F - lands ON the character
    Find,
    /// t or T - lands BEFORE/AFTER the character
    Till,
}

/// State of the last character search motion
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindState {
    pub target_char: char,
    pub kind: FindKind,
    pub direction: Direction,
}

/// Global storage for the last character search state
static LAST_FIND: Mutex<Option<FindState>> = Mutex::new(None);
static BUFFER_POOL: OnceLock<RwLock<BufferPool>> = OnceLock::new();

/// Set the last character search state
pub fn set_last_find(state: FindState) {
    let mut last = LAST_FIND.lock().unwrap();
    *last = Some(state);
}

/// Get the last character search state
pub fn get_last_find() -> Option<FindState> {
    let last = LAST_FIND.lock().unwrap();
    last.clone()
}

/// Returns the global buffer pool read-write lock, initializing it on first use.
pub fn buffer_pool() -> &'static RwLock<BufferPool> {
    BUFFER_POOL.get_or_init(|| RwLock::new(BufferPool::new()))
}

/// Runs a closure with mutable access to the global buffer pool.
pub fn with_buffer_pool<R>(f: impl FnOnce(&mut BufferPool) -> R) -> R {
    let mut pool = buffer_pool().write().unwrap();
    f(&mut pool)
}

/// Runs a closure with shared access to a live buffer entry.
pub fn with_buffer<R>(id: BufferId, f: impl FnOnce(&Buffer) -> R) -> Option<R> {
    let pool = buffer_pool().read().unwrap();
    pool.get(id).map(f)
}

/// Runs a closure with mutable access to a live buffer entry.
pub fn with_buffer_mut<R>(id: BufferId, f: impl FnOnce(&mut Buffer) -> R) -> Option<R> {
    let mut pool = buffer_pool().write().unwrap();
    pool.with_buffer_mut(id, f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_last_find() {
        let state = FindState {
            target_char: 'x',
            kind: FindKind::Find,
            direction: Direction::Forward,
        };
        set_last_find(state.clone());
        assert_eq!(get_last_find(), Some(state));
    }

    #[test]
    fn test_get_last_find_empty() {
        // Ensure we start with None
        let mut last = LAST_FIND.lock().unwrap();
        *last = None;
        drop(last);

        assert_eq!(get_last_find(), None);
    }

    #[test]
    fn test_with_buffer_reads_live_buffer() {
        let id = with_buffer_pool(|pool| {
            let id = pool.create_buffer();
            pool.with_buffer_mut(id, |buffer| {
                buffer.insert_text(crate::buffer::Cursor::new(0, 0), "alpha");
            });
            id
        });

        let text = with_buffer(id, |buffer| buffer.as_str());

        assert_eq!(text.as_deref(), Some("alpha"));
    }

    #[test]
    fn test_with_buffer_missing_id_returns_none() {
        assert!(with_buffer(BufferId::new(usize::MAX), |_| ()).is_none());
    }
}
