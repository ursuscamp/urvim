//! Character scan keymap for f/F/t/T motions.
//!
//! This module provides a stateless keymap that matches two-key sequences for
//! character scan motions where the first key is the trigger (f, F, t, T)
//! and the second key is the target character (a runtime parameter).

use crate::editor::{Action, Keymap};
use crate::globals::{Direction, FindKind, FindState};

/// A stateless keymap for character scan motions (f, F, t, T).
///
/// Matches two-key sequences where:
/// - First key is a character scan trigger (f, F, t, T)
/// - Second key is any character (the target)
///
/// Returns `Action::find_forward(char)`, `Action::find_backward(char)`,
/// `Action::till_forward(char)`, or `Action::till_backward(char)`.
#[derive(Debug, Clone, Default)]
pub struct CharScanKeymap;

impl CharScanKeymap {
    /// Creates a new CharScanKeymap.
    pub fn new() -> Self {
        Self
    }

    /// Parses a two-key character scan sequence into its trigger and target.
    ///
    /// The trigger determines the motion family and direction, while the target
    /// is the runtime character to search for.
    pub fn parse_find_state(keys: &[String]) -> Option<FindState> {
        let [trigger, target] = keys else {
            return None;
        };

        let target_char = target.chars().next()?;
        match trigger.as_str() {
            "f" => Some(FindState {
                target_char,
                kind: FindKind::Find,
                direction: Direction::Forward,
            }),
            "F" => Some(FindState {
                target_char,
                kind: FindKind::Find,
                direction: Direction::Backward,
            }),
            "t" => Some(FindState {
                target_char,
                kind: FindKind::Till,
                direction: Direction::Forward,
            }),
            "T" => Some(FindState {
                target_char,
                kind: FindKind::Till,
                direction: Direction::Backward,
            }),
            _ => None,
        }
    }
}

impl Keymap for CharScanKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action> {
        Self::parse_find_state(keys).map(|find_state| {
            match (find_state.kind, find_state.direction) {
                (FindKind::Find, Direction::Forward) => {
                    Action::find_forward(find_state.target_char)
                }
                (FindKind::Find, Direction::Backward) => {
                    Action::find_backward(find_state.target_char)
                }
                (FindKind::Till, Direction::Forward) => {
                    Action::till_forward(find_state.target_char)
                }
                (FindKind::Till, Direction::Backward) => {
                    Action::till_backward(find_state.target_char)
                }
            }
        })
    }

    fn is_prefix(&self, keys: &[String]) -> bool {
        keys.len() == 1 && matches!(keys[0].as_str(), "f" | "F" | "t" | "T")
    }

    fn has_children(&self, keys: &[String]) -> bool {
        // Same as is_prefix for char scan: single trigger keys can be extended with target char
        self.is_prefix(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_action_finds_forward() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["f".to_string(), "x".to_string()]);
        assert_eq!(action, Some(Action::find_forward('x')));
    }

    #[test]
    fn test_get_action_finds_backward() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["F".to_string(), "y".to_string()]);
        assert_eq!(action, Some(Action::find_backward('y')));
    }

    #[test]
    fn test_get_action_till_forward() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["t".to_string(), "z".to_string()]);
        assert_eq!(action, Some(Action::till_forward('z')));
    }

    #[test]
    fn test_get_action_till_backward() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["T".to_string(), "w".to_string()]);
        assert_eq!(action, Some(Action::till_backward('w')));
    }

    #[test]
    fn test_get_action_single_key_returns_none() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["f".to_string()]);
        assert_eq!(action, None);
    }

    #[test]
    fn test_get_action_non_trigger_returns_none() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["g".to_string(), "g".to_string()]);
        assert_eq!(action, None);
    }

    #[test]
    fn test_get_action_empty_returns_none() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&[]);
        assert_eq!(action, None);
    }

    #[test]
    fn test_get_action_three_keys_returns_none() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["f".to_string(), "x".to_string(), "y".to_string()]);
        assert_eq!(action, None);
    }

    #[test]
    fn test_is_prefix_f_trigger() {
        let keymap = CharScanKeymap::new();
        assert!(keymap.is_prefix(&["f".to_string()]));
    }

    #[test]
    fn test_is_prefix_uppercase_f_trigger() {
        let keymap = CharScanKeymap::new();
        assert!(keymap.is_prefix(&["F".to_string()]));
    }

    #[test]
    fn test_is_prefix_t_trigger() {
        let keymap = CharScanKeymap::new();
        assert!(keymap.is_prefix(&["t".to_string()]));
    }

    #[test]
    fn test_is_prefix_uppercase_t_trigger() {
        let keymap = CharScanKeymap::new();
        assert!(keymap.is_prefix(&["T".to_string()]));
    }

    #[test]
    fn test_is_prefix_complete_sequence_returns_false() {
        let keymap = CharScanKeymap::new();
        assert!(!keymap.is_prefix(&["f".to_string(), "x".to_string()]));
    }

    #[test]
    fn test_is_prefix_non_trigger_returns_false() {
        let keymap = CharScanKeymap::new();
        assert!(!keymap.is_prefix(&["g".to_string()]));
    }

    #[test]
    fn test_is_prefix_empty_returns_false() {
        let keymap = CharScanKeymap::new();
        assert!(!keymap.is_prefix(&[]));
    }
}
