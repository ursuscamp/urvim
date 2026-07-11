use crate::buffer::Boundary;
use crate::editor::{
    BoundaryMotion, EditorAction, EditorOperation, LinewiseMotion, ModeKind, Operator,
    OperatorTarget, TrieKeymap,
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
        Some(mode) => EditorAction::operation(operator, target).with_to_mode(mode),
        None => EditorAction::operation(operator, target),
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
    trie_keymap.insert_str("h", EditorAction::new(EditorOperation::MoveLeft));
    trie_keymap.insert_str("j", EditorAction::new(EditorOperation::MoveDown));
    trie_keymap.insert_str("k", EditorAction::new(EditorOperation::MoveUp));
    trie_keymap.insert_str("l", EditorAction::new(EditorOperation::MoveRight));
}

fn register_motion_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("w", EditorAction::forward_to(Boundary::Word));
    trie_keymap.insert_str("b", EditorAction::back_to(Boundary::Word));
    trie_keymap.insert_str("e", EditorAction::forward_to(Boundary::WordEnd));

    trie_keymap.insert_str("W", EditorAction::forward_to(Boundary::BigWord));
    trie_keymap.insert_str("B", EditorAction::back_to(Boundary::BigWord));
    trie_keymap.insert_str("E", EditorAction::forward_to(Boundary::BigWordEnd));

    trie_keymap.insert_str("$", EditorAction::new(EditorOperation::MoveToLineEnd));
    trie_keymap.insert_str("0", EditorAction::new(EditorOperation::MoveToLineStart));
    trie_keymap.insert_str(
        "^",
        EditorAction::new(EditorOperation::MoveToLineContentStart),
    );

    trie_keymap.insert_str("gg", EditorAction::new(EditorOperation::MoveToFirstLine));
    trie_keymap.insert_str("G", EditorAction::new(EditorOperation::MoveToLastLine));
    trie_keymap.insert_str("H", EditorAction::new(EditorOperation::MoveToScreenTop));
    trie_keymap.insert_str("M", EditorAction::new(EditorOperation::MoveToScreenMiddle));
    trie_keymap.insert_str("L", EditorAction::new(EditorOperation::MoveToScreenBottom));
    trie_keymap.insert_str("zt", EditorAction::new(EditorOperation::ViewportCursorTop));
    trie_keymap.insert_str(
        "zz",
        EditorAction::new(EditorOperation::ViewportCursorCenter),
    );
    trie_keymap.insert_str(
        "zb",
        EditorAction::new(EditorOperation::ViewportCursorBottom),
    );
    trie_keymap.insert_str("za", EditorAction::new(EditorOperation::ToggleFold));
    trie_keymap.insert_str("zo", EditorAction::new(EditorOperation::OpenFold));
    trie_keymap.insert_str("zc", EditorAction::new(EditorOperation::CloseFold));
    trie_keymap.insert_str(
        "{",
        EditorAction::new(EditorOperation::MoveToPreviousParagraph),
    );
    trie_keymap.insert_str("}", EditorAction::new(EditorOperation::MoveToNextParagraph));
    trie_keymap.insert_str(
        "[h",
        EditorAction::new(EditorOperation::MoveToPreviousDiffHunk),
    );
    trie_keymap.insert_str("]h", EditorAction::new(EditorOperation::MoveToNextDiffHunk));
    trie_keymap.insert_str(
        "[H",
        EditorAction::new(EditorOperation::MoveToPreviousDiffHunkEnd),
    );
    trie_keymap.insert_str(
        "]H",
        EditorAction::new(EditorOperation::MoveToNextDiffHunkEnd),
    );
    trie_keymap.insert_str("J", EditorAction::new(EditorOperation::JoinWithSpace));
    trie_keymap.insert_str("gJ", EditorAction::new(EditorOperation::JoinWithoutSpace));
    trie_keymap.insert_str("gO", Command::OpenDocumentSymbolsPicker);
    trie_keymap.insert_str("grr", Command::LspReferences);
    trie_keymap.insert_str("grS", Command::OpenWorkspaceSymbolsPicker);
}

