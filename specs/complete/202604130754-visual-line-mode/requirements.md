# Visual Line Mode

## Summary

Add Vim-compatible linewise visual mode to urvim. Users should be able to enter linewise selection from normal mode, extend the selection with motions, and apply delete and change operations to the selected lines.

## Problem Statement

urvim already has character-wise visual mode, but it does not yet support the linewise workflow many Vim users rely on.

Without visual line mode, operations that are meant to act on whole lines require less direct command sequences and break the expected `V`-based editing pattern for selecting, deleting, copying, changing, and pasting line ranges.

## User Stories

- As a Vim user, I want to enter linewise visual mode with `V`, so that I can select whole lines directly.
- As a Vim user, I want motions in linewise visual mode to extend the selection by whole lines, so that I can quickly edit contiguous line ranges.
- As a Vim user, I want to delete and change selected lines, so that I can use the same visual workflow I expect from Vim.

## Functional Requirements

- [ ] **REQ-001**: Urvim shall support linewise visual mode entered from normal mode.
- [ ] **REQ-002**: Pressing `V` in normal mode shall enter linewise visual mode with the current line selected.
- [ ] **REQ-003**: Linewise visual mode shall keep the active selection aligned to whole-line boundaries at all times.
- [ ] **REQ-004**: Motions used while in linewise visual mode shall expand or contract the selection by whole lines.
- [ ] **REQ-005**: Linewise visual mode shall support delete of the active selection.
- [ ] **REQ-006**: Linewise visual mode shall support change of the active selection.
- [ ] **REQ-007**: Linewise delete shall remove the selected lines and leave the cursor at the start of the line where the deleted range began.
- [ ] **REQ-008**: Linewise change shall replace the selected lines with a single empty blank line, leave the cursor at the start of that blank line, and enter insert mode immediately after the replacement.
- [ ] **REQ-009**: Linewise visual mode shall exit when the user presses `Esc`.
- [ ] **REQ-010**: Linewise visual mode shall exit when the user presses `V` again, consistent with Vim behavior.
- [ ] **REQ-011**: Linewise visual mode shall not alter the behavior of character-wise visual mode or normal mode outside visual selection workflows.

## Non-Functional Requirements

- **Compatibility**: The interaction model should match Vim's linewise visual mode closely enough that common `V`-based selection, delete, yank, change, and paste workflows feel familiar.
- **Usability**: The active linewise selection should remain visually clear while motions change the selected line range.
- **Reliability**: Exiting linewise visual mode and applying linewise operations should leave the editor in a predictable mode and cursor state.
- **Compatibility**: Linewise selection should reuse the editor's existing visual selection styling conventions.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `V` in normal mode enters linewise visual mode with the current line selected.
- [ ] **AC-002**: Moving the cursor with a motion while in linewise visual mode expands or contracts the selection by whole lines only.
- [ ] **AC-003**: Pressing `d` in linewise visual mode deletes the selected lines and leaves the cursor at the deletion site.
- [ ] **AC-004**: Pressing `c` in linewise visual mode replaces the selected lines with a single empty blank line, leaves the cursor at the blank line, and enters insert mode.
- [ ] **AC-005**: Pressing `Esc` in linewise visual mode returns the editor to normal mode without modifying the buffer.
- [ ] **AC-006**: Pressing `V` again in linewise visual mode returns the editor to normal mode without modifying the buffer.
- [ ] **AC-007**: Character-wise visual mode continues to work as before after linewise visual mode is added.

## Out of Scope

- Blockwise visual mode
- Changing the behavior of normal mode motions outside of visual mode
- Visual-mode dot repeat
- Reworking the editor's existing register model

## Assumptions

- Urvim already has character-wise visual mode and the mode switching infrastructure needed to add a linewise variant.
- The editor already has motion, delete, and change workflows that can be reused for linewise selection.

## Dependencies

- Existing normal mode motion handling
- Existing visual mode infrastructure
- Existing buffer deletion, insertion, and clipboard/register workflows
- Existing mode switching infrastructure
