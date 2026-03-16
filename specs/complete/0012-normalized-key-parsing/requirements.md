# Normalized Key Parsing for Multi-Key Combinations

## Summary

This feature refactors the key parsing mechanism in urvim to use normalized string representations (via `canonical_string()`) instead of raw `Key` data. Each mode will maintain a buffer of normalized keys that is evaluated only when the key sequence conclusively maps to exactly one action or no action at all. This provides a robust foundation for multi-key combinations (like Vim's `dd`, `3j`, `ciw`) and enables future configurable keybindings.

## Problem Statement

Currently, key parsing in urvim uses direct pattern matching on `(KeyCode, Modifiers)` tuples in the `Mode::handle_key()` method. This approach has several limitations:

1. **No multi-key support**: Single-key presses are immediately mapped to actions with no ability to recognize multi-key sequences (e.g., `dd` for delete line, `3j` for move down 3 lines)
2. **Hardcoded keybindings**: Keybindings are embedded in pattern matching code, making it difficult to allow users to customize keybindings
3. **Inconsistent representation**: Different parts of the codebase may represent the same key combination differently
4. **No prefix matching**: There's no mechanism to hold a partial key sequence while waiting for more input to determine the action

The canonical string representation exists (spec 0011) but is not being leveraged for action mapping.

## User Stories

- **As a** developer, **I want** key parsing to use normalized strings, **so that** keybindings can be configured via data structures rather than hardcoded pattern matching.

- **As a** user, **I want** multi-key commands like `dd` (delete line) and `3j` (move down 3 lines) to work, **so that** I can use Vim-style key sequences.

- **As a** future developer, **I want** a keymap data structure that maps normalized key strings to actions, **so that** I can implement user-configurable keybindings without changing core logic.

## Functional Requirements

- [ ] **REQ-001**: Refactor `NormalMode` to maintain a `Vec<String>` of normalized keys (via `canonical_string()`) as pending input buffer
- [ ] **REQ-002**: Refactor `InsertMode` to maintain a `Vec<String>` of normalized keys as pending input buffer
- [ ] **REQ-003**: Implement a method on each mode to process a new key and return an `Action`, evaluating the pending key buffer
- [ ] **REQ-004**: The mode should evaluate the pending buffer when:
  - The current key unambiguously maps to exactly one action (execute immediately)
  - The current key results in no possible action (discard buffer and ignore)
  - The current key could be a prefix of a longer sequence (hold for more input)
- [ ] **REQ-005**: The mode should support a "complete" state where a key sequence has been fully recognized and executed
- [ ] **REQ-006**: Create a `Keymap` trait or struct that defines the mapping from normalized key strings to actions
- [ ] **REQ-007**: Initial implementation should support single-key actions (backward compatible with current behavior)
- [ ] **REQ-008**: The key parsing should handle the case where a partial sequence matches multiple possible actions (wait for more input)
- [ ] **REQ-009**: Escape key should always clear the pending buffer and return to a clean state

## Non-Functional Requirements

- **Performance**: Key lookup should be O(1) using HashMap for single-key resolution
- **Reliability**: The system must not lose keypresses or produce unexpected actions
- **Maintainability**: The normalized string approach should make it easy to add new keybindings

## Acceptance Criteria

- [ ] **AC-001**: `NormalMode` maintains a `Vec<String>` of normalized keys internally
- [ ] **AC-002**: `InsertMode` maintains a `Vec<String>` of normalized keys internally
- [ ] **AC-003**: Single keys that map to actions work identically to before (backward compatible)
- [ ] **AC-004**: Keys that don't map to any action return `Action::None` and clear the buffer
- [ ] **AC-005**: The canonical string representation is used for all key comparisons
- [ ] **AC-006**: Escape key clears any pending buffer and returns to idle state
- [ ] **AC-007**: The design supports future multi-key sequences (e.g., `dd`, `3j`)
- [ ] **AC-008**: The mode can report its current state (idle vs. waiting for more keys)
- [ ] **AC-009**: All existing tests pass with the refactored implementation

## Out of Scope

- Implementing specific multi-key commands (like `dd`, `ciw`, `3j`)
- User-facing keybinding configuration UI
- Loading/saving keybindings from files
- Mouse events
- Key sequence recording/playback (macros)

## Assumptions

- The existing `canonical_string()` method on `Key` produces consistent, unique strings for all key combinations
- The `Mode` trait will need to be modified to support stateful key handling
- The main event loop will need minimal changes to accommodate the new mode interface

## Dependencies

- **Internal**: Depends on existing `canonical_string()` implementation in `src/terminal/keys.rs` (spec 0011)
- **Blocked by**: None - canonical string feature is complete

## Future Considerations

This feature creates the foundation for:
- Multi-key command sequences (operator-motion, count-prefixes)
- User-configurable keybindings via config file
- Key sequence macros and replay
- Command-line keybinding inspection (`:map` command)