fn register_character_scan_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("f<Space>", EditorAction::find_forward(' '));
    trie_keymap.insert_str("F<Space>", EditorAction::find_backward(' '));
    trie_keymap.insert_str("t<Space>", EditorAction::till_forward(' '));
    trie_keymap.insert_str("T<Space>", EditorAction::till_backward(' '));
}

fn register_mode_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("i", EditorAction::mode_transition(ModeKind::Insert));
    trie_keymap.insert_str("v", EditorAction::mode_transition(ModeKind::Visual));
    trie_keymap.insert_str("V", EditorAction::mode_transition(ModeKind::VisualLine));
    trie_keymap.insert_str("R", EditorAction::mode_transition(ModeKind::Replace));
    trie_keymap.insert_str("<C-s>", Command::SaveBuffer(None));
    trie_keymap.insert_str(
        "a",
        EditorAction::new(EditorOperation::AppendAfterCursor).with_to_mode(ModeKind::Insert),
    );
    trie_keymap.insert_str(
        "A",
        EditorAction::new(EditorOperation::AppendToLineEnd).with_to_mode(ModeKind::Insert),
    );
    trie_keymap.insert_str(
        "I",
        EditorAction::new(EditorOperation::InsertAtLineStart).with_to_mode(ModeKind::Insert),
    );
    trie_keymap.insert_str(
        "o",
        EditorAction::new(EditorOperation::OpenLineBelow).with_to_mode(ModeKind::Insert),
    );
    trie_keymap.insert_str(
        "O",
        EditorAction::new(EditorOperation::OpenLineAbove).with_to_mode(ModeKind::Insert),
    );
    trie_keymap.insert_str(
        "<LessThan><LessThan>",
        EditorAction::new(EditorOperation::IndentDecrease),
    );
    trie_keymap.insert_str(
        "<GreaterThan><GreaterThan>",
        EditorAction::new(EditorOperation::IndentIncrease),
    );
}

fn register_window_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("<C-o>", EditorAction::jump_backward());
    trie_keymap.insert_str("<C-i>", EditorAction::jump_forward());
    trie_keymap.insert_str("<C-w>v", Command::SplitVertical);
    trie_keymap.insert_str("<C-w>s", Command::SplitHorizontal);
    trie_keymap.insert_str("<C-w>h", Command::FocusPaneLeft);
    trie_keymap.insert_str("<C-w>j", Command::FocusPaneDown);
    trie_keymap.insert_str("<C-w>k", Command::FocusPaneUp);
    trie_keymap.insert_str("<C-w>l", Command::FocusPaneRight);
    trie_keymap.insert_str("<C-w>n", Command::FocusNextWindow);
    trie_keymap.insert_str("<C-w>p", Command::FocusPreviousWindow);
    trie_keymap.insert_str("<C-w>q", Command::ClosePane);
    trie_keymap.insert_str("<C-w>=", Command::EqualizeSplits);
    trie_keymap.insert_str("<C-w>w", Command::ToggleWrap);
    trie_keymap.insert_str("<C-w>r", EditorAction::mode_transition(ModeKind::Resizing));
}

fn register_edit_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("gcc", EditorAction::toggle_line_comment());
    trie_keymap.insert_str("[d", Command::LspPreviousDiagnostic);
    trie_keymap.insert_str("]d", Command::LspNextDiagnostic);
    trie_keymap.insert_str("[e", Command::LspPreviousErrorDiagnostic);
    trie_keymap.insert_str("]e", Command::LspNextErrorDiagnostic);
    trie_keymap.insert_str("[b", Command::PreviousTab(1));
    trie_keymap.insert_str("]b", Command::NextTab(1));
    trie_keymap.insert_str("x", EditorAction::new(EditorOperation::DeleteForward));
    trie_keymap.insert_str("X", EditorAction::new(EditorOperation::DeleteBackward));
    trie_keymap.insert_str("dd", EditorAction::new(EditorOperation::DeleteLine));
    trie_keymap.insert_str("yy", EditorAction::new(EditorOperation::YankLine));
    trie_keymap.insert_str("p", EditorAction::new(EditorOperation::PasteAfter));
    trie_keymap.insert_str("P", EditorAction::new(EditorOperation::PasteBefore));
    trie_keymap.insert_str(
        "cc",
        EditorAction::new(EditorOperation::ChangeLine).with_to_mode(ModeKind::Insert),
    );
    trie_keymap.insert_str(
        "C",
        EditorAction::new(EditorOperation::ChangeToLineEnd).with_to_mode(ModeKind::Insert),
    );
}

