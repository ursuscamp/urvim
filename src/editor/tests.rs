use super::*;
use crate::config::Config;
use crate::globals::set_test_config;
use crate::editor::ActionKind;
use crate::terminal::Key;

fn key(c: char) -> Key {
    Key::new(crate::terminal::KeyCode::Char(c))
}

fn handle_and_unwrap(mode: &mut impl Mode, k: &Key) -> Action {
    match mode.handle_key(k) {
        HandleKeyResult::Complete(action) => {
            let to_mode = action.to_mode;
            action.with_mode(None, to_mode)
        }
        HandleKeyResult::WaitForMore => Action::none(),
        HandleKeyResult::InvalidSequence => Action::none(),
    }
}

fn complete_action_kind(mode_result: HandleKeyResult) -> ActionKind {
    match mode_result {
        HandleKeyResult::Complete(action) => action.kind.expect("expected a complete action"),
        HandleKeyResult::WaitForMore => panic!("expected a complete action, got wait"),
        HandleKeyResult::InvalidSequence => panic!("expected a complete action, got invalid"),
    }
}

fn configured_test_config(insert_escape: Option<&str>) -> Config {
    Config {
        theme: "test-theme".to_string(),
        insert_escape: insert_escape.map(str::to_owned),
        syntax: true,
        auto_close_pairs: true,
    }
}

fn configured_test_config_with_pairs(
    insert_escape: Option<&str>,
    auto_close_pairs: bool,
) -> Config {
    Config {
        theme: "test-theme".to_string(),
        insert_escape: insert_escape.map(str::to_owned),
        syntax: true,
        auto_close_pairs,
    }
}

#[test]
fn test_normal_mode_move_left() {
    let mut mode = NormalMode::new();
    assert_eq!(handle_and_unwrap(&mut mode, &key('h')), Action::new(ActionKind::MoveLeft));
}

#[test]
fn test_normal_mode_dot_repeat_action() {
    let mut mode = NormalMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('.')),
        Action::new(ActionKind::RepeatLastChange)
    );
}

#[test]
fn test_mode_kind_reflects_mode_type() {
    let normal = NormalMode::new();
    let insert = InsertMode::new();

    assert_eq!(normal.kind(), ModeKind::Normal);
    assert_eq!(insert.kind(), ModeKind::Insert);
}

#[test]
fn test_insert_mode_escape_switches_to_normal() {
    let mut mode = InsertMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(crate::terminal::KeyCode::Esc)),
        Action::mode_transition(ModeKind::Normal)
    );
}

#[test]
fn test_insert_mode_configured_escape_binding_switches_to_normal() {
    let _guard = set_test_config(configured_test_config(Some("jk")));
    let mut mode = InsertMode::new();

    assert!(matches!(
        mode.handle_key(&key('j')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('k')),
        Action::mode_transition(ModeKind::Normal)
    );
    assert_eq!(mode.take_repeat_text(), None);
}

#[test]
fn test_insert_mode_configured_escape_binding_keeps_builtin_escape() {
    let _guard = set_test_config(configured_test_config(Some("jk")));
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(crate::terminal::KeyCode::Esc)),
        Action::mode_transition(ModeKind::Normal)
    );
}

#[test]
fn test_configured_escape_binding_does_not_affect_normal_mode() {
    let _guard = set_test_config(configured_test_config(Some("jk")));
    let mut mode = NormalMode::new();

    assert_eq!(handle_and_unwrap(&mut mode, &key('j')), Action::new(ActionKind::MoveDown));
}

