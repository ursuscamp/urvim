use super::Action;
use std::collections::BTreeMap;
use std::fmt;

/// A mapping from key sequences to actions.
pub trait Keymap {
    /// Returns the action for an exact key sequence, if present.
    fn get_action(&self, keys: &[String]) -> Option<Action>;
    /// Returns `true` when the key sequence is a prefix of at least one binding.
    fn is_prefix(&self, keys: &[String]) -> bool;
    /// Returns `true` when the key sequence has one or more child bindings.
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
    /// Creates a new empty trie keymap.
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    /// Inserts a single-key binding.
    pub fn insert(&mut self, key: String, action: Action) {
        self.insert_str(&key, action);
    }

    /// Inserts a binding from a canonical key string.
    ///
    /// The string uses the same canonical notation produced by
    /// `Key::canonical_string()`.
    pub fn insert_str(&mut self, keys: &str, action: Action) {
        let parsed = validate_key_string(keys).expect("invalid canonical key string");
        self.insert_sequence(parsed, action);
    }

    /// Inserts a multi-key binding from an already parsed sequence.
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

    /// Returns the action bound to an exact key sequence.
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

    /// Returns `true` if the provided key sequence is a valid prefix in the trie.
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
    use crate::editor::ActionKind;

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
            Some(Action::new(ActionKind::MoveUp))
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
