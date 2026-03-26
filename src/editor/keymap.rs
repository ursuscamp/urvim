use super::Action;
use std::collections::BTreeMap;

pub trait Keymap {
    fn get_action(&self, keys: &[String]) -> Option<Action>;
    fn is_prefix(&self, keys: &[String]) -> bool;
    fn has_children(&self, keys: &[String]) -> bool;
}

pub(super) const MAX_COUNT: usize = 9999;

pub(super) fn extract_leading_count(keys: &[String]) -> (usize, Vec<String>) {
    let mut count_str = String::new();
    let mut remaining = Vec::new();
    let mut found_non_digit = false;

    for key in keys {
        let is_digit = key.len() == 1
            && key
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false);

        if !found_non_digit && is_digit {
            count_str.push_str(key);
        } else {
            found_non_digit = true;
            remaining.push(key.clone());
        }
    }

    if count_str.is_empty() || !CountParser::is_valid_count(&count_str) {
        return (0, keys.to_vec());
    }

    let count: usize = count_str.parse().unwrap_or(0);
    (count, remaining)
}

struct TrieNode {
    children: BTreeMap<String, TrieNode>,
    action: Option<Action>,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: BTreeMap::new(),
            action: None,
        }
    }
}

/// Trie-based keymap for efficient key sequence matching.
pub struct TrieKeymap {
    root: TrieNode,
}

impl TrieKeymap {
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    pub fn insert(&mut self, key: String, action: Action) {
        self.insert_sequence(vec![key], action);
    }

    pub fn insert_sequence(&mut self, keys: Vec<String>, action: Action) {
        let mut current = &mut self.root;
        for key in &keys {
            current = current
                .children
                .entry(key.clone())
                .or_insert_with(TrieNode::new);
        }
        current.action = Some(action);
    }

    pub fn get_action(&self, keys: &[String]) -> Option<Action> {
        let mut current = &self.root;
        for key in keys {
            match current.children.get(key) {
                Some(node) => current = node,
                None => return None,
            }
        }
        current.action.clone()
    }

    pub fn is_prefix(&self, keys: &[String]) -> bool {
        let mut current = &self.root;
        for key in keys {
            match current.children.get(key) {
                Some(node) => current = node,
                None => return false,
            }
        }
        !current.children.is_empty() || current.action.is_some()
    }

    pub fn has_children(&self, keys: &[String]) -> bool {
        let mut current = &self.root;
        for key in keys {
            match current.children.get(key) {
                Some(node) => current = node,
                None => return false,
            }
        }
        !current.children.is_empty()
    }
}

impl Keymap for TrieKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action> {
        TrieKeymap::get_action(self, keys)
    }

    fn is_prefix(&self, keys: &[String]) -> bool {
        TrieKeymap::is_prefix(self, keys)
    }

    fn has_children(&self, keys: &[String]) -> bool {
        TrieKeymap::has_children(self, keys)
    }
}

impl Default for TrieKeymap {
    fn default() -> Self {
        Self::new()
    }
}

/// Parser that extracts action keys and multiplicative count from key sequences.
pub struct CountParser;

impl CountParser {
    pub fn is_count_digit(s: &str) -> bool {
        s.len() == 1
            && s.chars()
                .next()
                .map(|c| ('1'..='9').contains(&c))
                .unwrap_or(false)
    }

    pub fn is_valid_count(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let first_char = s.chars().next().unwrap();
        if !('1'..='9').contains(&first_char) {
            return false;
        }
        s.chars().all(|c| c.is_ascii_digit())
    }

    pub fn parse(keys: &[String]) -> (Vec<String>, usize) {
        let mut action_keys = Vec::new();
        let mut total_count: usize = 1;
        let mut current_count: usize = 0;

        for key in keys {
            if Self::is_count_digit(key) {
                let digit: usize = key.parse().unwrap_or(0);
                current_count = current_count * 10 + digit;
            } else {
                if current_count > 0 {
                    total_count = total_count.saturating_mul(current_count);
                    if total_count > MAX_COUNT {
                        total_count = MAX_COUNT;
                    }
                    current_count = 0;
                }
                action_keys.push(key.clone());
            }
        }

        if current_count > 0 {
            total_count = total_count.saturating_mul(current_count);
            if total_count > MAX_COUNT {
                total_count = MAX_COUNT;
            }
        }

        (action_keys, total_count)
    }
}
