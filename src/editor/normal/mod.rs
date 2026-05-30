use super::keymap::MAX_COUNT;
use super::text_object::{self, TextObjectScope};
use super::{
    Action, ActionKind, DelimiterFamily, HandleKeyResult, Mode, ModeKind, Operator, OperatorTarget,
    TextObject, TrieKeymap,
};
use crate::globals;
use crate::globals::{Direction, FindKind};
use crate::register::RegisterName;
use crate::terminal::{CursorStyle, Key, KeyCode};

mod bindings;

/// Normal mode for vim-style navigation and commands.
pub struct NormalMode {
    keymap: TrieKeymap,
    state: State,
    count: usize,
    register: Option<RegisterName>,
    pending_register: bool,
    trie_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Idle,
    OperatorPending(OperatorPendingData),
    OperatorTextObjectPending(OperatorTextObjectData),
    ReplacePending,
    CharScanPending(CharScanData),
    OpCharScanPending(OpCharScanData),
    SurroundCommandPending,
    SurroundAddTextObjectPending,
    SurroundAddTargetPending(TextObjectScope),
    SurroundAddDelimiterPending(TextObject),
    SurroundDeleteDelimiterPending,
    SurroundReplaceTargetPending,
    SurroundReplaceReplacementPending(DelimiterFamily),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OperatorPendingData {
    operator: Operator,
    to_mode: Option<ModeKind>,
    sub_count: usize,
    /// Accumulated motion keys (e.g. ["i", "w"] for "diw").
    motion_keys: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CharScanData {
    kind: FindKind,
    direction: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpCharScanData {
    operator: Operator,
    to_mode: Option<ModeKind>,
    kind: FindKind,
    direction: Direction,
    sub_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OperatorTextObjectData {
    operator: Operator,
    to_mode: Option<ModeKind>,
    sub_count: usize,
    scope: TextObjectScope,
}

impl Default for NormalMode {
    fn default() -> Self {
        Self::new()
    }
}

impl NormalMode {
    pub fn new() -> Self {
        let mut keymap = TrieKeymap::new();
        bindings::register(&mut keymap);

        NormalMode {
            keymap,
            state: State::Idle,
            count: 0,
            register: None,
            pending_register: false,
            trie_keys: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.state = State::Idle;
        self.count = 0;
        self.register = None;
        self.pending_register = false;
        self.trie_keys.clear();
    }

    fn resolve_register_selector(selector: char) -> Option<RegisterName> {
        let defaults =
            globals::with_config(|config| config.default_registers.clone()).unwrap_or_default();
        RegisterName::from_prefix(selector, &defaults)
    }

    fn operator_prefix_for_keys(keys: &[String]) -> Option<(Operator, Option<ModeKind>, usize)> {
        let first = keys.first()?;
        match first.as_str() {
            "d" => Some((Operator::Delete, None, 1)),
            "y" => Some((Operator::Yank, None, 1)),
            "c" => Some((Operator::Change, Some(ModeKind::Insert), 1)),
            "g" => match keys.get(1).map(String::as_str) {
                None => None,
                Some("u") => Some((Operator::Lowercase, None, 2)),
                Some("U") => Some((Operator::Uppercase, None, 2)),
                Some("~") => Some((Operator::ToggleCase, None, 2)),
                _ => None,
            },
            _ => None,
        }
    }

    fn apply_register(&self, action: Action) -> Action {
        match self.register {
            Some(reg) => action.with_register(reg),
            None => action,
        }
    }

    fn is_case_operator(operator: Operator) -> bool {
        matches!(
            operator,
            Operator::Lowercase | Operator::Uppercase | Operator::ToggleCase
        )
    }

    fn wrap_count(&self, action: Action) -> Action {
        let mut action = action.with_from_mode(ModeKind::Normal);
        if self.count > 1 {
            if let Some(counted) = action.clone().with_count(self.count) {
                action = counted;
            }
        }
        action
    }

    fn wrap_count_with(&self, action: Action, sub_count: usize) -> Action {
        let mut action = action.with_from_mode(ModeKind::Normal);
        let total = if self.count > 0 {
            self.count.saturating_mul(sub_count.max(1)).min(MAX_COUNT)
        } else {
            sub_count.max(1)
        };
        if total > 1 {
            if let Some(counted) = action.clone().with_count(total) {
                action = counted;
            }
        }
        action
    }

    fn char_scan_action(kind: FindKind, direction: Direction, target: char) -> Action {
        match (kind, direction) {
            (FindKind::Find, Direction::Forward) => Action::find_forward(target),
            (FindKind::Find, Direction::Backward) => Action::find_backward(target),
            (FindKind::Till, Direction::Forward) => Action::till_forward(target),
            (FindKind::Till, Direction::Backward) => Action::till_backward(target),
        }
    }

    fn complete_operator_text_object(
        &mut self,
        operator: Operator,
        to_mode: Option<ModeKind>,
        sub_count: usize,
        text_object: TextObject,
    ) -> HandleKeyResult {
        let mut action = Action::operation(operator, OperatorTarget::TextObject(text_object));
        action = self.apply_register(action);
        if let Some(mode) = to_mode {
            action = action.with_to_mode(mode);
        }
        action = self.wrap_count_with(action, sub_count);
        self.reset();
        HandleKeyResult::Complete(action.into())
    }

    fn complete_surround_add(
        &mut self,
        target: TextObject,
        delimiter: DelimiterFamily,
    ) -> HandleKeyResult {
        let action = Action::new(ActionKind::SurroundAdd { target, delimiter })
            .with_from_mode(ModeKind::Normal);
        self.reset();
        HandleKeyResult::Complete(action.into())
    }

    fn complete_surround_delete(&mut self, target: DelimiterFamily) -> HandleKeyResult {
        let action =
            Action::new(ActionKind::SurroundDelete { target }).with_from_mode(ModeKind::Normal);
        self.reset();
        HandleKeyResult::Complete(action.into())
    }

    fn complete_surround_replace(
        &mut self,
        target: DelimiterFamily,
        replacement: DelimiterFamily,
    ) -> HandleKeyResult {
        let action = Action::new(ActionKind::SurroundReplace {
            target,
            replacement,
        })
        .with_from_mode(ModeKind::Normal);
        self.reset();
        HandleKeyResult::Complete(action.into())
    }

    fn operator_trie_keys(operator: Operator) -> Vec<String> {
        match operator {
            Operator::Delete => vec!["d".to_string()],
            Operator::Yank => vec!["y".to_string()],
            Operator::Change => vec!["c".to_string()],
            Operator::Lowercase => vec!["g".to_string(), "u".to_string()],
            Operator::Uppercase => vec!["g".to_string(), "U".to_string()],
            Operator::ToggleCase => vec!["g".to_string(), "~".to_string()],
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
                && let Some(reg) = Self::resolve_register_selector(c)
            {
                self.register = Some(reg);
                return HandleKeyResult::WaitForMore;
            }
            self.reset();
            return HandleKeyResult::InvalidSequence;
        }

        if self.trie_keys.is_empty() {
            if let KeyCode::Char(c) = key.code {
                if c.is_ascii_digit() {
                    if self.count == 0 && c == '0' {
                        // '0' alone → line-start motion. Fall through to Trie lookup.
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
                "r" => {
                    self.state = State::ReplacePending;
                    return HandleKeyResult::WaitForMore;
                }
                "f" => {
                    self.state = State::CharScanPending(CharScanData {
                        kind: FindKind::Find,
                        direction: Direction::Forward,
                    });
                    return HandleKeyResult::WaitForMore;
                }
                "F" => {
                    self.state = State::CharScanPending(CharScanData {
                        kind: FindKind::Find,
                        direction: Direction::Backward,
                    });
                    return HandleKeyResult::WaitForMore;
                }
                "t" => {
                    self.state = State::CharScanPending(CharScanData {
                        kind: FindKind::Till,
                        direction: Direction::Forward,
                    });
                    return HandleKeyResult::WaitForMore;
                }
                "T" => {
                    self.state = State::CharScanPending(CharScanData {
                        kind: FindKind::Till,
                        direction: Direction::Backward,
                    });
                    return HandleKeyResult::WaitForMore;
                }
                _ => {}
            }
        }

        self.trie_keys.push(key_str);

        if self.trie_keys == ["g", "s", "a"] {
            self.trie_keys.clear();
            self.state = State::SurroundAddTextObjectPending;
            return HandleKeyResult::WaitForMore;
        }

        if self.trie_keys == ["g", "s"] {
            self.trie_keys.clear();
            self.state = State::SurroundCommandPending;
            return HandleKeyResult::WaitForMore;
        }

        if let Some((operator, to_mode, consumed)) = Self::operator_prefix_for_keys(&self.trie_keys)
            && self.trie_keys.len() == consumed
        {
            self.state = State::OperatorPending(OperatorPendingData {
                operator,
                to_mode,
                sub_count: 0,
                motion_keys: Vec::new(),
            });
            self.trie_keys.clear();
            return HandleKeyResult::WaitForMore;
        }

        if let Some(intent) = self.keymap.get_action(&self.trie_keys) {
            let result = match intent.as_action().cloned() {
                Some(mut action) => {
                    action = self.apply_register(action);
                    action = self.wrap_count(action);
                    HandleKeyResult::Complete(action.into())
                }
                None => HandleKeyResult::complete(intent),
            };
            self.reset();
            return result;
        }

        if let Some((operator, to_mode, _)) = Self::operator_prefix_for_keys(&self.trie_keys) {
            let motion_keys = if self.trie_keys.len() > 2 {
                self.trie_keys[2..].to_vec()
            } else {
                Vec::new()
            };
            self.state = State::OperatorPending(OperatorPendingData {
                operator,
                to_mode,
                sub_count: 0,
                motion_keys,
            });
            self.trie_keys.clear();
            return HandleKeyResult::WaitForMore;
        }

        if self.keymap.is_prefix(&self.trie_keys) {
            return HandleKeyResult::WaitForMore;
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn handle_replace_pending(&mut self, key: &Key) -> HandleKeyResult {
        if let KeyCode::Char(c) = key.code
            && !key.modifiers.has_ctrl()
        {
            let mut action = Action::new(ActionKind::ReplaceChar(c));
            action = self.apply_register(action);
            action = self.wrap_count(action);
            self.reset();
            return HandleKeyResult::Complete(action.into());
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }
}

impl Mode for NormalMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        if key.code == KeyCode::Esc {
            self.reset();
            return HandleKeyResult::InvalidSequence;
        }

        let current = std::mem::replace(&mut self.state, State::Idle);

        match current {
            State::Idle => self.handle_idle(key),
            State::ReplacePending => self.handle_replace_pending(key),
            State::CharScanPending(data) => self.char_scan_pending_with_data(key, data),
            State::OperatorPending(data) => self.operator_pending_with_data(key, data),
            State::OperatorTextObjectPending(data) => {
                self.operator_text_object_pending_with_data(key, data)
            }
            State::OpCharScanPending(data) => self.op_char_scan_pending_with_data(key, data),
            State::SurroundCommandPending => self.surround_command_pending(key),
            State::SurroundAddTextObjectPending => self.surround_add_text_object_pending(key),
            State::SurroundAddTargetPending(scope) => self.surround_add_target_pending(key, scope),
            State::SurroundAddDelimiterPending(target) => {
                self.surround_add_delimiter_pending(key, target)
            }
            State::SurroundDeleteDelimiterPending => self.surround_delete_delimiter_pending(key),
            State::SurroundReplaceTargetPending => self.surround_replace_target_pending(key),
            State::SurroundReplaceReplacementPending(target) => {
                self.surround_replace_replacement_pending(key, target)
            }
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBlock
    }

    fn is_waiting(&self) -> bool {
        !matches!(self.state, State::Idle)
            || self.count > 0
            || self.register.is_some()
            || self.pending_register
            || !self.trie_keys.is_empty()
    }

    fn clear_buffer(&mut self) {
        self.reset();
    }

    fn kind(&self) -> ModeKind {
        ModeKind::Normal
    }
}

// ---- Pending-state handlers that own their data ----

impl NormalMode {
    fn char_scan_pending_with_data(&mut self, key: &Key, data: CharScanData) -> HandleKeyResult {
        if let KeyCode::Char(c) = key.code
            && !key.modifiers.has_ctrl()
        {
            let mut action = Self::char_scan_action(data.kind, data.direction, c);
            action = self.apply_register(action);
            action = self.wrap_count(action);
            self.reset();
            return HandleKeyResult::Complete(action.into());
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn operator_pending_with_data(
        &mut self,
        key: &Key,
        mut data: OperatorPendingData,
    ) -> HandleKeyResult {
        if key.code == KeyCode::Esc {
            self.reset();
            return HandleKeyResult::InvalidSequence;
        }

        if let KeyCode::Char(c) = key.code {
            if c.is_ascii_digit() {
                let digit = c as usize - '0' as usize;
                if data.sub_count > 0 || c != '0' {
                    data.sub_count = data.sub_count.saturating_mul(10).saturating_add(digit);
                    if data.sub_count > MAX_COUNT {
                        data.sub_count = MAX_COUNT;
                    }
                    self.state = State::OperatorPending(data);
                    return HandleKeyResult::WaitForMore;
                }
            }
        }

        let key_str = key.canonical_string();
        match key_str.as_str() {
            scope_key @ ("i" | "a")
                if data.motion_keys.is_empty() && !Self::is_case_operator(data.operator) =>
            {
                let scope = TextObjectScope::from_key(scope_key).expect("valid text object scope");
                self.state = State::OperatorTextObjectPending(OperatorTextObjectData {
                    operator: data.operator,
                    to_mode: data.to_mode,
                    sub_count: data.sub_count,
                    scope,
                });
                return HandleKeyResult::WaitForMore;
            }
            "f" => {
                self.state = State::OpCharScanPending(OpCharScanData {
                    operator: data.operator,
                    to_mode: data.to_mode,
                    kind: FindKind::Find,
                    direction: Direction::Forward,
                    sub_count: data.sub_count,
                });
                return HandleKeyResult::WaitForMore;
            }
            "F" => {
                self.state = State::OpCharScanPending(OpCharScanData {
                    operator: data.operator,
                    to_mode: data.to_mode,
                    kind: FindKind::Find,
                    direction: Direction::Backward,
                    sub_count: data.sub_count,
                });
                return HandleKeyResult::WaitForMore;
            }
            "t" => {
                self.state = State::OpCharScanPending(OpCharScanData {
                    operator: data.operator,
                    to_mode: data.to_mode,
                    kind: FindKind::Till,
                    direction: Direction::Forward,
                    sub_count: data.sub_count,
                });
                return HandleKeyResult::WaitForMore;
            }
            "T" => {
                self.state = State::OpCharScanPending(OpCharScanData {
                    operator: data.operator,
                    to_mode: data.to_mode,
                    kind: FindKind::Till,
                    direction: Direction::Backward,
                    sub_count: data.sub_count,
                });
                return HandleKeyResult::WaitForMore;
            }
            _ => {}
        }

        data.motion_keys.push(key_str);
        let op_keys = Self::operator_trie_keys(data.operator);
        let full_keys: Vec<String> = op_keys
            .iter()
            .chain(data.motion_keys.iter())
            .cloned()
            .collect();

        if let Some(intent) = self.keymap.get_action(&full_keys) {
            let result = match intent.as_action().cloned() {
                Some(mut action) => {
                    action = self.apply_register(action);
                    action = self.wrap_count_with(action, data.sub_count);
                    if let Some(mode) = data.to_mode {
                        action = action.with_to_mode(mode);
                    }
                    HandleKeyResult::Complete(action.into())
                }
                None => HandleKeyResult::complete(intent),
            };
            self.reset();
            return result;
        }

        if self.keymap.is_prefix(&full_keys) {
            self.state = State::OperatorPending(data);
            return HandleKeyResult::WaitForMore;
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn operator_text_object_pending_with_data(
        &mut self,
        key: &Key,
        data: OperatorTextObjectData,
    ) -> HandleKeyResult {
        let key_str = key.canonical_string();
        if let Some(text_object) = text_object::resolve(data.scope, &key_str) {
            return self.complete_operator_text_object(
                data.operator,
                data.to_mode,
                data.sub_count,
                text_object,
            );
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn op_char_scan_pending_with_data(
        &mut self,
        key: &Key,
        data: OpCharScanData,
    ) -> HandleKeyResult {
        if let KeyCode::Char(c) = key.code
            && !key.modifiers.has_ctrl()
        {
            let find_state = globals::FindState {
                target_char: c,
                kind: data.kind,
                direction: data.direction,
            };
            let mut action =
                Action::operation(data.operator, OperatorTarget::CharacterScan(find_state));
            action = self.apply_register(action);
            if let Some(mode) = data.to_mode {
                action = action.with_to_mode(mode);
            }
            action = self.wrap_count_with(action, data.sub_count);
            self.reset();
            return HandleKeyResult::Complete(action.into());
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn surround_command_pending(&mut self, key: &Key) -> HandleKeyResult {
        match key.code {
            KeyCode::Char('a') if key.modifiers.is_empty() => {
                self.state = State::SurroundAddTextObjectPending;
                HandleKeyResult::WaitForMore
            }
            KeyCode::Char('d') if key.modifiers.is_empty() => {
                self.state = State::SurroundDeleteDelimiterPending;
                HandleKeyResult::WaitForMore
            }
            KeyCode::Char('r') if key.modifiers.is_empty() => {
                self.state = State::SurroundReplaceTargetPending;
                HandleKeyResult::WaitForMore
            }
            _ => {
                self.reset();
                HandleKeyResult::InvalidSequence
            }
        }
    }

    fn surround_add_text_object_pending(&mut self, key: &Key) -> HandleKeyResult {
        let key_str = key.canonical_string();
        if let Some(scope) = TextObjectScope::from_key(&key_str) {
            self.state = State::SurroundAddTargetPending(scope);
            return HandleKeyResult::WaitForMore;
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn surround_add_target_pending(
        &mut self,
        key: &Key,
        scope: TextObjectScope,
    ) -> HandleKeyResult {
        let key_str = key.canonical_string();
        if let Some(target) = text_object::resolve(scope, &key_str) {
            self.state = State::SurroundAddDelimiterPending(target);
            return HandleKeyResult::WaitForMore;
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn surround_add_delimiter_pending(&mut self, key: &Key, target: TextObject) -> HandleKeyResult {
        let key_str = key.canonical_string();
        if let Some(delimiter) = DelimiterFamily::from_selector_key(&key_str) {
            return self.complete_surround_add(target, delimiter);
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn surround_delete_delimiter_pending(&mut self, key: &Key) -> HandleKeyResult {
        let key_str = key.canonical_string();
        if let Some(target) = DelimiterFamily::from_selector_key(&key_str) {
            return self.complete_surround_delete(target);
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn surround_replace_target_pending(&mut self, key: &Key) -> HandleKeyResult {
        let key_str = key.canonical_string();
        if let Some(target) = DelimiterFamily::from_selector_key(&key_str) {
            self.state = State::SurroundReplaceReplacementPending(target);
            return HandleKeyResult::WaitForMore;
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }

    fn surround_replace_replacement_pending(
        &mut self,
        key: &Key,
        target: DelimiterFamily,
    ) -> HandleKeyResult {
        let key_str = key.canonical_string();
        if let Some(replacement) = DelimiterFamily::from_selector_key(&key_str) {
            return self.complete_surround_replace(target, replacement);
        }

        self.reset();
        HandleKeyResult::InvalidSequence
    }
}
