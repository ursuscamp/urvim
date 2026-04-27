use crate::buffer::Boundary;
use crate::editor::surround;
use crate::editor::{
    Action, ActionKind, BoundaryMotion, BracketKind, LinewiseMotion, ModeKind, Operator,
    OperatorTarget, QuoteKind, TextObject, TrieKeymap,
};
use crate::ui::Command;

#[derive(Clone, Copy)]
struct OperatorSequenceSpec {
    suffix: &'static str,
    target: OperatorTarget,
    to_mode: Option<ModeKind>,
}

pub(super) fn register(trie_keymap: &mut TrieKeymap) {
    register_cursor_bindings(trie_keymap);
    register_motion_bindings(trie_keymap);
    register_character_scan_bindings(trie_keymap);
    register_mode_bindings(trie_keymap);
    register_window_bindings(trie_keymap);
    register_edit_bindings(trie_keymap);
    register_operator_bindings(trie_keymap);
    register_misc_bindings(trie_keymap);
}

fn insert_operator_sequence(
    trie_keymap: &mut TrieKeymap,
    sequence: String,
    operator: Operator,
    target: OperatorTarget,
    to_mode: Option<ModeKind>,
) {
    let action = match to_mode {
        Some(mode) => Action::operation(operator, target).with_to_mode(mode),
        None => Action::operation(operator, target),
    };
    trie_keymap.insert_str(&sequence, action);
}

fn insert_operator_sequences(
    trie_keymap: &mut TrieKeymap,
    prefix: &str,
    operator: Operator,
    sequences: &[OperatorSequenceSpec],
) {
    for spec in sequences {
        insert_operator_sequence(
            trie_keymap,
            format!("{prefix}{}", spec.suffix),
            operator,
            spec.target,
            spec.to_mode,
        );
    }
}

fn register_cursor_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("h", Action::new(ActionKind::MoveLeft));
    trie_keymap.insert_str("j", Action::new(ActionKind::MoveDown));
    trie_keymap.insert_str("k", Action::new(ActionKind::MoveUp));
    trie_keymap.insert_str("l", Action::new(ActionKind::MoveRight));
}

fn register_motion_bindings(trie_keymap: &mut TrieKeymap) {
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
    trie_keymap.insert_str("zt", Action::new(ActionKind::ViewportCursorTop));
    trie_keymap.insert_str("zz", Action::new(ActionKind::ViewportCursorCenter));
    trie_keymap.insert_str("zb", Action::new(ActionKind::ViewportCursorBottom));
    trie_keymap.insert_str("{", Action::new(ActionKind::MoveToPreviousParagraph));
    trie_keymap.insert_str("}", Action::new(ActionKind::MoveToNextParagraph));
    trie_keymap.insert_str("J", Action::new(ActionKind::JoinWithSpace));
    trie_keymap.insert_str("gJ", Action::new(ActionKind::JoinWithoutSpace));
}

fn register_character_scan_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("f<Space>", Action::find_forward(' '));
    trie_keymap.insert_str("F<Space>", Action::find_backward(' '));
    trie_keymap.insert_str("t<Space>", Action::till_forward(' '));
    trie_keymap.insert_str("T<Space>", Action::till_backward(' '));
}

fn register_mode_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("i", Action::mode_transition(ModeKind::Insert));
    trie_keymap.insert_str("v", Action::mode_transition(ModeKind::Visual));
    trie_keymap.insert_str("V", Action::mode_transition(ModeKind::VisualLine));
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
}

fn register_window_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("<C-o>", Action::jump_backward());
    trie_keymap.insert_str("<C-i>", Action::jump_forward());
    trie_keymap.insert_str("<C-w>v", Command::SplitVertical);
    trie_keymap.insert_str("<C-w>s", Command::SplitHorizontal);
    trie_keymap.insert_str("<C-w>h", Command::FocusPaneLeft);
    trie_keymap.insert_str("<C-w>j", Command::FocusPaneDown);
    trie_keymap.insert_str("<C-w>k", Command::FocusPaneUp);
    trie_keymap.insert_str("<C-w>l", Command::FocusPaneRight);
    trie_keymap.insert_str("<C-w>q", Command::ClosePane);
    trie_keymap.insert_str("<C-w>=", Command::EqualizeSplits);
    trie_keymap.insert_str("<C-w>w", Command::ToggleWrap);
    trie_keymap.insert_str("<C-w>r", Action::mode_transition(ModeKind::Resizing));
}

