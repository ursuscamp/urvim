# Line Start Motions - Vim-Compatible Navigation

## Summary

Add two new key bindings in Normal mode: `0` to move to the absolute start of the line (column 0), and `^` to move to the first non-whitespace character of the line. Both motions are repeatable, enabling efficient line-wise navigation similar to vim.

## Problem Statement

Currently, urvim lacks standard vim-style `0` and `^` key bindings for moving to the beginning of lines. Users familiar with vim expect these keys to navigate to line starts - `0` goes to column 0, while `^` goes to the first non-whitespace character. These are fundamental navigation features that should be available in Normal mode, complementing the existing `$` key for end-of-line navigation.

## User Stories

- **As a** vim user, **I want** to press `0` to jump to the absolute start of the current line (column 0), **so that** I can quickly position the cursor at the beginning regardless of whitespace.

- **As a** vim user, **I want** to press `^` to jump to the first non-whitespace character of the current line, **so that** I can quickly skip past leading indentation.

- **As a** user, **I want** to press `^^` to move to the first non-whitespace of the previous line, **so that** I can efficiently navigate up through multiple lines.

## Functional Requirements

- [ ] **REQ-001**: The `0` key in Normal mode shall move the cursor to column 0 of the current line (absolute line start).
- [ ] **REQ-002**: The `^` key in Normal mode shall move the cursor to the first non-whitespace character of the current line.
- [ ] **REQ-003**: If the cursor is already at the first non-whitespace position, pressing `^` again shall move to the first non-whitespace of the previous line.
- [ ] **REQ-004**: If the cursor is on the first line of the buffer and already at its first non-whitespace position, pressing `^` shall have no effect (stay in place).
- [ ] **REQ-005**: If the cursor is on the first line of the buffer and already at column 0, pressing `0` shall have no effect.
- [ ] **REQ-006**: The `0` and `^` keys shall be handled in the `NormalMode::handle_key` method similar to other movement keys.
- [ ] **REQ-007**: New `Action::MoveToLineStart` and `Action::MoveToLineContentStart` variants shall be added to represent these actions.
- [ ] **REQ-008**: The movements shall be processed by the main event loop and update the cursor position appropriately.

## Non-Functional Requirements

- **Performance**: The `0` and `^` key presses should respond instantly, with no perceptible delay.
- **Compatibility**: The behavior should match vim's `0` and `^` keys as closely as possible.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `0` when cursor is at position 5 in "  hello" moves cursor to position 0.
- [ ] **AC-002**: Pressing `^` when cursor is at position 0 in "  hello" moves cursor to position 2 (first non-whitespace).
- [ ] **AC-003**: Pressing `^` twice from "  hello\n  world" first moves to position 2, then to position 2 of line 1 (previous line).
- [ ] **AC-004**: Pressing `0` on a line with leading whitespace moves to column 0.
- [ ] **AC-005**: Pressing `^` on a line with no leading whitespace (e.g., "hello") should stay at position 0.
- [ ] **AC-006**: If cursor is on the first line at first non-whitespace and `^` is pressed, no movement occurs.
- [ ] **AC-007**: If cursor is on the first line at column 0 and `0` is pressed, no movement occurs.
- [ ] **AC-008**: Unit tests verify the `0` and `^` keys produce the correct actions.
- [ ] **AC-009**: `cargo check` passes with no warnings related to the new code.
- [ ] **AC-010**: `cargo test` passes all existing tests plus new tests.

## Out of Scope

- Visual mode `0` and `^` behavior (not implemented yet)
- Count prefix (e.g., `3^` to move to first non-whitespace of 3rd line above - not in initial version)
- Combining with operators (e.g., `d^` to delete to first non-whitespace - separate feature)

## Assumptions

- The cursor position is maintained as a line/column pair
- The buffer provides a method to get line content
- The buffer provides a method to get line length
- The action system can handle new action types

## Dependencies

- None - this feature is self-contained and builds on existing infrastructure.
- Related: The `$` key (end of line) was implemented in spec 0008 and serves as a reference.

## Implementation Notes

- The `^` motion is analogous to `$` but in the opposite direction (wrapping to previous line, not next).
- The `0` motion is simpler - it always goes to column 0 and doesn't wrap.
- Similar patterns from the `$` key implementation (spec 0008) should be followed.
