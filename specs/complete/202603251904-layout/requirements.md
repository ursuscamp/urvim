# Layout

## Summary
urvim should introduce a root `layout` container above the existing tab group so the UI can manage positioning and sizing for higher-level widgets. In this first stage, the layout needs to host exactly one tab group and preserve the current editing experience while establishing the container layer for future UI expansion.

## Problem Statement
The current UI root is the tab group itself, which makes it difficult to grow the editor into a richer container hierarchy later. urvim needs a dedicated layout layer that can control where top-level widgets live on screen and how much space they receive, starting with the existing tab group as the only child.

## User Stories
- As a user, I want the editor to keep its current tab-group behavior, so that the new layout layer does not change how I edit text.
- As a user, I want the terminal UI to reserve space for the layout container, so that top-level widgets can be positioned without overlapping each other.
- As a user, I want resizing the terminal to update the visible UI cleanly, so that the active content stays within its assigned area.

## Functional Requirements
- [ ] **REQ-001**: The editor must have a root `layout` container above the existing tab group.
- [ ] **REQ-002**: The layout container must own exactly one tab group in this initial stage.
- [ ] **REQ-003**: The layout container must assign the tab group a defined screen region for rendering.
- [ ] **REQ-004**: The layout container must ensure the tab group stays within its assigned region and does not overlap unrelated UI space.
- [ ] **REQ-005**: The layout container must update the tab group’s usable size when the terminal is resized.
- [ ] **REQ-006**: The layout container must continue to route user actions to the active tab group without changing existing editing behavior.
- [ ] **REQ-007**: The layout container must preserve the active tab group’s state when redrawing or resizing.
- [ ] **REQ-008**: The layout container must keep the current tab group behavior visible to the user, including the existing tab bar and editing area.

## Non-Functional Requirements
- [ ] **REQ-009**: The layout layer must remain responsive during redraws and terminal resizes.
- [ ] **REQ-010**: The layout layer must remain compatible with the existing modal editing and action-processing flow.
- [ ] **REQ-011**: The layout layer must be covered by unit tests for rendering bounds, resize handling, and action routing.

## Acceptance Criteria
- [ ] **AC-001**: Launching urvim still presents a usable editor with the tab group rendered inside the new layout layer.
- [ ] **AC-002**: The tab group renders only inside the region assigned by the layout container.
- [ ] **AC-003**: Resizing the terminal keeps the tab group content and tab bar within bounds.
- [ ] **AC-004**: Editing actions continue to reach the active tab group exactly as before.
- [ ] **AC-005**: The editor still starts with a single tab-group-based workspace, with no visible multi-widget layout exposed yet.

## Out of Scope
- Multiple tab groups in the same layout.
- Split panes, floating windows, and nested layout trees.
- Drag-and-drop widget rearrangement.
- Persisting layout state across editor restarts.
- User-facing commands for creating, closing, or reordering layout regions.

## Assumptions
- The first layout implementation is the new root UI container.
- The initial layout contains one tab group and no other widget types.
- The layout occupies the full terminal area available to the editor.
- Existing tab-group behavior should remain unchanged from the user’s point of view.
- Future support for additional tab groups or other window types will be added in later stages.

## Dependencies
- Existing `TabGroup` rendering and action routing.
- Existing `Widget` trait and action-processing flow.
- Existing `Screen` rendering and terminal resize handling.
- Existing `Window` and `Buffer` behavior inside the tab group.
