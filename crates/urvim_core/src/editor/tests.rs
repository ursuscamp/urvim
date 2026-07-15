use super::*;
use crate::buffer::{Buffer, Cursor};
use crate::config::{AutoIndentMode, TabBehavior, TabInsertion};
use crate::config::{Config, DefaultRegisters, KeymapsConfig};
use crate::editor::{DelimiterFamily, EditorOperation};
use crate::globals;
use crate::globals::set_test_config;
use crate::globals::{Direction, FindKind, FindState};
use crate::register::RegisterName;
use std::collections::BTreeSet;
use urvim_terminal::{Key, KeyCode, Modifiers};

fn key(c: char) -> Key {
    Key::new(urvim_terminal::KeyCode::Char(c))
}

fn ctrl_key(c: char) -> Key {
    Key::with_modifiers(KeyCode::Char(c), Modifiers::CTRL)
}

fn handle_and_unwrap(mode: &mut impl Mode, k: &Key) -> EditorAction {
    match mode.handle_key(k) {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            let to_mode = action.to_mode;
            action.clone().with_mode(None, to_mode)
        }
        HandleKeyResult::WaitForMore => EditorAction::none(),
        HandleKeyResult::InvalidSequence => EditorAction::none(),
    }
}

fn complete_action_kind(mode_result: HandleKeyResult) -> EditorOperation {
    match mode_result {
        HandleKeyResult::Complete(intent) => intent
            .as_editor_action()
            .expect("expected a complete action")
            .kind
            .clone()
            .expect("expected a complete action"),
        HandleKeyResult::WaitForMore => panic!("expected a complete action, got wait"),
        HandleKeyResult::InvalidSequence => panic!("expected a complete action, got invalid"),
    }
}

fn configured_test_config() -> Config {
    Config {
        theme: "test-theme".to_string(),
        syntax: true,
        auto_close_pairs: true,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    }
}

fn configured_test_config_with_insert_keymap(keys: &str, command: &str) -> Config {
    Config {
        keymaps: KeymapsConfig {
            insert: std::collections::BTreeMap::from([(keys.to_string(), command.to_string())]),
            ..Default::default()
        },
        ..configured_test_config()
    }
}

fn configured_test_config_with_pairs(auto_close_pairs: bool) -> Config {
    Config {
        theme: "test-theme".to_string(),
        syntax: true,
        auto_close_pairs,
        auto_indent: AutoIndentMode::Off,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    }
}

fn configured_test_config_with_auto_indent(auto_indent: AutoIndentMode) -> Config {
    Config {
        theme: "test-theme".to_string(),
        syntax: true,
        auto_close_pairs: true,
        auto_indent,
        advanced_glyphs: BTreeSet::new(),
        ..Default::default()
    }
}

fn set_active_buffer(text: &str) {
    let buffer_id = globals::with_buffer_pool(|pool| pool.register_buffer(Buffer::from_str(text)));
    globals::set_active_buffer_id(buffer_id);
}

#[test]
fn test_normal_mode_move_left() {
    let mut mode = NormalMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('h')),
        EditorAction::new(EditorOperation::MoveLeft)
    );
}

#[test]
fn test_normal_mode_configured_keymap_overrides_builtin() {
    let mut config = configured_test_config();
    config
        .keymaps
        .normal
        .insert("h".to_string(), "action cursor right".to_string());
    let _guard = set_test_config(config);
    let mut mode = NormalMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('h')),
        EditorAction::new(EditorOperation::MoveRight)
    );
}

#[test]
fn test_normal_mode_configured_keymap_supports_viewport_command() {
    let mut config = configured_test_config();
    config
        .keymaps
        .normal
        .insert("h".to_string(), "action viewport center".to_string());
    let _guard = set_test_config(config);
    let mut mode = NormalMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('h')),
        EditorAction::new(EditorOperation::ViewportCursorCenter)
    );
}

#[test]
fn test_normal_mode_page_keys() {
    let mut mode = NormalMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(KeyCode::PageUp)),
        EditorAction::new(EditorOperation::MovePageUp)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(KeyCode::PageDown)),
        EditorAction::new(EditorOperation::MovePageDown)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &ctrl_key('u')),
        EditorAction::new(EditorOperation::MoveHalfPageUp)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &ctrl_key('d')),
        EditorAction::new(EditorOperation::MoveHalfPageDown)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &ctrl_key('o')),
        EditorAction::jump_backward()
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &ctrl_key('i')),
        EditorAction::jump_forward()
    );
}

#[test]
fn test_normal_mode_split_management_bindings() {
    let mut mode = NormalMode::new();

    assert!(matches!(
        mode.handle_key(&ctrl_key('w')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('v')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::SplitVertical))
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&ctrl_key('w')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('s')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::SplitHorizontal))
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&ctrl_key('w')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('h')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::FocusPaneLeft))
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&ctrl_key('w')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('q')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ClosePane))
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&ctrl_key('w')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('n')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::FocusNextWindow))
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&ctrl_key('w')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('p')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::FocusPreviousWindow))
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&ctrl_key('w')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('r')),
        EditorAction::mode_transition(ModeKind::Resizing)
    );
}

