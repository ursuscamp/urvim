use super::keymap::{MAX_COUNT, extract_leading_count};
use super::{
    Action, BoundaryMotion, BracketKind, CountParser, HandleKeyResult, Keymap, LinewiseMotion,
    Mode, ModeKind, Operator, OperatorTarget, QuoteKind, TextObject, TrieKeymap,
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

        trie_keymap.insert_str("h", Action::MoveLeft);
        trie_keymap.insert_str("j", Action::MoveDown);
        trie_keymap.insert_str("k", Action::MoveUp);
        trie_keymap.insert_str("l", Action::MoveRight);

        trie_keymap.insert_str("w", Action::ForwardTo(Boundary::Word));
        trie_keymap.insert_str("b", Action::BackTo(Boundary::Word));
        trie_keymap.insert_str("e", Action::ForwardTo(Boundary::WordEnd));

        trie_keymap.insert_str("W", Action::ForwardTo(Boundary::BigWord));
        trie_keymap.insert_str("B", Action::BackTo(Boundary::BigWord));
        trie_keymap.insert_str("E", Action::ForwardTo(Boundary::BigWordEnd));

        trie_keymap.insert_str("$", Action::MoveToLineEnd);
        trie_keymap.insert_str("0", Action::MoveToLineStart);
        trie_keymap.insert_str("^", Action::MoveToLineContentStart);

        trie_keymap.insert_str("gg", Action::MoveToFirstLine);
        trie_keymap.insert_str("G", Action::MoveToLastLine);
        trie_keymap.insert_str("H", Action::MoveToScreenTop);
        trie_keymap.insert_str("M", Action::MoveToScreenMiddle);
        trie_keymap.insert_str("L", Action::MoveToScreenBottom);
        trie_keymap.insert_str("{", Action::MoveToPreviousParagraph);
        trie_keymap.insert_str("}", Action::MoveToNextParagraph);
        trie_keymap.insert_str("J", Action::JoinWithSpace);
        trie_keymap.insert_str("gJ", Action::JoinWithoutSpace);
        trie_keymap.insert_str("i", Action::SwitchToInsert);
        trie_keymap.insert_str("<C-s>", Action::SaveBuffer(None));
        trie_keymap.insert_str("a", Action::AppendAfterCursor);
        trie_keymap.insert_str("A", Action::AppendToLineEnd);
        trie_keymap.insert_str("I", Action::InsertAtLineStart);
        trie_keymap.insert_str("o", Action::OpenLineBelow);
        trie_keymap.insert_str("O", Action::OpenLineAbove);
        trie_keymap.insert_str("[b", Action::PreviousTab);
        trie_keymap.insert_str("]b", Action::NextTab);
        trie_keymap.insert_str("x", Action::DeleteForward);
        trie_keymap.insert_str("X", Action::DeleteBackward);
        trie_keymap.insert_str("dd", Action::DeleteLine);
        trie_keymap.insert_str(
            "diw",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::InnerWord),
            ),
        );
        trie_keymap.insert_str(
            "daw",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::AroundWord),
            ),
        );
        trie_keymap.insert_str(
            "diW",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::InnerBigWord),
            ),
        );
        trie_keymap.insert_str(
            "daW",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::AroundBigWord),
            ),
        );
        trie_keymap.insert_str(
            "ciW",
            Action::Operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::InnerBigWord),
            ),
        );
        trie_keymap.insert_str(
            "caW",
            Action::Operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::AroundBigWord),
            ),
        );
        for (kind, key) in [
            (QuoteKind::Single, "'"),
            (QuoteKind::Double, "\""),
            (QuoteKind::Backtick, "`"),
        ] {
            trie_keymap.insert_str(
                &format!("di{key}"),
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerQuote(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("da{key}"),
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::AroundQuote(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("ci{key}"),
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::InnerQuote(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("ca{key}"),
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundQuote(kind)),
                ),
            );
        }
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
                &format!("di{open_key}"),
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("di{close_key}"),
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("da{open_key}"),
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("da{close_key}"),
                Action::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("ci{open_key}"),
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("ci{close_key}"),
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("ca{open_key}"),
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("ca{close_key}"),
                Action::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
        }
        trie_keymap.insert_str(
            "dw",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            ),
        );
        trie_keymap.insert_str(
            "de",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
            ),
        );
        trie_keymap.insert_str(
            "db",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
            ),
        );
        trie_keymap.insert_str(
            "d$",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
            ),
        );
        trie_keymap.insert_str(
            "d0",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
            ),
        );
        trie_keymap.insert_str(
            "d^",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
            ),
        );
        trie_keymap.insert_str(
            "dW",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
            ),
        );
        trie_keymap.insert_str(
            "dE",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordEnd),
            ),
        );
        trie_keymap.insert_str(
            "dB",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
            ),
        );
        trie_keymap.insert_str(
            "dgg",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
            ),
        );
        trie_keymap.insert_str(
            "dG",
            Action::Operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
            ),
        );
        trie_keymap.insert_str(
            "cw",
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            ),
        );
        trie_keymap.insert_str(
            "ce",
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
            ),
        );
        trie_keymap.insert_str(
            "cb",
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
            ),
        );
        trie_keymap.insert_str(
            "cW",
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
            ),
        );
        trie_keymap.insert_str(
            "cE",
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordEnd),
            ),
        );
        trie_keymap.insert_str(
            "cB",
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
            ),
        );
        trie_keymap.insert_str(
            "ciw",
            Action::Operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::InnerWord),
            ),
        );
        trie_keymap.insert_str(
            "caw",
            Action::Operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::AroundWord),
            ),
        );
        trie_keymap.insert_str(
            "c$",
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
            ),
        );
        trie_keymap.insert_str(
            "c0",
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
            ),
        );
        trie_keymap.insert_str(
            "c^",
            Action::Operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
            ),
        );
        trie_keymap.insert_str(
            "cgg",
            Action::Operation(
                Operator::Change,
                OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
            ),
        );
        trie_keymap.insert_str(
            "cG",
            Action::Operation(
                Operator::Change,
                OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
            ),
        );
        trie_keymap.insert_str("cc", Action::ChangeLine);
        trie_keymap.insert_str("C", Action::ChangeToLineEnd);
        trie_keymap.insert_str("%", Action::MoveToMatchingBracket);
        trie_keymap.insert_str(";", Action::RepeatLastFind);
        trie_keymap.insert_str(",", Action::RepeatLastFindReverse);
        trie_keymap.insert_str("<C-q>", Action::Quit);
        trie_keymap.insert_str("u", Action::Undo);
        trie_keymap.insert_str("U", Action::Redo);
        trie_keymap.insert_str(".", Action::RepeatLastChange);
        trie_keymap.insert_str("<Left>", Action::MoveLeft);
        trie_keymap.insert_str("<Down>", Action::MoveDown);
        trie_keymap.insert_str("<Up>", Action::MoveUp);
        trie_keymap.insert_str("<Right>", Action::MoveRight);

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
