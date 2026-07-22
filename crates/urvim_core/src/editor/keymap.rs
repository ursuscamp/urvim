use super::HandleKeyResult;
use crate::ui::{Intent, KeymapInheritance};
use std::collections::BTreeMap;
use std::fmt;
use urvim_terminal::Key;

/// A mapping from key sequences to values.
pub trait Keymap {
    /// Returns the intent for an exact key sequence, if present.
    fn get_action(&self, keys: &[String]) -> Option<Intent>;
    /// Returns `true` when the key sequence is a prefix of at least one binding.
    fn is_prefix(&self, keys: &[String]) -> bool;
    /// Returns `true` when the key sequence has one or more child bindings.
    fn has_children(&self, keys: &[String]) -> bool;
}

pub(super) const MAX_COUNT: usize = 9999;

#[derive(Debug)]
struct TrieNode<T> {
    children: BTreeMap<String, TrieNode<T>>,
    value: Option<T>,
}

impl<T> TrieNode<T> {
    fn new() -> Self {
        Self {
            children: BTreeMap::new(),
            value: None,
        }
    }
}

/// Trie-based keymap for efficient key sequence matching.
#[derive(Debug)]
pub struct TrieKeymap<T = Intent> {
    root: TrieNode<T>,
    descriptions: BTreeMap<Vec<String>, String>,
}

impl<T> TrieKeymap<T> {
    /// Creates a new empty trie keymap.
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
            descriptions: BTreeMap::new(),
        }
    }

    /// Inserts a single-key binding.
    pub fn insert<V: Into<T>>(&mut self, key: String, value: V) {
        self.insert_str(&key, value);
    }

    /// Inserts a binding from a canonical key string.
    ///
    /// The string uses the same canonical notation produced by
    /// `Key::canonical_string()`.
    pub fn insert_str<V: Into<T>>(&mut self, keys: &str, value: V) {
        let parsed = validate_key_string(keys).expect("invalid canonical key string");
        self.insert_sequence(parsed, value);
    }

    /// Inserts a binding and its optional key-guide description.
    pub fn insert_str_described<V: Into<T>>(
        &mut self,
        keys: &str,
        value: V,
        description: Option<String>,
    ) {
        let parsed = validate_key_string(keys).expect("invalid canonical key string");
        self.insert_sequence(parsed.clone(), value);
        if let Some(description) = description {
            self.descriptions.insert(parsed, description);
        }
    }

    /// Inserts a multi-key binding from an already parsed sequence.
    pub fn insert_sequence<V: Into<T>>(&mut self, keys: Vec<String>, value: V) {
        self.descriptions.remove(&keys);
        let mut current = &mut self.root;
        for key in &keys {
            current = current
                .children
                .entry(key.clone())
                .or_insert_with(TrieNode::new);
        }
        current.value = Some(value.into());
    }

    /// Returns the value bound to an exact key sequence.
    pub fn get(&self, keys: &[String]) -> Option<&T> {
        self.node(keys)?.value.as_ref()
    }

    /// Returns the exact value when it satisfies `eligible`.
    pub fn get_filtered(&self, keys: &[String], eligible: impl Fn(&T) -> bool) -> Option<&T> {
        self.get(keys).filter(|value| eligible(value))
    }

    /// Removes an exact key sequence and returns its value, if present.
    pub fn remove_sequence(&mut self, keys: &[String]) -> Option<T> {
        fn remove<T>(node: &mut TrieNode<T>, keys: &[String]) -> Option<T> {
            if keys.is_empty() {
                return node.value.take();
            }

            let key = keys[0].clone();
            let value = node
                .children
                .get_mut(&key)
                .and_then(|child| remove(child, &keys[1..]));
            if node
                .children
                .get(&key)
                .is_some_and(|child| child.value.is_none() && child.children.is_empty())
            {
                node.children.remove(&key);
            }
            value
        }

        self.descriptions.remove(keys);
        remove(&mut self.root, keys)
    }

    /// Returns all exact bindings and their key sequences.
    pub fn bindings(&self) -> Vec<(Vec<String>, &T)> {
        fn collect<'a, T>(
            node: &'a TrieNode<T>,
            prefix: &mut Vec<String>,
            bindings: &mut Vec<(Vec<String>, &'a T)>,
        ) {
            if let Some(value) = node.value.as_ref() {
                bindings.push((prefix.clone(), value));
            }
            for (key, child) in &node.children {
                prefix.push(key.clone());
                collect(child, prefix, bindings);
                prefix.pop();
            }
        }

        let mut bindings = Vec::new();
        collect(&self.root, &mut Vec::new(), &mut bindings);
        bindings
    }

    /// Returns `true` if the provided key sequence is a valid prefix in the trie.
    pub fn is_prefix(&self, keys: &[String]) -> bool {
        let Some(current) = self.node(keys) else {
            return false;
        };
        !current.children.is_empty() || current.value.is_some()
    }

    /// Returns whether the sequence reaches an eligible value or descendant binding.
    pub fn is_prefix_filtered(&self, keys: &[String], eligible: impl Fn(&T) -> bool) -> bool {
        self.node(keys)
            .is_some_and(|node| Self::subtree_has_filtered_value(node, &eligible))
    }

    /// Returns `true` if the provided key sequence has at least one child binding.
    pub fn has_children(&self, keys: &[String]) -> bool {
        self.node(keys)
            .is_some_and(|current| !current.children.is_empty())
    }

    /// Returns whether the sequence has an eligible descendant binding.
    pub fn has_children_filtered(&self, keys: &[String], eligible: impl Fn(&T) -> bool) -> bool {
        self.node(keys).is_some_and(|node| {
            node.children
                .values()
                .any(|child| Self::subtree_has_filtered_value(child, &eligible))
        })
    }

    fn node(&self, keys: &[String]) -> Option<&TrieNode<T>> {
        let mut current = &self.root;
        for key in keys {
            current = current.children.get(key)?;
        }
        Some(current)
    }

    fn subtree_has_filtered_value(node: &TrieNode<T>, eligible: &impl Fn(&T) -> bool) -> bool {
        node.value.as_ref().is_some_and(eligible)
            || node
                .children
                .values()
                .any(|child| Self::subtree_has_filtered_value(child, eligible))
    }
}