#[test]
fn test_insert_mode_partial_escape_sequence_falls_back_to_literal_text() {
    let _guard = set_test_config(configured_test_config(Some("jk")));
    let mut mode = InsertMode::new();

    assert!(matches!(
        mode.handle_key(&key('j')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('j')),
        Action::insert_text("jj".to_string())
    );
    assert_eq!(mode.take_repeat_text().as_deref(), Some("jj"));
}

#[test]
fn test_insert_mode_partial_escape_sequence_keeps_following_text() {
    let _guard = set_test_config(configured_test_config(Some("jk")));
    let mut mode = InsertMode::new();

    assert!(matches!(
        mode.handle_key(&key('j')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('u')),
        Action::insert_text("ju".to_string())
    );
    assert_eq!(mode.take_repeat_text().as_deref(), Some("ju"));
}

#[test]
fn test_insert_mode_emits_pair_action_for_supported_opener() {
    let _guard = set_test_config(configured_test_config(None));
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('(')),
        Action::insert_char('(')
    );
}

#[test]
fn test_insert_mode_emits_skip_closer_action_for_supported_closer() {
    let _guard = set_test_config(configured_test_config(None));
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key(')')),
        Action::insert_char(')')
    );
}

#[test]
fn test_insert_mode_emits_skip_action_for_quote_closer() {
    let _guard = set_test_config(configured_test_config(None));
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('"')),
        Action::insert_char('"')
    );
}

#[test]
fn test_insert_mode_disabled_auto_close_keeps_plain_insertion() {
    let _guard = set_test_config(configured_test_config_with_pairs(None, false));
    let mut mode = InsertMode::new();

    assert_eq!(handle_and_unwrap(&mut mode, &key('(')), Action::insert_char('('));
}

#[test]
fn test_insert_mode_captures_repeat_text() {
    let mut mode = InsertMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('h')),
        Action::insert_char('h')
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('i')),
        Action::insert_char('i')
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(crate::terminal::KeyCode::Esc)),
        Action::mode_transition(ModeKind::Normal)
    );

    assert_eq!(mode.take_repeat_text().as_deref(), Some("hi"));
    assert_eq!(mode.take_repeat_text(), None);
}

#[test]
fn test_insert_mode_captured_repeat_text_tracks_backspace() {
    let mut mode = InsertMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('h')),
        Action::insert_char('h')
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('i')),
        Action::insert_char('i')
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(crate::terminal::KeyCode::Backspace)),
        Action::new(ActionKind::DeleteBackward)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(crate::terminal::KeyCode::Esc)),
        Action::mode_transition(ModeKind::Normal)
    );

    assert_eq!(mode.take_repeat_text().as_deref(), Some("h"));
}

#[test]
fn test_gg_motion() {
    let mut mode = NormalMode::new();
    let result = mode.handle_key(&key('g'));
    assert!(matches!(result, HandleKeyResult::WaitForMore));
    let result = mode.handle_key(&key('g'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::MoveToFirstLine
    ));
}

#[test]
fn test_count_diw() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('3')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('i')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('w'));
    assert!(matches!(complete_action_kind(result), ActionKind::Count(3, _)));
}

#[test]
fn test_dw_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('w'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )
    ));
}

#[test]
fn test_cw_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('c')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('w'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Change,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )
    ));
}

#[test]
fn test_ciw_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('c')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('i')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('w'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Change,
            OperatorTarget::TextObject(TextObject::InnerWord),
        )
    ));
}

#[test]
fn test_diw_capital_w_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('i')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('W'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::InnerBigWord),
        )
    ));
}

#[test]
fn test_daw_capital_w_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('W'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::AroundBigWord),
        )
    ));
}

#[test]
fn test_ciw_capital_w_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('c')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('i')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('W'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Change,
            OperatorTarget::TextObject(TextObject::InnerBigWord),
        )
    ));
}

#[test]
fn test_caw_capital_w_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('c')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('W'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Change,
            OperatorTarget::TextObject(TextObject::AroundBigWord),
        )
    ));
}

