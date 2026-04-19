use super::keymap::{MAX_COUNT, extract_leading_count};
use super::{Action, CountParser, HandleKeyResult, Keymap, Mode, ModeKind, TrieKeymap};
use crate::editor::ActionKind;
use crate::globals;
use crate::motion::chained_keymap::ChainedKeymap;
use crate::terminal::{CursorStyle, Key, KeyCode};

mod bindings;

/// Normal mode for vim-style navigation and commands.
pub struct NormalMode {
    keymap: ChainedKeymap,
    buffer: Vec<String>,
    waiting: bool,
}

impl Default for NormalMode {
    fn default() -> Self {
        Self::new()
    }
}

impl NormalMode {
    /// Creates a normal-mode keymap with Vim-style navigation and operators.
    pub fn new() -> Self {
        let mut trie_keymap = TrieKeymap::new();
        bindings::register(&mut trie_keymap);

        let mut keymap = ChainedKeymap::new();
        keymap.add(Box::new(trie_keymap));
        keymap.add(Box::new(
            crate::motion::char_scan_keymap::CharScanKeymap::new(),
        ));

        NormalMode {
            keymap,
            buffer: Vec::new(),
            waiting: false,
        }
    }

    fn resolve_register_selector(selector: char) -> Option<crate::register::RegisterName> {
        let defaults =
            globals::with_config(|config| config.default_registers.clone()).unwrap_or_default();
        crate::register::RegisterName::from_prefix(selector, &defaults)
    }

    fn parse_buffered_action(&self) -> HandleKeyResult {
        let keys = self.buffer.clone();
        let (leading_count, remaining_keys) = extract_leading_count(&keys);
        let mut action_keys = remaining_keys;
        let mut register_prefix = None;

        if action_keys.first().is_some_and(|key| key == "\"") {
            if action_keys.len() == 1 {
                return HandleKeyResult::WaitForMore;
            }

            let selector = action_keys[1].chars().next();
            let Some(selector) = selector.filter(|ch| ch.is_ascii_lowercase()) else {
                return HandleKeyResult::InvalidSequence;
            };

            let Some(register) = Self::resolve_register_selector(selector) else {
                return HandleKeyResult::InvalidSequence;
            };

            register_prefix = Some(register);
            action_keys.drain(0..2);
        }

        if action_keys.is_empty() {
            return if register_prefix.is_some() || leading_count > 0 {
                HandleKeyResult::WaitForMore
            } else {
                HandleKeyResult::InvalidSequence
            };
        }

        let (action_keys, count) = CountParser::parse(&action_keys);
        let total_count = if leading_count > 0 {
            leading_count.saturating_mul(count).min(MAX_COUNT)
        } else {
            count
        };
        if let Some(mut action) = self.keymap.get_action(&action_keys) {
            if self.keymap.has_children(&action_keys) {
                return HandleKeyResult::WaitForMore;
            }

            if let Some(register) = register_prefix {
                action = action.with_register(register);
            }

            if Self::ignores_count_wrapping(&action) {
                return HandleKeyResult::Complete(action.with_from_mode(ModeKind::Normal));
            }

            if total_count > 1
                && let Some(counted_action) = action.clone().with_count(total_count)
            {
                return HandleKeyResult::Complete(counted_action.with_from_mode(ModeKind::Normal));
            }
            return HandleKeyResult::Complete(action.with_from_mode(ModeKind::Normal));
        }

        if self.keymap.is_prefix(&action_keys) {
            return HandleKeyResult::WaitForMore;
        }

        if leading_count > 0 || register_prefix.is_some() {
            HandleKeyResult::InvalidSequence
        } else {
            HandleKeyResult::InvalidSequence
        }
    }

    // These are mode commands rather than motions, so we never wrap them in a
    // numeric repeat count.
    fn ignores_count_wrapping(action: &Action) -> bool {
        matches!(
            action.kind.as_ref(),
            Some(ActionKind::DeleteSelection)
                | Some(ActionKind::ChangeSelection)
                | Some(ActionKind::YankSelection)
        ) || (action.kind.is_none() && action.to_mode == Some(ModeKind::Normal))
    }
}

impl Mode for NormalMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        if key.code == KeyCode::Esc {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::InvalidSequence;
        }

        self.buffer.push(key.canonical_string());

        let buffer_str: String = self.buffer.iter().cloned().collect();
        let all_digits = self.buffer.iter().all(|k| {
            k.len() == 1
                && k.chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
        });

        if all_digits && CountParser::is_valid_count(&buffer_str) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        match self.parse_buffered_action() {
            HandleKeyResult::WaitForMore => {
                self.waiting = true;
                HandleKeyResult::WaitForMore
            }
            HandleKeyResult::Complete(action) => {
                self.buffer.clear();
                self.waiting = false;
                HandleKeyResult::Complete(action)
            }
            HandleKeyResult::InvalidSequence => {
                self.buffer.clear();
                self.waiting = false;
                HandleKeyResult::InvalidSequence
            }
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBlock
    }

    fn is_waiting(&self) -> bool {
        self.waiting
    }

    fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.waiting = false;
    }

    fn kind(&self) -> ModeKind {
        ModeKind::Normal
    }
}