impl TrieKeymap<Intent> {
    /// Inserts configured mappings from canonical key strings to command strings.
    pub fn insert_configured(
        &mut self,
        mappings: &BTreeMap<String, String>,
        descriptions: Option<&BTreeMap<String, String>>,
    ) {
        for (keys, command) in mappings {
            let parsed_keys =
                validate_key_string(keys).expect("validated configured keymap key should parse");
            let intent = crate::command::parse(command)
                .expect("validated configured keymap command should parse");
            self.insert_sequence(parsed_keys.clone(), intent);
            if let Some(description) = descriptions.and_then(|values| values.get(keys)) {
                self.descriptions.insert(parsed_keys, description.clone());
            }
        }
    }

    /// Inserts runtime mappings from canonical key strings to resolved intents.
    pub fn insert_intents(&mut self, mappings: &BTreeMap<String, Intent>) {
        for (keys, intent) in mappings {
            let parsed_keys =
                validate_key_string(keys).expect("validated runtime keymap key should parse");
            self.insert_sequence(parsed_keys, intent.clone());
        }
    }

    /// Adds descriptions for bindings that have already been inserted.
    pub fn insert_descriptions(&mut self, descriptions: &BTreeMap<String, String>) {
        for (keys, description) in descriptions {
            let parsed_keys =
                validate_key_string(keys).expect("validated runtime keymap key should parse");
            if self.get(&parsed_keys).is_some() {
                self.descriptions.insert(parsed_keys, description.clone());
            }
        }
    }

    /// Returns the intent bound to an exact key sequence.
    pub fn get_action(&self, keys: &[String]) -> Option<Intent> {
        self.get(keys).cloned()
    }

    /// Returns the immediate keys available after a sequence.
    pub fn continuations(&self, keys: &[String]) -> Vec<KeyGuideEntry> {
        let Some(node) = self.node(keys) else {
            return Vec::new();
        };
        node.children
            .iter()
            .map(|(key, child)| {
                let mut full = keys.to_vec();
                full.push(key.clone());
                let description = self
                    .descriptions
                    .get(&full)
                    .cloned()
                    .or_else(|| child.value.as_ref().map(describe_intent))
                    .unwrap_or_else(|| "Prefix".to_string());
                KeyGuideEntry {
                    key: key.clone(),
                    description,
                    is_prefix: !child.children.is_empty(),
                }
            })
            .collect()
    }
}

/// One entry displayed by the pending-key guide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyGuideEntry {
    /// Canonical next key.
    pub key: String,
    /// User-facing action description.
    pub description: String,
    /// Whether additional bindings exist below this key.
    pub is_prefix: bool,
}

/// Current pending editor sequence and its available continuations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyGuideSnapshot {
    /// Canonical keys entered so far.
    pub prefix: Vec<String>,
    /// Available immediate continuations.
    pub entries: Vec<KeyGuideEntry>,
}

fn describe_intent(intent: &Intent) -> String {
    let identifier = match intent {
        Intent::Command(command) => command.event_name().into_owned(),
        Intent::Editor(action) => match action.kind.as_ref() {
            Some(super::EditorOperation::Operation(_, target)) => {
                return target.description().to_string();
            }
            Some(super::EditorOperation::VisualTextObject(text_object)) => {
                return text_object.description().to_string();
            }
            Some(operation) => format!("{operation:?}"),
            None => "Action".to_string(),
        },
    };
    let identifier = identifier
        .split(['(', '{'])
        .next()
        .unwrap_or(identifier.as_str());
    let mut words = String::new();
    let mut previous_lowercase = false;
    for character in identifier.chars() {
        if matches!(character, '.' | '-' | '_') {
            words.push(' ');
            previous_lowercase = false;
        } else {
            if character.is_uppercase() && previous_lowercase {
                words.push(' ');
            }
            words.push(character);
            previous_lowercase = character.is_lowercase();
        }
    }
    let mut characters = words.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().chain(characters).collect(),
        None => "Action".to_string(),
    }
}