#[test]
fn test_bracket_text_object_sequences() {
    let cases = [
        (
            '(',
            BracketKind::Paren,
            TextObject::InnerBracket(BracketKind::Paren),
        ),
        (
            ')',
            BracketKind::Paren,
            TextObject::InnerBracket(BracketKind::Paren),
        ),
        (
            '[',
            BracketKind::Square,
            TextObject::InnerBracket(BracketKind::Square),
        ),
        (
            ']',
            BracketKind::Square,
            TextObject::InnerBracket(BracketKind::Square),
        ),
        (
            '{',
            BracketKind::Curly,
            TextObject::InnerBracket(BracketKind::Curly),
        ),
        (
            '}',
            BracketKind::Curly,
            TextObject::InnerBracket(BracketKind::Curly),
        ),
        (
            '<',
            BracketKind::Angle,
            TextObject::InnerBracket(BracketKind::Angle),
        ),
        (
            '>',
            BracketKind::Angle,
            TextObject::InnerBracket(BracketKind::Angle),
        ),
    ];

    for (delimiter, kind, _) in cases {
        let mut mode = NormalMode::new();
        assert!(matches!(
            mode.handle_key(&key('d')),
            HandleKeyResult::WaitForMore
        ));
        assert!(matches!(
            mode.handle_key(&key('i')),
            HandleKeyResult::WaitForMore
        ));
        let result = mode.handle_key(&key(delimiter));
        match result {
            HandleKeyResult::Complete(action) => match action.kind.as_ref() {
                Some(ActionKind::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerBracket(actual)),
                )) => assert_eq!(*actual, kind),
                other => panic!("unexpected result: {other:?}"),
            },
            other => panic!("unexpected result: {other:?}"),
        }
    }

    let around_cases = [
        ('(', BracketKind::Paren),
        ('[', BracketKind::Square),
        ('{', BracketKind::Curly),
        ('<', BracketKind::Angle),
    ];

    for (delimiter, kind) in around_cases {
        let mut mode = NormalMode::new();
        assert!(matches!(
            mode.handle_key(&key('c')),
            HandleKeyResult::WaitForMore
        ));
        assert!(matches!(
            mode.handle_key(&key('a')),
            HandleKeyResult::WaitForMore
        ));
        let result = mode.handle_key(&key(delimiter));
        match result {
            HandleKeyResult::Complete(action) => match action.kind.as_ref() {
                Some(ActionKind::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundBracket(actual)),
                )) => assert_eq!(*actual, kind),
                other => panic!("unexpected result: {other:?}"),
            },
            other => panic!("unexpected result: {other:?}"),
        }
    }
}

#[test]
fn test_quote_text_object_sequences() {
    let cases = [
        ('\'', QuoteKind::Single),
        ('"', QuoteKind::Double),
        ('`', QuoteKind::Backtick),
    ];

    for (delimiter, kind) in cases {
        let mut mode = NormalMode::new();
        assert!(matches!(
            mode.handle_key(&key('d')),
            HandleKeyResult::WaitForMore
        ));
        assert!(matches!(
            mode.handle_key(&key('i')),
            HandleKeyResult::WaitForMore
        ));
        let result = mode.handle_key(&key(delimiter));
        match result {
            HandleKeyResult::Complete(action) => match action.kind.as_ref() {
                Some(ActionKind::Operation(
                    Operator::Delete,
                    OperatorTarget::TextObject(TextObject::InnerQuote(actual)),
                )) => assert_eq!(*actual, kind),
                other => panic!("unexpected result: {other:?}"),
            },
            other => panic!("unexpected result: {other:?}"),
        }
    }

    for (delimiter, kind) in cases {
        let mut mode = NormalMode::new();
        assert!(matches!(
            mode.handle_key(&key('c')),
            HandleKeyResult::WaitForMore
        ));
        assert!(matches!(
            mode.handle_key(&key('a')),
            HandleKeyResult::WaitForMore
        ));
        let result = mode.handle_key(&key(delimiter));
        match result {
            HandleKeyResult::Complete(action) => match action.kind.as_ref() {
                Some(ActionKind::Operation(
                    Operator::Change,
                    OperatorTarget::TextObject(TextObject::AroundQuote(actual)),
                )) => assert_eq!(*actual, kind),
                other => panic!("unexpected result: {other:?}"),
            },
            other => panic!("unexpected result: {other:?}"),
        }
    }
}