#[test]
fn test_picker_open_binding_is_available_in_modes() {
    let mut normal = NormalMode::new();
    assert!(matches!(
        normal.handle_key(&Key::new(KeyCode::F1)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenFilePicker))
    ));
    assert!(matches!(
        normal.handle_key(&Key::new(KeyCode::F2)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenGrepPicker))
    ));
    assert!(matches!(
        normal.handle_key(&Key::new(KeyCode::F3)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenBufferPicker))
    ));
    assert!(matches!(
        normal.handle_key(&Key::new(KeyCode::F4)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenGitPicker))
    ));
    assert!(matches!(
        normal.handle_key(&Key::new(KeyCode::F5)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenColorschemePicker))
    ));
    assert!(matches!(
        normal.handle_key(&Key::new(KeyCode::F6)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenFiletypePicker))
    ));
    assert!(matches!(
        normal.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        normal.handle_key(&key('O')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenDocumentSymbolsPicker))
    ));
    assert!(matches!(
        normal.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        normal.handle_key(&key('r')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        normal.handle_key(&key('a')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::LspCodeActions))
    ));
    assert!(matches!(
        normal.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        normal.handle_key(&key('r')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        normal.handle_key(&key('S')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenWorkspaceSymbolsPicker))
    ));
    assert!(matches!(
        normal.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        normal.handle_key(&key('r')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        normal.handle_key(&key('r')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::LspReferences))
    ));

    let mut insert = InsertMode::new();
    assert!(matches!(
        insert.handle_key(&Key::new(KeyCode::F1)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenFilePicker))
    ));
    assert!(matches!(
        insert.handle_key(&Key::new(KeyCode::F2)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenGrepPicker))
    ));
    assert!(matches!(
        insert.handle_key(&Key::new(KeyCode::F3)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenBufferPicker))
    ));
    assert!(matches!(
        insert.handle_key(&Key::new(KeyCode::F4)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenGitPicker))
    ));
    assert!(matches!(
        insert.handle_key(&Key::new(KeyCode::F5)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenColorschemePicker))
    ));
    assert!(matches!(
        insert.handle_key(&Key::new(KeyCode::F6)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenFiletypePicker))
    ));

    let mut replace = ReplaceMode::new();
    assert!(matches!(
        replace.handle_key(&Key::new(KeyCode::F6)),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::OpenFiletypePicker))
    ));
}

#[test]
fn test_normal_mode_equalize_binding() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&ctrl_key('w')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('=')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::EqualizeSplits))
    ));
}

#[test]
fn test_normal_mode_comment_toggle_binding() {
    let mut mode = NormalMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('g')),
        EditorAction::none()
    );
    assert!(matches!(
        mode.handle_key(&key('c')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('c')),
        EditorAction::toggle_line_comment()
    );
}

#[test]
fn test_comment_toggle_does_not_switch_to_insert_mode() {
    assert!(!EditorAction::toggle_line_comment().switches_to_insert_mode());
}

#[test]
fn test_normal_mode_append_after_cursor_sets_insert_mode() {
    let mut mode = NormalMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('a')),
        EditorAction::new(EditorOperation::AppendAfterCursor).with_to_mode(ModeKind::Insert)
    );
}

#[test]
fn test_normal_mode_visual_binding_switches_to_visual_mode() {
    let mut mode = NormalMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('v')),
        EditorAction::mode_transition(ModeKind::Visual)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('V')),
        EditorAction::mode_transition(ModeKind::VisualLine)
    );
}

#[test]
fn test_normal_mode_indent_bindings() {
    let mut mode = NormalMode::new();

    assert!(matches!(
        mode.handle_key(&key('<')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('<')),
        EditorAction::new(EditorOperation::IndentDecrease)
    );

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('>')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('>')),
        EditorAction::new(EditorOperation::IndentIncrease)
    );
}

#[test]
fn test_normal_mode_dot_repeat_action() {
    let mut mode = NormalMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('.')),
        EditorAction::new(EditorOperation::RepeatLastChange)
    );
}

#[test]
fn test_mode_kind_reflects_mode_type() {
    let normal = NormalMode::new();
    let insert = InsertMode::new();
    let visual = VisualMode::new();
    let visual_line = VisualLineMode::new();
    let resizing = ResizingMode::new();

    assert_eq!(normal.kind(), ModeKind::Normal);
    assert_eq!(insert.kind(), ModeKind::Insert);
    assert_eq!(visual.kind(), ModeKind::Visual);
    assert_eq!(visual_line.kind(), ModeKind::VisualLine);
    assert_eq!(resizing.kind(), ModeKind::Resizing);
}

