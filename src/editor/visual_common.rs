use super::keymap::{MAX_COUNT, extract_leading_count};
use super::surround;
use super::{Action, ActionKind, CountParser, HandleKeyResult, Keymap, ModeKind, TrieKeymap};
use crate::buffer::Boundary;
use crate::editor::{BracketKind, Operator, OperatorTarget, QuoteKind, TextObject};
use crate::motion::chained_keymap::ChainedKeymap;
use crate::motion::char_scan_keymap::CharScanKeymap;
use crate::terminal::{Key, KeyCode};

/// Shared state and key handling for visual modes.
pub(super) struct VisualModeState {
    keymap: ChainedKeymap,
    buffer: Vec<String>,
    waiting: bool,
    mode_kind: ModeKind,
}

impl VisualModeState {
    pub(super) fn new(
        mode_kind: ModeKind,
        exit_key: &str,
        switch_key: &str,
        switch_to: ModeKind,
    ) -> Self {
        let keymap = build_visual_keymap(exit_key, switch_key, switch_to);
        Self {
            keymap,
            buffer: Vec::new(),
            waiting: false,
            mode_kind,
        }
    }

    pub(super) fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        if key.code == KeyCode::Esc {
            self.clear_buffer();
            return HandleKeyResult::Complete(
                Action::mode_transition(ModeKind::Normal).with_from_mode(self.mode_kind),
            );
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

    pub(super) fn is_waiting(&self) -> bool {
        self.waiting
    }

    pub(super) fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.waiting = false;
    }

    pub(super) fn kind(&self) -> ModeKind {
        self.mode_kind
    }

    // These are mode commands rather than motions, so we never wrap them in a
    // numeric repeat count.
    fn ignores_count_wrapping(action: &Action) -> bool {
        matches!(
            action.kind.as_ref(),
            Some(ActionKind::DeleteSelection)
                | Some(ActionKind::ChangeSelection)
                | Some(ActionKind::YankSelection)
                | Some(ActionKind::Operation(
                    Operator::Lowercase,
                    OperatorTarget::Selection,
                ))
                | Some(ActionKind::Operation(
                    Operator::Uppercase,
                    OperatorTarget::Selection,
                ))
                | Some(ActionKind::Operation(
                    Operator::ToggleCase,
                    OperatorTarget::Selection,
                ))
                | Some(ActionKind::SurroundAddSelection { .. })
        ) || (action.kind.is_none() && action.to_mode == Some(ModeKind::Normal))
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

            let defaults = crate::globals::with_config(|config| config.default_registers.clone())
                .unwrap_or_default();
            let Some(register) = crate::register::RegisterName::from_prefix(selector, &defaults)
            else {
                return HandleKeyResult::InvalidSequence;
            };

            register_prefix = Some(register);
            action_keys.drain(0..2);
        }

        if action_keys.is_empty() {
            return HandleKeyResult::WaitForMore;
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
                return HandleKeyResult::Complete(action.with_from_mode(self.mode_kind));
            }

            if total_count > 1
                && let Some(counted_action) = action.clone().with_count(total_count)
            {
                return HandleKeyResult::Complete(counted_action.with_from_mode(self.mode_kind));
            }
            return HandleKeyResult::Complete(action.with_from_mode(self.mode_kind));
        }

        if self.keymap.is_prefix(&action_keys) || leading_count > 0 || register_prefix.is_some() {
            HandleKeyResult::WaitForMore
        } else {
            HandleKeyResult::InvalidSequence
        }
    }
}

