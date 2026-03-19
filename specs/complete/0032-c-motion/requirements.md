# C Motion - Change to End of Line

## Summary

Implement the "C" normal mode command that deletes text from cursor position to the end of the line and enters insert mode. With a count n, it removes the remainder of the current line plus n-1 following lines.

## Problem Statement

Currently, urvim lacks a command to quickly change text from the cursor to the end of the line. In Vim, "C" (uppercase) is a fundamental editing command that deletes from cursor to line end (and subsequent lines with count) and places the cursor in insert mode. This enables efficient line editing without navigating to line ends manually.

## User Stories

- As a user, I want to replace text from cursor to end of line with "C", so that I can quickly type new ending text.
- As a user, I want to change multiple lines with a count prefix (e.g., "2C"), so that I can efficiently edit consecutive lines from cursor position.
- As a Vim user, I expect "C" to behave similarly to Vim, so that my muscle memory transfers to urvim.

## Functional Requirements

- [ ] **REQ-001**: Pressing "C" in normal mode deletes all text from cursor position to the end of the current line and enters insert mode at that position.
- [ ] **REQ-002**: With count n, "C" deletes from cursor to end of current line plus n-1 following lines (total n lines affected).
- [ ] **REQ-003**: After "C", the cursor positions at the end of the remaining text on that line (where deletion ended), ready to insert.
- [ ] **REQ-004**: If the count would exceed available lines, delete until the last line (no error).
- [ ] **REQ-005**: The deleted text is not placed in a register (simple implementation).
- [ ] **REQ-006**: After "C", the editor is in insert mode ready for typing.

## Non-Functional Requirements

- **Performance**: Line change should complete in O(1) time regardless of line length.
- **Compatibility**: Behavior should match Vim's "C" command as closely as possible.

## Acceptance Criteria

- [ ] **AC-001**: In a buffer with line "hello world" with cursor on "o" in "hello" (i.e., "hell|o" where "|" represents the cursor position), pressing "C" results in line "hell" with cursor after "hell" in insert mode.
- [ ] **AC-002**: In a buffer with line "hello world" with cursor at position 0 (before "h"), pressing "C" results in an empty line in insert mode.
- [ ] **AC-003**: In a buffer with lines ["hello world", "second line", "third line"], positioned on line 1 with cursor after "hello" (i.e., "hello| world"), pressing "2C" deletes from " world" to end of line 2 (leaving ["hello", "third line"]), with cursor at end of "hello" in insert mode.
- [ ] **AC-004**: Pressing "3C" on the last 3 lines of a buffer deletes the remainder of the current line and the 2 following lines.
- [ ] **AC-005**: When deleting to end of line with count that extends beyond buffer, delete until last line without error.
- [ ] **AC-006**: In insert mode after "C", typing characters inserts them at the cursor position (at the end of remaining text).

## Out of Scope

- Register integration (yanking deleted lines)
- Visual mode "C" (different behavior in visual mode)
- "c" operator with other motions (e.g., "cw", "c$") - though "C" is essentially "c$"
- Undo/redo for the change operation (handled by existing undo system)

## Assumptions

- The existing count handling mechanism (Action::Count) works correctly for motions
- The buffer's remove() method can handle deleting portions of lines
- The normal mode keymap supports single key handling
- Insert mode entry works correctly from normal mode

## Dependencies

- **Internal**: Count prefix handling (already implemented), buffer remove() method (already exists), normal mode key handling (already exists), insert mode transition (similar to "i" command)
- **External**: None