#[test]
fn test_resizing_mode_key_bindings() {
    let mut mode = ResizingMode::new();

    assert_eq!(
        mode.cursor_style(),
        urvim_terminal::CursorStyle::SteadyUnderline
    );
    assert!(matches!(
        mode.handle_key(&key('h')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ResizePaneLeft(1)))
    ));
    assert!(matches!(
        mode.handle_key(&key('H')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ResizePaneLeft(5)))
    ));
    assert!(matches!(
        mode.handle_key(&key('l')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ResizePaneRight(1)))
    ));
    assert!(matches!(
        mode.handle_key(&key('L')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ResizePaneRight(5)))
    ));
    assert!(matches!(
        mode.handle_key(&key('k')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ResizePaneUp(1)))
    ));
    assert!(matches!(
        mode.handle_key(&key('j')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ResizePaneDown(1)))
    ));
    assert!(matches!(
        mode.handle_key(&key('K')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ResizePaneUp(5)))
    ));
    assert!(matches!(
        mode.handle_key(&key('J')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ResizePaneDown(5)))
    ));
    assert!(matches!(
        mode.handle_key(&key('=')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::EqualizeSplits))
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(KeyCode::Esc)),
        EditorAction::mode_transition(ModeKind::Normal)
    );
    assert!(matches!(
        mode.handle_key(&key('x')),
        HandleKeyResult::InvalidSequence
    ));
}

#[test]
fn test_resizing_mode_configured_keymap_overrides_builtin() {
    let mut config = configured_test_config();
    config
        .keymaps
        .resizing
        .insert("h".to_string(), "pane resize-right count=1".to_string());
    let _guard = set_test_config(config);
    let mut mode = ResizingMode::new();

    assert!(matches!(
        mode.handle_key(&key('h')),
        HandleKeyResult::Complete(intent)
            if matches!(intent, crate::ui::Intent::Command(crate::ui::Command::ResizePaneRight(1)))
    ));
}

#[test]
fn test_visual_mode_motion_and_exit_bindings() {
    let mut mode = VisualMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('l')),
        EditorAction::new(EditorOperation::MoveRight)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key(';')),
        EditorAction::new(EditorOperation::RepeatLastFind)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key(',')),
        EditorAction::new(EditorOperation::RepeatLastFindReverse)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('d')),
        EditorAction::new(EditorOperation::DeleteSelection).with_to_mode(ModeKind::Normal)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('y')),
        EditorAction::new(EditorOperation::YankSelection).with_to_mode(ModeKind::Normal)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('V')),
        EditorAction::mode_transition(ModeKind::VisualLine)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('c')),
        EditorAction::new(EditorOperation::ChangeSelection).with_to_mode(ModeKind::Insert)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('g')),
        EditorAction::none()
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('u')),
        EditorAction::operation(Operator::Lowercase, OperatorTarget::Selection)
            .with_to_mode(ModeKind::Normal)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(urvim_terminal::KeyCode::Esc)),
        EditorAction::mode_transition(ModeKind::Normal)
    );
}

#[test]
fn test_visual_mode_configured_keymap_overrides_builtin() {
    let mut config = configured_test_config();
    config
        .keymaps
        .visual
        .insert("l".to_string(), "action cursor left".to_string());
    let _guard = set_test_config(config);
    let mut mode = VisualMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('l')),
        EditorAction::new(EditorOperation::MoveLeft)
    );
}

#[test]
fn test_visual_mode_text_object_bindings() {
    let mut mode = VisualMode::new();

    assert!(matches!(
        mode.handle_key(&key('i')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('w')),
        EditorAction::new(EditorOperation::VisualTextObject(TextObject::InnerWord))
    );

    let mut mode = VisualMode::new();
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('W')),
        EditorAction::new(EditorOperation::VisualTextObject(TextObject::AroundBigWord))
    );
}

#[test]
fn test_visual_mode_surround_add_binding() {
    let mut mode = VisualMode::new();

    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('s')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('"')),
        EditorAction::new(EditorOperation::SurroundAddSelection {
            delimiter: DelimiterFamily::DoubleQuote,
        })
        .with_to_mode(ModeKind::Normal)
    );
}

#[test]
fn test_visual_mode_v_exits_to_normal() {
    let mut mode = VisualMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('v')),
        EditorAction::mode_transition(ModeKind::Normal)
    );
}

#[test]
fn test_visual_line_mode_motion_and_exit_bindings() {
    let mut mode = VisualLineMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('l')),
        EditorAction::new(EditorOperation::MoveRight)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('v')),
        EditorAction::mode_transition(ModeKind::Visual)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('d')),
        EditorAction::new(EditorOperation::DeleteSelection).with_to_mode(ModeKind::Normal)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('y')),
        EditorAction::new(EditorOperation::YankSelection).with_to_mode(ModeKind::Normal)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('c')),
        EditorAction::new(EditorOperation::ChangeSelection).with_to_mode(ModeKind::Insert)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('g')),
        EditorAction::none()
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('U')),
        EditorAction::operation(Operator::Uppercase, OperatorTarget::Selection)
            .with_to_mode(ModeKind::Normal)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(urvim_terminal::KeyCode::Esc)),
        EditorAction::mode_transition(ModeKind::Normal)
    );
}

#[test]
fn test_visual_line_mode_configured_keymap_overrides_builtin() {
    let mut config = configured_test_config();
    config
        .keymaps
        .visual_line
        .insert("l".to_string(), "action cursor left".to_string());
    let _guard = set_test_config(config);
    let mut mode = VisualLineMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('l')),
        EditorAction::new(EditorOperation::MoveLeft)
    );
}

#[test]
fn test_visual_line_mode_surround_add_binding_accepts_closer() {
    let mut mode = VisualLineMode::new();

    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('s')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key(']')),
        EditorAction::new(EditorOperation::SurroundAddSelection {
            delimiter: DelimiterFamily::Square,
        })
        .with_to_mode(ModeKind::Normal)
    );
}

