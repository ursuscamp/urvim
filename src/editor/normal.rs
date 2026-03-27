use super::keymap::{MAX_COUNT, extract_leading_count};
use super::{
    Action, BoundaryMotion, BracketKind, CountParser, HandleKeyResult, Keymap, LinewiseMotion,
    Mode, ModeKind, Operator, OperatorTarget, TextObject, TrieKeymap,
};
use crate::buffer::Boundary;
use crate::motion::chained_keymap::ChainedKeymap;
use crate::motion::char_scan_keymap::CharScanKeymap;
use crate::terminal::{CursorStyle, Key, KeyCode};

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
    pub fn new() -> Self {
        let mut trie_keymap = TrieKeymap::new();

        trie_keymap.insert("h".to_string(), Action::MoveLeft);
        trie_keymap.insert("j".to_string(), Action::MoveDown);
        trie_keymap.insert("k".to_string(), Action::MoveUp);
        trie_keymap.insert("l".to_string(), Action::MoveRight);

        trie_keymap.insert("w".to_string(), Action::ForwardTo(Boundary::Word));
        trie_keymap.insert("b".to_string(), Action::BackTo(Boundary::Word));
        trie_keymap.insert("e".to_string(), Action::ForwardTo(Boundary::WordEnd));

        trie_keymap.insert("W".to_string(), Action::ForwardTo(Boundary::BigWord));
        trie_keymap.insert("B".to_string(), Action::BackTo(Boundary::BigWord));
        trie_keymap.insert("E".to_string(), Action::ForwardTo(Boundary::BigWordEnd));

        trie_keymap.insert("$".to_string(), Action::MoveToLineEnd);
        trie_keymap.insert("0".to_string(), Action::MoveToLineStart);
        trie_keymap.insert("^".to_string(), Action::MoveToLineContentStart);

        trie_keymap.insert_sequence(
            vec!["g".to_string(), "g".to_string()],
            Action::MoveToFirstLine,
        );
        trie_keymap.insert("G".to_string(), Action::MoveToLastLine);
        trie_keymap.insert("H".to_string(), Action::MoveToScreenTop);
        trie_keymap.insert("M".to_string(), Action::MoveToScreenMiddle);
        trie_keymap.insert("L".to_string(), Action::MoveToScreenBottom);
        trie_keymap.insert("{".to_string(), Action::MoveToPreviousParagraph);
        trie_keymap.insert("}".to_string(), Action::MoveToNextParagraph);
        trie_keymap.insert("J".to_string(), Action::JoinWithSpace);
        trie_keymap.insert_sequence(
            vec!["g".to_string(), "J".to_string()],
            Action::JoinWithoutSpace,
        );
        trie_keymap.insert("i".to_string(), Action::SwitchToInsert);
        trie_keymap.insert("a".to_string(), Action::AppendAfterCursor);
        trie_keymap.insert("A".to_string(), Action::AppendToLineEnd);
        trie_keymap.insert("I".to_string(), Action::InsertAtLineStart);
        trie_keymap.insert("o".to_string(), Action::OpenLineBelow);
        trie_keymap.insert("O".to_string(), Action::OpenLineAbove);
        trie_keymap.insert_sequence(vec!["[".to_string(), "b".to_string()], Action::PreviousTab);
        trie_keymap.insert_sequence(vec!["]".to_string(), "b".to_string()], Action::NextTab);
        trie_keymap.insert("x".to_string(), Action::DeleteForward);
        trie_keymap.insert("X".to_string(), Action::DeleteBackward);
        trie_keymap.insert_sequence(vec!["d".to_string(), "d".to_string()], Action::DeleteLine);
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "i".to_string(), "w".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::InnerWord),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "a".to_string(), "w".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::AroundWord),
            ),
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
            trie_keymap.insert_sequence(
                vec!["d".to_string(), "i".to_string(), open_key.clone()],
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_sequence(
                vec!["d".to_string(), "i".to_string(), close_key.clone()],
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_sequence(
                vec!["d".to_string(), "a".to_string(), open_key.clone()],
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
            trie_keymap.insert_sequence(
                vec!["d".to_string(), "a".to_string(), close_key.clone()],
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
            trie_keymap.insert_sequence(
                vec!["c".to_string(), "i".to_string(), open_key.clone()],
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_sequence(
                vec!["c".to_string(), "i".to_string(), close_key.clone()],
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_sequence(
                vec!["c".to_string(), "a".to_string(), open_key.clone()],
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
            trie_keymap.insert_sequence(
                vec!["c".to_string(), "a".to_string(), close_key.clone()],
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
        }
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "w".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "e".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "b".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "$".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "0".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "^".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "W".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "E".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordEnd),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "B".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "g".to_string(), "g".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "G".to_string()],
            Action::Operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "w".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "e".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "b".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "W".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "E".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordEnd),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "B".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "i".to_string(), "w".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::InnerWord),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "a".to_string(), "w".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::AroundWord),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "$".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "0".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "^".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "g".to_string(), "g".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
            ),
        );
        trie_keymap.insert_sequence(
            vec!["c".to_string(), "G".to_string()],
            Action::Operation(
                Operator::Change,
                OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
            ),
        );
        trie_keymap.insert_sequence(vec!["c".to_string(), "c".to_string()], Action::ChangeLine);
        trie_keymap.insert("C".to_string(), Action::ChangeToLineEnd);
        trie_keymap.insert("%".to_string(), Action::MoveToMatchingBracket);
        trie_keymap.insert(";".to_string(), Action::RepeatLastFind);
        trie_keymap.insert(",".to_string(), Action::RepeatLastFindReverse);
        trie_keymap.insert("<C-q>".to_string(), Action::Quit);
        trie_keymap.insert("u".to_string(), Action::Undo);
        trie_keymap.insert("U".to_string(), Action::Redo);
        trie_keymap.insert("<Left>".to_string(), Action::MoveLeft);
        trie_keymap.insert("<Down>".to_string(), Action::MoveDown);
        trie_keymap.insert("<Up>".to_string(), Action::MoveUp);
        trie_keymap.insert("<Right>".to_string(), Action::MoveRight);

        let mut keymap = ChainedKeymap::new();
        keymap.add(Box::new(trie_keymap));
        keymap.add(Box::new(CharScanKeymap::new()));

        NormalMode {
            keymap,
            buffer: Vec::new(),
            waiting: false,
        }
    }
}

impl Mode for NormalMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        if key.code == KeyCode::Esc {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::InvalidSequence;
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

                if total_count > 1
                    && let Some(counted_action) = action.clone().with_count(total_count)
                {
                    return HandleKeyResult::Complete(counted_action);
                }
                return HandleKeyResult::Complete(action);
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

            if count > 1
                && let Some(counted_action) = action.clone().with_count(count)
            {
                return HandleKeyResult::Complete(counted_action);
            }
            return HandleKeyResult::Complete(action);
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
        ModeKind::Normal
    }
}