#[test]
fn test_cgg_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('c')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('g'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Change,
            OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
        )
    ));
}

#[test]
fn test_dollar_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('$'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
        )
    ));
}

#[test]
fn test_d0_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('0'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
        )
    ));
}

#[test]
fn test_dcaret_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('^'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
        )
    ));
}

#[test]
fn test_dbigword_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('W'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
        )
    ));
}

#[test]
fn test_dgg_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('g'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Delete,
            OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
        )
    ));
}

#[test]
fn test_dg_prefix_waits() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
}

#[test]
fn test_c_prefix_waits() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('c')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('i')),
        HandleKeyResult::WaitForMore
    ));
}

#[test]
fn test_d_g_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('G'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::Operation(
            Operator::Delete,
            OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
        )
    ));
}

#[test]
fn test_d5_g_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('5')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('G'));
    if let ActionKind::Count(5, inner) = complete_action_kind(result) {
        assert!(matches!(
            inner.kind.as_ref(),
            Some(ActionKind::Operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine)
            ))
        ));
    } else {
        panic!("expected counted delete motion");
    }
}

#[test]
fn test_d5gg_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('5')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('g'));
    if let ActionKind::Count(5, inner) = complete_action_kind(result) {
        assert!(matches!(
            inner.kind.as_ref(),
            Some(ActionKind::Operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine)
            ))
        ));
    } else {
        panic!("expected counted delete motion");
    }
}

#[test]
fn test_d_counted_word_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('2')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('w'));
    assert!(matches!(complete_action_kind(result), ActionKind::Count(2, _)));
}

#[test]
fn test_action_with_count() {
    let action = Action::new(ActionKind::MoveDown).clone().with_count(5);
    assert!(matches!(
        action,
        Some(action) if matches!(action.kind.as_ref(), Some(ActionKind::Count(5, _)))
    ));
}

#[test]
fn test_dot_repeat_source_classification() {
    assert!(Action::new(ActionKind::DeleteLine).is_dot_repeat_source());
    assert!(!Action::mode_transition(ModeKind::Insert).is_dot_repeat_source());
    assert!(
        Action::operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )
        .is_dot_repeat_source()
    );
    assert!(!Action::new(ActionKind::MoveDown).is_dot_repeat_source());
    assert!(!Action::new(ActionKind::RepeatLastChange).is_dot_repeat_source());
}

#[test]
fn test_change_operation_traits() {
    let action = Action::operation(
        Operator::Change,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    );

    assert!(action.is_snapshottable());
    assert!(action.switches_to_insert_mode());
    assert!(action.is_countable());
}

#[test]
fn test_tab_navigation_key_sequences() {
    let mut mode = NormalMode::new();

    assert!(matches!(
        mode.handle_key(&key('[')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('b'));
    assert!(matches!(
        complete_action_kind(result),
        ActionKind::PreviousTab
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key(']')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('b'));
    assert!(matches!(complete_action_kind(result), ActionKind::NextTab));
}

#[test]
fn test_tab_navigation_action_traits() {
    let previous = Action::new(ActionKind::PreviousTab);
    let next = Action::new(ActionKind::NextTab);

    assert!(previous.is_countable());
    assert!(next.is_countable());
    assert!(!previous.is_snapshottable());
    assert!(!next.is_snapshottable());
    assert!(!previous.switches_to_insert_mode());
    assert!(!next.switches_to_insert_mode());
    assert!(!previous.updates_snapshot_cursor());
    assert!(!next.updates_snapshot_cursor());

    assert!(matches!(
        previous.clone().with_count(3),
        Some(action) if matches!(action.kind.as_ref(), Some(ActionKind::Count(3, _)))
    ));
    assert!(matches!(
        next.clone().with_count(4),
        Some(action) if matches!(action.kind.as_ref(), Some(ActionKind::Count(4, _)))
    ));
}