fn register_edit_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("gcc", Action::toggle_line_comment());
    trie_keymap.insert_str("[b", Action::new(ActionKind::PreviousTab));
    trie_keymap.insert_str("]b", Action::new(ActionKind::NextTab));
    trie_keymap.insert_str("x", Action::new(ActionKind::DeleteForward));
    trie_keymap.insert_str("X", Action::new(ActionKind::DeleteBackward));
    trie_keymap.insert_str("dd", Action::new(ActionKind::DeleteLine));
    trie_keymap.insert_str("yy", Action::new(ActionKind::YankLine));
    trie_keymap.insert_str("p", Action::new(ActionKind::PasteAfter));
    trie_keymap.insert_str("P", Action::new(ActionKind::PasteBefore));
    trie_keymap.insert_str(
        "cc",
        Action::new(ActionKind::ChangeLine).with_to_mode(ModeKind::Insert),
    );
    trie_keymap.insert_str(
        "C",
        Action::new(ActionKind::ChangeToLineEnd).with_to_mode(ModeKind::Insert),
    );
    register_surround_bindings(trie_keymap);
}

fn register_surround_bindings(trie_keymap: &mut TrieKeymap) {
    let selectors = surround::delimiter_selectors();

    for (selector, target) in selectors {
        trie_keymap.insert_str(
            &format!("gsd{selector}"),
            Action::new(ActionKind::SurroundDelete { target: *target }),
        );
    }

    for (target_selector, target) in selectors {
        for (replacement_selector, replacement) in selectors {
            trie_keymap.insert_str(
                &format!("gsr{target_selector}{replacement_selector}"),
                Action::new(ActionKind::SurroundReplace {
                    target: *target,
                    replacement: *replacement,
                }),
            );
        }
    }

    for (target_sequence, target) in surround::text_object_sequences() {
        for (delimiter_selector, delimiter) in selectors {
            trie_keymap.insert_str(
                &format!("gsa{target_sequence}{delimiter_selector}"),
                Action::new(ActionKind::SurroundAdd {
                    target: *target,
                    delimiter: *delimiter,
                }),
            );
        }
    }
}

fn register_operator_bindings(trie_keymap: &mut TrieKeymap) {
    register_operator_family_bindings(trie_keymap, "d", Operator::Delete, None);
    register_operator_family_bindings(trie_keymap, "y", Operator::Yank, None);
    register_operator_family_bindings(trie_keymap, "c", Operator::Change, Some(ModeKind::Insert));
    register_text_object_operator_bindings(trie_keymap);
    register_case_operator_bindings(trie_keymap);
}

fn register_operator_family_bindings(
    trie_keymap: &mut TrieKeymap,
    prefix: &str,
    operator: Operator,
    to_mode: Option<ModeKind>,
) {
    insert_operator_sequences(trie_keymap, prefix, operator, &operator_sequences(to_mode));
}

fn register_text_object_operator_bindings(trie_keymap: &mut TrieKeymap) {
    register_quote_operator_bindings(trie_keymap);
    register_bracket_operator_bindings(trie_keymap);
}