#[test]
fn test_visual_line_mode_v_exits_to_normal() {
    let mut mode = VisualLineMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('V')),
        EditorAction::mode_transition(ModeKind::Normal)
    );
}

#[test]
fn test_insert_mode_escape_switches_to_normal() {
    let mut mode = InsertMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(urvim_terminal::KeyCode::Esc)),
        EditorAction::mode_transition(ModeKind::Normal)
    );
}

#[test]
fn test_replace_mode_backspace_restores_last_replace() {
    let mut mode = ReplaceMode::new();

    assert_eq!(
        complete_action_kind(mode.handle_key(&key('a'))),
        EditorOperation::ReplaceChar('a')
    );

    assert_eq!(
        complete_action_kind(mode.handle_key(&Key::new(KeyCode::Backspace))),
        EditorOperation::ReplaceBackspaceLast
    );
}

#[test]
fn test_insert_mode_shift_tab_binds_to_indent_decrease() {
    let _guard = set_test_config(configured_test_config());
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(
            &mut mode,
            &Key::with_modifiers(KeyCode::Tab, Modifiers::SHIFT)
        ),
        EditorAction::new(EditorOperation::IndentDecrease)
    );
}

#[test]
fn test_insert_mode_configured_keymap_switches_to_normal() {
    let _guard = set_test_config(configured_test_config_with_insert_keymap(
        "jk",
        "mode normal",
    ));
    let mut mode = InsertMode::new();

    assert!(matches!(
        mode.handle_key(&key('j')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('k')),
        EditorAction::mode_transition(ModeKind::Normal)
    );
    assert_eq!(mode.take_repeat_text(), None);
}

#[test]
fn test_insert_mode_configured_keymap_keeps_builtin_escape() {
    let _guard = set_test_config(configured_test_config_with_insert_keymap(
        "jk",
        "mode normal",
    ));
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(urvim_terminal::KeyCode::Esc)),
        EditorAction::mode_transition(ModeKind::Normal)
    );
}

#[test]
fn test_insert_mode_configured_keymap_does_not_affect_normal_mode() {
    let _guard = set_test_config(configured_test_config_with_insert_keymap(
        "jk",
        "mode normal",
    ));
    let mut mode = NormalMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('j')),
        EditorAction::new(EditorOperation::MoveDown)
    );
}

#[test]
fn test_insert_mode_page_keys() {
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(KeyCode::PageUp)),
        EditorAction::new(EditorOperation::MovePageUp)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(KeyCode::PageDown)),
        EditorAction::new(EditorOperation::MovePageDown)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &ctrl_key('u')),
        EditorAction::new(EditorOperation::MoveHalfPageUp)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &ctrl_key('d')),
        EditorAction::new(EditorOperation::MoveHalfPageDown)
    );
}

#[test]
fn test_insert_mode_enter_emits_plain_newline() {
    let _guard = set_test_config(configured_test_config_with_auto_indent(
        AutoIndentMode::Neighbor,
    ));
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(urvim_terminal::KeyCode::Enter)),
        EditorAction::insert_newline()
    );
    assert_eq!(mode.take_repeat_text().as_deref(), Some("\n"));
}

#[test]
fn test_insert_mode_enter_emits_plain_newline_when_disabled() {
    let _guard = set_test_config(configured_test_config_with_auto_indent(AutoIndentMode::Off));
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(urvim_terminal::KeyCode::Enter)),
        EditorAction::insert_newline()
    );
    assert_eq!(mode.take_repeat_text().as_deref(), Some("\n"));
}

#[test]
fn test_insert_mode_partial_escape_sequence_falls_back_to_literal_text() {
    let _guard = set_test_config(configured_test_config_with_insert_keymap(
        "jk",
        "mode normal",
    ));
    let mut mode = InsertMode::new();

    assert!(matches!(
        mode.handle_key(&key('j')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('j')),
        EditorAction::insert_text("jj".to_string())
    );
    assert_eq!(mode.take_repeat_text().as_deref(), Some("jj"));
}

#[test]
fn test_insert_mode_partial_escape_sequence_keeps_following_text() {
    let _guard = set_test_config(configured_test_config_with_insert_keymap(
        "jk",
        "mode normal",
    ));
    let mut mode = InsertMode::new();

    assert!(matches!(
        mode.handle_key(&key('j')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('u')),
        EditorAction::insert_text("ju".to_string())
    );
    assert_eq!(mode.take_repeat_text().as_deref(), Some("ju"));
}

#[test]
fn test_insert_mode_emits_pair_action_for_supported_opener() {
    let _guard = set_test_config(configured_test_config());
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('(')),
        EditorAction::insert_char('(')
    );
}

#[test]
fn test_insert_mode_emits_skip_closer_action_for_supported_closer() {
    let _guard = set_test_config(configured_test_config());
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key(')')),
        EditorAction::insert_char(')')
    );
}

#[test]
fn test_insert_mode_emits_skip_action_for_quote_closer() {
    let _guard = set_test_config(configured_test_config());
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('"')),
        EditorAction::insert_char('"')
    );
}

