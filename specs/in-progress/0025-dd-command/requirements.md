# dd Command - Delete Line

## Summary

Implement the "dd" normal mode command to delete the current line, with support for count prefixes (e.g., "2dd" deletes two lines). This brings urvim closer to Vim-compatible behavior.

## Problem Statement

Currently, urvim lacks a line-level delete command. Users can only delete characters individually using "x" (delete forward) or "X" (delete backward). In Vim, "dd" is a fundamental command for line-based editing operations. The absence of this command creates a friction point for users familiar with Vim behavior.

## User Stories

- As a user, I want to delete the current line with "dd", so that I can quickly remove lines without selecting them first.
- As a user, I want to delete multiple lines with a count prefix (e.g., "3dd"), so that I can efficiently delete consecutive lines.
- As a Vim user, I expect "dd" to behave similarly to Vim, so that my muscle memory transfers to urvim.

## Functional Requirements

- [ ] **REQ-001**: Pressing "d" followed by "d" in normal mode deletes the current line.
- [ ] **REQ-002**: The cursor moves to the start of the next line after deletion (or previous line if deleting the last line).
- [ ] **REQ-003**: A count prefix (1-9) before "dd" causes that many lines to be deleted (e.g., "3dd" deletes 3 lines).
- [ ] **REQ-004**: The count is processed before "dd" - "2dd" is equivalent to "d2d" (d with count 2, then motion to line).
- [ ] **REQ-005**: If the count would exceed the number of available lines, delete until the last line (no error).
- [ ] **REQ-006**: The deleted text is not placed in a register (simple implementation).

## Non-Functional Requirements

- **Performance**: Line deletion should complete in O(1) time regardless of line length.
- **Compatibility**: Behavior should match Vim's "dd" command as closely as possible.

## Acceptance Criteria

- [ ] **AC-001**: In a buffer with lines [1, 2, 3, 4, 5], positioned on line 2, pressing "dd" results in lines [1, 3, 4, 5] with cursor on line 2 (now containing "3").
- [ ] **AC-002**: In a buffer with lines [1, 2, 3], positioned on line 1, pressing "2dd" results in lines [3] with cursor on line 1.
- [ ] **AC-003**: Pressing "3dd" on a 5-line buffer deletes 3 lines starting from cursor position.
- [ ] **AC-004**: Deleting the last line moves cursor to the previous line.
- [ ] **AC-005**: Deleting a line when there is only one line in the buffer leaves an empty buffer.

## Out of Scope

- Register integration (yanking deleted lines)
- Visual mode "dd" (different behavior in visual mode)
- "d" operator with other motions (e.g., "dw", "d$")
- Undo/redo for the delete operation (handled by existing undo system)

## Assumptions

- The existing count handling mechanism (Action::Count) works correctly for motions
- The buffer's remove() method can handle deleting entire lines
- The normal mode keymap supports sequence handling (e.g., "gg", "gJ")

## Dependencies

- **Internal**: Count prefix handling (already implemented), buffer remove() method (already exists), normal mode sequence handling (already exists)
- **External**: None
