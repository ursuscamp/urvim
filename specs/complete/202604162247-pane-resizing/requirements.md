# Pane Resizing

## Summary
urvim should add a resizing mode for split panes. Entering the mode with `Ctrl-w r` should let the user resize the focused pane relative to its adjacent splits using Vim-style directional keys, then return to normal mode with `Esc`.

## Problem Statement
The editor can create and navigate nested splits, but it does not yet provide a dedicated keyboard mode for resizing those panes. Users need a predictable way to adjust pane proportions without leaving keyboard-driven workflows or triggering unrelated editing actions.

## User Stories
- As a user working in split layouts, I want to enter a resizing mode from normal mode, so that I can adjust pane sizes without using the mouse.
- As a user resizing a pane, I want `h`, `j`, `k`, and `l` to change the focused split in the expected directions, so that pane sizing feels Vim-like.
- As a user resizing a pane, I want the editor to stop shrinking once a pane reaches its minimum size, so that the layout remains usable.
- As a user in resizing mode, I want non-resize keys to be ignored, so that I do not accidentally trigger other editor actions while adjusting the layout.
- As a user, I want `Esc` to exit resizing mode, so that I can return to normal editing quickly.

## Functional Requirements
- [ ] **REQ-001**: The editor must enter resizing mode when the user presses `Ctrl-w r` from normal mode.
- [ ] **REQ-002**: While resizing mode is active, the editor must resize the focused pane horizontally when the user presses `h` or `l`.
- [ ] **REQ-003**: While resizing mode is active, the editor must resize the focused pane vertically when the user presses `j` or `k`.
- [ ] **REQ-004**: Horizontal resizing must adjust the focused pane relative to its adjacent split neighbors rather than resizing the entire layout symmetrically.
- [ ] **REQ-005**: Vertical resizing must adjust the focused pane relative to its adjacent split neighbors rather than resizing the entire layout symmetrically.
- [ ] **REQ-006**: Resizing must clamp at the editor's minimum pane size so that repeated shrinking stops once a pane can no longer become smaller.
- [ ] **REQ-007**: Resizing mode must ignore keys other than `h`, `j`, `k`, `l`, and `Esc` without leaving resizing mode.
- [ ] **REQ-008**: Pressing `Esc` while resizing mode is active must return the editor to normal mode.
- [ ] **REQ-009**: Entering and exiting resizing mode must not alter the active buffer, cursor position, or tab selection in the focused pane.
- [ ] **REQ-010**: Pane resizing must work for panes inside nested split layouts, not just the top-level split structure.

## Non-Functional Requirements
- [ ] **NFR-001**: Resizing interactions must remain responsive during normal keyboard-driven editing.
- [ ] **NFR-002**: The new mode must remain compatible with the existing modal input and action dispatch flow.
- [ ] **NFR-003**: The resizing behavior must be covered by unit tests for mode entry, directional resize handling, minimum-size clamping, ignored keys, and mode exit.

## Acceptance Criteria
- [ ] **AC-001**: Pressing `Ctrl-w r` from normal mode enters resizing mode.
- [ ] **AC-002**: Pressing `h` or `l` in resizing mode changes the focused pane width relative to its adjacent split neighbors.
- [ ] **AC-003**: Pressing `j` or `k` in resizing mode changes the focused pane height relative to its adjacent split neighbors.
- [ ] **AC-004**: Repeatedly shrinking a pane stops once the pane reaches its minimum size.
- [ ] **AC-005**: Pressing any non-resize key other than `Esc` in resizing mode leaves the editor in resizing mode and does not trigger unrelated actions.
- [ ] **AC-006**: Pressing `Esc` in resizing mode returns the editor to normal mode.
- [ ] **AC-007**: Resizing works in nested split layouts without disturbing the contents of unrelated panes.

## Out of Scope
- Mouse-driven pane resizing.
- Persisting pane sizes across editor restarts.
- Arbitrary numeric resize counts or repeat counts beyond the basic directional keys.
- Drag-and-drop pane rearrangement.
- Changing how splits are created, closed, or navigated.

## Assumptions
- The editor already has a split layout tree with resizable pane regions.
- The current modal input system can represent a dedicated resizing mode alongside the existing modes.
- The layout layer already knows the minimum size constraints needed to keep panes usable.
- `Ctrl-w r` is reserved for entering this resizing mode and does not need to perform any other action.

## Dependencies
- Existing nested split layout and pane sizing logic.
- Existing modal input handling and key parsing.
- Existing terminal redraw behavior for layout changes.
- Existing pane focus and split navigation state.
