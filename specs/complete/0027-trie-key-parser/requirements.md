# Trie-Based Key Parser with Multiplicative Counts

## Summary

Refactor the key parser from a linear-search Vec-based implementation to a trie data structure, enabling efficient O(k) key sequence lookups (where k is the key sequence length) and supporting multiplicative sub-counts that allow counts to appear anywhere in a key sequence.

## Problem Statement

The current key parsing implementation has two limitations:

1. **Linear Search Performance**: The `SimpleKeymap` uses a `Vec<(Vec<String>, Action)>` with linear search for both `get_action()` and `is_prefix()`. With ~50 key bindings, this is acceptable, but as the keymap grows, performance degrades.

2. **Single Count Prefix**: The current implementation only supports a count prefix at the start of a key sequence (e.g., `5dd`). Users cannot specify sub-counts multiplicatively (e.g., `d4d` or `2d2d`).

These limitations prevent the editor from supporting more advanced count patterns that some users prefer, and the linear search is a technical debt item noted in the code (line 191 comment: "linear search for now, can upgrade to trie later").

## User Stories

- **As a** user, **I want** to type `4dd`, `d4d`, or `2d2d` and have them all result in deleting 4 lines **so that** I can use count prefixes flexibly based on my muscle memory.

- **As a** developer, **I want** the key parser to use a trie for O(k) lookups **so that** key sequence matching remains fast as the keymap grows.

- **As a** developer, **I want** the trie to support partial key sequences for timeout-based key chord resolution **so that** multi-key sequences work correctly.

## Functional Requirements

- [ ] **REQ-001**: Replace `SimpleKeymap` Vec-based implementation with a Trie-based implementation
- [ ] **REQ-002**: Implement `TrieKeymap` struct with nodes containing children map and optional action
- [ ] **REQ-003**: Implement `get_action()` method with O(k) time complexity
- [ ] **REQ-004**: Implement `is_prefix()` method with O(k) time complexity
- [ ] **REQ-005**: Support multi-digit counts (e.g., `55`, `123`) anywhere in key sequence
- [ ] **REQ-006**: Support sub-counts that multiply together (e.g., `5d5d` = 25, `12d34d` = 408)
- [ ] **REQ-007**: Digits immediately after an action key start a new sub-count that accumulates until next action key
- [ ] **REQ-008**: Leading digits before the first action key form a multi-digit count (e.g., `55dd` = 55 deletions)
- [ ] **REQ-009**: Handle edge case where "0" alone is MoveToLineStart (not a count)
- [ ] **REQ-010**: Remove count keys from the sequence before action lookup
- [ ] **REQ-011**: Preserve backward compatibility with existing count behavior
- [ ] **REQ-012**: Update `NormalMode::handle_key()` to use the new count parsing logic
- [ ] **REQ-013**: All existing tests pass without modification

## Non-Functional Requirements

- **Performance**: Key lookup must be O(k) where k is the key sequence length
- **Memory**: Trie should use less memory than Vec for typical keymaps (≥10 bindings)
- **Maintainability**: Trie implementation should be self-documenting with clear node structure
- **Extensibility**: Adding new key bindings should not require changes to lookup algorithm

## Acceptance Criteria

- [ ] **AC-001**: `5j`, `10k`, `100l` work as before (single digit count prefix on motions)
- [ ] **AC-002**: `5dd` works as before (single count prefix on operators)
- [ ] **AC-003**: `55dd` results in deleting 55 lines (multi-digit count)
- [ ] **AC-004**: `d5d` results in deleting 5 lines (count after first d, applied to second d)
- [ ] **AC-005**: `d55d` results in deleting 55 lines (multi-digit sub-count)
- [ ] **AC-006**: `2d2d` results in deleting 4 lines (2 * 2 = 4)
- [ ] **AC-007**: `3d3d` results in deleting 9 lines (3 * 3 = 9)
- [ ] **AC-008**: `5d5d5d` results in deleting 125 lines (5 * 5 * 5 = 125)
- [ ] **AC-009**: `12d34d` results in deleting 408 lines (12 * 34 = 408)
- [ ] **AC-010**: `55d5d` results in deleting 275 lines (55 * 5 = 275)
- [ ] **AC-011**: `0` alone is still MoveToLineStart, not a count
- [ ] **AC-012**: Multi-key sequences like `gg`, `gj`, `gk` still work correctly
- [ ] **AC-013**: `5gg`, `5G` work as before (count with line motion)
- [ ] **AC-014**: All existing keymap bindings work without modification

## Out of Scope

- Adding new action types
- Changing the Action enum structure
- Refactoring the Count handler in Window (covered by spec 0026)
- Supporting count modifiers on non-countable actions (handled by existing `with_count` logic)
- Supporting counts after operators that don't take motions (like `c` for change)

## Assumptions

- The canonical string format used for key serialization remains stable
- Count values will remain bounded (≤ 9999) to prevent overflow
- The trie will be rebuilt on each mode creation, not mutated after
- Multi-key sequences will have ≤ 10 keys (typical for vim-like editors)

## Dependencies

- **Blocked by**: None - this is an independent feature
- **Related to**: Spec 0026 (Count Action Handler Refactor) - the trie parser feeds into the count handler

## Count Parsing Algorithm

```rust
/// Parse a key sequence to extract:
/// 1. The action keys (non-count keys)
/// 2. The total multiplicative count
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
fn parse_key_sequence(keys: &[String]) -> (Vec<String>, usize) {
    let mut action_keys = Vec::new();
    let mut total_count: usize = 1;
    let mut current_count: usize = 0;
    let mut has_seen_action = false;

    for key in keys {
        if is_count_digit(key) && !(key.len() == 1 && key == "0") {
            // This is a count digit (but not "0" alone)
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
                total_count *= current_count;
                current_count = 0;
            }
            has_seen_action = true;
            action_keys.push(key.clone());
        }
    }

    // Multiply in any remaining count
    if current_count > 0 {
        total_count *= current_count;
    }

    (action_keys, total_count)
}
```

## Trie Data Structure

```rust
/// A node in the trie, representing a partial key sequence.
struct TrieNode {
    /// Child nodes keyed by the next key in the sequence
    children: HashMap<String, TrieNode>,
    /// Action associated with this complete key sequence (if any)
    action: Option<Action>,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            action: None,
        }
    }
}

/// Trie-based keymap for efficient key sequence matching.
pub struct TrieKeymap {
    root: TrieNode,
}

impl TrieKeymap {
    pub fn new() -> Self {
        Self { root: TrieNode::new() }
    }

    pub fn insert(&mut self, keys: Vec<String>, action: Action) {
        // Walk/create path in trie, set action at leaf
    }

    pub fn get_action(&self, keys: &[String]) -> Option<Action> {
        // Walk trie by keys, return action at leaf if exists
    }

    pub fn is_prefix(&self, keys: &[String]) -> bool {
        // Walk trie by keys, return true if any child exists
    }
}
```
