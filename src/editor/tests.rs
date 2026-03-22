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
fn test_action_with_count() {
    let action = Action::MoveDown.clone().with_count(5);
    assert!(matches!(action, Some(Action::Count(5, _))));
}
