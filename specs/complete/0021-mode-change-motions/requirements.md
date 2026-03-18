# Mode-Change Motions: a, A, I

## Summary

Implement vim-style mode-change motions `a`, `A`, and `I` in normal mode. These motions switch from normal mode to insert mode at specific cursor positions: after the cursor (`a`), at the end of the current line (`A`), and at the first non-whitespace of the current line (`I`).

## Problem Statement

Currently, urvim only supports `i` for entering insert mode at the current cursor position. Vim users expect additional mode-change motions that position the cursor before switching modes:

- `a` (append) - enters insert mode one character to the right of the current position
- `A` (append to line end) - enters insert mode at the end of the current line  
- `I` (insert at line start) - enters insert mode at the first non-whitespace character of the current line

These are fundamental vim motions that enhance editing productivity.

## User Stories

- As a vim user, I want `a` to append after the current character so I can quickly insert text after my cursor position.
- As a vim user, I want `A` to append at the end of the line so I can quickly append to the current line without navigating to the end first.
- As a vim user, I want `I` to insert at the line's first non-whitespace so I can quickly edit at the beginning of a line (after any leading whitespace).
- As a vim user, I want `3I` to go to line 3 and insert at its first non-whitespace, and `3A` to go to line 3 and append at its end, so I can efficiently edit multiple lines.

## Functional Requirements

- [ ] **REQ-001**: `a` key in normal mode moves cursor one character to the right (if possible) and switches to insert mode.
- [ ] **REQ-002**: `A` key in normal mode moves cursor to end of current line and switches to insert mode.
- [ ] **REQ-003**: `I` key in normal mode moves cursor to first non-whitespace of current line and switches to insert mode.
- [ ] **REQ-004**: `I` with a count prefix (e.g., `3I`) treats `I` as a line action - goes to the specified line (1-indexed), then moves to its first non-whitespace and enters insert mode.
- [ ] **REQ-005**: `A` with a count prefix (e.g., `3A`) treats `A` as a line action - goes to the specified line (1-indexed), then moves to its end and enters insert mode.
- [ ] **REQ-006**: `a` does NOT support count prefixes (not a line action, behaves like other non-countable motions).
- [ ] **REQ-007**: At end of line, `a` should attempt to move right (wrapping to next line if needed), matching vim behavior.
- [ ] **REQ-008**: At empty lines, `A` and `I` should both enter insert mode at position 0 (start of line).

## Non-Functional Requirements

- **Performance**: These are single-key actions with immediate response; no performance concerns.
- **Compatibility**: Behavior should match vim for these motions as closely as possible within urvim's existing architecture.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `a` in normal mode on "hel|lo" (where | is cursor) enters insert mode with cursor after "hell" (before "lo").
- [ ] **AC-002**: Pressing `A` in normal mode on "hello" enters insert mode with cursor after "o" (at end of line).
- [ ] **AC-003**: Pressing `I` in normal mode on "  hello" (with leading spaces) enters insert mode with cursor before the first 'h'.
- [ ] **AC-004**: Pressing `3I` goes to line 3 and enters insert mode at its first non-whitespace.
- [ ] **AC-005**: Pressing `3A` goes to line 3 and enters insert mode at its end.
- [ ] **AC-006**: Pressing `a` at end of line attempts to move to next line (if exists), matching vim's wrap behavior.
- [ ] **AC-007**: All existing motions continue to work correctly.

## Out of Scope

- `o` and `O` (insert new line above/below) - separate feature
- Repeat with `.` - separate feature  
- Text objects and operators with these motions - separate features

## Assumptions

- The existing `Action::SwitchToInsert` and mode switching infrastructure is already functional (verified: `i` key works).
- The `is_line_action()` and `with_count()` infrastructure correctly handles line actions with counts.

## Dependencies

- None - all required infrastructure (mode switching, actions, key bindings) already exists.
