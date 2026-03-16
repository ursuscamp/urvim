# Dollar Key - End of Line Navigation

## Summary

Add a new key binding `$` in Normal mode that moves the cursor to the end of the current line. Pressing `$` repeatedly continues moving to the end of subsequent lines, enabling efficient line-wise navigation.

## Problem Statement

Currently, urvim lacks a standard vim-style `$` key binding for moving to the end of lines. Users familiar with vim expect `$` to navigate to the end of the current line, similar to how `^` moves to the beginning (excluding whitespace). This is a fundamental navigation feature that should be available in Normal mode.

## User Stories

- **As a** vim user, **I want** to press `$` to quickly jump to the end of the current line, **so that** I can efficiently navigate to where text editing typically ends.

- **As a** user, **I want** to press `$$` to move to the end of the next line, **so that** I can quickly scan through multiple lines from their endings.

- **As a** user, **I want** the `$` key to behave similarly to other motion keys (like `h`, `j`, `k`, `l`, `w`, `b`, `e`), **so that** the editor feels consistent and intuitive.

## Functional Requirements

- [ ] **REQ-001**: The `$` key in Normal mode shall move the cursor to the last non-whitespace character of the current line.
- [ ] **REQ-002**: If the cursor is already at the end of the current line, pressing `$` again shall move to the end of the next line.
- [ ] **REQ-003**: If the cursor is on the last line of the buffer and already at its end, pressing `$` shall have no effect (stay in place).
- [ ] **REQ-004**: The `$` key shall be handled in the `NormalMode::handle_key` method similar to other movement keys.
- [ ] **REQ-005**: A new `Action::MoveToLineEnd` variant shall be added to represent this action.
- [ ] **REQ-006**: The movement shall be processed by the main event loop and update the cursor position appropriately.

## Non-Functional Requirements

- **Performance**: The `$` key press should respond instantly, with no perceptible delay.
- **Compatibility**: The behavior should match vim's `$` key as closely as possible.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `$` when cursor is at position 0 in "hello" moves cursor to position 4 (last character).
- [ ] **AC-002**: Pressing `$` twice from "hello\nworld" first moves to position 4, then to position 4 of line 2 (end of "world").
- [ ] **AC-003**: Pressing `$` on a single-line buffer with cursor at end of line does nothing.
- [ ] **AC-004**: Unit tests verify the `$` key produces the correct action.
- [ ] **AC-005**: `cargo check` passes with no warnings related to the new code.
- [ ] **AC-006**: `cargo test` passes all existing tests plus new tests.

## Out of Scope

- Visual mode `$` behavior (not implemented yet)
- Count prefix (e.g., `3$` to move to end of 3rd line below - not in initial version)
- Line-beginning navigation (handled by `^` or `0` - separate feature if needed)

## Assumptions

- The cursor position is maintained as a line/column pair
- The buffer provides a method to get line length
- The action system can handle new action types

## Dependencies

- None - this feature is self-contained and builds on existing infrastructure.

## Implementation Notes

- A new `Boundary::LineEnd` may be needed in the buffer module
- The `$` key should be added to the match statement in `NormalMode::handle_key`
- The main event loop needs to handle the new `Action::MoveToLineEnd` variant