#[test]
fn test_insert_mode_disabled_auto_close_keeps_plain_insertion() {
    let _guard = set_test_config(configured_test_config_with_pairs(false));
    let mut mode = InsertMode::new();

    assert_eq!(
        handle_and_unwrap(&mut mode, &key('(')),
        EditorAction::insert_char('(')
    );
}

#[test]
fn test_insert_mode_tab_simple_uses_configured_insertion_setting() {
    let mut config = configured_test_config();
    config.tab_insertion = TabInsertion::Spaces;
    config.tab_behavior = TabBehavior::Simple;
    config.tab_width = 4;
    let _guard = set_test_config(config);
    let mut mode = InsertMode::new();

    match mode.handle_key(&Key::new(KeyCode::Tab)) {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            assert_eq!(
                action.kind,
                Some(EditorOperation::InsertText("    ".to_string()))
            );
        }
        other => panic!("expected complete action, got {other:?}"),
    }
}

#[test]
fn test_insert_mode_tab_smart_infers_tabs_from_buffer_contents() {
    let mut config = configured_test_config();
    config.tab_insertion = TabInsertion::Spaces;
    config.tab_behavior = TabBehavior::Smart;
    config.tab_width = 4;
    let _guard = set_test_config(config);
    set_active_buffer("fn main() {\n\tprintln!(\"hi\");\n}");
    let mut mode = InsertMode::new();

    match mode.handle_key(&Key::new(KeyCode::Tab)) {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            assert_eq!(
                action.kind,
                Some(EditorOperation::InsertText("\t".to_string()))
            );
        }
        other => panic!("expected complete action, got {other:?}"),
    }
}

#[test]
fn test_insert_mode_tab_smart_falls_back_to_configured_insertion_setting() {
    let mut config = configured_test_config();
    config.tab_insertion = TabInsertion::Tabs;
    config.tab_behavior = TabBehavior::Smart;
    config.tab_width = 4;
    let _guard = set_test_config(config);
    set_active_buffer("fn main() {\nprintln!(\"hi\");\n}");
    let mut mode = InsertMode::new();

    match mode.handle_key(&Key::new(KeyCode::Tab)) {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            assert_eq!(
                action.kind,
                Some(EditorOperation::InsertText("\t".to_string()))
            );
        }
        other => panic!("expected complete action, got {other:?}"),
    }
}

#[test]
fn test_insert_mode_captures_repeat_text() {
    let mut mode = InsertMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('h')),
        EditorAction::insert_char('h')
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('i')),
        EditorAction::insert_char('i')
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(urvim_terminal::KeyCode::Esc)),
        EditorAction::mode_transition(ModeKind::Normal)
    );

    assert_eq!(mode.take_repeat_text().as_deref(), Some("hi"));
    assert_eq!(mode.take_repeat_text(), None);
}

#[test]
fn test_insert_mode_captured_repeat_text_tracks_backspace() {
    let mut mode = InsertMode::new();
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('h')),
        EditorAction::insert_char('h')
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &key('i')),
        EditorAction::insert_char('i')
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(urvim_terminal::KeyCode::Backspace)),
        EditorAction::new(EditorOperation::DeleteBackward)
    );
    assert_eq!(
        handle_and_unwrap(&mut mode, &Key::new(urvim_terminal::KeyCode::Esc)),
        EditorAction::mode_transition(ModeKind::Normal)
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
        EditorOperation::MoveToFirstLine
    ));
}

#[test]
fn test_z_viewport_sequences() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('z')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        complete_action_kind(mode.handle_key(&key('t'))),
        EditorOperation::ViewportCursorTop
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('z')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        complete_action_kind(mode.handle_key(&key('z'))),
        EditorOperation::ViewportCursorCenter
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('z')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        complete_action_kind(mode.handle_key(&key('b'))),
        EditorOperation::ViewportCursorBottom
    ));
}

#[test]
fn test_counted_z_viewport_sequence_ignores_count() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('3')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('z')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        complete_action_kind(mode.handle_key(&key('z'))),
        EditorOperation::ViewportCursorCenter
    ));
}

#[test]
fn test_unsupported_z_sequence_is_invalid() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('z')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('x')),
        HandleKeyResult::InvalidSequence
    ));
}

#[test]
fn test_delimiter_family_selector_parsing() {
    assert_eq!(
        DelimiterFamily::from_selector_key("("),
        Some(DelimiterFamily::Paren)
    );
    assert_eq!(
        DelimiterFamily::from_selector_key(")"),
        Some(DelimiterFamily::Paren)
    );
    assert_eq!(
        DelimiterFamily::from_selector_key("<LessThan>"),
        Some(DelimiterFamily::Angle)
    );
    assert_eq!(
        DelimiterFamily::from_selector_key("<GreaterThan>"),
        Some(DelimiterFamily::Angle)
    );
    assert_eq!(
        DelimiterFamily::from_selector_key("\""),
        Some(DelimiterFamily::DoubleQuote)
    );
    assert_eq!(DelimiterFamily::from_selector_key("x"), None);
}

#[test]
fn test_gsd_quote_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('s')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        complete_action_kind(mode.handle_key(&key('"'))),
        EditorOperation::SurroundDelete {
            target: DelimiterFamily::DoubleQuote,
        }
    );
}