impl Keymap for TrieKeymap<Intent> {
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

/// Resolves inherited mappings without involving editor mode state.
#[derive(Debug)]
pub struct InheritedKeymap {
    keymap: TrieKeymap<Intent>,
    pending: Vec<String>,
}

impl InheritedKeymap {
    /// Creates an inherited resolver over an effective keymap.
    pub fn new(keymap: TrieKeymap<Intent>) -> Self {
        Self {
            keymap,
            pending: Vec::new(),
        }
    }

    /// Routes a key through mappings whose inheritance satisfies `eligible`.
    pub fn handle_key(
        &mut self,
        key: &Key,
        eligible: impl Fn(KeymapInheritance) -> bool,
    ) -> HandleKeyResult {
        self.pending.push(key.canonical_string());

        if let Some(intent) = self
            .keymap
            .get_filtered(&self.pending, |intent| {
                eligible(intent.keymap_inheritance())
            })
            .cloned()
        {
            self.pending.clear();
            return HandleKeyResult::Complete(intent);
        }

        if self.keymap.is_prefix_filtered(&self.pending, |intent| {
            eligible(intent.keymap_inheritance())
        }) {
            return HandleKeyResult::WaitForMore;
        }

        self.pending.clear();
        HandleKeyResult::InvalidSequence
    }

    /// Clears any partially entered inherited key sequence.
    pub fn clear_pending(&mut self) {
        self.pending.clear();
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
    use crate::editor::{EditorAction, EditorOperation};
    use crate::ui::Command;

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
        keymap.insert_str("gg", EditorAction::new(EditorOperation::MoveUp));

        assert_eq!(
            keymap.get_action(&["g".to_string(), "g".to_string()]),
            Some(EditorAction::new(EditorOperation::MoveUp).into())
        );
    }

    #[test]
    fn generic_trie_keymap_supports_bindings_and_removal() {
        let mut keymap = TrieKeymap::<String>::new();
        keymap.insert_sequence(vec!["g".to_string(), "g".to_string()], "first".to_string());
        keymap.insert_sequence(vec!["g".to_string(), "h".to_string()], "second".to_string());

        assert_eq!(keymap.get(&["g".to_string()]), None);
        assert!(keymap.is_prefix(&["g".to_string()]));
        assert_eq!(
            keymap
                .get(&["g".to_string(), "g".to_string()])
                .map(String::as_str),
            Some("first")
        );
        assert_eq!(keymap.bindings().len(), 2);
        assert_eq!(
            keymap.remove_sequence(&["g".to_string(), "g".to_string()]),
            Some("first".to_string())
        );
        assert!(keymap.is_prefix(&["g".to_string()]));
        assert_eq!(
            keymap.remove_sequence(&["g".to_string(), "g".to_string()]),
            None
        );
    }

    #[test]
    fn filtered_prefix_ignores_ineligible_descendants() {
        let mut keymap = TrieKeymap::<Intent>::new();
        keymap.insert_str("gd", Command::LspDefinition);

        assert!(!keymap.is_prefix_filtered(&["g".to_string()], |intent| {
            intent.keymap_inheritance() == KeymapInheritance::Focus
        }));
    }

    #[test]
    fn filtered_prefix_keeps_mixed_eligible_descendants() {
        let mut keymap = TrieKeymap::<Intent>::new();
        keymap.insert_str("gd", Command::LspDefinition);
        keymap.insert_str("gp", Command::FocusPreviousTarget);

        assert!(keymap.is_prefix_filtered(&["g".to_string()], |intent| {
            intent.keymap_inheritance() == KeymapInheritance::Focus
        }));
        assert!(
            keymap
                .get_filtered(&["g".to_string(), "p".to_string()], |intent| {
                    intent.keymap_inheritance() == KeymapInheritance::Focus
                })
                .is_some()
        );
        assert!(
            keymap
                .get_filtered(&["g".to_string(), "d".to_string()], |intent| {
                    intent.keymap_inheritance() == KeymapInheritance::Focus
                })
                .is_none()
        );
    }

    #[test]
    fn continuations_include_descriptions_and_prefixes() {
        let mut keymap = TrieKeymap::<Intent>::new();
        keymap.insert_str_described(
            "gd",
            Command::LspDefinition,
            Some("Go to definition".to_string()),
        );
        keymap.insert_str("grr", Command::LspReferences);

        assert_eq!(
            keymap.continuations(&["g".to_string()]),
            vec![
                KeyGuideEntry {
                    key: "d".to_string(),
                    description: "Go to definition".to_string(),
                    is_prefix: false,
                },
                KeyGuideEntry {
                    key: "r".to_string(),
                    description: "Prefix".to_string(),
                    is_prefix: true,
                },
            ]
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
