//! Editor module for vim-style modal editing.
//!
//! This module provides the Mode trait and implementations for Normal and Insert modes,
//! along with the Action enum that represents actions triggered by keypresses.

use crate::buffer::Boundary;
use crate::motion::chained_keymap::ChainedKeymap;
use crate::motion::char_scan_keymap::CharScanKeymap;
use crate::terminal::{CursorStyle, Key, KeyCode};
use std::collections::BTreeMap;

/// Operators that wait for a motion or text object to define the target region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
}

/// Text objects that define a selection region for use with operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObject {
    InnerWord,
    AroundWord,
}

/// Actions that the main event loop processes.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Move cursor left
    MoveLeft,
    /// Move cursor down
    MoveDown,
    /// Move cursor up
    MoveUp,
    /// Move cursor right
    MoveRight,
    /// Insert a character at cursor position
    InsertChar(char),
    /// Switch to Normal mode
    SwitchToNormal,
    /// Switch to Insert mode
    SwitchToInsert,
    /// Quit the editor
    Quit,
    /// No action (ignored key)
    None,
    /// Move forward to boundary
    ForwardTo(Boundary),
    /// Move backward to boundary
    BackTo(Boundary),
    /// Move cursor to end of current line
    MoveToLineEnd,
    /// Move cursor to absolute start of line (column 0)
    MoveToLineStart,
    /// Move cursor to first non-whitespace of line
    MoveToLineContentStart,
    /// Move cursor to first line of file (or line N with count)
    MoveToFirstLine,
    /// Move cursor to last line of file (or line N with count)
    MoveToLastLine,
    /// Move cursor to top of screen (or N lines from top with count)
    MoveToScreenTop,
    /// Move cursor to middle of screen
    MoveToScreenMiddle,
    /// Move cursor to bottom of screen (or N lines from bottom with count)
    MoveToScreenBottom,
    /// Delete character before cursor (backspace)
    DeleteBackward,
    /// Delete character at cursor (delete key)
    DeleteForward,
    /// Join current line with next line (with space)
    JoinWithSpace,
    /// Join current line with next line (without space)
    JoinWithoutSpace,
    /// Delete current line (or N lines with count prefix)
    DeleteLine,
    /// Change current line: delete line(s) and enter insert mode, leaving one blank line
    ChangeLine,
    /// Change from cursor to end of line: delete to EOL and enter insert mode
    ChangeToLineEnd,
    /// Append after cursor position and enter insert mode
    AppendAfterCursor,
    /// Append to end of line and enter insert mode
    AppendToLineEnd,
    /// Insert at first non-whitespace of line and enter insert mode
    InsertAtLineStart,
    /// Open a new line below current line and enter insert mode
    OpenLineBelow,
    /// Open a new line above current line and enter insert mode
    OpenLineAbove,
    /// Move cursor to matching bracket
    MoveToMatchingBracket,
    /// Move cursor to blank line before the previous paragraph
    MoveToPreviousParagraph,
    /// Move cursor to blank line before the next paragraph
    MoveToNextParagraph,
    /// Find forward: move cursor to the next occurrence of char
    FindForward(char),
    /// Find backward: move cursor to the previous occurrence of char
    FindBackward(char),
    /// Till forward: move cursor to the position before the next occurrence of char
    TillForward(char),
    /// Till backward: move cursor to the position after the previous occurrence of char
    TillBackward(char),
    /// Repeat the last character search in the same direction (';')
    RepeatLastFind,
    /// Repeat the last character search in the opposite direction (',')
    RepeatLastFindReverse,
    /// Undo the last change
    Undo,
    /// Redo the last undone change
    Redo,
    /// Count prefix: repeats the inner action the specified number of times,
    /// or goes to the target absolute line number for line actions.
    Count(usize, Box<Action>),
    /// Compositional operation: apply an operator to a text object.
    /// Examples: Operation(Delete, InnerWord) = "diw", Operation(Delete, AroundWord) = "daw"
    Operation(Operator, TextObject),
}

impl Action {
    /// Returns true if this action is a horizontal movement that should reset
    /// the remembered visual column to the current position.
    pub fn resets_remembered_column(&self) -> bool {
        matches!(
            self,
            Action::MoveLeft
                | Action::MoveRight
                | Action::ForwardTo(_)
                | Action::BackTo(_)
                | Action::MoveToLineEnd
                | Action::MoveToLineStart
                | Action::MoveToLineContentStart
                | Action::InsertChar(_)
                | Action::DeleteBackward
                | Action::DeleteForward
                | Action::DeleteLine
                | Action::ChangeLine
                | Action::ChangeToLineEnd
                | Action::JoinWithSpace
                | Action::JoinWithoutSpace
                | Action::AppendAfterCursor
                | Action::AppendToLineEnd
                | Action::InsertAtLineStart
                | Action::OpenLineBelow
                | Action::OpenLineAbove
                | Action::FindForward(_)
                | Action::FindBackward(_)
                | Action::TillForward(_)
                | Action::TillBackward(_)
                | Action::RepeatLastFind
                | Action::RepeatLastFindReverse
        )
    }

    /// Returns true if this action is a vertical movement that should use
    /// and update the remembered visual column.
    pub fn uses_remembered_column(&self) -> bool {
        matches!(
            self,
            Action::MoveUp
                | Action::MoveDown
                | Action::MoveToFirstLine
                | Action::MoveToLastLine
                | Action::MoveToScreenTop
                | Action::MoveToScreenMiddle
                | Action::MoveToScreenBottom
                | Action::MoveToPreviousParagraph
                | Action::MoveToNextParagraph
        )
    }

    /// Returns true if this action is a repeatable motion that can be executed
    /// multiple times with a count prefix. These actions repeat from current position.
    /// Examples: h, j, k, l, w, b, e, W, B, E, gg, G, H, L
    pub fn is_countable(&self) -> bool {
        matches!(
            self,
            Action::MoveLeft
                | Action::MoveRight
                | Action::MoveUp
                | Action::MoveDown
                | Action::ForwardTo(_)
                | Action::BackTo(_)
                | Action::MoveToFirstLine
                | Action::MoveToLastLine
                | Action::MoveToScreenTop
                | Action::MoveToScreenBottom
                | Action::JoinWithSpace
                | Action::JoinWithoutSpace
                | Action::DeleteLine
                | Action::ChangeLine
                | Action::ChangeToLineEnd
                | Action::OpenLineBelow
                | Action::OpenLineAbove
                | Action::FindForward(_)
                | Action::FindBackward(_)
                | Action::TillForward(_)
                | Action::TillBackward(_)
                | Action::RepeatLastFind
                | Action::Operation(_, _)
                | Action::RepeatLastFindReverse
                | Action::MoveToPreviousParagraph
                | Action::MoveToNextParagraph
        )
    }

    /// Returns true if this action is a line action that takes an absolute line count.
    /// The count specifies the target line number (1-indexed), then performs the action.
    /// Examples: $, 0, ^, gg, G, A, I
    pub fn is_line_action(&self) -> bool {
        matches!(
            self,
            Action::MoveToLineEnd
                | Action::MoveToLineStart
                | Action::MoveToLineContentStart
                | Action::MoveToFirstLine
                | Action::MoveToLastLine
                | Action::AppendToLineEnd
                | Action::InsertAtLineStart
        )
    }

    /// Wraps this action in a Count variant if it's countable or a line action.
    pub fn with_count(self, count: usize) -> Option<Action> {
        if (self.is_countable() || self.is_line_action()) && count > 0 && count < 10000 {
            Some(Action::Count(count, Box::new(self)))
        } else {
            None
        }
    }

