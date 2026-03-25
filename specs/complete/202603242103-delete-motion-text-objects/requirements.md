# Delete Motion Text Objects

## Summary

Add delete-target text objects for `dw`, `de`, `db`, `dW`, `dE`, and `dB` so the `d` operator can delete text using the existing word and BigWord boundary families without requiring `iw` or `aw`.

## Problem Statement

urvim already supports:

- word and BigWord motions (`w`, `e`, `b`, `W`, `E`, `B`)
- delete with word text objects (`diw`, `daw`)
- operator-pending mode for text-object based delete actions

Missing: delete targets that match the familiar Vim commands `dw`, `de`, `db`, `dW`, `dE`, and `dB`.

Without these commands, users can navigate by words but cannot delete by those same boundary shapes, which makes normal-mode editing feel incomplete and inconsistent.

## User Stories

- As a vim user, I want `dw`, `de`, and `db` to delete text using word boundaries, so that navigation and deletion behave like a matched pair.
- As a vim user, I want `dW`, `dE`, and `dB` to delete text using BigWord boundaries, so that I can remove non-whitespace spans efficiently.
- As a urvim developer, I want these delete targets to reuse the editor's existing boundary terminology and count behavior, so that future operator-pending features stay consistent.

## Functional Requirements

- [ ] **REQ-001**: Pressing `d` followed by `w`, `e`, `b`, `W`, `E`, or `B` executes a delete operation targeting the corresponding boundary shape.
- [ ] **REQ-002**: `dw` deletes from the cursor position up to, but not including, the start position reached by the existing `w` boundary semantics for the same count.
- [ ] **REQ-003**: `de` deletes from the cursor position through the end position reached by the existing `e` boundary semantics for the same count.
- [ ] **REQ-004**: `db` deletes from the start position reached by the existing `b` boundary semantics for the same count through the original cursor position.
- [ ] **REQ-005**: `dW`, `dE`, and `dB` mirror `dw`, `de`, and `db` respectively, but use existing BigWord boundary semantics.
- [ ] **REQ-006**: Leading counts and operator sub-counts are supported for these delete targets, and combined counts multiply consistently with existing operator-pending count behavior.
- [ ] **REQ-007**: Forward delete targets (`dw`, `de`, `dW`, `dE`) delete within the current line or across line boundaries exactly as their paired motion semantics imply for the resolved target position.
- [ ] **REQ-008**: Backward delete targets (`db`, `dB`) delete backward to the resolved boundary without removing text before that boundary.
- [ ] **REQ-009**: If the target boundary cannot move because the cursor is already at the relevant buffer edge, the operation leaves the buffer unchanged.
- [ ] **REQ-010**: After a successful delete operation, the cursor lands at the start of the deleted region.
- [ ] **REQ-011**: Each successful delete operation creates an undo snapshot consistent with existing delete commands.

## Non-Functional Requirements

- **Compatibility**: Delete-target resolution must stay aligned with the existing `w`, `e`, `b`, `W`, `E`, and `B` motion behavior documented by urvim.
- **Reliability**: Forward and backward delete targets must behave deterministically at word boundaries, whitespace runs, punctuation runs, and buffer edges.
- **Usability**: The feature should feel like a natural extension of existing operator-pending delete behavior and existing motion count rules.

## Acceptance Criteria

- [ ] **AC-001**: On `hello world` with cursor at the `h`, `dw` deletes `hello ` and leaves `world`.
- [ ] **AC-002**: On `hello world` with cursor at the `h`, `de` deletes `hello` and leaves ` world`.
- [ ] **AC-003**: On `hello world` with cursor at the `w`, `db` deletes `hello ` and leaves `world`, with the cursor at the start of `world`.
- [ ] **AC-004**: On `alpha   beta` with cursor at the `a`, `d2w` deletes the text up to the start of the third word boundary as defined by urvim's `w` semantics.
- [ ] **AC-005**: On `alpha---beta` with cursor at the `a`, `dw` deletes text using urvim's existing non-word boundary handling for `w`, not Vim's delimiter-skipping behavior.
- [ ] **AC-006**: On `alpha---beta` with cursor at the `a`, `dW` deletes `alpha---` and leaves `beta`.
- [ ] **AC-007**: On a cursor position where `b` or `B` cannot move farther backward, `db` or `dB` leaves the buffer unchanged.
- [ ] **AC-008**: Combined counts such as `3d2w` delete six word-forward targets under the same multiplicative rules already used by operator-pending text objects.
- [ ] **AC-009**: Each successful delete target participates in undo/redo as one logical edit.

## Out of Scope

- Change operator support (`cw`, `ce`, `cb`, `cW`, `cE`, `cB`)
- Yank operator support (`yw`, `ye`, `yb`, `yW`, `yE`, `yB`)
- New non-word boundary definitions beyond the existing Word and BigWord behavior
- Additional text objects such as sentence, paragraph, quote, or bracket objects

## Assumptions

- These commands should align with urvim's current motion semantics, even where those semantics intentionally differ from Vim.
- `dw` and related commands are being specified as delete targets reachable directly after `d`, regardless of whether the internal implementation models them as motions, text objects, or another operator-pending target type.
- Existing count parsing infrastructure can be reused without changing the user-visible count rules.

## Dependencies

- Existing boundary and motion resolution logic for `w`, `e`, `b`, `W`, `E`, and `B`
- Existing operator-pending delete flow
- Existing buffer deletion and undo/redo infrastructure
