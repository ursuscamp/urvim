# Delete Line Motions

## Summary

Add delete-operator support for the existing line-oriented motions `$`, `0`, `^`, `gg`, and `G` so normal-mode users can remove text to line boundaries and file-position boundaries using familiar Vim-style `d{motion}` commands.

## Problem Statement

urvim already supports:

- line motions `$`, `0`, and `^`
- file motions `gg` and `G`
- delete with `dd`, `diw`, `daw`, and word-family delete motions such as `dw`

Missing: delete targets for the existing line and file motions. Without them, users can navigate to these locations but cannot delete using the same motion vocabulary, which makes the operator-pending editing model feel incomplete.

The new `gg` and `G` delete forms also need a stricter rule than ordinary counted operator targets: when a count is present with `dgg` or `dG`, that count must be interpreted as the motion's destination line number, and the delete must remain linewise.

## User Stories

- As a Vim user, I want `d$`, `d0`, and `d^` so that I can delete to line-local boundaries without switching to insert mode.
- As a Vim user, I want `dgg` and `dG` so that I can delete from my current line to the start or end of the file using familiar motion commands.
- As a Vim user, I want counted `dgg` and `dG` to use the count as the target line number, so that `d5G` and `d5gg` behave like deletes to line 5 rather than repeated motion counts.
- As a urvim developer, I want delete-to-motion behavior to stay aligned with the existing motion model, so future operators can reuse the same semantics consistently.

## Functional Requirements

- [ ] **REQ-001**: Pressing `d$` in normal mode deletes from the cursor position through the end of the current line using the existing `$` motion semantics.
- [ ] **REQ-002**: Pressing `d0` in normal mode deletes from the start of the current line through the original cursor position using the existing `0` motion semantics.
- [ ] **REQ-003**: Pressing `d^` in normal mode deletes from the first non-whitespace character of the current line through the original cursor position using the existing `^` motion semantics.
- [ ] **REQ-004**: Pressing `dgg` in normal mode deletes linewise from the current line through line 1.
- [ ] **REQ-005**: Pressing `dG` in normal mode deletes linewise from the current line through the last line of the buffer.
- [ ] **REQ-006**: `dgg` and `dG` operate on whole lines, including line breaks between the affected lines, and leave the cursor at the first remaining line of the deleted span.
- [ ] **REQ-007**: A count with `dgg` or `dG` is interpreted as the motion's destination line number, not as a repeat count for the delete operator.
- [ ] **REQ-008**: Counted `dgg` and `dG` clamp the destination line to the valid buffer range.
- [ ] **REQ-009**: `d0` must treat `0` as the line-start motion after `d`, not as the start of an operator sub-count.
- [ ] **REQ-010**: If a resolved delete target produces an empty range, the buffer remains unchanged and no delete occurs.
- [ ] **REQ-011**: Each successful delete operation creates a single undo snapshot consistent with existing delete commands.

## Non-Functional Requirements

- **Compatibility**: `$`, `0`, `^`, `gg`, and `G` delete targets must follow urvim's existing motion semantics, including its current count parsing model.
- **Reliability**: Linewise deletes must behave deterministically at the first line, last line, and when the current line is already the target line.
- **Usability**: The new delete commands should feel like a natural extension of urvim's existing operator-pending delete support.

## Acceptance Criteria

- [ ] **AC-001**: On `hello world` with the cursor at the `w`, `d$` deletes `world` and leaves `hello `.
- [ ] **AC-002**: On `hello world` with the cursor at the `w`, `d0` deletes `hello ` and leaves `world`, with the cursor at the start of `world`.
- [ ] **AC-003**: On `    hello world` with the cursor at the `w`, `d^` deletes `hello ` but preserves the leading indentation.
- [ ] **AC-004**: On a multi-line buffer with the cursor on line 4, `dgg` deletes lines 1 through 4 linewise.
- [ ] **AC-005**: On a multi-line buffer with the cursor on line 4, `dG` deletes lines 4 through the last line linewise.
- [ ] **AC-006**: On a multi-line buffer with the cursor on line 8, `d5gg` deletes linewise from line 8 through line 5.
- [ ] **AC-007**: On a multi-line buffer with the cursor on line 3, `d5G` deletes linewise from line 3 through line 5.
- [ ] **AC-008**: On a multi-line buffer with the cursor on line 3, `d999G` deletes linewise from line 3 through the last line.
- [ ] **AC-009**: Entering `d0` resolves as a valid delete-to-line-start command rather than waiting for additional count digits.
- [ ] **AC-010**: Each successful command participates in undo/redo as one logical edit.

## Out of Scope

- Change-operator variants such as `c$`, `c0`, `c^`, `cgg`, and `cG`
- Yank-operator variants such as `y$`, `y0`, `y^`, `ygg`, and `yG`
- Visual mode behavior for these motions
- New motion primitives beyond the existing `$`, `0`, `^`, `gg`, and `G` actions

## Assumptions

- urvim's existing delete flow can be extended to represent both characterwise and linewise operator targets.
- The current count parser can be taught to reserve `0` as a post-operator motion key when the pending operator expects a motion.
- For counted `dgg` and `dG`, a single explicit line-number target should take precedence over the operator's normal multiplicative count model.

## Dependencies

- Existing `$`, `0`, `^`, `gg`, and `G` motion behavior
- Existing operator-pending delete execution path
- Existing count parsing and trie-prefix handling in normal mode
- Existing buffer deletion and undo/redo infrastructure