    /// Returns true if this action switches to insert mode.
    /// For Count actions, recursively checks the inner action.
    pub fn switches_to_insert_mode(&self) -> bool {
        match self {
            Action::SwitchToInsert
            | Action::AppendAfterCursor
            | Action::AppendToLineEnd
            | Action::InsertAtLineStart
            | Action::ChangeLine
            | Action::ChangeToLineEnd
            | Action::OpenLineBelow
            | Action::OpenLineAbove => true,
            Action::Count(_, inner) => inner.switches_to_insert_mode(),
            // Operation doesn't switch to insert mode (only change would)
            Action::Operation(_, _) => false,
            _ => false,
        }
    }

    /// Returns true if this action should trigger a snapshot before execution.
    ///
    /// Mode switches and text-modifying actions in normal mode create snapshots.
    /// Individual insert characters do NOT create snapshots (batched at SwitchToNormal).
    pub fn is_snapshottable(&self) -> bool {
        match self {
            // SwitchToNormal captures post-edit state for redo
            Action::SwitchToNormal => true,

            // Text-modifying actions in normal mode - snapshot
            Action::DeleteBackward
            | Action::DeleteForward
            | Action::DeleteLine
            | Action::ChangeLine
            | Action::ChangeToLineEnd
            | Action::JoinWithSpace
            | Action::JoinWithoutSpace
            | Action::AppendAfterCursor
            | Action::AppendToLineEnd
            | Action::InsertAtLineStart
            | Action::OpenLineBelow
            | Action::OpenLineAbove => true,

            // InsertChar - NO snapshot (handled by SwitchToNormal)
            Action::InsertChar(_) => false,

            // Undo/Redo - no snapshot (they ARE the undo/redo)
            Action::Undo | Action::Redo => false,

            // Count wraps the inner action
            Action::Count(_, inner) => inner.is_snapshottable(),

            // Operation(Delete, _) modifies text - snapshot needed
            Action::Operation(Operator::Delete, _) => true,

            // Everything else (movement, Quit, etc.) - no snapshot
            _ => false,
        }
    }

    /// Returns true if this action should update the cursor in the active snapshot.
    ///
    /// All movement actions update the cursor for undo/redo purposes.
    pub fn updates_snapshot_cursor(&self) -> bool {
        match self {
            // All movement actions update cursor in active snapshot
            Action::MoveLeft
            | Action::MoveDown
            | Action::MoveUp
            | Action::MoveRight
            | Action::ForwardTo(_)
            | Action::BackTo(_)
            | Action::MoveToLineEnd
            | Action::MoveToLineStart
            | Action::MoveToLineContentStart
            | Action::MoveToFirstLine
            | Action::MoveToLastLine
            | Action::MoveToScreenTop
            | Action::MoveToScreenMiddle
            | Action::MoveToScreenBottom
            | Action::MoveToMatchingBracket
            | Action::MoveToPreviousParagraph
            | Action::MoveToNextParagraph
            | Action::FindForward(_)
            | Action::FindBackward(_)
            | Action::TillForward(_)
            | Action::TillBackward(_)
            | Action::RepeatLastFind
            | Action::RepeatLastFindReverse => true,

            Action::Count(_, inner) => inner.updates_snapshot_cursor(),

            // Operation doesn't update cursor during snapshot cursor update phase
            Action::Operation(_, _) => false,

            _ => false,
        }
    }
}

/// Result of processing a key in a mode.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleKeyResult {
    /// A complete action is ready to execute.
    Complete(Action),
    /// Waiting for more keys to complete a sequence.
    WaitForMore,
    /// The key sequence was invalid or incomplete with no possible match.
    InvalidSequence,
}

/// Trait for mapping normalized key sequences to actions.
pub trait Keymap {
    /// Get the action for a key sequence, if one exists.
    fn get_action(&self, keys: &[String]) -> Option<Action>;

    /// Check if the given key sequence could be a prefix of a longer binding.
    fn is_prefix(&self, keys: &[String]) -> bool;

    /// Check if the given key sequence has children (could be extended).
    /// Returns true if there are possible extensions, false otherwise.
    fn has_children(&self, keys: &[String]) -> bool;
}

/// Maximum count value to prevent overflow.
const MAX_COUNT: usize = 9999;

/// Extract leading count digits from a key sequence.
/// Returns (leading_count, remaining_keys).
/// For example: ["1", "0", "w", "d"] → (10, ["w", "d"])
fn extract_leading_count(keys: &[String]) -> (usize, Vec<String>) {
    let mut count_str = String::new();
    let mut remaining = Vec::new();
    let mut found_non_digit = false;

    for key in keys {
        let is_digit = key.len() == 1
            && key
                .chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false);

        if !found_non_digit && is_digit {
            count_str.push_str(key);
        } else {
            found_non_digit = true;
            remaining.push(key.clone());
        }
    }

    // Check if the count string forms a valid count
    if count_str.is_empty() || !CountParser::is_valid_count(&count_str) {
        return (0, keys.to_vec());
    }

    let count: usize = count_str.parse().unwrap_or(0);
    (count, remaining)
}

/// A node in the trie, representing a partial key sequence.
struct TrieNode {
    /// Child nodes keyed by the next key in the sequence.
    /// Using BTreeMap for deterministic iteration order (useful for debugging).
    children: std::collections::BTreeMap<String, TrieNode>,
    /// Action associated with this complete key sequence (if any).
    action: Option<Action>,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: BTreeMap::new(),
            action: None,
        }
    }
}

/// Trie-based keymap for efficient key sequence matching.
///
/// Time complexity:
/// - get_action: O(k) where k = key sequence length
/// - is_prefix: O(k) where k = key sequence length
pub struct TrieKeymap {
    root: TrieNode,
}

impl TrieKeymap {
    /// Creates a new empty keymap.
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    /// Inserts a single-key binding.
    pub fn insert(&mut self, key: String, action: Action) {
        self.insert_sequence(vec![key], action);
    }

    /// Inserts a multi-key sequence binding.
    pub fn insert_sequence(&mut self, keys: Vec<String>, action: Action) {
        let mut current = &mut self.root;
        for key in &keys {
            current = current
                .children
                .entry(key.clone())
                .or_insert_with(TrieNode::new);
        }
        current.action = Some(action);
    }

    /// Get the action for a key sequence, if one exists.
    pub fn get_action(&self, keys: &[String]) -> Option<Action> {
        let mut current = &self.root;
        for key in keys {
            match current.children.get(key) {
                Some(node) => current = node,
                None => return None,
            }
        }
        current.action.clone()
    }

    /// Check if the given key sequence could be a prefix of a longer binding.
    pub fn is_prefix(&self, keys: &[String]) -> bool {
        let mut current = &self.root;
        for key in keys {
            match current.children.get(key) {
                Some(node) => current = node,
                None => return false,
            }
        }
        // Has children OR has an action (complete sequence is also a prefix of itself)
        !current.children.is_empty() || current.action.is_some()
    }

    /// Check if the given key sequence has children (could be extended).
    /// Returns true if there are possible extensions, false otherwise.
    pub fn has_children(&self, keys: &[String]) -> bool {
        let mut current = &self.root;
        for key in keys {
            match current.children.get(key) {
                Some(node) => current = node,
                None => return false,
            }
        }
        !current.children.is_empty()
    }
}

impl Keymap for TrieKeymap {
    fn get_action(&self, keys: &[String]) -> Option<Action> {
        TrieKeymap::get_action(self, keys)
    }

