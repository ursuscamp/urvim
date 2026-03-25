# Change Operator

## Summary

Implement the vim-style change operator for operator-pending commands such as `cw`, `ce`, `cb`, `cW`, `cE`, `cB`, `ciw`, `caw`, `c$`, `c0`, `c^`, `cgg`, and `cG`. The new behavior should mirror the existing delete-target resolution, but after removing the targeted text the editor should enter insert mode at the start of the changed region.

## Problem Statement

urvim already supports delete-style editing commands and several text objects, but it lacks the matching change operator flow that users expect from Vim. Without `c`-based operator-pending editing, users can delete text but cannot immediately begin replacing it in one command sequence.

## User Stories

- As a Vim user, I want to use `cw` and related commands, so that I can replace text with the same key sequences I already know.
- As a Vim user, I want `ciw` and `caw` to work, so that I can quickly replace words using text objects.
- As a user, I want the editor to enter insert mode after a successful change, so that I can continue typing immediately.

## Functional Requirements

- [ ] **REQ-001**: Pressing `c` followed by a supported motion or text object performs the same text removal as the corresponding delete operation.
- [ ] **REQ-002**: After a successful change operation, the editor enters insert mode.
- [ ] **REQ-003**: After a successful change operation, the cursor is positioned at the start of the changed region, ready for insertion.
- [ ] **REQ-004**: `cw`, `ce`, `cb`, `cW`, `cE`, and `cB` remove text using the same target resolution as the existing delete-target equivalents.
- [ ] **REQ-005**: `ciw` and `caw` remove text using the same text-object resolution as the existing delete equivalents.
- [ ] **REQ-006**: `c$`, `c0`, `c^`, `cgg`, and `cG` remove text using the same linewise or line-anchor target resolution as the corresponding delete-target equivalents.
- [ ] **REQ-007**: A change command that resolves to an empty region leaves the buffer unchanged and keeps the editor in normal mode.
- [ ] **REQ-008**: Count prefixes continue to apply multiplicatively and consistently with existing operator-pending count behavior.
- [ ] **REQ-009**: Successful change operations preserve existing undo behavior by recording the edit as a single logical change.
- [ ] **REQ-010**: The change operator must not alter the behavior of existing linewise commands such as `cc` and `C`.

## Non-Functional Requirements

- **Compatibility**: Change commands should match Vim's change-operator feel as closely as possible while reusing urvim's existing delete and text-object semantics.
- **Usability**: A successful change should transition immediately into insert mode without requiring any additional keystrokes.
- **Reliability**: Change operations should behave deterministically at word boundaries, whitespace boundaries, and buffer edges.

## Acceptance Criteria

- [ ] **AC-001**: On `hello world` with the cursor on the `h`, `cw` deletes `hello ` and leaves the editor in insert mode at the start of `world`.
- [ ] **AC-002**: On `hello world` with the cursor inside `hello`, `ciw` deletes `hello` and leaves the editor in insert mode at the start of the deleted word.
- [ ] **AC-003**: On `hello   world` with the cursor inside `hello`, `caw` deletes `hello   ` and leaves the editor in insert mode at the start of `world`.
- [ ] **AC-004**: On `alpha---beta` with the cursor on the `a`, `cW` removes the same text as the corresponding delete-target command and then enters insert mode.
- [ ] **AC-005**: On `hello world` with the cursor on the `h`, `c$` deletes from the cursor through the end of the line and enters insert mode at the truncation point.
- [ ] **AC-006**: On a buffer with multiple lines, `cG` changes from the cursor line to the end of the file using the same linewise target selection as `dG`.
- [ ] **AC-007**: A count prefix such as `2cw` changes text using the same multiplicative count rules as existing operator-pending commands.
- [ ] **AC-008**: When a change operation resolves to an empty region at a buffer edge, the buffer is unchanged and normal mode is preserved.
- [ ] **AC-009**: After a successful change, typing text inserts at the start of the changed region.

## Out of Scope

- Linewise change commands already covered elsewhere, including `cc` and `C`
- Register integration for changed text
- Visual mode change commands
- New motion or text-object definitions beyond the existing delete-target and text-object set
- Undo/redo redesign beyond the existing snapshot-based behavior

## Assumptions

- The existing delete-target and text-object resolution already defines the regions for `cw`, `ce`, `cb`, `cW`, `cE`, `cB`, `ciw`, and `caw`.
- The editor already knows how to enter insert mode from normal mode.
- Existing count parsing and operator-pending handling can be reused without changing the user-visible count syntax.

## Dependencies

- Existing operator-pending delete flow
- Existing word and BigWord boundary semantics
- Existing `iw` and `aw` text-object resolution
- Existing insert mode transition behavior
