# Motion Count Parsing

## Summary

Implement count prefix parsing for motions in normal mode. Users can prefix motions with a numeric count (e.g., `5j` to move down 5 lines, `10w` to move forward 10 words) following the pattern `[1-9][0-9]+`.

## Problem Statement

Currently, urvim only supports single-key motions without any count prefix. In vim-like editors, users expect to be able to prefix motions with a count to repeat them multiple times (e.g., `5j` moves down 5 lines). This feature is essential for efficient navigation in a vim-style editor.

## User Stories

- **As a** vim user, **I want** to type `5j` to move down 5 lines, **so that** I can quickly navigate through the buffer without pressing `j` multiple times.

- **As a** vim user, **I want** to type `10w` to move forward 10 words, **so that** I can efficiently jump across large distances.

- **As a** vim user, **I want** to type `3b` to move backward 3 words, **so that** I can navigate backwards by word count.

## Functional Requirements

- [ ] **REQ-001**: Parse numeric prefixes (matching `[1-9][0-9]*`) before motions
- [ ] **REQ-002**: Support basic motion keys with count: `h`, `j`, `k`, `l` for directional movement (repeatable)
- [ ] **REQ-003**: Support word motion keys with count: `w`, `b`, `e` for word navigation (repeatable)
- [ ] **REQ-004**: Support bigword motion keys with count: `W`, `B`, `E` for WORD navigation (repeatable)
- [ ] **REQ-005**: Support line position commands with count: `$` (line end), `0` (line start), `^` (line content start) - go to target absolute line number, then do action
- [ ] **REQ-006**: Repeatable actions (h,j,k,l,w,b,e,W,B,E): apply count by repeating the motion N times
- [ ] **REQ-007**: Line actions ($,0,^): go to target absolute line number, then perform the action
- [ ] **REQ-008**: If count prefix is entered but no valid motion follows, treat as invalid sequence
- [ ] **REQ-009**: Escape key should clear any pending count prefix

## Non-Functional Requirements

- **Performance**: Count parsing should have negligible overhead - O(1) string check
- **Compatibility**: Behavior should match vim's count prefix behavior as closely as reasonable

## Acceptance Criteria

- [ ] **AC-001**: Pressing `5j` moves cursor down 5 lines
- [ ] **AC-002**: Pressing `10w` moves cursor forward 10 words
- [ ] **AC-003**: Pressing `3b` moves cursor backward 3 words
- [ ] **AC-004**: Pressing `2k` moves cursor up 2 lines
- [ ] **AC-005**: Pressing `2l` moves cursor right 2 characters
- [ ] **AC-006**: Pressing `2h` moves cursor left 2 characters
- [ ] **AC-007**: Pressing `4e` moves cursor forward to 4th word end
- [ ] **AC-008**: Pressing `3W` moves cursor forward 3 BIGWORDs
- [ ] **AC-009**: Pressing `2B` moves cursor backward 2 BIGWORDs
- [ ] **AC-010**: Pressing `2E` moves cursor forward to 2nd BIGWORD end
- [ ] **AC-011**: Pressing `2$` moves cursor to end of line 2 (absolute line number)
- [ ] **AC-012**: Pressing `3^` moves cursor to content start of line 3 (absolute line number)
- [ ] **AC-013**: Pressing `2^` moves cursor to content start of line 2 (absolute line number)
- [ ] **AC-014**: Pressing `Esc` after typing digits clears the count
- [ ] **AC-015**: Single digit counts (1-9) work correctly (e.g., `5j`)
- [ ] **AC-016**: Large counts work correctly (e.g., `100j`)

## Out of Scope

- Operator pending mode (e.g., `d5j` for delete 5 lines) - this requires operator support first
- Count display in status line - UI enhancement can be added later

## Assumptions

- The count pattern `[1-9][0-9]*` means: at least one non-zero digit followed by zero or more additional digits
- **Repeatable actions** (h,j,k,l,w,b,e,W,B,E): count means repeat motion N times from current position
- **Line actions** ($,0,^): count is absolute target line number, then perform the action

## Dependencies

- No external dependencies
- Depends on existing motion infrastructure in `editor.rs` and `window.rs`

## Implementation Notes

- The count pattern `[1-9][0-9]*` means: at least one non-zero digit followed by zero or more additional digits
- Single digit counts (1-9) work correctly (e.g., `5j` moves 5 lines)
- Multi-digit counts (e.g., `10w`, `100j`) also work
- Need to modify `NormalMode` to track pending count state
- Need to add `Action::Count(usize, Box<Action>)` variant
- Two types of counted actions:
  - **Repeatable**: loop count times, executing motion each time
  - **Line-absolute**: move to target absolute line number first, then execute action