    fn is_prefix(&self, keys: &[String]) -> bool {
        TrieKeymap::is_prefix(self, keys)
    }

    fn has_children(&self, keys: &[String]) -> bool {
        TrieKeymap::has_children(self, keys)
    }
}

impl Default for TrieKeymap {
    fn default() -> Self {
        Self::new()
    }
}

/// Parser that extracts action keys and multiplicative count from key sequences.
pub struct CountParser;

impl CountParser {
    /// Check if a key string is a count digit (1-9).
    /// Note: "0" is NOT a count digit when starting a count (it's MoveToLineStart).
    /// But "0" CAN be part of a multi-digit count (e.g., "10", "100").
    pub fn is_count_digit(s: &str) -> bool {
        s.len() == 1
            && s.chars()
                .next()
                .map(|c| ('1'..='9').contains(&c))
                .unwrap_or(false)
    }

    /// Check if a string is a valid count that can start with 1-9 and contain any digits.
    /// This is different from is_count_digit - it checks the entire string.
    pub fn is_valid_count(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        // Must start with 1-9 (non-zero)
        let first_char = s.chars().next().unwrap();
        if !('1'..='9').contains(&first_char) {
            return false;
        }
        // All characters must be digits
        s.chars().all(|c| c.is_ascii_digit())
    }

    /// Check if a string is a valid count digit that should be treated as part of a count.
    /// This is different from is_count_digit because it includes "0" when it follows other digits.
    /// This should only be called when we know we're building a count (i.e., after seeing a 1-9).
    fn is_count_continuation(s: &str, is_accumulating_count: bool) -> bool {
        // If we're already accumulating a count, allow 0-9
        // If we're starting fresh, only allow 1-9
        if is_accumulating_count {
            Self::is_count_digit(s)
        } else {
            // Only 1-9 when starting fresh (can't have "0" as first digit)
            s.len() == 1
                && s.chars()
                    .next()
                    .map(|c| ('1'..='9').contains(&c))
                    .unwrap_or(false)
        }
    }

    /// Parse a key sequence to extract action keys and total count.
    ///
    /// Returns (action_keys, total_count) where:
    /// - action_keys: The keys that form the actual keybinding (counts removed)
    /// - total_count: The multiplicative product of all count components (always >= 1)
    ///
    /// Rules:
    /// - Leading digits form a multi-digit count (e.g., "55" → 55)
    /// - After each action key, a new sub-count starts (resets accumulator)
    /// - Digits after an action form a new sub-count that multiplies with previous
    /// - "0" alone is NOT a count (it's MoveToLineStart)
    ///
    /// Examples:
    /// - ["5", "j"] → action_keys: ["j"], count: 5
    /// - ["5", "5", "d", "d"] → action_keys: ["d", "d"], count: 55
    /// - ["d", "5", "d"] → action_keys: ["d", "d"], count: 5
    /// - ["d", "5", "5", "d"] → action_keys: ["d", "d"], count: 55
    /// - ["2", "d", "2", "d"] → action_keys: ["d", "d"], count: 4 (2*2)
    /// - ["5", "d", "5", "d", "5", "d"] → action_keys: ["d", "d", "d"], count: 125 (5*5*5)
    /// - ["1", "2", "d", "3", "4", "d"] → action_keys: ["d", "d"], count: 408 (12*34)
    /// - ["0"] → action_keys: ["0"], count: 1 (special case: 0 is motion)
    pub fn parse(keys: &[String]) -> (Vec<String>, usize) {
        let mut action_keys = Vec::new();
        let mut total_count: usize = 1;
        let mut current_count: usize = 0;
        let mut has_seen_action = false;

        for key in keys {
            if Self::is_count_digit(key) {
                // This is a count digit (1-9)
                let digit: usize = key.parse().unwrap_or(0);

                if has_seen_action {
                    // After an action, digits form a NEW sub-count
                    current_count = current_count * 10 + digit;
                } else {
                    // Before first action, accumulate multi-digit count
                    current_count = current_count * 10 + digit;
                }
            } else {
                // This is an action key
                if current_count > 0 {
                    total_count = total_count.saturating_mul(current_count);
                    // Cap at MAX_COUNT to prevent overflow
                    if total_count > MAX_COUNT {
                        total_count = MAX_COUNT;
                    }
                    current_count = 0;
                }
                has_seen_action = true;
                action_keys.push(key.clone());
            }
        }

        // Multiply in any remaining count
        if current_count > 0 {
            total_count = total_count.saturating_mul(current_count);
            if total_count > MAX_COUNT {
                total_count = MAX_COUNT;
            }
        }

        (action_keys, total_count)
    }
}

pub trait Mode {
    /// Process a key event and return the corresponding result.
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult;

    /// Get the cursor style for this mode.
    fn cursor_style(&self) -> CursorStyle;

    /// Whether the mode is waiting for more keys to complete a sequence.
    fn is_waiting(&self) -> bool;

