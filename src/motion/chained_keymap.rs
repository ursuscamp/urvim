//! Chained keymap that delegates to multiple keymaps in sequence.
//!
//! This module provides a keymap wrapper that tries multiple sub-keymaps
//! in order until one returns a non-None result.

use crate::editor::{Action, Keymap};

/// A keymap wrapper that chains multiple keymaps together.
///
/// Tries each sub-keymap in sequence until one returns a non-None result.
/// Order matters: first keymap in the chain has priority.
pub struct ChainedKeymap {
    keymaps: Vec<Box<dyn Keymap>>,
}

impl Default for ChainedKeymap {
    fn default() -> Self {
        Self::new()
    }
}

impl ChainedKeymap {
    /// Creates a new empty ChainedKeymap.
    pub fn new() -> Self {
        Self {
            keymaps: Vec::new(),
        }
    }

    /// Creates a new ChainedKeymap with the given keymaps.
    pub fn with_keymaps(keymaps: Vec<Box<dyn Keymap>>) -> Self {
        Self { keymaps }
    }

    /// Adds a keymap to the chain.
    pub fn add(&mut self, keymap: Box<dyn Keymap>) {
        self.keymaps.push(keymap);
    }
}

impl Keymap for ChainedKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action> {
        for keymap in &self.keymaps {
            if let Some(action) = keymap.get_action(keys) {
                return Some(action);
            }
        }
        None
    }

    fn is_prefix(&self, keys: &[String]) -> bool {
        for keymap in &self.keymaps {
            if keymap.is_prefix(keys) {
                return true;
            }
        }
        false
    }

    fn has_children(&self, keys: &[String]) -> bool {
        for keymap in &self.keymaps {
            if keymap.has_children(keys) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::TrieKeymap;
    use crate::motion::char_scan_keymap::CharScanKeymap;

    #[test]
    fn test_chained_get_action_trie_first() {
        let mut trie = TrieKeymap::new();
        trie.insert_str("gg", Action::MoveUp);

        let mut chained = ChainedKeymap::new();
        chained.add(Box::new(trie));
        chained.add(Box::new(CharScanKeymap::new()));

        // gg should match trie first
        let action = chained.get_action(&["g".to_string(), "g".to_string()]);
        assert_eq!(action, Some(Action::MoveUp));
    }

    #[test]
    fn test_chained_get_action_falls_back_to_char_scan() {
        let trie = TrieKeymap::new();

        let mut chained = ChainedKeymap::new();
        chained.add(Box::new(trie));
        chained.add(Box::new(CharScanKeymap::new()));

        // fx should fall back to char scan
        let action = chained.get_action(&["f".to_string(), "x".to_string()]);
        assert_eq!(action, Some(Action::FindForward('x')));
    }

    #[test]
    fn test_chained_is_prefix_trie() {
        let mut trie = TrieKeymap::new();
        trie.insert_str("gg", Action::MoveUp);

        let mut chained = ChainedKeymap::new();
        chained.add(Box::new(trie));
        chained.add(Box::new(CharScanKeymap::new()));

        // g is a prefix in trie
        assert!(chained.is_prefix(&["g".to_string()]));
    }

    #[test]
    fn test_chained_is_prefix_char_scan() {
        let trie = TrieKeymap::new();

        let mut chained = ChainedKeymap::new();
        chained.add(Box::new(trie));
        chained.add(Box::new(CharScanKeymap::new()));

        // f is a prefix in char scan
        assert!(chained.is_prefix(&["f".to_string()]));
    }

    #[test]
    fn test_chained_is_prefix_returns_true_if_any() {
        let mut trie = TrieKeymap::new();
        trie.insert_str("gg", Action::MoveUp);

        let mut chained = ChainedKeymap::new();
        chained.add(Box::new(trie));
        chained.add(Box::new(CharScanKeymap::new()));

        // g is a prefix (from trie)
        assert!(chained.is_prefix(&["g".to_string()]));
    }

    #[test]
    fn test_chained_no_match_returns_none() {
        let trie = TrieKeymap::new();
        let char_scan = CharScanKeymap::new();

        let mut chained = ChainedKeymap::new();
        chained.add(Box::new(trie));
        chained.add(Box::new(char_scan));

        // zx doesn't match anything
        let action = chained.get_action(&["z".to_string(), "x".to_string()]);
        assert_eq!(action, None);
    }
}
