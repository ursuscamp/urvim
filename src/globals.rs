//! Global state for the editor.
//!
//! This module stores persistent state that needs to survive across mode switches
//! and future multi-window support.

use std::sync::Mutex;

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
}
