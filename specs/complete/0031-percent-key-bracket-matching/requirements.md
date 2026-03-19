# Percent Key Bracket Matching

## Summary

Add support for the `%` key in normal mode to jump between matching opening and closing brackets (parentheses, square brackets, curly braces), matching vim's behavior.

## Problem Statement

Users familiar with vim expect to be able to quickly navigate between matching brackets using the `%` key. Without this feature, users must manually find matching brackets, which slows down navigation in code files with frequent bracket pairs.

## User Stories

- As a user, I want to press `%` on a `(` character to jump to its matching `)`, so that I can quickly navigate between bracket pairs.
- As a user, I want to press `%` on a `)` character to jump back to its matching `(`, so that I can navigate backward through bracket pairs.
- As a user, I want `%` to work with all common bracket types (`()`, `[]`, `{}`), so that I can navigate any bracket type in my code.
- As a user, I want `%` to do nothing when not on a bracket character, so that I get predictable behavior.

## Functional Requirements

- [ ] **REQ-001**: Pressing `%` when cursor is on `(` should jump to matching `)`
- [ ] **REQ-002**: Pressing `%` when cursor is on `)` should jump to matching `(`
- [ ] **REQ-003**: Pressing `%` when cursor is on `[` should jump to matching `]`
- [ ] **REQ-004**: Pressing `%` when cursor is on `]` should jump to matching `[`
- [ ] **REQ-005**: Pressing `%` when cursor is on `{` should jump to matching `}`
- [ ] **REQ-006**: Pressing `%` when cursor is on `}` should jump to matching `{`
- [ ] **REQ-007**: Pressing `%` when cursor is NOT on a bracket should do nothing (no error, no movement)
- [ ] **REQ-008**: The `%` key should work in normal mode only
- [ ] **REQ-009**: When no matching bracket exists in the buffer, `%` should do nothing

## Non-Functional Requirements

- **Performance**: Bracket matching should complete in O(n) time where n is the distance to the matching bracket
- **Reliability**: Bracket matching must correctly handle nested brackets (e.g., `((foo))`)

## Acceptance Criteria

- [ ] **AC-001**: Pressing `%` on `(` at position 5 jumps to the matching `)` at position 15 in text `function(foo) {`
- [ ] **AC-002**: Pressing `%` on `)` at position 15 jumps back to matching `(` at position 7 in text `function(foo)`
- [ ] **AC-003**: Pressing `%` on `[` in `[foo, bar]` jumps to matching `]`
- [ ] **AC-004**: Pressing `%` on `{` in `{ a: 1 }` jumps to matching `}`
- [ ] **AC-005**: Pressing `%` on nested brackets `((a))` works correctly (first `%` jumps to middle, second to end)
- [ ] **AC-006**: Pressing `%` on a non-bracket character does nothing
- [ ] **AC-007**: Pressing `%` when no matching bracket exists does nothing

## Out of Scope

- Counting support (e.g., `3%` to jump to 3rd bracket pair) - this feature will not be countable
- Angle brackets `<>` support
- Quote matching (`"`, `'`)
- Vim's `%` command for jumping between if/else/endif blocks

## Assumptions

- The buffer provides access to characters by line and column
- The cursor position is tracked as (line, column)
- Bracket matching considers the entire buffer (not just current line)

## Dependencies

- None - this is a self-contained feature
