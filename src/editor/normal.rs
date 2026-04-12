use super::keymap::{MAX_COUNT, extract_leading_count};
use super::{
    Action, BoundaryMotion, BracketKind, CountParser, HandleKeyResult, Keymap, LinewiseMotion,
    Mode, ModeKind, Operator, OperatorTarget, QuoteKind, TextObject, TrieKeymap,
};
use crate::buffer::Boundary;
use crate::editor::ActionKind;
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

        trie_keymap.insert_str("$", Action::new(ActionKind::MoveToLineEnd));
        trie_keymap.insert_str("0", Action::new(ActionKind::MoveToLineStart));
        trie_keymap.insert_str("^", Action::new(ActionKind::MoveToLineContentStart));

        trie_keymap.insert_str("gg", Action::new(ActionKind::MoveToFirstLine));
        trie_keymap.insert_str("G", Action::new(ActionKind::MoveToLastLine));
        trie_keymap.insert_str("H", Action::new(ActionKind::MoveToScreenTop));
        trie_keymap.insert_str("M", Action::new(ActionKind::MoveToScreenMiddle));
        trie_keymap.insert_str("L", Action::new(ActionKind::MoveToScreenBottom));
        trie_keymap.insert_str("{", Action::new(ActionKind::MoveToPreviousParagraph));
        trie_keymap.insert_str("}", Action::new(ActionKind::MoveToNextParagraph));
        trie_keymap.insert_str("J", Action::new(ActionKind::JoinWithSpace));
        trie_keymap.insert_str("gJ", Action::new(ActionKind::JoinWithoutSpace));
        trie_keymap.insert_str("i", Action::mode_transition(ModeKind::Insert));
        trie_keymap.insert_str("<C-s>", Action::save_buffer(None));
        trie_keymap.insert_str(
            "a",
            Action::new(ActionKind::AppendAfterCursor).with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "A",
            Action::new(ActionKind::AppendToLineEnd).with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "I",
            Action::new(ActionKind::InsertAtLineStart).with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "o",
            Action::new(ActionKind::OpenLineBelow).with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "O",
            Action::new(ActionKind::OpenLineAbove).with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "<LessThan><LessThan>",
            Action::new(ActionKind::IndentDecrease),
        );
        trie_keymap.insert_str(
            "<GreaterThan><GreaterThan>",
            Action::new(ActionKind::IndentIncrease),
        );
        trie_keymap.insert_str("gcc", Action::toggle_line_comment());
        trie_keymap.insert_str("[b", Action::new(ActionKind::PreviousTab));
        trie_keymap.insert_str("]b", Action::new(ActionKind::NextTab));
        trie_keymap.insert_str("x", Action::new(ActionKind::DeleteForward));
        trie_keymap.insert_str("X", Action::new(ActionKind::DeleteBackward));
        trie_keymap.insert_str("dd", Action::new(ActionKind::DeleteLine));
        trie_keymap.insert_str(
            "diw",
            Action::operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::InnerWord),
            ),
        );
        trie_keymap.insert_str(
            "daw",
            Action::operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::AroundWord),
            ),
        );
        trie_keymap.insert_str(
            "diW",
            Action::operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::InnerBigWord),
            ),
        );
        trie_keymap.insert_str(
            "daW",
            Action::operation(
                Operator::Delete,
                OperatorTarget::TextObject(TextObject::AroundBigWord),
            ),
        );
        trie_keymap.insert_str(
            "ciW",
            Action::operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::InnerBigWord),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "caW",
            Action::operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::AroundBigWord),
            )
            .with_to_mode(ModeKind::Insert),
        );
        for (kind, key) in [
            (QuoteKind::Single, "'"),
            (QuoteKind::Double, "\""),
            (QuoteKind::Backtick, "`"),
        ] {
            trie_keymap.insert_str(
                &format!("di{key}"),
                Action::operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerQuote(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("da{key}"),
                Action::operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::AroundQuote(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("ci{key}"),
                Action::operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::InnerQuote(kind)),
                )
                .with_to_mode(ModeKind::Insert),
            );
            trie_keymap.insert_str(
                &format!("ca{key}"),
                Action::operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundQuote(kind)),
                )
                .with_to_mode(ModeKind::Insert),
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
                Action::operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("di{close_key}"),
                Action::operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("da{open_key}"),
                Action::operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("da{close_key}"),
                Action::operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                ),
            );
            trie_keymap.insert_str(
                &format!("ci{open_key}"),
                Action::operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                )
                .with_to_mode(ModeKind::Insert),
            );
            trie_keymap.insert_str(
                &format!("ci{close_key}"),
                Action::operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
                )
                .with_to_mode(ModeKind::Insert),
            );
            trie_keymap.insert_str(
                &format!("ca{open_key}"),
                Action::operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                )
                .with_to_mode(ModeKind::Insert),
            );
            trie_keymap.insert_str(
                &format!("ca{close_key}"),
                Action::operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
                )
                .with_to_mode(ModeKind::Insert),
            );
        }
        trie_keymap.insert_str(
            "dw",
            Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            ),
        );
        trie_keymap.insert_str(
            "de",
            Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
            ),
        );
        trie_keymap.insert_str(
            "db",
            Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
            ),
        );
        trie_keymap.insert_str(
            "d$",
            Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
            ),
        );
        trie_keymap.insert_str(
            "d0",
            Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
            ),
        );
        trie_keymap.insert_str(
            "d^",
            Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
            ),
        );
        trie_keymap.insert_str(
            "dW",
            Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
            ),
        );
        trie_keymap.insert_str(
            "dE",
            Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordEnd),
            ),
        );
        trie_keymap.insert_str(
            "dB",
            Action::operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
            ),
        );
        trie_keymap.insert_str(
            "dgg",
            Action::operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
            ),
        );
        trie_keymap.insert_str(
            "dG",
            Action::operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
            ),
        );
        trie_keymap.insert_str(
            "cw",
            Action::operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "ce",
            Action::operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "cb",
            Action::operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "cW",
            Action::operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "cE",
            Action::operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordEnd),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "cB",
            Action::operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "ciw",
            Action::operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::InnerWord),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "caw",
            Action::operation(
                Operator::Change,
                OperatorTarget::TextObject(TextObject::AroundWord),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "c$",
            Action::operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "c0",
            Action::operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "c^",
            Action::operation(
                Operator::Change,
                OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "cgg",
            Action::operation(
                Operator::Change,
                OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "cG",
            Action::operation(
                Operator::Change,
                OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
            )
            .with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "cc",
            Action::new(ActionKind::ChangeLine).with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str(
            "C",
            Action::new(ActionKind::ChangeToLineEnd).with_to_mode(ModeKind::Insert),
        );
        trie_keymap.insert_str("%", Action::new(ActionKind::MoveToMatchingBracket));
        trie_keymap.insert_str(";", Action::new(ActionKind::RepeatLastFind));
        trie_keymap.insert_str(",", Action::new(ActionKind::RepeatLastFindReverse));
        trie_keymap.insert_str("<C-q>", Action::new(ActionKind::Quit));
        trie_keymap.insert_str("u", Action::new(ActionKind::Undo));
        trie_keymap.insert_str("U", Action::new(ActionKind::Redo));
        trie_keymap.insert_str(".", Action::new(ActionKind::RepeatLastChange));
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
                    return HandleKeyResult::Complete(
                        counted_action.with_from_mode(ModeKind::Normal),
                    );
                }
                return HandleKeyResult::Complete(action.with_from_mode(ModeKind::Normal));
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
                return HandleKeyResult::Complete(counted_action.with_from_mode(ModeKind::Normal));
            }
            return HandleKeyResult::Complete(action.with_from_mode(ModeKind::Normal));
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
