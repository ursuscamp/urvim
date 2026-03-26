use super::*;
use crate::terminal::Key;

fn key(c: char) -> Key {
    Key::new(crate::terminal::KeyCode::Char(c))
}

fn handle_and_unwrap(mode: &mut impl Mode, k: &Key) -> Action {
    match mode.handle_key(k) {
        HandleKeyResult::Complete(action) => action,
        HandleKeyResult::WaitForMore => Action::None,
        HandleKeyResult::InvalidSequence => Action::None,
    }
}

#[test]
fn test_normal_mode_move_left() {
    let mut mode = NormalMode::new();
    assert_eq!(handle_and_unwrap(&mut mode, &key('h')), Action::MoveLeft);
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
        Action::SwitchToNormal
    );
}

#[test]
fn test_gg_motion() {
    let mut mode = NormalMode::new();
    let result = mode.handle_key(&key('g'));
    assert!(matches!(result, HandleKeyResult::WaitForMore));
    let result = mode.handle_key(&key('g'));
    assert!(matches!(
        result,
        HandleKeyResult::Complete(Action::MoveToFirstLine)
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
    assert!(matches!(
        result,
        HandleKeyResult::Complete(Action::Count(3, _))
    ));
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        ))
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Change,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        ))
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Change,
            OperatorTarget::TextObject(TextObject::InnerWord),
        ))
    ));
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Change,
            OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
        ))
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
        ))
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
        ))
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
        ))
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
        ))
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Delete,
            OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
        ))
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
        result,
        HandleKeyResult::Complete(Action::Operation(
            Operator::Delete,
            OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
        ))
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
    if let HandleKeyResult::Complete(Action::Count(5, inner)) = result {
        assert!(matches!(
            *inner,
            Action::Operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine)
            )
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
    if let HandleKeyResult::Complete(Action::Count(5, inner)) = result {
        assert!(matches!(
            *inner,
            Action::Operation(
                Operator::Delete,
                OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine)
            )
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
    assert!(matches!(
        result,
        HandleKeyResult::Complete(Action::Count(2, _))
    ));
}

#[test]
fn test_action_with_count() {
    let action = Action::MoveDown.clone().with_count(5);
    assert!(matches!(action, Some(Action::Count(5, _))));
}

#[test]
fn test_change_operation_traits() {
    let action = Action::Operation(
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
        result,
        HandleKeyResult::Complete(Action::PreviousTab)
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key(']')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('b'));
    assert!(matches!(result, HandleKeyResult::Complete(Action::NextTab)));
}

#[test]
fn test_tab_navigation_action_traits() {
    let previous = Action::PreviousTab;
    let next = Action::NextTab;

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
        Some(Action::Count(3, _))
    ));
    assert!(matches!(
        next.clone().with_count(4),
        Some(Action::Count(4, _))
    ));
}
