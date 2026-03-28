# Dot Repeat

## Summary

Implement basic Vim-style dot repeat for urvim so that pressing `.` repeats the most recent successful normal-mode buffer modification. This first slice covers direct buffer-editing actions already supported by the editor and does not include insert-mode repeat playback yet.

## Problem Statement

urvim already has a range of normal-mode editing actions, but users cannot replay the last change with `.`. That makes repeated edits slower and forces users to retype the same buffer modification sequence multiple times.

## User Stories

- As a Vim user, I want `.` to repeat my last edit, so that I can make the same change across multiple locations quickly.
- As a user, I want repeated edits to respect the same target and count as the original change, so that the replay feels predictable.
- As a user, I want non-editing actions to remain unaffected, so that navigation and mode switching still behave normally.

## Functional Requirements

- [ ] **REQ-001**: Pressing `.` in normal mode repeats the most recent successful buffer modification action.
- [ ] **REQ-002**: The repeated action uses the same logical edit target as the original change when replayed at a new cursor position.
- [ ] **REQ-003**: The repeated action preserves the original change kind, including delete-style and change-style buffer edits.
- [ ] **REQ-004**: Counted edits record the count used by the original change and replay consistently when `.` is pressed.
- [ ] **REQ-005**: If the user supplies a count before `.`, that count is applied to the repeat request instead of the original count.
- [ ] **REQ-006**: A successful repeat produces the same kind of buffer mutation as the original change, including resulting cursor placement rules for the supported actions.
- [ ] **REQ-007**: Failed or empty repeat attempts do not corrupt the stored repeatable change state.
- [ ] **REQ-008**: Only successful normal-mode buffer modifications update the stored repeatable change.
- [ ] **REQ-009**: Non-mutating actions do not replace the stored repeatable change.
- [ ] **REQ-010**: Basic dot repeat does not require insert-mode replay support in this phase.

## Non-Functional Requirements

- **Compatibility**: The repeat behavior should match Vim's basic dot-repeat feel for buffer edits that urvim already supports.
- **Reliability**: Repeating the same edit should be deterministic and should not depend on incidental editor state beyond the current cursor position and count.
- **Usability**: Repeating a change with `.` should feel immediate and require no extra confirmation.

## Acceptance Criteria

- [ ] **AC-001**: After deleting text with a supported normal-mode command, pressing `.` repeats that deletion at the next target location.
- [ ] **AC-002**: After changing text with a supported normal-mode command, pressing `.` repeats the same kind of buffer change at a later cursor position.
- [ ] **AC-003**: When the original edit used a count, the repeat uses the same logical edit size unless the user supplies a new count for `.`.
- [ ] **AC-004**: Pressing `.` after a non-mutating action does not overwrite the previously stored repeatable edit.
- [ ] **AC-005**: Pressing `.` after a failed repeat does not lose the last valid repeatable edit.
- [ ] **AC-006**: Basic dot repeat works for current supported normal-mode buffer mutations without needing insert-mode replay.

## Out of Scope

- Insert-mode repeat playback, including repeated inserted text
- Visual-mode repeat behavior
- Macro replay
- Command-line `:` repeat behavior
- Register/redo integration beyond what is needed to replay the last basic buffer mutation
- Adding new editing commands solely to support dot repeat

## Assumptions

- The editor already has a clear notion of successful buffer modification actions in normal mode.
- The existing action system can distinguish mutation actions from non-mutating actions.
- Counted actions already flow through the normal-mode action pipeline in a way that can be observed and stored.
- The initial implementation can focus on the repeat of existing supported buffer-editing actions rather than inventing new repeatable commands.

## Dependencies

- Existing normal-mode editing actions and action dispatch
- Existing count parsing and counted action handling
- Existing buffer mutation and cursor update behavior
- Existing undo state, if needed, to ensure repeat does not break editing history