#[test]
fn test_gsr_bracket_replace_sequence_accepts_closer_target() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('s')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('r')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('}')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        complete_action_kind(mode.handle_key(&key('['))),
        EditorOperation::SurroundReplace {
            target: DelimiterFamily::Curly,
            replacement: DelimiterFamily::Square,
        }
    );
}

#[test]
fn test_gsr_angle_to_quote_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('s')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('r')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('<')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        complete_action_kind(mode.handle_key(&key('"'))),
        EditorOperation::SurroundReplace {
            target: DelimiterFamily::Angle,
            replacement: DelimiterFamily::DoubleQuote,
        }
    );
}

#[test]
fn test_gsa_inner_word_to_quote_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('s')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('i')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('w')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        complete_action_kind(mode.handle_key(&key('"'))),
        EditorOperation::SurroundAdd {
            target: TextObject::InnerWord,
            delimiter: DelimiterFamily::DoubleQuote,
        }
    );
}

#[test]
fn test_gsa_bracket_sequence_accepts_closer_text_object_and_delimiter() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('s')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('}')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        complete_action_kind(mode.handle_key(&key(']'))),
        EditorOperation::SurroundAdd {
            target: TextObject::AroundBracket(BracketKind::Curly),
            delimiter: DelimiterFamily::Square,
        }
    );
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
        complete_action_kind(result),
        EditorOperation::Count(3, _)
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
        complete_action_kind(result),
        EditorOperation::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )
    ));
}