fn build_visual_keymap(exit_key: &str, switch_key: &str, switch_to: ModeKind) -> ChainedKeymap {
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
    trie_keymap.insert_str(
        "gu",
        Action::operation(Operator::Lowercase, OperatorTarget::Selection)
            .with_to_mode(ModeKind::Normal),
    );
    trie_keymap.insert_str(
        "gU",
        Action::operation(Operator::Uppercase, OperatorTarget::Selection)
            .with_to_mode(ModeKind::Normal),
    );
    trie_keymap.insert_str(
        "g~",
        Action::operation(Operator::ToggleCase, OperatorTarget::Selection)
            .with_to_mode(ModeKind::Normal),
    );
    trie_keymap.insert_str("{", Action::new(ActionKind::MoveToPreviousParagraph));
    trie_keymap.insert_str("}", Action::new(ActionKind::MoveToNextParagraph));
    trie_keymap.insert_str("%", Action::new(ActionKind::MoveToMatchingBracket));
    trie_keymap.insert_str(";", Action::new(ActionKind::RepeatLastFind));
    trie_keymap.insert_str(",", Action::new(ActionKind::RepeatLastFindReverse));
    trie_keymap.insert_str(exit_key, Action::mode_transition(ModeKind::Normal));
    trie_keymap.insert_str(switch_key, Action::mode_transition(switch_to));
    trie_keymap.insert_str(
        "d",
        Action::new(ActionKind::DeleteSelection).with_to_mode(ModeKind::Normal),
    );
    trie_keymap.insert_str(
        "y",
        Action::new(ActionKind::YankSelection).with_to_mode(ModeKind::Normal),
    );
    trie_keymap.insert_str(
        "c",
        Action::new(ActionKind::ChangeSelection).with_to_mode(ModeKind::Insert),
    );
    register_visual_surround_bindings(&mut trie_keymap);
    register_visual_text_object_bindings(&mut trie_keymap);
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
    keymap
}

fn register_visual_surround_bindings(trie_keymap: &mut TrieKeymap) {
    for (selector, delimiter) in surround::delimiter_selectors() {
        trie_keymap.insert_str(
            &format!("gsa{selector}"),
            Action::new(ActionKind::SurroundAddSelection {
                delimiter: *delimiter,
            })
            .with_to_mode(ModeKind::Normal),
        );
    }
}

fn register_visual_text_object_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str(
        "iw",
        Action::new(ActionKind::VisualTextObject(TextObject::InnerWord)),
    );
    trie_keymap.insert_str(
        "aw",
        Action::new(ActionKind::VisualTextObject(TextObject::AroundWord)),
    );
    trie_keymap.insert_str(
        "iW",
        Action::new(ActionKind::VisualTextObject(TextObject::InnerBigWord)),
    );
    trie_keymap.insert_str(
        "aW",
        Action::new(ActionKind::VisualTextObject(TextObject::AroundBigWord)),
    );

    for (kind, open, close) in [
        (BracketKind::Paren, '(', ')'),
        (BracketKind::Square, '[', ']'),
        (BracketKind::Curly, '{', '}'),
        (BracketKind::Angle, '<', '>'),
    ] {
        let open_key = match open {
            '<' => "<LessThan>".to_string(),
            _ => open.to_string(),
        };
        let close_key = match close {
            '>' => "<GreaterThan>".to_string(),
            _ => close.to_string(),
        };

        trie_keymap.insert_str(
            &format!("i{open_key}"),
            Action::new(ActionKind::VisualTextObject(TextObject::InnerBracket(kind))),
        );
        trie_keymap.insert_str(
            &format!("a{open_key}"),
            Action::new(ActionKind::VisualTextObject(TextObject::AroundBracket(
                kind,
            ))),
        );
        trie_keymap.insert_str(
            &format!("i{close_key}"),
            Action::new(ActionKind::VisualTextObject(TextObject::InnerBracket(kind))),
        );
        trie_keymap.insert_str(
            &format!("a{close_key}"),
            Action::new(ActionKind::VisualTextObject(TextObject::AroundBracket(
                kind,
            ))),
        );
    }

    for (kind, key) in [
        (QuoteKind::Single, "'"),
        (QuoteKind::Double, "\""),
        (QuoteKind::Backtick, "`"),
    ] {
        trie_keymap.insert_str(
            &format!("i{key}"),
            Action::new(ActionKind::VisualTextObject(TextObject::InnerQuote(kind))),
        );
        trie_keymap.insert_str(
            &format!("a{key}"),
            Action::new(ActionKind::VisualTextObject(TextObject::AroundQuote(kind))),
        );
    }
}
