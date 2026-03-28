# Insert-Mode Dot Repeat

## Summary

Extend dot-repeat so that a normal-mode change command that enters insert mode, such as `cw`, records the inserted text and can replay the full edit with `.`. The replay should restore the same edit shape and insert the same text, not just re-run the operator portion.

## Problem Statement

urvim can already repeat some normal-mode edits with `.`, but edits that transition into insert mode only replay the operator portion. That means a user who performs `cw`, types replacement text, and exits insert mode cannot reliably repeat the same completed change elsewhere with `.`.

## User Stories

- As a Vim user, I want `.` to repeat a change like `cw` including the text I inserted, so that I can apply the same replacement at another location without retyping it.
- As a user, I want the repeat to behave like a single completed edit, so that the operator and inserted text are replayed together.
- As a user, I want failed or partial insert sessions to not break dot-repeat, so that the last successful change remains repeatable.

## Functional Requirements

- [ ] **REQ-001**: A normal-mode change that enters insert mode and succeeds must record the inserted text as part of the repeatable edit.
- [ ] **REQ-002**: Pressing `.` after a successful change-with-insert must repeat both the original change operation and the inserted text.
- [ ] **REQ-003**: The repeated edit must apply the same logical replacement behavior as the original change, including deleting the original target before inserting the recorded text.
- [ ] **REQ-004**: If the user supplies a count before `.`, that count must affect the repeat request in the same way as existing dot-repeat behavior.
- [ ] **REQ-005**: The recorded repeatable edit must not be replaced by an incomplete insert session that never successfully commits.
- [ ] **REQ-006**: Exiting insert mode after a successful insertion must finalize the change so it becomes repeatable.
- [ ] **REQ-007**: Repeating the same inserted text at a new location must preserve the inserted characters exactly as entered, including multi-character text.
- [ ] **REQ-008**: Insert-mode repeat support must integrate with the editor's existing dot-repeat behavior rather than creating a separate repeat command.
- [ ] **REQ-009**: Non-mutating actions performed before or after insert-mode edits must not overwrite the stored repeatable change.

## Non-Functional Requirements

- **Compatibility**: The feature should match Vim's expectation that `.` replays the last complete change, including text entered in insert mode.
- **Reliability**: Repeat playback must be deterministic and must not depend on stale intermediate insert-mode state.
- **Usability**: Users should not need any additional command to make inserted text repeatable beyond completing the edit normally.

## Acceptance Criteria

- [ ] **AC-001**: After typing `cw`, entering replacement text, and leaving insert mode, pressing `.` repeats the same replacement at another cursor location.
- [ ] **AC-002**: After a successful insert-mode change, the repeated edit inserts the exact same text that the user entered originally.
- [ ] **AC-003**: If the user starts an insert-mode change but abandons it without completing the edit, the last successful repeatable change remains available.
- [ ] **AC-004**: Dot-repeat continues to work for existing non-insert buffer edits while also covering change-then-insert edits.

## Out of Scope

- Visual-mode repeat behavior
- Macro recording and playback
- Register replay beyond what dot-repeat needs for the last change
- Command-line `:` repeat behavior
- Repeating partially typed insert text before the change is finalized

## Assumptions

- The editor already distinguishes a successful change operator from an unfinished insert session.
- The current dot-repeat system can store enough information to represent both the change operator and the inserted text.
- Existing undo behavior does not need to change for this feature beyond preserving normal edit history.

## Dependencies

- Existing dot-repeat infrastructure
- Existing change operator handling
- Existing insert-mode text entry and exit flow
- Existing cursor movement and buffer mutation behavior used by change commands