fn register_operator_bindings(trie_keymap: &mut TrieKeymap) {
    register_operator_family_bindings(trie_keymap, "d", Operator::Delete, None);
    register_operator_family_bindings(trie_keymap, "y", Operator::Yank, None);
    register_operator_family_bindings(trie_keymap, "c", Operator::Change, Some(ModeKind::Insert));
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

fn register_case_operator_bindings(trie_keymap: &mut TrieKeymap) {
    let sequences = operator_sequences(None);
    insert_operator_sequences(trie_keymap, "gu", Operator::Lowercase, &sequences);
    insert_operator_sequences(trie_keymap, "gU", Operator::Uppercase, &sequences);
    insert_operator_sequences(trie_keymap, "g~", Operator::ToggleCase, &sequences);
}

fn register_misc_bindings(trie_keymap: &mut TrieKeymap) {
    trie_keymap.insert_str("<F1>", Command::OpenFilePicker);
    trie_keymap.insert_str("<F2>", Command::OpenGrepPicker);
    trie_keymap.insert_str("<F3>", Command::OpenBufferPicker);
    trie_keymap.insert_str("<F4>", Command::OpenGitPicker);
    trie_keymap.insert_str("<F5>", Command::OpenColorschemePicker);
    trie_keymap.insert_str("<F6>", Command::OpenFiletypePicker);
    trie_keymap.insert_str("K", Command::LspHover);
    trie_keymap.insert_str("gd", Command::LspDefinition);
    trie_keymap.insert_str("gra", Command::LspCodeActions);
    trie_keymap.insert_str("grn", Command::LspRenamePrompt);
    trie_keymap.insert_str(
        "%",
        EditorAction::new(EditorOperation::MoveToMatchingBracket),
    );
    trie_keymap.insert_str(";", EditorAction::new(EditorOperation::RepeatLastFind));
    trie_keymap.insert_str(
        ",",
        EditorAction::new(EditorOperation::RepeatLastFindReverse),
    );
    trie_keymap.insert_str("<C-q>", Command::TryQuit);
    trie_keymap.insert_str("u", EditorAction::new(EditorOperation::Undo));
    trie_keymap.insert_str("U", EditorAction::new(EditorOperation::Redo));
    trie_keymap.insert_str(".", EditorAction::new(EditorOperation::RepeatLastChange));
    trie_keymap.insert_str(":", Command::OpenCommandLine);
    trie_keymap.insert_str("<Left>", EditorAction::new(EditorOperation::MoveLeft));
    trie_keymap.insert_str("<Down>", EditorAction::new(EditorOperation::MoveDown));
    trie_keymap.insert_str("<Up>", EditorAction::new(EditorOperation::MoveUp));
    trie_keymap.insert_str("<Right>", EditorAction::new(EditorOperation::MoveRight));
    trie_keymap.insert_str("<PageUp>", EditorAction::new(EditorOperation::MovePageUp));
    trie_keymap.insert_str(
        "<PageDown>",
        EditorAction::new(EditorOperation::MovePageDown),
    );
    trie_keymap.insert_str("<C-u>", EditorAction::new(EditorOperation::MoveHalfPageUp));
    trie_keymap.insert_str(
        "<C-d>",
        EditorAction::new(EditorOperation::MoveHalfPageDown),
    );
}

fn operator_sequences(to_mode: Option<ModeKind>) -> [OperatorSequenceSpec; 11] {
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
    ]
}
