# Nested Splits

## Summary
urvim should support a nested layout tree made of vertical and horizontal splits. Each split should own a tab group or another split, and users should be able to create, close, and move between panes with Vim-style window mappings.

## Problem Statement
The current editor layout only supports a flat container structure, which prevents users from arranging multiple work areas in a nested split tree. That makes it difficult to mirror Vim-like workflows where panes can be split recursively and navigated with familiar window commands.

## User Stories
- As a user, I want to split the current pane vertically or horizontally, so that I can arrange work areas side by side or stacked.
- As a user, I want split layouts to nest, so that I can build more complex arrangements without losing Vim-like control.
- As a user, I want to move focus with `Ctrl-w h/j/k/l`, so that pane navigation feels familiar.
- As a user, I want empty tab groups to collapse automatically, so that the layout shrinks back cleanly when I close the last window in a region.
- As a user, I want the editor to close when the last window is gone, so that I do not end up with an empty shell.

## Functional Requirements
- [ ] **REQ-001**: The editor must support a nested layout tree composed of vertical and horizontal split nodes.
- [ ] **REQ-002**: A split node must be able to contain either another split node or a tab group.
- [ ] **REQ-003**: The split tree must allow arbitrary nesting of vertical and horizontal splits.
- [ ] **REQ-004**: The editor must support splitting the currently focused pane vertically with `Ctrl-w v`.
- [ ] **REQ-005**: The editor must support splitting the currently focused pane horizontally with `Ctrl-w s`.
- [ ] **REQ-006**: Creating a split must leave the existing pane usable and must create a new adjacent pane that contains a tab group.
- [ ] **REQ-007**: Creating a split must preserve the focused window content so the new pane is immediately usable.
- [ ] **REQ-008**: The editor must support moving focus to the pane on the left with `Ctrl-w h`.
- [ ] **REQ-009**: The editor must support moving focus to the pane below with `Ctrl-w j`.
- [ ] **REQ-010**: The editor must support moving focus to the pane above with `Ctrl-w k`.
- [ ] **REQ-011**: The editor must support moving focus to the pane on the right with `Ctrl-w l`.
- [ ] **REQ-012**: The editor must support closing the current pane with `Ctrl-w q`.
- [ ] **REQ-013**: Closing a pane whose tab group becomes empty must close that tab group and remove the hosting split node.
- [ ] **REQ-014**: When a split node is removed because one child became empty, the sibling child must reclaim the freed screen space.
- [ ] **REQ-015**: When the final window in the editor closes, the editor must exit cleanly.
- [ ] **REQ-016**: Pane focus movement must operate across nested split levels, not only across siblings at the same depth.
- [ ] **REQ-017**: Split creation, closing, and focus movement must not disturb the contents or cursor state of unrelated panes.
- [ ] **REQ-018**: Existing tab-group behavior inside each pane must continue to support multiple windows.

## Non-Functional Requirements
- [ ] **NFR-001**: Split rendering and pane resizing must remain responsive during normal terminal redraws and resizes.
- [ ] **NFR-002**: Existing single-pane editing behavior must remain unchanged when no additional splits are created.
- [ ] **NFR-003**: The split-tree implementation must remain compatible with the existing modal editing and action-processing flow.
- [ ] **NFR-004**: The feature must be covered by unit tests for split creation, split collapse, pane navigation, and last-window exit behavior.

## Acceptance Criteria
- [ ] **AC-001**: Pressing `Ctrl-w v` creates a vertical split for the current pane.
- [ ] **AC-002**: Pressing `Ctrl-w s` creates a horizontal split for the current pane.
- [ ] **AC-003**: Pressing `Ctrl-w h`, `Ctrl-w j`, `Ctrl-w k`, and `Ctrl-w l` moves focus to the left, down, up, and right panes respectively when they exist.
- [ ] **AC-004**: Closing the last window in a tab group removes that tab group and the parent split collapses so the sibling pane expands into the freed area.
- [ ] **AC-005**: Nested split layouts can be created and navigated without breaking existing window editing inside each tab group.
- [ ] **AC-006**: Closing the last remaining window in the editor exits the application.

## Out of Scope
- Persisting split layouts across editor restarts.
- Drag-and-drop pane rearrangement.
- Arbitrary resizing commands beyond the automatic space reclamation that happens when panes close.
- Introducing non-Vim key bindings for split management.
- Changing tab-group window management beyond what is required for split hosting and collapse behavior.

## Assumptions
- A pane is the visible region owned by a tab group inside the split tree.
- A split command creates a new sibling pane rather than replacing the existing one.
- The new pane starts with usable editor content so it can be edited immediately.
- The editor already has a single active root layout that can be extended into a split tree.
- Existing tab-group window management remains the source of truth for what a pane contains.

## Dependencies
- Existing `Layout` and `Tab Group` containers.
- Existing `Window` lifecycle and tab-group close behavior.
- Existing Vim-style key parsing and action dispatch.
- Existing terminal resize and redraw handling.
