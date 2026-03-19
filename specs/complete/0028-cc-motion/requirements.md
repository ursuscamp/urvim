# cc Motion - Change Line

## Summary

Implement the "cc" normal mode command that deletes the current line(s) and enters insert mode, leaving a single blank line. This is analogous to Vim's "cc" command for line replacement.

## Problem Statement

Currently, urvim lacks a command to replace an entire line while entering insert mode. In Vim, "cc" is a fundamental editing command that deletes the current line(s) and places the cursor in insert mode on a blank line. This enables quick line replacement without manually deleting characters or navigating to line ends.

## User Stories

- As a user, I want to replace the current line with "cc", so that I can quickly type a new line in place.
- As a user, I want to replace multiple lines with a count prefix (e.g., "3cc"), so that I can efficiently replace consecutive lines.
- As a Vim user, I expect "cc" to behave similarly to Vim, so that my muscle memory transfers to urvim.

## Functional Requirements

- [ ] **REQ-001**: Pressing "c" followed by "c" in normal mode deletes the current line and enters insert mode on a blank line.
- [ ] **REQ-002**: The cursor positions at the start of the blank line in insert mode.
- [ ] **REQ-003**: A count prefix (1-9) before "cc" causes that many lines to be deleted and replaced (e.g., "3cc" replaces 3 lines with a blank line).
- [ ] **REQ-004**: The count is processed before "cc" - "2cc" is equivalent to "c2c" (c with count 2, then motion to line).
- [ ] **REQ-005**: If the count would exceed the number of available lines, replace until the last line (no error).
- [ ] **REQ-006**: The deleted text is not placed in a register (simple implementation).
- [ ] **REQ-007**: After "cc", the editor is in insert mode ready for typing.

## Non-Functional Requirements

- **Performance**: Line replacement should complete in O(1) time regardless of line length.
- **Compatibility**: Behavior should match Vim's "cc" command as closely as possible.

## Acceptance Criteria

- [ ] **AC-001**: In a buffer with lines [1, 2, 3, 4, 5], positioned on line 2, pressing "cc" results in lines [1, "", 3, 4, 5] with cursor in insert mode on the empty line (line 2).
- [ ] **AC-002**: In a buffer with lines [1, 2, 3], positioned on line 1, pressing "2cc" results in lines ["", 3] with cursor in insert mode on the empty line.
- [ ] **AC-003**: Pressing "3cc" on a 5-line buffer replaces 3 lines starting from cursor position with a single blank line.
- [ ] **AC-004**: When replacing the last line, cursor moves to the previous line's blank replacement.
- [ ] **AC-005**: Replacing a line when there is only one line in the buffer leaves a single empty line in insert mode.
- [ ] **AC-006**: In insert mode after "cc", typing characters inserts them at the beginning of the blank line.

## Out of Scope

- Register integration (yanking deleted lines)
- Visual mode "cc" (different behavior in visual mode)
- "c" operator with other motions (e.g., "cw", "c$")
- Undo/redo for the replace operation (handled by existing undo system)

## Assumptions

- The existing count handling mechanism (Action::Count) works correctly for motions
- The buffer's remove() method can handle deleting entire lines
- The normal mode keymap supports sequence handling (similar to "dd")
- Insert mode entry works correctly from normal mode

## Dependencies

- **Internal**: Count prefix handling (already implemented), buffer remove() method (already exists), normal mode sequence handling (already exists), insert mode transition (similar to "i" command)
- **External**: None