    /// Clear the pending key buffer.
    fn clear_buffer(&mut self);
}

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

        // Movement keys (h, j, k, l)
        trie_keymap.insert("h".to_string(), Action::MoveLeft);
        trie_keymap.insert("j".to_string(), Action::MoveDown);
        trie_keymap.insert("k".to_string(), Action::MoveUp);
        trie_keymap.insert("l".to_string(), Action::MoveRight);

        // Word motions
        trie_keymap.insert("w".to_string(), Action::ForwardTo(Boundary::Word));
        trie_keymap.insert("b".to_string(), Action::BackTo(Boundary::Word));
        trie_keymap.insert("e".to_string(), Action::ForwardTo(Boundary::WordEnd));

        // BigWord motions
        trie_keymap.insert("W".to_string(), Action::ForwardTo(Boundary::BigWord));
        trie_keymap.insert("B".to_string(), Action::BackTo(Boundary::BigWord));
        trie_keymap.insert("E".to_string(), Action::ForwardTo(Boundary::BigWordEnd));

        // Line end navigation
        trie_keymap.insert("$".to_string(), Action::MoveToLineEnd);

        // Line start navigation
        trie_keymap.insert("0".to_string(), Action::MoveToLineStart);
        trie_keymap.insert("^".to_string(), Action::MoveToLineContentStart);

        // gg and G line motions
        trie_keymap.insert_sequence(
            vec!["g".to_string(), "g".to_string()],
            Action::MoveToFirstLine,
        );
        trie_keymap.insert("G".to_string(), Action::MoveToLastLine);

        // H/M/L screen-relative motions
        trie_keymap.insert("H".to_string(), Action::MoveToScreenTop);
        trie_keymap.insert("M".to_string(), Action::MoveToScreenMiddle);
        trie_keymap.insert("L".to_string(), Action::MoveToScreenBottom);

        // Paragraph motions
        trie_keymap.insert("{".to_string(), Action::MoveToPreviousParagraph);
        trie_keymap.insert("}".to_string(), Action::MoveToNextParagraph);

        // Join line motions
        trie_keymap.insert("J".to_string(), Action::JoinWithSpace);
        trie_keymap.insert_sequence(
            vec!["g".to_string(), "J".to_string()],
            Action::JoinWithoutSpace,
        );

        // Mode switching
        trie_keymap.insert("i".to_string(), Action::SwitchToInsert);
        trie_keymap.insert("a".to_string(), Action::AppendAfterCursor);
        trie_keymap.insert("A".to_string(), Action::AppendToLineEnd);
        trie_keymap.insert("I".to_string(), Action::InsertAtLineStart);

        // Open line below/above
        trie_keymap.insert("o".to_string(), Action::OpenLineBelow);
        trie_keymap.insert("O".to_string(), Action::OpenLineAbove);

        // Delete operations
        trie_keymap.insert("x".to_string(), Action::DeleteForward);
        trie_keymap.insert("X".to_string(), Action::DeleteBackward);
        trie_keymap.insert_sequence(vec!["d".to_string(), "d".to_string()], Action::DeleteLine);
        // Delete with text objects (diw, daw)
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "i".to_string(), "w".to_string()],
            Action::Operation(Operator::Delete, TextObject::InnerWord),
        );
        trie_keymap.insert_sequence(
            vec!["d".to_string(), "a".to_string(), "w".to_string()],
            Action::Operation(Operator::Delete, TextObject::AroundWord),
        );
        trie_keymap.insert_sequence(vec!["c".to_string(), "c".to_string()], Action::ChangeLine);
        // Change to end of line
        trie_keymap.insert("C".to_string(), Action::ChangeToLineEnd);

        // Bracket matching
        trie_keymap.insert("%".to_string(), Action::MoveToMatchingBracket);

        // Repeat character search (after f/F/t/T)
        trie_keymap.insert(";".to_string(), Action::RepeatLastFind);
        trie_keymap.insert(",".to_string(), Action::RepeatLastFindReverse);

        // Quit (Ctrl-q)
        trie_keymap.insert("<C-q>".to_string(), Action::Quit);

        // Undo/Redo
        trie_keymap.insert("u".to_string(), Action::Undo);
        trie_keymap.insert("U".to_string(), Action::Redo);

        // Arrow keys for convenience
        trie_keymap.insert("<Left>".to_string(), Action::MoveLeft);
        trie_keymap.insert("<Down>".to_string(), Action::MoveDown);
        trie_keymap.insert("<Up>".to_string(), Action::MoveUp);
        trie_keymap.insert("<Right>".to_string(), Action::MoveRight);

        // Create chained keymap: trie first, then char scan as fallback
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
        // Escape always clears buffer and returns to idle
        if key.code == KeyCode::Esc {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::InvalidSequence;
        }

        // Convert key to canonical string
        let key_str = key.canonical_string();

        // Add key to buffer
        self.buffer.push(key_str.clone());

        // Check if buffer could be a valid count prefix (e.g., "1", "10", "100")
        // If the buffer consists entirely of digits (including 0 in multi-digit numbers)
        // and forms a valid count, wait for more
        let buffer_str: String = self.buffer.iter().cloned().collect();
        let all_digits = self.buffer.iter().all(|k| {
            k.len() == 1
                && k.chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
        });

        if all_digits && CountParser::is_valid_count(&buffer_str) {
            // Could be a count prefix - wait for more keys
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        // Not a valid count prefix - extract leading count digits, then use CountParser for the rest
        // This handles cases like "10w" where "10" is the count and "w" is the action
        let (leading_count, remaining_keys) = extract_leading_count(&self.buffer);

        // If we have a leading count, check if remaining keys form a valid action sequence
        if leading_count > 0 && !remaining_keys.is_empty() {
            // Parse remaining keys for action and any additional counts
            let (action_keys, sub_count) = CountParser::parse(&remaining_keys);

            // Combine counts: leading_count * sub_count
            let total_count = leading_count.saturating_mul(sub_count).min(MAX_COUNT);

            // Try to find action
            if let Some(action) = self.keymap.get_action(&action_keys) {
                // Check if there could be a longer sequence
                if self.keymap.has_children(&action_keys) {
                    self.waiting = true;
                    return HandleKeyResult::WaitForMore;
                }

                self.buffer.clear();
                self.waiting = false;

                // Only wrap with Count if count > 1 and action supports counting
                if total_count > 1
                    && let Some(counted_action) = action.clone().with_count(total_count)
                {
                    return HandleKeyResult::Complete(counted_action);
                }
                return HandleKeyResult::Complete(action);
            }

            // Check if it could be a prefix
            if self.keymap.is_prefix(&action_keys) {
                self.waiting = true;
                return HandleKeyResult::WaitForMore;
            }
        }

        // Fall back to simple parsing
        let (action_keys, count) = CountParser::parse(&self.buffer);

        // Check 1: Is there an exact match?
        if let Some(action) = self.keymap.get_action(&action_keys) {
            // Check if there could be a longer sequence (has children)
            // If yes, wait for more keys. If no, complete the action.
            if self.keymap.has_children(&action_keys) {
                self.waiting = true;
                return HandleKeyResult::WaitForMore;
            }

            // Exact match with no possible extension - complete the action
            self.buffer.clear();
            self.waiting = false;

            // Only wrap with Count if count > 1 and action supports counting
            if count > 1
                && let Some(counted_action) = action.clone().with_count(count)
            {
                return HandleKeyResult::Complete(counted_action);
            }
            // Return action without wrapping (count is 1 or action doesn't support counting)
            return HandleKeyResult::Complete(action);
        }

        // Check 2: Could action_keys be a prefix of a longer sequence?
        if self.keymap.is_prefix(&action_keys) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        // No match found - clear buffer and return invalid
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
}

/// Insert mode for text input.
pub struct InsertMode {
    keymap: TrieKeymap,
    buffer: Vec<String>,
    waiting: bool,
}

impl InsertMode {
    pub fn new() -> Self {
        let mut keymap = TrieKeymap::new();

        // Mode switching
        keymap.insert("<Esc>".to_string(), Action::SwitchToNormal);

        // Quit (Ctrl-q)
        keymap.insert("<C-q>".to_string(), Action::Quit);

        // Arrow keys for cursor movement while in insert mode
        keymap.insert("<Left>".to_string(), Action::MoveLeft);
        keymap.insert("<Down>".to_string(), Action::MoveDown);
        keymap.insert("<Up>".to_string(), Action::MoveUp);
        keymap.insert("<Right>".to_string(), Action::MoveRight);

        // Enter inserts newline
        keymap.insert("<Enter>".to_string(), Action::InsertChar('\n'));

        // Delete operations
        keymap.insert("<Backspace>".to_string(), Action::DeleteBackward);
        keymap.insert("<Delete>".to_string(), Action::DeleteForward);

        InsertMode {
            keymap,
            buffer: Vec::new(),
            waiting: false,
        }
    }
}

impl Default for InsertMode {
    fn default() -> Self {
        InsertMode::new()
    }
}