#[test]
fn test_dfx_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('f')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('x'));
    match complete_action_kind(result) {
        EditorOperation::Operation(
            Operator::Delete,
            OperatorTarget::CharacterScan(FindState {
                target_char,
                kind,
                direction,
            }),
        ) => {
            assert_eq!(target_char, 'x');
            assert_eq!(kind, FindKind::Find);
            assert_eq!(direction, Direction::Forward);
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn test_space_character_scan_bindings() {
    let mut mode = NormalMode::new();

    assert!(matches!(
        mode.handle_key(&key('f')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        complete_action_kind(mode.handle_key(&key(' '))),
        EditorOperation::FindForward(' ')
    );

    assert!(matches!(
        mode.handle_key(&key('F')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        complete_action_kind(mode.handle_key(&key(' '))),
        EditorOperation::FindBackward(' ')
    );

    assert!(matches!(
        mode.handle_key(&key('t')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        complete_action_kind(mode.handle_key(&key(' '))),
        EditorOperation::TillForward(' ')
    );

    assert!(matches!(
        mode.handle_key(&key('T')),
        HandleKeyResult::WaitForMore
    ));
    assert_eq!(
        complete_action_kind(mode.handle_key(&key(' '))),
        EditorOperation::TillBackward(' ')
    );
}

#[test]
fn test_df_space_sequence() {
    let mut mode = NormalMode::new();

    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('f')),
        HandleKeyResult::WaitForMore
    ));

    match complete_action_kind(mode.handle_key(&key(' '))) {
        EditorOperation::Operation(
            Operator::Delete,
            OperatorTarget::CharacterScan(FindState {
                target_char,
                kind,
                direction,
            }),
        ) => {
            assert_eq!(target_char, ' ');
            assert_eq!(kind, FindKind::Find);
            assert_eq!(direction, Direction::Forward);
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn test_cf_space_sequence_enters_insert_mode() {
    let mut mode = NormalMode::new();

    assert!(matches!(
        mode.handle_key(&key('c')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('f')),
        HandleKeyResult::WaitForMore
    ));

    let result = mode.handle_key(&key(' '));
    match result {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            assert_eq!(action.to_mode, Some(ModeKind::Insert));
            match action.kind.as_ref() {
                Some(EditorOperation::Operation(
                    Operator::Change,
                    OperatorTarget::CharacterScan(FindState {
                        target_char,
                        kind,
                        direction,
                    }),
                )) => {
                    assert_eq!(*target_char, ' ');
                    assert_eq!(*kind, FindKind::Find);
                    assert_eq!(*direction, Direction::Forward);
                }
                other => panic!("unexpected result: {other:?}"),
            }
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn test_ct_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('c')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('t')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key(':'));
    match result {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            assert_eq!(action.to_mode, Some(ModeKind::Insert));
            match action.kind.as_ref() {
                Some(EditorOperation::Operation(
                    Operator::Change,
                    OperatorTarget::CharacterScan(FindState {
                        target_char,
                        kind,
                        direction,
                    }),
                )) => {
                    assert_eq!(*target_char, ':');
                    assert_eq!(*kind, FindKind::Till);
                    assert_eq!(*direction, Direction::Forward);
                }
                other => panic!("unexpected result: {other:?}"),
            }
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn test_gufx_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('u')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('f')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('x'));
    match complete_action_kind(result) {
        EditorOperation::Operation(
            Operator::Lowercase,
            OperatorTarget::CharacterScan(FindState {
                target_char,
                kind,
                direction,
            }),
        ) => {
            assert_eq!(target_char, 'x');
            assert_eq!(kind, FindKind::Find);
            assert_eq!(direction, Direction::Forward);
        }
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn test_count_dfx_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('2')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('d')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('f')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('x'));
    assert!(matches!(
        complete_action_kind(result),
        EditorOperation::Count(2, _)
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
            HandleKeyResult::Complete(intent) => match intent
                .as_editor_action()
                .expect("expected a complete action")
                .kind
                .as_ref()
            {
                Some(EditorOperation::Operation(
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
            HandleKeyResult::Complete(intent) => match intent
                .as_editor_action()
                .expect("expected a complete action")
                .kind
                .as_ref()
            {
                Some(EditorOperation::Operation(
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
            HandleKeyResult::Complete(intent) => match intent
                .as_editor_action()
                .expect("expected a complete action")
                .kind
                .as_ref()
            {
                Some(EditorOperation::Operation(
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
            HandleKeyResult::Complete(intent) => match intent
                .as_editor_action()
                .expect("expected a complete action")
                .kind
                .as_ref()
            {
                Some(EditorOperation::Operation(
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
        EditorOperation::Operation(
            Operator::Change,
            OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
        )
    ));
}

#[test]
fn test_gu_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('u')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('w'));
    assert!(matches!(
        complete_action_kind(result),
        EditorOperation::Operation(
            Operator::Lowercase,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )
    ));
}

#[test]
fn test_g_upper_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('U')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('w'));
    assert!(matches!(
        complete_action_kind(result),
        EditorOperation::Operation(
            Operator::Uppercase,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )
    ));
}

#[test]
fn test_g_tilde_sequence() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('g')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('~')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('w'));
    assert!(matches!(
        complete_action_kind(result),
        EditorOperation::Operation(
            Operator::ToggleCase,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
        EditorOperation::Operation(
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
    if let EditorOperation::Count(5, inner) = complete_action_kind(result) {
        assert!(matches!(
            inner.kind.as_ref(),
            Some(EditorOperation::Operation(
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
    if let EditorOperation::Count(5, inner) = complete_action_kind(result) {
        assert!(matches!(
            inner.kind.as_ref(),
            Some(EditorOperation::Operation(
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
    assert!(matches!(
        complete_action_kind(result),
        EditorOperation::Count(2, _)
    ));
}

#[test]
fn test_action_with_count() {
    let action = EditorAction::new(EditorOperation::MoveDown)
        .clone()
        .with_count(5);
    assert!(matches!(
        action,
        Some(action) if matches!(action.kind.as_ref(), Some(EditorOperation::Count(5, _)))
    ));
}

#[test]
fn test_dot_repeat_source_classification() {
    assert!(EditorAction::new(EditorOperation::DeleteLine).is_dot_repeat_source());
    assert!(EditorAction::new(EditorOperation::IndentDecrease).is_dot_repeat_source());
    assert!(EditorAction::new(EditorOperation::IndentIncrease).is_dot_repeat_source());
    assert!(
        EditorAction::operation(Operator::Lowercase, OperatorTarget::Selection,)
            .is_dot_repeat_source()
    );
    assert!(!EditorAction::mode_transition(ModeKind::Insert).is_dot_repeat_source());
    assert!(
        EditorAction::operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )
        .is_dot_repeat_source()
    );
    assert!(
        EditorAction::new(EditorOperation::SurroundAdd {
            target: TextObject::InnerWord,
            delimiter: DelimiterFamily::DoubleQuote,
        })
        .is_dot_repeat_source()
    );
    assert!(
        EditorAction::new(EditorOperation::SurroundAddSelection {
            delimiter: DelimiterFamily::Curly,
        })
        .is_dot_repeat_source()
    );
    assert!(!EditorAction::new(EditorOperation::MoveDown).is_dot_repeat_source());
    assert!(!EditorAction::new(EditorOperation::RepeatLastChange).is_dot_repeat_source());
}

#[test]
fn test_indent_action_traits() {
    let decrease = EditorAction::new(EditorOperation::IndentDecrease);
    let increase = EditorAction::new(EditorOperation::IndentIncrease);

    assert!(decrease.is_countable());
    assert!(increase.is_countable());
    assert!(decrease.is_line_action());
    assert!(increase.is_line_action());
    assert!(decrease.is_snapshottable());
    assert!(increase.is_snapshottable());
    assert!(
        EditorAction::operation(Operator::ToggleCase, OperatorTarget::Selection,)
            .is_snapshottable()
    );

    let surround_add = EditorAction::new(EditorOperation::SurroundAdd {
        target: TextObject::InnerWord,
        delimiter: DelimiterFamily::DoubleQuote,
    });
    assert!(surround_add.resets_remembered_column());
    assert!(surround_add.is_snapshottable());

    let surround_selection = EditorAction::new(EditorOperation::SurroundAddSelection {
        delimiter: DelimiterFamily::Curly,
    });
    assert!(surround_selection.resets_remembered_column());
    assert!(surround_selection.is_snapshottable());
}

#[test]
fn test_replace_mode_character_actions_are_not_individually_snapshottable() {
    assert!(
        EditorAction::new(EditorOperation::ReplaceChar('x'))
            .with_from_mode(ModeKind::Normal)
            .is_snapshottable()
    );
    assert!(
        !EditorAction::new(EditorOperation::ReplaceChar('x'))
            .with_from_mode(ModeKind::Replace)
            .is_snapshottable()
    );
    assert!(
        !EditorAction::new(EditorOperation::ReplaceBackspaceLast)
            .with_from_mode(ModeKind::Replace)
            .is_snapshottable()
    );
    assert!(
        !EditorAction::new(EditorOperation::ReplaceBackspace {
            cursor: Cursor::new(0, 0),
            replaced: Some('a'),
            inserted: 'x',
        })
        .with_from_mode(ModeKind::Replace)
        .is_snapshottable()
    );
}

#[test]
fn test_change_operation_traits() {
    let action = EditorAction::operation(
        Operator::Change,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    );
    let insert_action = action.clone().with_to_mode(ModeKind::Insert);
    let counted_insert_action = EditorAction::count(2, Box::new(insert_action.clone()));

    assert!(!action.is_snapshottable());
    assert!(action.is_countable());
    assert!(!action.switches_to_insert_mode());
    assert!(insert_action.switches_to_insert_mode());
    assert!(counted_insert_action.switches_to_insert_mode());
    assert!(matches!(
        counted_insert_action.kind.as_ref(),
        Some(EditorOperation::Count(2, inner)) if inner.to_mode == Some(ModeKind::Insert)
    ));
}

#[test]
fn test_jump_action_traits() {
    let backward = EditorAction::jump_backward();
    let forward = EditorAction::jump_forward();

    assert!(!backward.is_countable());
    assert!(!forward.is_countable());
    assert!(!backward.is_snapshottable());
    assert!(!forward.is_snapshottable());
    assert!(!backward.uses_remembered_column());
    assert!(!forward.uses_remembered_column());
    assert!(!backward.updates_snapshot_cursor());
    assert!(!forward.updates_snapshot_cursor());
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
        HandleKeyResult::Complete(crate::ui::Intent::Command(crate::ui::Command::PreviousTab(
            1
        )))
    ));

    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key(']')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('b'));
    assert!(matches!(
        result,
        HandleKeyResult::Complete(crate::ui::Intent::Command(crate::ui::Command::NextTab(1)))
    ));
}

#[test]
fn test_tab_navigation_commands_preserve_counts() {
    let mut mode = NormalMode::new();
    assert!(matches!(
        mode.handle_key(&key('3')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key(']')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('b')),
        HandleKeyResult::Complete(crate::ui::Intent::Command(crate::ui::Command::NextTab(3)))
    ));
}

#[test]
fn test_register_prefix_uses_configured_default_and_named_registers() {
    let _config_guard = set_test_config(Config {
        default_registers: DefaultRegisters {
            yank: 'm',
            delete: 'n',
            change: 'o',
        },
        ..configured_test_config()
    });
    let mut mode = NormalMode::new();

    assert!(matches!(
        mode.handle_key(&key('"')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('y')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('p'));
    match result {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            assert_eq!(action.register, Some(RegisterName('m')));
            assert!(matches!(
                action.kind.as_ref(),
                Some(EditorOperation::PasteAfter)
            ));
        }
        other => panic!("expected complete action, got {other:?}"),
    }

    assert!(matches!(
        mode.handle_key(&key('"')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    let result = mode.handle_key(&key('p'));
    match result {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            assert_eq!(action.register, Some(RegisterName('a')));
            assert!(matches!(
                action.kind.as_ref(),
                Some(EditorOperation::PasteAfter)
            ));
        }
        other => panic!("expected complete action, got {other:?}"),
    }
}

#[test]
fn test_visual_mode_register_prefix_applies_to_yank() {
    let _config_guard = set_test_config(Config {
        default_registers: DefaultRegisters {
            yank: 'm',
            delete: 'n',
            change: 'o',
        },
        ..configured_test_config()
    });
    let mut mode = VisualMode::new();

    assert!(matches!(
        mode.handle_key(&key('"')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('a')),
        HandleKeyResult::WaitForMore
    ));
    match mode.handle_key(&key('y')) {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            assert_eq!(action.register, Some(RegisterName('a')));
            assert!(matches!(
                action.kind.as_ref(),
                Some(EditorOperation::YankSelection)
            ));
            assert_eq!(action.to_mode, Some(ModeKind::Normal));
        }
        other => panic!("expected complete action, got {other:?}"),
    }
}

#[test]
fn test_visual_line_mode_register_prefix_applies_to_yank() {
    let _config_guard = set_test_config(Config {
        default_registers: DefaultRegisters {
            yank: 'm',
            delete: 'n',
            change: 'o',
        },
        ..configured_test_config()
    });
    let mut mode = VisualLineMode::new();

    assert!(matches!(
        mode.handle_key(&key('"')),
        HandleKeyResult::WaitForMore
    ));
    assert!(matches!(
        mode.handle_key(&key('b')),
        HandleKeyResult::WaitForMore
    ));
    match mode.handle_key(&key('y')) {
        HandleKeyResult::Complete(intent) => {
            let action = intent
                .as_editor_action()
                .expect("expected a complete action");
            assert_eq!(action.register, Some(RegisterName('b')));
            assert!(matches!(
                action.kind.as_ref(),
                Some(EditorOperation::YankSelection)
            ));
            assert_eq!(action.to_mode, Some(ModeKind::Normal));
        }
        other => panic!("expected complete action, got {other:?}"),
    }
}
