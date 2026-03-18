# Join Line Motions (J and gJ)

## Summary

Implement two new normal-mode motions in urvim: `J` to join the current line with the next line (with a space separator), and `gJ` to join lines without a space. Both motions support count prefixes for joining multiple lines.

## Problem Statement

Users need the ability to join lines together, a fundamental Vim editing operation. Currently, urvim lacks this essential motion. The `J` command (with space) and `gJ` command (without space) are standard Vim motions that should be available for efficient text editing.

## User Stories

- **As a** text editor user, **I want** to join two lines with a space between them **so that** I can quickly combine sentences or fix line breaks in paragraphs.
- **As a** text editor user, **I want** to join lines without adding a space **so that** I can join words that should be contiguous.
- **As a** power user, **I want** to specify a count (e.g., `3J`) **so that** I can join multiple lines at once efficiently.

## Functional Requirements

- [ ] **REQ-001**: `J` motion joins the current line with the next line, inserting a single space character between them
- [ ] **REQ-002**: `gJ` motion joins the current line with the next line without inserting any space
- [ ] **REQ-003**: Both motions accept an optional count prefix (e.g., `2J`, `5gJ`)
- [ ] **REQ-004**: With a count of N, the motion joins N+1 lines (the current line and N subsequent lines)
- [ ] **REQ-005**: After joining, the cursor is positioned at the final join point (end of joined content)
- [ ] **REQ-006**: When joining at the last line of the buffer, the motion does nothing (no next line to join)
- [ ] **REQ-007**: When joining would exceed the buffer end, only join available lines up to the last line
- [ ] **REQ-008**: Both motions are repeatable via the count prefix

## Non-Functional Requirements

- **Performance**: Join operation should be O(n) where n is the total length of joined lines
- **Reliability**: Motion should handle edge cases gracefully (empty lines, single line buffer)

## Acceptance Criteria

- [ ] **AC-001**: Pressing `J` on "hello" (newline) "world" produces "hello world" with cursor at position after 'd'
- [ ] **AC-002**: Pressing `gJ` on "hello" (newline) "world" produces "helloworld" with cursor at position after 'd'
- [ ] **AC-003**: Pressing `2J` on line 1 of "a\nb\nc\nd" produces "a b c" (joins 3 lines)
- [ ] **AC-004**: Pressing `J` on the last line of buffer does nothing
- [ ] **AC-005**: Cursor ends at the end of the joined result after the operation
- [ ] **AC-006**: `3gJ` joins 4 lines without spaces

## Out of Scope

- Visual mode line joining
- Operator-pending mode usage (e.g., `dJ`, `yJ`)
- Join with custom separator characters
- Reversing the join operation

## Assumptions

- The editor has a valid text buffer with lines
- Normal mode is the active mode when the motion is invoked
- The count is parsed from key input before the motion executes

## Dependencies

- Normal mode key handling system
- Count prefix parsing (already implemented in similar motions)
- Text buffer line manipulation API