fn register_quote_operator_bindings(trie_keymap: &mut TrieKeymap) {
    for (kind, key) in [
        (QuoteKind::Single, "'"),
        (QuoteKind::Double, "\""),
        (QuoteKind::Backtick, "`"),
    ] {
        insert_operator_sequence(
            trie_keymap,
            format!("di{key}"),
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::InnerQuote(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("da{key}"),
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::AroundQuote(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("ci{key}"),
            Operator::Change,
            OperatorTarget::TextObject(TextObject::InnerQuote(kind)),
            Some(ModeKind::Insert),
        );
        insert_operator_sequence(
            trie_keymap,
            format!("ca{key}"),
            Operator::Change,
            OperatorTarget::TextObject(TextObject::AroundQuote(kind)),
            Some(ModeKind::Insert),
        );
        insert_operator_sequence(
            trie_keymap,
            format!("yi{key}"),
            Operator::Yank,
            OperatorTarget::TextObject(TextObject::InnerQuote(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("ya{key}"),
            Operator::Yank,
            OperatorTarget::TextObject(TextObject::AroundQuote(kind)),
            None,
        );
    }
}

fn register_bracket_operator_bindings(trie_keymap: &mut TrieKeymap) {
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
        insert_operator_sequence(
            trie_keymap,
            format!("di{open_key}"),
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("di{close_key}"),
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("da{open_key}"),
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("da{close_key}"),
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("ci{open_key}"),
            Operator::Change,
            OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
            Some(ModeKind::Insert),
        );
        insert_operator_sequence(
            trie_keymap,
            format!("ci{close_key}"),
            Operator::Change,
            OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
            Some(ModeKind::Insert),
        );
        insert_operator_sequence(
            trie_keymap,
            format!("ca{open_key}"),
            Operator::Change,
            OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
            Some(ModeKind::Insert),
        );
        insert_operator_sequence(
            trie_keymap,
            format!("ca{close_key}"),
            Operator::Change,
            OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
            Some(ModeKind::Insert),
        );
        insert_operator_sequence(
            trie_keymap,
            format!("yi{open_key}"),
            Operator::Yank,
            OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("yi{close_key}"),
            Operator::Yank,
            OperatorTarget::TextObject(TextObject::InnerBracket(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("ya{open_key}"),
            Operator::Yank,
            OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
            None,
        );
        insert_operator_sequence(
            trie_keymap,
            format!("ya{close_key}"),
            Operator::Yank,
            OperatorTarget::TextObject(TextObject::AroundBracket(kind)),
            None,
        );
    }
}

fn register_case_operator_bindings(trie_keymap: &mut TrieKeymap) {
    let sequences = operator_sequences(None);
    insert_operator_sequences(trie_keymap, "gu", Operator::Lowercase, &sequences);
    insert_operator_sequences(trie_keymap, "gU", Operator::Uppercase, &sequences);
    insert_operator_sequences(trie_keymap, "g~", Operator::ToggleCase, &sequences);
}

fn register_misc_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("<F1>", Command::OpenFilePicker);
    trie_keymap.insert_str("%", Action::new(ActionKind::MoveToMatchingBracket));
    trie_keymap.insert_str(";", Action::new(ActionKind::RepeatLastFind));
    trie_keymap.insert_str(",", Action::new(ActionKind::RepeatLastFindReverse));
    trie_keymap.insert_str("<C-q>", Command::TryQuit);
    trie_keymap.insert_str("u", Action::new(ActionKind::Undo));
    trie_keymap.insert_str("U", Action::new(ActionKind::Redo));
    trie_keymap.insert_str(".", Action::new(ActionKind::RepeatLastChange));
    trie_keymap.insert_str(":", Command::OpenCommandLine);
    trie_keymap.insert_str("<Left>", Action::new(ActionKind::MoveLeft));
    trie_keymap.insert_str("<Down>", Action::new(ActionKind::MoveDown));
    trie_keymap.insert_str("<Up>", Action::new(ActionKind::MoveUp));
    trie_keymap.insert_str("<Right>", Action::new(ActionKind::MoveRight));
    trie_keymap.insert_str("<PageUp>", Action::new(ActionKind::MovePageUp));
    trie_keymap.insert_str("<PageDown>", Action::new(ActionKind::MovePageDown));
    trie_keymap.insert_str("<C-u>", Action::new(ActionKind::MoveHalfPageUp));
    trie_keymap.insert_str("<C-d>", Action::new(ActionKind::MoveHalfPageDown));
}

fn operator_sequences(to_mode: Option<ModeKind>) -> [OperatorSequenceSpec; 15] {
    [
        OperatorSequenceSpec {
            suffix: "w",
            target: OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "e",
            target: OperatorTarget::BoundaryMotion(BoundaryMotion::WordEnd),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "b",
            target: OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "W",
            target: OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "E",
            target: OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordEnd),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "B",
            target: OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "$",
            target: OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "0",
            target: OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "^",
            target: OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "gg",
            target: OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "G",
            target: OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "iw",
            target: OperatorTarget::TextObject(TextObject::InnerWord),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "aw",
            target: OperatorTarget::TextObject(TextObject::AroundWord),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "iW",
            target: OperatorTarget::TextObject(TextObject::InnerBigWord),
            to_mode,
        },
        OperatorSequenceSpec {
            suffix: "aW",
            target: OperatorTarget::TextObject(TextObject::AroundBigWord),
            to_mode,
        },
    ]
}
