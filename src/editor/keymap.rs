use crate::ui::Intent;
use std::collections::BTreeMap;
use std::fmt;

/// A mapping from key sequences to intents.
pub trait Keymap {
    /// Returns the intent for an exact key sequence, if present.
    fn get_action(&self, keys: &[String]) -> Option<Intent>;
    /// Returns `true` when the key sequence is a prefix of at least one binding.
    fn is_prefix(&self, keys: &[String]) -> bool;
    /// Returns `true` when the key sequence has one or more child bindings.
    fn has_children(&self, keys: &[String]) -> bool;
}

pub(super) const MAX_COUNT: usize = 9999;

struct TrieNode {
    children: BTreeMap<String, TrieNode>,
    intent: Option<Intent>,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: BTreeMap::new(),
            intent: None,
        }
    }
}

/// Trie-based keymap for efficient key sequence matching.
pub struct TrieKeymap {
    root: TrieNode,
}

impl TrieKeymap {
    /// Creates a new empty trie keymap.
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    /// Inserts a single-key binding.
    pub fn insert<T: Into<Intent>>(&mut self, key: String, intent: T) {
        self.insert_str(&key, intent);
    }

    /// Inserts a binding from a canonical key string.
    ///
    /// The string uses the same canonical notation produced by
    /// `Key::canonical_string()`.
    pub fn insert_str<T: Into<Intent>>(&mut self, keys: &str, intent: T) {
        let parsed = validate_key_string(keys).expect("invalid canonical key string");
        self.insert_sequence(parsed, intent);
    }

    /// Inserts a multi-key binding from an already parsed sequence.
    pub fn insert_sequence<T: Into<Intent>>(&mut self, keys: Vec<String>, intent: T) {
        let mut current = &mut self.root;
        for key in &keys {
            current = current
                .children
                .entry(key.clone())
                .or_insert_with(TrieNode::new);
        }
        current.intent = Some(intent.into());
    }

    /// Returns the intent bound to an exact key sequence.
    pub fn get_action(&self, keys: &[String]) -> Option<Intent> {
        let mut current = &self.root;
        for key in keys {
            match current.children.get(key) {
                Some(node) => current = node,
                None => return None,
            }
        }
        current.intent.clone()
    }

    /// Returns `true` if the provided key sequence is a valid prefix in the trie.
    pub fn is_prefix(&self, keys: &[String]) -> bool {
        let mut current = &self.root;
        for key in keys {
            match current.children.get(key) {
                Some(node) => current = node,
                None => return false,
            }
        }
        !current.children.is_empty() || current.intent.is_some()
    }

    /// Returns `true` if the provided key sequence has at least one child binding.
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
    fn get_action(&self, keys: &[String]) -> Option<Intent> {
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

/// Errors that can occur while parsing a canonical key string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStringParseError {
    /// The input string was empty or only whitespace.
    Empty,
    /// A `<...>` token started but did not terminate with `>`.
    UnterminatedSpecialToken,
    /// An empty special token `<>` was provided.
    EmptySpecialToken,
}

impl fmt::Display for KeyStringParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "key string must not be empty or whitespace"),
            Self::UnterminatedSpecialToken => {
                write!(f, "key string contains an unterminated special token")
            }
            Self::EmptySpecialToken => write!(f, "key string contains an empty special token"),
        }
    }
}

impl std::error::Error for KeyStringParseError {}

/// Validates a canonical key string and returns its parsed token sequence.
pub fn validate_key_string(keys: &str) -> Result<Vec<String>, KeyStringParseError> {
    if keys.trim().is_empty() {
        return Err(KeyStringParseError::Empty);
    }

    let mut tokens = Vec::new();
    let mut chars = keys.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut token = String::from("<");
            let mut found_closing = false;

            for next in chars.by_ref() {
                token.push(next);
                if next == '>' {
                    found_closing = true;
                    break;
                }
            }

            if !found_closing {
                return Err(KeyStringParseError::UnterminatedSpecialToken);
            }

            if token == "<>" {
                return Err(KeyStringParseError::EmptySpecialToken);
            }

            tokens.push(token);
            continue;
        }

        tokens.push(ch.to_string());
    }

    if tokens.is_empty() {
        return Err(KeyStringParseError::Empty);
    }

    Ok(tokens)
}

/// Parser that extracts action keys and multiplicative count from key sequences.
pub struct CountParser;

impl CountParser {
    /// Returns `true` when the string is a single count digit (`1` through `9`).
    pub fn is_count_digit(s: &str) -> bool {
        s.len() == 1
            && s.chars()
                .next()
                .map(|c| ('1'..='9').contains(&c))
                .unwrap_or(false)
    }

    /// Returns `true` when the string is a valid numeric count.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{Action, ActionKind};

    #[test]
    fn test_parse_key_string_single_key() {
        assert_eq!(
            validate_key_string("g").expect("should parse"),
            vec!["g".to_string()]
        );
    }

    #[test]
    fn test_parse_key_string_multi_key() {
        assert_eq!(
            validate_key_string("gg").expect("should parse"),
            vec!["g".to_string(), "g".to_string()]
        );
    }

    #[test]
    fn test_parse_key_string_special_token() {
        assert_eq!(
            validate_key_string("<C-s>").expect("should parse"),
            vec!["<C-s>".to_string()]
        );
    }

    #[test]
    fn test_parse_key_string_mixed_sequence() {
        assert_eq!(
            validate_key_string("d<LessThan>").expect("should parse"),
            vec!["d".to_string(), "<LessThan>".to_string()]
        );
    }

    #[test]
    fn test_insert_str_matches_sequence_lookup() {
        let mut keymap = TrieKeymap::new();
        keymap.insert_str("gg", Action::new(ActionKind::MoveUp));

        assert_eq!(
            keymap.get_action(&["g".to_string(), "g".to_string()]),
            Some(Action::new(ActionKind::MoveUp).into())
        );
    }

    #[test]
    fn test_validate_key_string_rejects_unterminated_special_token() {
        assert!(matches!(
            validate_key_string("<Esc"),
            Err(KeyStringParseError::UnterminatedSpecialToken)
        ));
    }

    #[test]
    fn test_validate_key_string_rejects_empty_input() {
        assert!(matches!(
            validate_key_string("   "),
            Err(KeyStringParseError::Empty)
        ));
    }
}
