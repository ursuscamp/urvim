# gg and G Line Motions

## Summary

Add the `gg` and `G` line motions to urvim, matching Vim's behavior. These motions allow users to quickly navigate to the first or last line of the file, or to a specific line when used with a count prefix.

## Problem Statement

Users of urvim who are familiar with Vim expect to be able to quickly jump to the beginning or end of a file using `gg` and `G` respectively. Without these motions, users must manually scroll or use alternative (slower) navigation methods. Additionally, when a count is applied (e.g., `5G`), these motions should behave as line motions to maintain consistency with Vim's behavior.

## User Stories

- As a Vim user, I want to press `gg` to jump to the first line of the file, so that I can quickly navigate to the start of a file.
- As a Vim user, I want to press `G` to jump to the last line of the file, so that I can quickly navigate to the end of a file.
- As a Vim user, I want to press a count followed by `G` (e.g., `5G`) to jump to a specific line number, so that I can quickly navigate to a particular location in the file.
- As a Vim user, I want the cursor column to be remembered when using these motions, so that I can return to my previous column position using vertical navigation.

## Functional Requirements

- [ ] **REQ-001**: The `g` key pressed twice in normal mode (`gg`) should move the cursor to the first line of the file (line 1).
- [ ] **REQ-002**: The `G` key pressed in normal mode should move the cursor to the last line of the file.
- [ ] **REQ-003**: A count prefix followed by `G` (e.g., `5G`) or `gg` (e.g., `5gg`) should move the cursor to the specified line number.
- [ ] **REQ-004**: When `gg` or `G` is used without a count, they should be treated as vertical motions for the purposes of maintaining the remembered column.
- [ ] **REQ-005**: When a count is applied to `gg` or `G`, they should be treated as line motions (the count specifies the target line).
- [ ] **REQ-006**: If the count exceeds the number of lines in the file, the cursor should move to the last line.

## Non-Functional Requirements

- **Performance**: The motion should be instantaneous with no perceptible delay.
- **Compatibility**: Behavior should match Vim's `gg`, `G`, and count-prefixed variants as closely as possible.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `gg` in normal mode moves cursor to line 1, column 1 (or remembered column).
- [ ] **AC-002**: Pressing `G` in normal mode moves cursor to the last line of the file.
- [ ] **AC-003**: Pressing `5G` (or any valid count) moves cursor to line 5.
- [ ] **AC-004**: After using `gg` or `G`, subsequent vertical motions (like `j` or `k`) should remember the column from before the motion.

## Out of Scope

- Visual mode variants (v`gg`, v`G`) - these can be added in a future feature.
- Counting lines (like `g` `g` in visual mode) - standard motion only.
- Operator combinations (e.g., `dgg`, `dG`) - operators are not yet implemented.

## Assumptions

- The editor already has a concept of "remembered column" for vertical motions.
- The existing motion system supports both vertical motions and line motions with appropriate flags/traits.
- The editor has a way to get the total number of lines in a buffer.

## Dependencies

- Existing motion infrastructure (motion traits, vertical motion behavior).
- Existing count parsing for motions.
- Column preservation/remembered column system (from spec 0018).
