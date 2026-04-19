# Character Scan Range Motions
## Summary

Add operator-pending range motions for the character scan family `f`, `F`, `t`, and `T`. These motions should let operators consume the span resolved by the character search, so workflows like `ct:` behave like Vim users expect.

## Problem Statement

urvim already supports character scan motions for normal-mode navigation, but users cannot currently use those motions as operator targets to edit text up to a searched character.

That makes common Vim editing patterns awkward:

- `ct:` to change text up to the next colon
- `dfx` to delete through the next `x`
- `dT)` to delete text after the previous `)`

Without range versions of the character scan motions, users must fall back to manual cursor movement or different commands, which slows down editing and weakens Vim compatibility.

## User Stories

- As a Vim user, I want to use `f`, `F`, `t`, and `T` after an operator so that I can target text up to or through a searched character.
- As a Vim user, I want `ct:` to work so that I can quickly replace text before the next colon.
- As a Vim user, I want `dfx` and `dTx` to behave predictably so that delete and change commands feel like Vim.
- As a urvim user, I want the range behavior to match the existing character scan semantics so that normal-mode motion and operator-pending use stay consistent.

## Functional Requirements

- [ ] **REQ-001**: In operator-pending mode, the character scan triggers `f`, `F`, `t`, and `T` shall resolve to range motions instead of moving the cursor immediately.
- [ ] **REQ-002**: Range motions shall search only the current line and shall not wrap to adjacent lines.
- [ ] **REQ-003**: Range motions shall preserve the same target character and scan direction as the corresponding normal-mode character scan motion.
- [ ] **REQ-004**: A range motion based on `f` or `F` shall resolve to the found character position, so the operator consumes text through that character.
- [ ] **REQ-005**: A range motion based on `t` shall resolve to the position before the found character, so the operator consumes text up to but not including that character.
- [ ] **REQ-006**: A range motion based on `T` shall resolve to the position after the found character, so the operator consumes text up to but not including that character when scanning backward.
- [ ] **REQ-007**: Counts shall apply to character scan range motions using the same count behavior as the corresponding normal-mode motions.
- [ ] **REQ-008**: If the target character is not found, the pending operator shall be canceled without modifying the buffer.
- [ ] **REQ-009**: Character scan range motions shall be available to all operators that already consume motion targets in urvim, including delete, change, yank, and case operators.
- [ ] **REQ-010**: Executing a character scan as an operator target shall continue to update the stored last character-search state so `;` and `,` repeat the most recent search.

## Non-Functional Requirements

- **Compatibility**: Range motions should match Vim's `d`, `c`, and `y` character-search workflows closely enough that common commands feel familiar.
- **Reliability**: Failed search resolution must leave the buffer unchanged and should not produce partial edits.
- **Usability**: Normal-mode navigation behavior for `f`, `F`, `t`, and `T` should remain unchanged outside operator-pending mode.

## Acceptance Criteria

- [ ] **AC-001**: On `foo:bar` with the cursor at the start of `foo`, `ct:` deletes `foo` and enters insert mode before the colon.
- [ ] **AC-002**: On `foo:bar` with the cursor at the start of `foo`, `cf:` deletes `foo:` and enters insert mode at the colon position.
- [ ] **AC-003**: On `abcxdef` with the cursor at `a`, `dfx` deletes `abcx` and leaves `def`.
- [ ] **AC-004**: On `abcxdef` with the cursor at `a`, `dtx` deletes `abc` and leaves `xdef`.
- [ ] **AC-005**: On `abcxdef` with the cursor after `d`, `dFx` deletes text back to the previous `x` using the backward search target.
- [ ] **AC-006**: On `abc)def` with the cursor at `d`, `dT)` deletes text up to the position after the previous `)` target using the backward till target.
- [ ] **AC-007**: `2dtx` and `d2tx` both respect the configured count semantics for the `t` motion.
- [ ] **AC-008**: If no matching character exists, `dfz` leaves the buffer unchanged and does not partially delete text.
- [ ] **AC-009**: After using an operator-pending range motion, `;` still repeats the most recent character search in the same direction.

## Out of Scope

- New character scan keys beyond `f`, `F`, `t`, and `T`
- Cross-line search or wraparound search behavior
- New operators that are not already supported by urvim's operator-pending mode
- Visual-mode selection behavior for character scans

## Assumptions

- The current character scan motions already provide the correct search target and boundary resolution logic for normal mode.
- Operator-pending mode can reuse the same underlying character scan resolution without changing the normal-mode keymap.
- Existing operator-pending edits already know how to apply a resolved motion target as a buffer mutation.

## Dependencies

- Existing normal-mode character scan motions
- Existing operator-pending mode and operator application flow
- Existing count parsing and motion repetition behavior
- Existing undo integration for single logical edits
