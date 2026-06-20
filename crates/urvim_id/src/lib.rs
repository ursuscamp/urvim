//! Shared identity types used across editor crates.
//!
//! This crate exists to break dependency cycles: both `urvim_core` and
//! `urvim_lsp` need `BufferId` without depending on each other.

/// Stable numeric identifier for an editor buffer.
///
/// Every buffer in the global buffer pool receives a unique `BufferId` at
/// allocation time. Once assigned, the id is stable for the lifetime of the
/// buffer and is used throughout the editor and LSP subsystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BufferId(usize);

impl BufferId {
    /// Creates a new buffer identifier from a raw numeric value.
    pub fn new(value: usize) -> Self {
        Self(value)
    }

    /// Returns the underlying numeric identifier.
    pub fn get(self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn new_and_get_round_trip() {
        let id = BufferId::new(42);
        assert_eq!(id.get(), 42);
    }

    #[test]
    fn equality_and_ordering() {
        let a = BufferId::new(1);
        let b = BufferId::new(1);
        let c = BufferId::new(2);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a < c);
    }

    #[test]
    fn hash_map_key() {
        let mut map = HashMap::new();
        map.insert(BufferId::new(10), "ten");
        map.insert(BufferId::new(20), "twenty");
        assert_eq!(map.get(&BufferId::new(10)), Some(&"ten"));
    }

    #[test]
    fn hash_set_deduplicates() {
        let mut set = HashSet::new();
        set.insert(BufferId::new(10));
        set.insert(BufferId::new(10));
        set.insert(BufferId::new(20));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn copy_behavior() {
        let a = BufferId::new(7);
        let b = a;
        assert_eq!(a, b);
    }
}
