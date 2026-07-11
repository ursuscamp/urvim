use super::keymap::MAX_COUNT;
use super::surround;
use super::text_object::{self, TextObjectScope};
use super::{EditorAction, EditorOperation, HandleKeyResult, ModeKind, TrieKeymap};
use crate::buffer::Boundary;
use crate::editor::{Operator, OperatorTarget};
use crate::globals;
use crate::globals::{Direction, FindKind};
use crate::register::RegisterName;
use crate::ui::Intent;
use urvim_terminal::{Key, KeyCode};

/// Shared state and key handling for visual modes.
pub(super) struct VisualModeState {
    keymap: TrieKeymap,
    state: VisualState,
    count: usize,
    register: Option<RegisterName>,
    pending_register: bool,
    trie_keys: Vec<String>,
    mode_kind: ModeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum VisualState {
    Idle,
    CharScanPending(CharScanData),
    TextObjectPending(TextObjectScope),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CharScanData {
    kind: FindKind,
    direction: Direction,
}

impl VisualModeState {
    pub(super) fn new(
        mode_kind: ModeKind,
        exit_key: &str,
        switch_key: &str,
        switch_to: ModeKind,
    ) -> Self {
        let mut keymap = build_visual_keymap(exit_key, switch_key, switch_to);
        globals::with_opt_config(|config| {
            if let Some(config) = config {
                let mappings = match mode_kind {
                    ModeKind::Visual => &config.keymaps.visual,
                    ModeKind::VisualLine => &config.keymaps.visual_line,
                    _ => unreachable!("visual mode state should only build visual modes"),
                };
                keymap.insert_configured(mappings);
            }
        });
        keymap.insert_intents(&globals::plugin_keymap_intents_for_mode(mode_kind));

        Self {
            keymap,
            state: VisualState::Idle,
            count: 0,
            register: None,
            pending_register: false,
            trie_keys: Vec::new(),
            mode_kind,
        }
    }

    fn reset(&mut self) {
        self.state = VisualState::Idle;
        self.count = 0;
        self.register = None;
        self.pending_register = false;
        self.trie_keys.clear();
    }

    pub(super) fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        if key.code == KeyCode::Esc {
            self.reset();
            return HandleKeyResult::complete(
                EditorAction::mode_transition(ModeKind::Normal).with_from_mode(self.mode_kind),
            );
        }

        let current = std::mem::replace(&mut self.state, VisualState::Idle);
        match current {
            VisualState::Idle => self.handle_idle(key),
            VisualState::CharScanPending(data) => self.handle_char_scan_pending(key, data),
            VisualState::TextObjectPending(scope) => self.handle_text_object_pending(key, scope),
        }
    }

    fn handle_idle(&mut self, key: &Key) -> HandleKeyResult {
        if self.trie_keys.is_empty() && !self.pending_register && key.canonical_string() == "\"" {
            self.pending_register = true;
            return HandleKeyResult::WaitForMore;
        }

        if self.pending_register {
            self.pending_register = false;
            if let KeyCode::Char(c) = key.code
                && c.is_ascii_lowercase()
            {
                let defaults =
                    crate::globals::with_config(|config| config.default_registers.clone())
                        .unwrap_or_default();
                if let Some(reg) = RegisterName::from_prefix(c, &defaults) {
                    self.register = Some(reg);
                    return HandleKeyResult::WaitForMore;
                }
            }
            self.reset();
            return HandleKeyResult::InvalidSequence;
        }

        if self.trie_keys.is_empty() {
            if let KeyCode::Char(c) = key.code {
                if c.is_ascii_digit() {
                    if self.count == 0 && c == '0' {
                        // '0' alone → line-start motion. Fall through.
                    } else {
                        self.count = self
                            .count
                            .saturating_mul(10)
                            .saturating_add(c as usize - '0' as usize);
                        if self.count > MAX_COUNT {
                            self.count = MAX_COUNT;
                        }
                        return HandleKeyResult::WaitForMore;
                    }
                }
            }
        }

        let key_str = key.canonical_string();
        if self.trie_keys.is_empty() {
            match key_str.as_str() {
                scope_key @ ("i" | "a") => {
                    let scope =
                        TextObjectScope::from_key(scope_key).expect("valid text object scope");
                    self.state = VisualState::TextObjectPending(scope);
                    return HandleKeyResult::WaitForMore;
                }
                "f" => {
                    self.state = VisualState::CharScanPending(CharScanData {
                        kind: FindKind::Find,
                        direction: Direction::Forward,
                    });
                    return HandleKeyResult::WaitForMore;
                }
                "F" => {
                    self.state = VisualState::CharScanPending(CharScanData {
                        kind: FindKind::Find,
                        direction: Direction::Backward,
                    });
                    return HandleKeyResult::WaitForMore;
                }
                "t" => {
                    self.state = VisualState::CharScanPending(CharScanData {
                        kind: FindKind::Till,
                        direction: Direction::Forward,
                    });
                    return HandleKeyResult::WaitForMore;
                }
                "T" => {
                    self.state = VisualState::CharScanPending(CharScanData {
                        kind: FindKind::Till,
                        direction: Direction::Backward,
                    });
                    return HandleKeyResult::WaitForMore;
                }
                _ => {}
            }
        }

        self.trie_keys.push(key_str);

        if let Some(intent) = self.keymap.get_action(&self.trie_keys) {
            let result = match intent.as_editor_action().cloned() {
                Some(mut action) => {
                    if let Some(reg) = self.register.take() {
                        action = action.with_register(reg);
                    }
                    if self.count > 1 {
                        if let Some(counted) = action.clone().with_count(self.count) {
                            action = counted;
                        }
                    }
                    HandleKeyResult::Complete(action.with_from_mode(self.mode_kind).into())
                }
                None => HandleKeyResult::complete(intent),
            };
            self.reset();
            return result;
        }

        if self.keymap.is_prefix(&self.trie_keys) || self.count > 0 || self.register.is_some() {
            return HandleKeyResult::WaitForMore;
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn handle_text_object_pending(&mut self, key: &Key, scope: TextObjectScope) -> HandleKeyResult {
        let key_str = key.canonical_string();
        if let Some(text_object) = text_object::resolve(scope, &key_str) {
            self.reset();
            return HandleKeyResult::Complete(
                EditorAction::new(EditorOperation::VisualTextObject(text_object))
                    .with_from_mode(self.mode_kind)
                    .into(),
            );
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn handle_char_scan_pending(&mut self, key: &Key, data: CharScanData) -> HandleKeyResult {
        if let KeyCode::Char(c) = key.code
            && !key.modifiers.has_ctrl()
        {
            let mut action = match (data.kind, data.direction) {
                (FindKind::Find, Direction::Forward) => EditorAction::find_forward(c),
                (FindKind::Find, Direction::Backward) => EditorAction::find_backward(c),
                (FindKind::Till, Direction::Forward) => EditorAction::till_forward(c),
                (FindKind::Till, Direction::Backward) => EditorAction::till_backward(c),
            };
            if let Some(reg) = self.register.take() {
                action = action.with_register(reg);
            }
            if self.count > 1 {
                if let Some(counted) = action.clone().with_count(self.count) {
                    action = counted;
                }
            }
            self.reset();
            return HandleKeyResult::Complete(action.with_from_mode(self.mode_kind).into());
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    pub(super) fn is_waiting(&self) -> bool {
        !matches!(self.state, VisualState::Idle)
            || self.count > 0
            || self.register.is_some()
            || self.pending_register
            || !self.trie_keys.is_empty()
    }

    pub(super) fn clear_buffer(&mut self) {
        self.reset();
    }

    pub(super) fn kind(&self) -> ModeKind {
        self.mode_kind
    }
}

fn build_visual_keymap(exit_key: &str, switch_key: &str, switch_to: ModeKind) -> TrieKeymap {
    let mut keymap = TrieKeymap::<Intent>::new();

    keymap.insert_str("h", EditorAction::new(EditorOperation::MoveLeft));
    keymap.insert_str("j", EditorAction::new(EditorOperation::MoveDown));
    keymap.insert_str("k", EditorAction::new(EditorOperation::MoveUp));
    keymap.insert_str("l", EditorAction::new(EditorOperation::MoveRight));

    keymap.insert_str("w", EditorAction::forward_to(Boundary::Word));
    keymap.insert_str("b", EditorAction::back_to(Boundary::Word));
    keymap.insert_str("e", EditorAction::forward_to(Boundary::WordEnd));

    keymap.insert_str("W", EditorAction::forward_to(Boundary::BigWord));
    keymap.insert_str("B", EditorAction::back_to(Boundary::BigWord));
    keymap.insert_str("E", EditorAction::forward_to(Boundary::BigWordEnd));

    keymap.insert_str("0", EditorAction::new(EditorOperation::MoveToLineStart));
    keymap.insert_str(
        "^",
        EditorAction::new(EditorOperation::MoveToLineContentStart),
    );
    keymap.insert_str("$", EditorAction::new(EditorOperation::MoveToLineEnd));
    keymap.insert_str("gg", EditorAction::new(EditorOperation::MoveToFirstLine));
    keymap.insert_str("G", EditorAction::new(EditorOperation::MoveToLastLine));
    keymap.insert_str("H", EditorAction::new(EditorOperation::MoveToScreenTop));
    keymap.insert_str("M", EditorAction::new(EditorOperation::MoveToScreenMiddle));
    keymap.insert_str("L", EditorAction::new(EditorOperation::MoveToScreenBottom));
    keymap.insert_str(
        "gu",
        EditorAction::operation(Operator::Lowercase, OperatorTarget::Selection)
            .with_to_mode(ModeKind::Normal),
    );
    keymap.insert_str(
        "gU",
        EditorAction::operation(Operator::Uppercase, OperatorTarget::Selection)
            .with_to_mode(ModeKind::Normal),
    );
    keymap.insert_str(
        "g~",
        EditorAction::operation(Operator::ToggleCase, OperatorTarget::Selection)
            .with_to_mode(ModeKind::Normal),
    );
    keymap.insert_str(
        "{",
        EditorAction::new(EditorOperation::MoveToPreviousParagraph),
    );
    keymap.insert_str("}", EditorAction::new(EditorOperation::MoveToNextParagraph));
    keymap.insert_str(
        "%",
        EditorAction::new(EditorOperation::MoveToMatchingBracket),
    );
    keymap.insert_str(";", EditorAction::new(EditorOperation::RepeatLastFind));
    keymap.insert_str(
        ",",
        EditorAction::new(EditorOperation::RepeatLastFindReverse),
    );
    keymap.insert_str(exit_key, EditorAction::mode_transition(ModeKind::Normal));
    keymap.insert_str(switch_key, EditorAction::mode_transition(switch_to));
    keymap.insert_str(
        "d",
        EditorAction::new(EditorOperation::DeleteSelection).with_to_mode(ModeKind::Normal),
    );
    keymap.insert_str(
        "y",
        EditorAction::new(EditorOperation::YankSelection).with_to_mode(ModeKind::Normal),
    );
    keymap.insert_str(
        "c",
        EditorAction::new(EditorOperation::ChangeSelection).with_to_mode(ModeKind::Insert),
    );
    register_visual_surround_bindings(&mut keymap);
    keymap.insert_str("<Left>", EditorAction::new(EditorOperation::MoveLeft));
    keymap.insert_str("<Down>", EditorAction::new(EditorOperation::MoveDown));
    keymap.insert_str("<Up>", EditorAction::new(EditorOperation::MoveUp));
    keymap.insert_str("<Right>", EditorAction::new(EditorOperation::MoveRight));
    keymap.insert_str("<PageUp>", EditorAction::new(EditorOperation::MovePageUp));
    keymap.insert_str(
        "<PageDown>",
        EditorAction::new(EditorOperation::MovePageDown),
    );
    keymap.insert_str("<C-u>", EditorAction::new(EditorOperation::MoveHalfPageUp));
    keymap.insert_str(
        "<C-d>",
        EditorAction::new(EditorOperation::MoveHalfPageDown),
    );

    keymap
}

fn register_visual_surround_bindings(trie_keymap: &mut TrieKeymap) {
    for (selector, delimiter) in surround::delimiter_selectors() {
        trie_keymap.insert_str(
            &format!("gsa{selector}"),
            EditorAction::new(EditorOperation::SurroundAddSelection {
                delimiter: *delimiter,
            })
            .with_to_mode(ModeKind::Normal),
        );
    }
}
