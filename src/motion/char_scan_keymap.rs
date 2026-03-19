//! Character scan keymap for f/F/t/T motions.
//!
//! This module provides a stateless keymap that matches two-key sequences for
//! character scan motions where the first key is the trigger (f, F, t, T)
//! and the second key is the target character (a runtime parameter).

use crate::editor::{Action, Keymap};

/// A stateless keymap for character scan motions (f, F, t, T).
///
/// Matches two-key sequences where:
/// - First key is a character scan trigger (f, F, t, T)
/// - Second key is any character (the target)
///
/// Returns `Action::FindForward(char)`, `Action::FindBackward(char)`,
/// `Action::TillForward(char)`, or `Action::TillBackward(char)`.
#[derive(Debug, Clone, Default)]
pub struct CharScanKeymap;

impl CharScanKeymap {
    /// Creates a new CharScanKeymap.
    pub fn new() -> Self {
        Self
    }
}

impl Keymap for CharScanKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action> {
        if keys.len() != 2 {
            return None;
        }

        let [trigger, target] = keys else {
            return None;
        };

        let target_char = target.chars().next()?;
        let key_str = trigger.as_str();

        match key_str {
            "f" => Some(Action::FindForward(target_char)),
            "F" => Some(Action::FindBackward(target_char)),
            "t" => Some(Action::TillForward(target_char)),
            "T" => Some(Action::TillBackward(target_char)),
            _ => None,
        }
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
        assert_eq!(action, Some(Action::FindForward('x')));
    }

    #[test]
    fn test_get_action_finds_backward() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["F".to_string(), "y".to_string()]);
        assert_eq!(action, Some(Action::FindBackward('y')));
    }

    #[test]
    fn test_get_action_till_forward() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["t".to_string(), "z".to_string()]);
        assert_eq!(action, Some(Action::TillForward('z')));
    }

    #[test]
    fn test_get_action_till_backward() {
        let keymap = CharScanKeymap::new();
        let action = keymap.get_action(&["T".to_string(), "w".to_string()]);
        assert_eq!(action, Some(Action::TillBackward('w')));
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
    fn test_is_prefix_F_trigger() {
        let keymap = CharScanKeymap::new();
        assert!(keymap.is_prefix(&["F".to_string()]));
    }

    #[test]
    fn test_is_prefix_t_trigger() {
        let keymap = CharScanKeymap::new();
        assert!(keymap.is_prefix(&["t".to_string()]));
    }

    #[test]
    fn test_is_prefix_T_trigger() {
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