impl Mode for InsertMode {
    fn handle_key(&mut self, key: &Key) -> HandleKeyResult {
        // Escape always clears buffer and switches to normal
        if key.code == KeyCode::Esc {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(Action::SwitchToNormal);
        }

        // Convert key to canonical string
        let key_str = key.canonical_string();

        // Check for special key bindings first
        let key_str_ref = std::slice::from_ref(&key_str);
        if let Some(action) = self.keymap.get_action(key_str_ref) {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(action);
        }

        // Check if it could be a prefix of a multi-key sequence
        self.buffer.push(key_str);
        if self.keymap.is_prefix(&self.buffer) {
            self.waiting = true;
            return HandleKeyResult::WaitForMore;
        }

        // For printable characters without Ctrl, insert them
        if let KeyCode::Char(c) = key.code
            && !key.modifiers.has_ctrl()
        {
            self.buffer.clear();
            self.waiting = false;
            return HandleKeyResult::Complete(Action::InsertChar(c));
        }

        // No match - clear buffer
        self.buffer.clear();
        self.waiting = false;
        HandleKeyResult::InvalidSequence
    }

    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBar
    }

    fn is_waiting(&self) -> bool {
        self.waiting
    }

    fn clear_buffer(&mut self) {
        self.buffer.clear();
        self.waiting = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::Key;

    fn key(c: char) -> Key {
        Key::new(KeyCode::Char(c))
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
    fn test_normal_mode_move_down() {
        let mut mode = NormalMode::new();
        assert_eq!(handle_and_unwrap(&mut mode, &key('j')), Action::MoveDown);
    }

    #[test]
    fn test_normal_mode_move_up() {
        let mut mode = NormalMode::new();
        assert_eq!(handle_and_unwrap(&mut mode, &key('k')), Action::MoveUp);
    }

    #[test]
    fn test_normal_mode_move_right() {
        let mut mode = NormalMode::new();
        assert_eq!(handle_and_unwrap(&mut mode, &key('l')), Action::MoveRight);
    }

    #[test]
    fn test_normal_mode_switch_to_insert() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('i')),
            Action::SwitchToInsert
        );
    }

    #[test]
    fn test_normal_mode_cursor_style() {
        let mode = NormalMode::new();
        assert_eq!(mode.cursor_style(), CursorStyle::SteadyBlock);
    }

    #[test]
    fn test_insert_mode_insert_char() {
        let mut mode = InsertMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('a')),
            Action::InsertChar('a')
        );
    }

    #[test]
    fn test_insert_mode_escape_switches_to_normal() {
        let mut mode = InsertMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Esc)),
            Action::SwitchToNormal
        );
    }

    #[test]
    fn test_insert_mode_cursor_style() {
        let mode = InsertMode::new();
        assert_eq!(mode.cursor_style(), CursorStyle::SteadyBar);
    }

    #[test]
    fn test_insert_mode_enter_inserts_newline() {
        let mut mode = InsertMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Enter)),
            Action::InsertChar('\n')
        );
    }

    #[test]
    fn test_insert_mode_delete_key() {
        use crate::terminal::Key;
        let mut mode = InsertMode::new();
        // Test Delete key
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Delete)),
            Action::DeleteForward
        );
    }

    #[test]
    fn test_insert_mode_backspace_key() {
        use crate::terminal::Key;
        let mut mode = InsertMode::new();
        // Test Backspace key
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Backspace)),
            Action::DeleteBackward
        );
    }

    #[test]
    fn test_insert_mode_delete_key_simulation() {
        // Simulate what happens when Delete is pressed in insert mode
        // by creating a buffer with text and checking delete behavior
        use crate::terminal::Key;
        let mut mode = InsertMode::new();

        // Verify Delete key triggers DeleteForward
        let action = handle_and_unwrap(&mut mode, &Key::new(KeyCode::Delete));
        assert_eq!(
            action,
            Action::DeleteForward,
            "Delete key should trigger DeleteForward"
        );

        // Verify Backspace triggers DeleteBackward
        let mut mode2 = InsertMode::new();
        let action2 = handle_and_unwrap(&mut mode2, &Key::new(KeyCode::Backspace));
        assert_eq!(
            action2,
            Action::DeleteBackward,
            "Backspace should trigger DeleteBackward"
        );
    }

    #[test]
    fn test_normal_mode_x_key() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('x')),
            Action::DeleteForward
        );
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('X'))),
            Action::DeleteBackward
        );
    }

    #[test]
    fn test_normal_mode_ignore_other_keys() {
        let mut mode = NormalMode::new();
        // 'x' and 'X' are now bound to DeleteForward/DeleteBackward
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('x')),
            Action::DeleteForward
        );
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('X'))),
            Action::DeleteBackward
        );
        // 'a', 'A', 'I' are now bound to mode-change motions
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('a')),
            Action::AppendAfterCursor
        );
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('A'))),
            Action::AppendToLineEnd
        );
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('I'))),
            Action::InsertAtLineStart
        );
        // Other keys still return None
        assert_eq!(handle_and_unwrap(&mut mode, &key('z')), Action::None);
    }

    #[test]
    fn test_normal_mode_word_forward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('w')),
            Action::ForwardTo(Boundary::Word)
        );
    }

    #[test]
    fn test_normal_mode_word_backward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('b')),
            Action::BackTo(Boundary::Word)
        );
    }

    #[test]
    fn test_normal_mode_word_end_forward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('e')),
            Action::ForwardTo(Boundary::WordEnd)
        );
    }

    #[test]
    fn test_normal_mode_bigword_forward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('W'))),
            Action::ForwardTo(Boundary::BigWord)
        );
    }

    #[test]
    fn test_normal_mode_bigword_backward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('B'))),
            Action::BackTo(Boundary::BigWord)
        );
    }

    #[test]
    fn test_normal_mode_bigword_end_forward() {
        use crate::buffer::Boundary;
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('E'))),
            Action::ForwardTo(Boundary::BigWordEnd)
        );
    }

    #[test]
    fn test_normal_mode_move_to_line_end() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('$'))),
            Action::MoveToLineEnd
        );
    }

    #[test]
    fn test_normal_mode_move_to_line_start() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('0'))),
            Action::MoveToLineStart
        );
    }

    #[test]
    fn test_normal_mode_move_to_line_content_start() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &Key::new(KeyCode::Char('^'))),
            Action::MoveToLineContentStart
        );
    }

    // Count parsing tests

    #[test]
    fn test_is_valid_count() {
        // Valid counts (must start with 1-9)
        assert!(CountParser::is_valid_count("1"));
        assert!(CountParser::is_valid_count("5"));
        assert!(CountParser::is_valid_count("9"));
        assert!(CountParser::is_valid_count("10"));
        assert!(CountParser::is_valid_count("99"));
        assert!(CountParser::is_valid_count("100"));
        assert!(CountParser::is_valid_count("9999"));

        // Invalid counts
        assert!(!CountParser::is_valid_count(""));
        assert!(!CountParser::is_valid_count("0")); // Starts with 0
        assert!(!CountParser::is_valid_count("05")); // Starts with 0
        assert!(!CountParser::is_valid_count("00")); // Starts with 0
    }

    #[test]
    fn test_action_is_countable() {
        // Repeatable motions
        assert!(Action::MoveLeft.is_countable());
        assert!(Action::MoveRight.is_countable());
        assert!(Action::MoveUp.is_countable());
        assert!(Action::MoveDown.is_countable());
        assert!(Action::ForwardTo(Boundary::Word).is_countable());
        assert!(Action::BackTo(Boundary::Word).is_countable());

        // Not countable - mode change motions
        assert!(!Action::SwitchToInsert.is_countable());
        assert!(!Action::InsertChar('a').is_countable());
        assert!(!Action::AppendAfterCursor.is_countable());
        assert!(!Action::AppendToLineEnd.is_countable());
        assert!(!Action::InsertAtLineStart.is_countable());
    }

    #[test]
    fn test_action_is_line_action() {
        // Line actions
        assert!(Action::MoveToLineEnd.is_line_action());
        assert!(Action::MoveToLineStart.is_line_action());
        assert!(Action::MoveToLineContentStart.is_line_action());
        // Mode-change line actions
        assert!(Action::AppendToLineEnd.is_line_action());
        assert!(Action::InsertAtLineStart.is_line_action());

        // Not line actions
        assert!(!Action::MoveLeft.is_line_action());
        assert!(!Action::MoveDown.is_line_action());
        assert!(!Action::AppendAfterCursor.is_line_action());
    }

    #[test]
    fn test_action_with_count() {
        // Test countable actions
        let action = Action::MoveDown.clone().with_count(5);
        assert!(action.is_some());
        match action {
            Some(Action::Count(count, inner)) => {
                assert_eq!(count, 5);
                assert_eq!(*inner, Action::MoveDown);
            }
            _ => panic!("Expected Count variant"),
        }

        // Test line actions
        let action = Action::MoveToLineEnd.clone().with_count(3);
        assert!(action.is_some());
        match action {
            Some(Action::Count(count, inner)) => {
                assert_eq!(count, 3);
                assert_eq!(*inner, Action::MoveToLineEnd);
            }
            _ => panic!("Expected Count variant"),
        }

        // Test non-countable actions return None
        let action = Action::SwitchToInsert.clone().with_count(5);
        assert!(action.is_none());

        // Test mode-change line actions (A and I) work with count
        let action = Action::AppendToLineEnd.clone().with_count(3);
        assert!(action.is_some());
        match action {
            Some(Action::Count(count, inner)) => {
                assert_eq!(count, 3);
                assert_eq!(*inner, Action::AppendToLineEnd);
            }
            _ => panic!("Expected Count variant"),
        }

        let action = Action::InsertAtLineStart.clone().with_count(5);
        assert!(action.is_some());
        match action {
            Some(Action::Count(count, inner)) => {
                assert_eq!(count, 5);
                assert_eq!(*inner, Action::InsertAtLineStart);
            }
            _ => panic!("Expected Count variant"),
        }

        // Test a (AppendAfterCursor) is not countable or line action
        let action = Action::AppendAfterCursor.clone().with_count(5);
        assert!(action.is_none());

        // Test invalid counts return None
        let action = Action::MoveDown.clone().with_count(0);
        assert!(action.is_none());

        // Test MoveToFirstLine with count
        let action = Action::MoveToFirstLine.clone().with_count(5);
        assert!(action.is_some());
        match action {
            Some(Action::Count(count, inner)) => {
                assert_eq!(count, 5);
                assert_eq!(*inner, Action::MoveToFirstLine);
            }
            _ => panic!("Expected Count variant"),
        }
    }

    #[test]
    fn test_action_switches_to_insert_mode() {
        // Actions that switch to insert mode
        assert!(Action::SwitchToInsert.switches_to_insert_mode());
        assert!(Action::AppendAfterCursor.switches_to_insert_mode());
        assert!(Action::AppendToLineEnd.switches_to_insert_mode());
        assert!(Action::InsertAtLineStart.switches_to_insert_mode());

        // Other actions do not switch to insert mode
        assert!(!Action::MoveLeft.switches_to_insert_mode());
        assert!(!Action::MoveDown.switches_to_insert_mode());
        assert!(!Action::MoveToLineEnd.switches_to_insert_mode());
        assert!(!Action::SwitchToNormal.switches_to_insert_mode());

        // Count actions with mode-change inner actions should switch to insert mode
        let action = Action::Count(3, Box::new(Action::AppendToLineEnd));
        assert!(action.switches_to_insert_mode());

        let action = Action::Count(5, Box::new(Action::InsertAtLineStart));
        assert!(action.switches_to_insert_mode());

        // Count actions with non-mode-change inner actions should not switch
        let action = Action::Count(3, Box::new(Action::MoveDown));
        assert!(!action.switches_to_insert_mode());
    }

    #[test]
    fn test_count_prefix_single_digit() {
        let mut mode = NormalMode::new();

        // Press '5' - should wait for more
        let result = mode.handle_key(&key('5'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'j' - should get Count(5, MoveDown)
        let result = mode.handle_key(&key('j'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Count(5, _))
        ));
    }

    #[test]
    fn test_count_prefix_multi_digit() {
        let mut mode = NormalMode::new();

        // Press '1' - should wait for more
        let result = mode.handle_key(&key('1'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press '0' - should still be valid count "10"
        let result = mode.handle_key(&key('0'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'w' - should get Count(10, ForwardTo(Word))
        let result = mode.handle_key(&key('w'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Count(10, _))
        ));
    }

    #[test]
    fn test_count_prefix_escape_clears() {
        let mut mode = NormalMode::new();

        // Press '5' - should wait for more
        let result = mode.handle_key(&key('5'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press Escape - should clear and return invalid
        let result = mode.handle_key(&Key::new(KeyCode::Esc));
        assert!(matches!(result, HandleKeyResult::InvalidSequence));

        // Now pressing 'j' should give MoveDown, not Count
        let result = mode.handle_key(&key('j'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::MoveDown)
        ));
    }

    #[test]
    fn test_zero_key_is_line_start() {
        let mut mode = NormalMode::new();

        // Press '0' directly - should be MoveToLineStart, not count
        let result = mode.handle_key(&key('0'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::MoveToLineStart)
        ));
    }

    #[test]
    fn test_gg_motion() {
        let mut mode = NormalMode::new();

        // Press 'g' twice - should get MoveToFirstLine
        let result = mode.handle_key(&key('g'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        let result = mode.handle_key(&key('g'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::MoveToFirstLine)
        ));
    }

    #[test]
    fn test_g_motion() {
        let mut mode = NormalMode::new();

        // Press 'G' - should get MoveToLastLine
        let result = mode.handle_key(&key('G'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::MoveToLastLine)
        ));
    }

    #[test]
    fn test_gg_with_count() {
        let mut mode = NormalMode::new();

        // Press '5' - should wait for more
        let result = mode.handle_key(&key('5'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'g' twice - should get Count(5, MoveToFirstLine)
        let result = mode.handle_key(&key('g'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        let result = mode.handle_key(&key('g'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Count(5, _))
        ));
    }

    #[test]
    fn test_g_with_count() {
        let mut mode = NormalMode::new();

        // Press '5' - should wait for more
        let result = mode.handle_key(&key('5'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'G' - should get Count(5, MoveToLastLine)
        let result = mode.handle_key(&key('G'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Count(5, _))
        ));
    }

    #[test]
    fn test_j_key_join_with_space() {
        let mut mode = NormalMode::new();
        assert_eq!(
            handle_and_unwrap(&mut mode, &key('J')),
            Action::JoinWithSpace
        );
    }

    #[test]
    fn test_gj_key_join_without_space() {
        let mut mode = NormalMode::new();

        // Press 'g' - should wait for more
        let result = mode.handle_key(&key('g'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'J' - should get JoinWithoutSpace
        let result = mode.handle_key(&key('J'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::JoinWithoutSpace)
        ));
    }

    #[test]
    fn test_dd_key_delete_line() {
        let mut mode = NormalMode::new();

        // Press 'd' - should wait for more
        let result = mode.handle_key(&key('d'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'd' - should get DeleteLine
        let result = mode.handle_key(&key('d'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::DeleteLine)
        ));
    }

    #[test]
    fn test_nd_with_count() {
        let mut mode = NormalMode::new();

        // Press '5' - should wait for more
        let result = mode.handle_key(&key('5'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'd' - should wait for more
        let result = mode.handle_key(&key('d'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'd' - should get Count(5, DeleteLine)
        let result = mode.handle_key(&key('d'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Count(5, inner)) if *inner == Action::DeleteLine
        ));
    }

    #[test]
    fn test_j_with_count() {
        let mut mode = NormalMode::new();

        // Press '5' - should wait for more
        let result = mode.handle_key(&key('5'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'J' - should get Count(5, JoinWithSpace)
        let result = mode.handle_key(&key('J'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Count(5, inner)) if *inner == Action::JoinWithSpace
        ));
    }

    #[test]
    fn test_action_join_is_countable() {
        assert!(Action::JoinWithSpace.is_countable());
        assert!(Action::JoinWithoutSpace.is_countable());
    }

    #[test]
    fn test_action_delete_line_is_countable() {
        assert!(Action::DeleteLine.is_countable());
    }

    #[test]
    fn test_action_delete_line_resets_remembered_column() {
        assert!(Action::DeleteLine.resets_remembered_column());
    }

    #[test]
    fn test_action_change_line_is_countable() {
        assert!(Action::ChangeLine.is_countable());
    }

    #[test]
    fn test_action_change_line_resets_remembered_column() {
        assert!(Action::ChangeLine.resets_remembered_column());
    }

    #[test]
    fn test_action_change_line_switches_to_insert_mode() {
        assert!(Action::ChangeLine.switches_to_insert_mode());
    }

    #[test]
    fn test_action_change_line_with_count() {
        let action = Action::ChangeLine.clone().with_count(5);
        assert!(action.is_some());
        if let Some(Action::Count(count, inner)) = action {
            assert_eq!(count, 5);
            assert_eq!(*inner, Action::ChangeLine);
        } else {
            panic!("Expected Count action");
        }
    }

    #[test]
    fn test_action_change_to_line_end_is_countable() {
        assert!(Action::ChangeToLineEnd.is_countable());
    }

    #[test]
    fn test_action_change_to_line_end_resets_remembered_column() {
        assert!(Action::ChangeToLineEnd.resets_remembered_column());
    }

    #[test]
    fn test_action_change_to_line_end_switches_to_insert_mode() {
        assert!(Action::ChangeToLineEnd.switches_to_insert_mode());
    }

    #[test]
    fn test_action_change_to_line_end_with_count() {
        let action = Action::ChangeToLineEnd.clone().with_count(5);
        assert!(action.is_some());
        if let Some(Action::Count(count, inner)) = action {
            assert_eq!(count, 5);
            assert_eq!(*inner, Action::ChangeToLineEnd);
        } else {
            panic!("Expected Count action");
        }
    }

    #[test]
    fn test_c_key_change_to_line_end() {
        let mut mode = NormalMode::new();
        let result = mode.handle_key(&key('C'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::ChangeToLineEnd)
        ));
    }

    #[test]
    fn test_o_key_opens_line_below() {
        let mut mode = NormalMode::new();
        let result = mode.handle_key(&key('o'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::OpenLineBelow)
        ));
    }

    #[test]
    fn test_o_key_is_countable() {
        assert!(Action::OpenLineBelow.is_countable());
    }

    #[test]
    fn test_o_with_count() {
        let mut mode = NormalMode::new();

        // Press '3' - should wait for more
        let result = mode.handle_key(&key('3'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Then press 'o' - should get Count(3, OpenLineBelow)
        let result = mode.handle_key(&key('o'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Count(3, inner)) if *inner == Action::OpenLineBelow
        ));
    }

    #[test]
    fn test_O_key_opens_line_above() {
        let mut mode = NormalMode::new();
        let result = mode.handle_key(&key('O'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::OpenLineAbove)
        ));
    }

    #[test]
    fn test_O_key_is_countable() {
        // O supports count prefix (e.g., 3O creates 3 lines above)
        assert!(Action::OpenLineAbove.is_countable());
    }

    #[test]
    fn test_action_open_line_below_resets_remembered_column() {
        assert!(Action::OpenLineBelow.resets_remembered_column());
    }

    #[test]
    fn test_action_open_line_above_resets_remembered_column() {
        assert!(Action::OpenLineAbove.resets_remembered_column());
    }

    #[test]
    fn test_action_open_line_below_switches_to_insert_mode() {
        assert!(Action::OpenLineBelow.switches_to_insert_mode());
    }

    #[test]
    fn test_action_open_line_above_switches_to_insert_mode() {
        assert!(Action::OpenLineAbove.switches_to_insert_mode());
    }

    #[test]
    fn test_percent_key_moves_to_matching_bracket() {
        let mut mode = NormalMode::new();
        let result = mode.handle_key(&Key::new(KeyCode::Char('%')));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::MoveToMatchingBracket)
        ));
    }

    #[test]
    fn test_percent_key_is_not_countable() {
        assert!(!Action::MoveToMatchingBracket.is_countable());
    }

    // =========================================================================
    // Action::is_snapshottable() and Action::updates_snapshot_cursor() Tests
    // =========================================================================

    #[test]
    fn test_is_snapshottable_mode_switches() {
        // SwitchToNormal should be snapshottable (captures post-edit state)
        assert!(Action::SwitchToNormal.is_snapshottable());
        // SwitchToInsert should NOT be snapshottable
        assert!(!Action::SwitchToInsert.is_snapshottable());
    }

    #[test]
    fn test_is_snapshottable_text_modifying() {
        // Text-modifying actions should be snapshottable
        assert!(Action::DeleteBackward.is_snapshottable());
        assert!(Action::DeleteForward.is_snapshottable());
        assert!(Action::DeleteLine.is_snapshottable());
        assert!(Action::ChangeLine.is_snapshottable());
        assert!(Action::ChangeToLineEnd.is_snapshottable());
        assert!(Action::JoinWithSpace.is_snapshottable());
        assert!(Action::JoinWithoutSpace.is_snapshottable());
        assert!(Action::AppendAfterCursor.is_snapshottable());
        assert!(Action::AppendToLineEnd.is_snapshottable());
        assert!(Action::InsertAtLineStart.is_snapshottable());
        assert!(Action::OpenLineBelow.is_snapshottable());
        assert!(Action::OpenLineAbove.is_snapshottable());
    }

    #[test]
    fn test_is_snapshottable_insert_char() {
        // InsertChar should NOT be snapshottable (batched at SwitchToNormal)
        assert!(!Action::InsertChar('a').is_snapshottable());
    }

    #[test]
    fn test_is_snapshottable_undo_redo() {
        // Undo and Redo should NOT be snapshottable
        assert!(!Action::Undo.is_snapshottable());
        assert!(!Action::Redo.is_snapshottable());
    }

    #[test]
    fn test_is_snapshottable_movement() {
        // Movement actions should NOT be snapshottable
        assert!(!Action::MoveLeft.is_snapshottable());
        assert!(!Action::MoveRight.is_snapshottable());
        assert!(!Action::MoveUp.is_snapshottable());
        assert!(!Action::MoveDown.is_snapshottable());
        assert!(!Action::MoveToLineEnd.is_snapshottable());
        assert!(!Action::MoveToLineStart.is_snapshottable());
        assert!(!Action::MoveToFirstLine.is_snapshottable());
        assert!(!Action::MoveToLastLine.is_snapshottable());
    }

    #[test]
    fn test_is_snapshottable_count() {
        // Count action delegates to inner action
        assert!(Action::Count(5, Box::new(Action::DeleteLine)).is_snapshottable());
        assert!(!Action::Count(5, Box::new(Action::MoveDown)).is_snapshottable());
    }

    #[test]
    fn test_updates_snapshot_cursor_movement() {
        // All movement actions should update snapshot cursor
        assert!(Action::MoveLeft.updates_snapshot_cursor());
        assert!(Action::MoveRight.updates_snapshot_cursor());
        assert!(Action::MoveUp.updates_snapshot_cursor());
        assert!(Action::MoveDown.updates_snapshot_cursor());
        assert!(Action::ForwardTo(Boundary::Word).updates_snapshot_cursor());
        assert!(Action::BackTo(Boundary::Word).updates_snapshot_cursor());
        assert!(Action::MoveToLineEnd.updates_snapshot_cursor());
        assert!(Action::MoveToLineStart.updates_snapshot_cursor());
        assert!(Action::MoveToLineContentStart.updates_snapshot_cursor());
        assert!(Action::MoveToFirstLine.updates_snapshot_cursor());
        assert!(Action::MoveToLastLine.updates_snapshot_cursor());
        assert!(Action::MoveToScreenTop.updates_snapshot_cursor());
        assert!(Action::MoveToScreenMiddle.updates_snapshot_cursor());
        assert!(Action::MoveToScreenBottom.updates_snapshot_cursor());
        assert!(Action::MoveToMatchingBracket.updates_snapshot_cursor());
        assert!(Action::MoveToPreviousParagraph.updates_snapshot_cursor());
        assert!(Action::MoveToNextParagraph.updates_snapshot_cursor());
        assert!(Action::FindForward('a').updates_snapshot_cursor());
        assert!(Action::FindBackward('a').updates_snapshot_cursor());
        assert!(Action::TillForward('a').updates_snapshot_cursor());
        assert!(Action::TillBackward('a').updates_snapshot_cursor());
        assert!(Action::RepeatLastFind.updates_snapshot_cursor());
        assert!(Action::RepeatLastFindReverse.updates_snapshot_cursor());
    }

    #[test]
    fn test_updates_snapshot_cursor_non_movement() {
        // Non-movement actions should NOT update snapshot cursor
        assert!(!Action::InsertChar('a').updates_snapshot_cursor());
        assert!(!Action::SwitchToInsert.updates_snapshot_cursor());
        assert!(!Action::SwitchToNormal.updates_snapshot_cursor());
        assert!(!Action::DeleteBackward.updates_snapshot_cursor());
        assert!(!Action::DeleteForward.updates_snapshot_cursor());
        assert!(!Action::DeleteLine.updates_snapshot_cursor());
        assert!(!Action::Undo.updates_snapshot_cursor());
        assert!(!Action::Redo.updates_snapshot_cursor());
    }

    #[test]
    fn test_updates_snapshot_cursor_count() {
        // Count action delegates to inner action
        assert!(Action::Count(5, Box::new(Action::MoveDown)).updates_snapshot_cursor());
        assert!(!Action::Count(5, Box::new(Action::DeleteLine)).updates_snapshot_cursor());
    }

    #[test]
    fn test_is_snapshottable_all_text_modifying() {
        // All text-modifying actions should be snapshottable
        assert!(Action::DeleteBackward.is_snapshottable());
        assert!(Action::DeleteForward.is_snapshottable());
        assert!(Action::DeleteLine.is_snapshottable());
        assert!(Action::ChangeLine.is_snapshottable());
        assert!(Action::ChangeToLineEnd.is_snapshottable());
        assert!(Action::JoinWithSpace.is_snapshottable());
        assert!(Action::JoinWithoutSpace.is_snapshottable());
        assert!(Action::AppendAfterCursor.is_snapshottable());
        assert!(Action::AppendToLineEnd.is_snapshottable());
        assert!(Action::InsertAtLineStart.is_snapshottable());
        assert!(Action::OpenLineBelow.is_snapshottable());
        assert!(Action::OpenLineAbove.is_snapshottable());
    }

    #[test]
    fn test_is_snapshottable_others() {
        // Quit and None should NOT be snapshottable
        assert!(!Action::Quit.is_snapshottable());
        assert!(!Action::None.is_snapshottable());
    }

    #[test]
    fn test_is_snapshottable_screen_motions() {
        // Screen-relative motions should NOT be snapshottable
        assert!(!Action::MoveToScreenTop.is_snapshottable());
        assert!(!Action::MoveToScreenMiddle.is_snapshottable());
        assert!(!Action::MoveToScreenBottom.is_snapshottable());
    }

    #[test]
    fn test_updates_snapshot_cursor_with_params() {
        // Actions with parameters that are movements should update cursor
        assert!(Action::ForwardTo(Boundary::WordEnd).updates_snapshot_cursor());
        assert!(Action::BackTo(Boundary::BigWord).updates_snapshot_cursor());
    }

    // Text object tests

    #[test]
    fn test_diw_sequence() {
        let mut mode = NormalMode::new();
        // Press 'd' - should wait for more
        let result = mode.handle_key(&key('d'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press 'i' - should still wait
        let result = mode.handle_key(&key('i'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press 'w' - should complete
        let result = mode.handle_key(&key('w'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Operation(Operator::Delete, TextObject::InnerWord))
        ));
    }

    #[test]
    fn test_daw_sequence() {
        let mut mode = NormalMode::new();
        // Press 'd'
        let result = mode.handle_key(&key('d'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press 'a'
        let result = mode.handle_key(&key('a'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press 'w' - should complete
        let result = mode.handle_key(&key('w'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Operation(Operator::Delete, TextObject::AroundWord))
        ));
    }

    #[test]
    fn test_d_alone_waits_for_more() {
        let mut mode = NormalMode::new();
        let result = mode.handle_key(&key('d'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));
    }

    #[test]
    fn test_di_alone_waits_for_more() {
        let mut mode = NormalMode::new();
        // Press 'd'
        let _ = mode.handle_key(&key('d'));
        // Press 'i'
        let result = mode.handle_key(&key('i'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));
    }

    #[test]
    fn test_escape_cancels_sequence() {
        let mut mode = NormalMode::new();
        // Press 'd' - wait
        let _ = mode.handle_key(&key('d'));
        // Press Escape - should cancel
        let result = mode.handle_key(&Key::new(KeyCode::Esc));
        assert!(matches!(result, HandleKeyResult::InvalidSequence));

        // Now pressing 'd' should work fresh
        let result = mode.handle_key(&key('d'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));
    }

    #[test]
    fn test_count_diw() {
        let mut mode = NormalMode::new();

        // Press '3'
        let result = mode.handle_key(&key('3'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press 'd'
        let result = mode.handle_key(&key('d'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press 'i'
        let result = mode.handle_key(&key('i'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press 'w'
        let result = mode.handle_key(&key('w'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Count(3, _))
        ));

        // Check it's the right action
        if let HandleKeyResult::Complete(Action::Count(count, inner)) = result {
            assert_eq!(count, 3);
            assert!(matches!(
                *inner,
                Action::Operation(Operator::Delete, TextObject::InnerWord)
            ));
        }
    }

    #[test]
    fn test_d_count_iw() {
        let mut mode = NormalMode::new();

        // Press 'd'
        let result = mode.handle_key(&key('d'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press '3'
        let result = mode.handle_key(&key('3'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press 'i'
        let result = mode.handle_key(&key('i'));
        assert!(matches!(result, HandleKeyResult::WaitForMore));

        // Press 'w'
        let result = mode.handle_key(&key('w'));
        assert!(matches!(
            result,
            HandleKeyResult::Complete(Action::Count(3, _))
        ));
    }

    #[test]
    fn test_d_operation_switches_to_insert_mode() {
        // Operation should NOT switch to insert mode (only Change would)
        assert!(
            !Action::Operation(Operator::Delete, TextObject::InnerWord).switches_to_insert_mode()
        );
        assert!(
            !Action::Operation(Operator::Delete, TextObject::AroundWord).switches_to_insert_mode()
        );
    }

    #[test]
    fn test_d_operation_is_snapshottable() {
        // Delete operation should be snapshottable
        assert!(Action::Operation(Operator::Delete, TextObject::InnerWord).is_snapshottable());
        assert!(Action::Operation(Operator::Delete, TextObject::AroundWord).is_snapshottable());
    }
}
