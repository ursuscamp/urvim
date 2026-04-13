# Visual Mode

## Summary

Add a simple Vim-compatible character-wise visual mode to urvim. Users should be able to enter visual mode, extend a selection with motions, and apply delete or change operations to the selected range.

## Problem Statement

urvim currently does not provide a basic visual selection workflow.

That creates a gap for common Vim editing patterns where users expect to highlight text directly with motions and then delete or change the selected region. Without visual mode, these edits require less direct command sequences and feel incomplete compared with familiar Vim behavior.

## User Stories

- As a Vim user, I want to enter visual mode and extend a selection with motions, so that I can edit text directly from the highlighted range.
- As a Vim user, I want to delete the selected text, so that I can remove ranges without switching to operator-pending commands.
- As a Vim user, I want to change the selected text, so that I can replace a highlighted range and continue typing immediately.

## Functional Requirements

- [ ] **REQ-001**: Urvim shall support a character-wise visual mode entered from normal mode.
- [ ] **REQ-002**: Visual mode shall begin with a single cursor position as the selection anchor and shall expand or contract the active selection as the cursor moves.
- [ ] **REQ-003**: Visual mode shall support motion-based selection updates using the editor's existing normal-mode motion keys.
- [ ] **REQ-004**: Visual mode shall support delete of the active selection.
- [ ] **REQ-005**: Visual mode shall support change of the active selection.
- [ ] **REQ-006**: Visual delete shall remove the selected text and leave the cursor at the start of the deleted range.
- [ ] **REQ-007**: Visual change shall remove the selected text, leave the cursor at the start of the changed range, and enter insert mode immediately after the deletion.
- [ ] **REQ-008**: Visual mode shall exit when the user presses `Esc`.
- [ ] **REQ-009**: Visual mode shall exit when the user presses `v` again, consistent with Vim behavior.
- [ ] **REQ-010**: Visual-mode delete and change shall preserve Vim-compatible selection semantics for the selected range, including the cursor position that results from the operation.
- [ ] **REQ-011**: The active visual selection shall use a dedicated theme-driven UI style so the appearance can be configured through the theme's UI section.
- [ ] **REQ-012**: Visual mode shall not implement linewise or blockwise selection in this release.
- [ ] **REQ-013**: Visual mode shall not change the behavior of normal mode motions outside of visual mode.

## Non-Functional Requirements

- **Compatibility**: The interaction model should match Vim's character-wise visual mode closely enough that common selection, delete, and change workflows feel familiar.
- **Usability**: The active selection should remain visually clear while the cursor is moved with motions.
- **Reliability**: Exiting visual mode and applying visual delete/change should leave the editor in a predictable mode and cursor state.
- **Compatibility**: Visual selection styling should be controlled by the theme's UI palette rather than hardcoded editor colors.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `v` in normal mode enters character-wise visual mode.
- [ ] **AC-002**: Moving the cursor with a motion while in visual mode expands the selection accordingly.
- [ ] **AC-003**: Pressing `d` in visual mode deletes the selected text and leaves the cursor at the start of the removed range.
- [ ] **AC-004**: Pressing `c` in visual mode deletes the selected text, leaves the cursor at the start of the removed range, and enters insert mode.
- [ ] **AC-005**: Pressing `Esc` in visual mode returns the editor to normal mode without modifying the buffer.
- [ ] **AC-006**: Pressing `v` again in visual mode returns the editor to normal mode without modifying the buffer.
- [ ] **AC-007**: Visual mode does not support linewise or blockwise selection commands in the initial release.
- [ ] **AC-008**: Visual-mode delete and change behave consistently on single-line and multi-line selections that are reachable through the editor's existing motions.

## Out of Scope

- Linewise visual mode
- Blockwise visual mode
- Visual paste, indent, substitute, or other visual operators beyond delete and change
- Visual-mode dot repeat
- Changing the behavior of non-visual normal mode commands

## Assumptions

- Urvim already has the normal-mode motion primitives needed to drive selection updates.
- The editor's existing delete and change behaviors can be reused for selected ranges rather than redefined from scratch.
- Visual mode will be introduced as a new editor state rather than as a modified form of operator-pending mode.

## Dependencies

- Existing normal mode motion handling
- Existing buffer deletion and change workflows
- Existing mode switching infrastructure
