use super::keymap::{MAX_COUNT, extract_leading_count};
use super::{Action, ActionKind, CountParser, HandleKeyResult, Keymap, Mode, ModeKind, TrieKeymap};
use crate::buffer::Boundary;
use crate::motion::chained_keymap::ChainedKeymap;
use crate::motion::char_scan_keymap::CharScanKeymap;
use crate::terminal::{CursorStyle, Key, KeyCode};

/// Linewise visual mode for whole-line selection.
pub struct VisualLineMode {
    keymap: ChainedKeymap,
    buffer: Vec<String>,
    waiting: bool,
}

impl Default for VisualLineMode {
    fn default() -> Self {
        Self::new()
    }
}

impl VisualLineMode {
    /// Creates a new linewise visual mode with motion bindings and selection actions.
    pub fn new() -> Self {
        let mut trie_keymap = TrieKeymap::new();

        trie_keymap.insert_str("h", Action::new(ActionKind::MoveLeft));
        trie_keymap.insert_str("j", Action::new(ActionKind::MoveDown));
        trie_keymap.insert_str("k", Action::new(ActionKind::MoveUp));
        trie_keymap.insert_str("l", Action::new(ActionKind::MoveRight));

        trie_keymap.insert_str("w", Action::forward_to(Boundary::Word));
        trie_keymap.insert_str("b", Action::back_to(Boundary::Word));
        trie_keymap.insert_str("e", Action::forward_to(Boundary::WordEnd));

        trie_keymap.insert_str("W", Action::forward_to(Boundary::BigWord));
        trie_keymap.insert_str("B", Action::back_to(Boundary::BigWord));
        trie_keymap.insert_str("E", Action::forward_to(Boundary::BigWordEnd));

        trie_keymap.insert_str("0", Action::new(ActionKind::MoveToLineStart));
        trie_keymap.insert_str("^", Action::new(ActionKind::MoveToLineContentStart));
        trie_keymap.insert_str("$", Action::new(ActionKind::MoveToLineEnd));
        trie_keymap.insert_str("gg", Action::new(ActionKind::MoveToFirstLine));
        trie_keymap.insert_str("G", Action::new(ActionKind::MoveToLastLine));
        trie_keymap.insert_str("H", Action::new(ActionKind::MoveToScreenTop));
        trie_keymap.insert_str("M", Action::new(ActionKind::MoveToScreenMiddle));
        trie_keymap.insert_str("L", Action::new(ActionKind::MoveToScreenBottom));
        trie_keymap.insert_str("{", Action::new(ActionKind::MoveToPreviousParagraph));
        trie_keymap.insert_str("}", Action::new(ActionKind::MoveToNextParagraph));
        trie_keymap.insert_str("%", Action::new(ActionKind::MoveToMatchingBracket));
        trie_keymap.insert_str(";", Action::new(ActionKind::RepeatLastFind));
        trie_keymap.insert_str(",", Action::new(ActionKind::RepeatLastFindReverse));
        trie_keymap.insert_str("v", Action::mode_transition(ModeKind::Visual));
        trie_keymap.insert_str("V", Action::mode_transition(ModeKind::Normal));
        trie_keymap.insert_str(
            "d",
            Action::new(ActionKind::DeleteSelection).with_to_mode(ModeKind::Normal),
        );
        trie_keymap.insert_str(
            "c",
            Action::new(ActionKind::ChangeSelection).with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str("<Left>", Action::new(ActionKind::MoveLeft));
        trie_keymap.insert_str("<Down>", Action::new(ActionKind::MoveDown));
        trie_keymap.insert_str("<Up>", Action::new(ActionKind::MoveUp));
        trie_keymap.insert_str("<Right>", Action::new(ActionKind::MoveRight));
        trie_keymap.insert_str("<PageUp>", Action::new(ActionKind::MovePageUp));
        trie_keymap.insert_str("<PageDown>", Action::new(ActionKind::MovePageDown));
        trie_keymap.insert_str("<C-u>", Action::new(ActionKind::MoveHalfPageUp));
        trie_keymap.insert_str("<C-d>", Action::new(ActionKind::MoveHalfPageDown));

        let mut keymap = ChainedKeymap::new();
        keymap.add(Box::new(trie_keymap));
        keymap.add(Box::new(CharScanKeymap::new()));

        Self {
            keymap,
            buffer: Vec::new(),
            waiting: false,
        }
    }
}

impl Mode for VisualLineMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        if key.code == KeyCode::Esc {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(
                Action::mode_transition(ModeKind::Normal).with_from_mode(ModeKind::VisualLine),
            );
        }

        let key_str = key.canonical_string();
        self.buffer.push(key_str.clone());

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

        let (leading_count, remaining_keys) = extract_leading_count(&self.buffer);
        if leading_count > 0 && !remaining_keys.is_empty() {
            let (action_keys, sub_count) = CountParser::parse(&remaining_keys);
            let total_count = leading_count.saturating_mul(sub_count).min(MAX_COUNT);

            if let Some(action) = self.keymap.get_action(&action_keys) {
                if self.keymap.has_children(&action_keys) {
                    self.waiting = true;
                    return HandleKeyResult::WaitForMore;
                }

                self.buffer.clear();
                self.waiting = false;

                if Self::ignores_count_wrapping(&action) {
                    return HandleKeyResult::Complete(action.with_from_mode(ModeKind::VisualLine));
                }

                if total_count > 1
                    && let Some(counted_action) = action.clone().with_count(total_count)
                {
                    return HandleKeyResult::Complete(
                        counted_action.with_from_mode(ModeKind::VisualLine),
                    );
                }
                return HandleKeyResult::Complete(action.with_from_mode(ModeKind::VisualLine));
            }

            if self.keymap.is_prefix(&action_keys) {
                self.waiting = true;
                return HandleKeyResult::WaitForMore;
            }
        }

        let (action_keys, count) = CountParser::parse(&self.buffer);
        if let Some(action) = self.keymap.get_action(&action_keys) {
            if self.keymap.has_children(&action_keys) {
                self.waiting = true;
                return HandleKeyResult::WaitForMore;
            }

            self.buffer.clear();
            self.waiting = false;

            if Self::ignores_count_wrapping(&action) {
                return HandleKeyResult::Complete(action.with_from_mode(ModeKind::VisualLine));
            }

            if count > 1
                && let Some(counted_action) = action.clone().with_count(count)
            {
                return HandleKeyResult::Complete(
                    counted_action.with_from_mode(ModeKind::VisualLine),
                );
            }
            return HandleKeyResult::Complete(action.with_from_mode(ModeKind::VisualLine));
        }

        if self.keymap.is_prefix(&action_keys) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        self.buffer.clear();
        self.waiting = false;
        HandleKeyResult::InvalidSequence
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
        ModeKind::VisualLine
    }
}

impl VisualLineMode {
    fn ignores_count_wrapping(action: &Action) -> bool {
        matches!(
            action.kind.as_ref(),
            Some(ActionKind::DeleteSelection) | Some(ActionKind::ChangeSelection)
        ) || (action.kind.is_none() && action.to_mode == Some(ModeKind::Normal))
    }
}
