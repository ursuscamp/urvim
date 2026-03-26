# Layout Status Bar

## Summary
urvim should add a persistent status bar to the root layout. The status bar must show the current mode, the active buffer file name, the cursor position with the column reported in bytes, and the percentage of the file that has been traversed. To support that display, the layout should also track a lightweight mode kind instead of deriving it indirectly from rendering state.

## Problem Statement
The editor currently exposes mode state only in `main`, and the top-level layout does not have a dedicated place to show editor metadata. That makes it impossible to present a consistent status line for the active buffer and cursor without duplicating state or threading mode information through the UI in an ad hoc way.

## User Stories
- As a user, I want to see my current mode, so that I know whether key presses will edit text or trigger commands.
- As a user, I want to see the active buffer name, so that I can confirm which file I am editing.
- As a user, I want to see my cursor position in line and byte column form, so that I can navigate precisely in the file.
- As a user, I want to see how far through the file I am, so that I can orient myself in longer buffers.

## Functional Requirements
- [ ] **REQ-001**: The root layout must reserve a dedicated status bar area separate from the tab-group content area.
- [ ] **REQ-002**: The layout must track a lightweight mode kind so the status bar can display mode information without duplicating state in the application loop.
- [ ] **REQ-003**: The status bar must display the current mode in a human-readable form.
- [ ] **REQ-004**: The status bar must display the active buffer name, using the buffer’s file name when available and a fallback label when the buffer is unnamed.
- [ ] **REQ-005**: The status bar must display the active cursor position using the current line and the cursor column measured in bytes.
- [ ] **REQ-006**: The status bar must display the percentage of the file traversed based on the active cursor line and total line count.
- [ ] **REQ-007**: When the cursor is on the last line of the buffer, the status bar must display `100%`.
- [ ] **REQ-008**: The status bar must update when the active buffer changes, the cursor moves, the mode kind changes, or the window is resized.
- [ ] **REQ-009**: The layout must keep existing editing and tab-switching behavior unchanged while adding the status bar.
- [ ] **REQ-010**: The layout must remain usable on narrow or short terminals by truncating or clamping status content instead of panicking.

## Non-Functional Requirements
- [ ] **REQ-011**: Status bar rendering must remain responsive during normal redraws and terminal resizes.
- [ ] **REQ-012**: The status bar and layout mode-kind tracking must remain compatible with the existing modal editing flow.
- [ ] **REQ-013**: The feature must be covered by unit tests for formatting, layout sizing, and mode-state routing.

## Acceptance Criteria
- [ ] **AC-001**: Launching urvim shows a status bar in the layout without breaking the existing editor view.
- [ ] **AC-002**: The status bar shows the active mode, active buffer name, cursor position, and file percentage.
- [ ] **AC-003**: The cursor column shown in the status bar matches the buffer cursor’s byte column.
- [ ] **AC-004**: The status bar shows `100%` when the cursor is on the last line of the active buffer.
- [ ] **AC-005**: Unnamed buffers show a fallback label instead of a blank file name.
- [ ] **AC-006**: Resizing the terminal keeps both the tab-group content and status bar within their assigned bounds.
- [ ] **AC-007**: Editing actions, tab switching, undo, and redo continue to work as they did before the status bar was added.

## Out of Scope
- Customizable status bar themes or user-configurable formatting.
- Multiple status bars or status bar widgets.
- Command-line options for toggling the status bar.
- Persisting mode or layout state across editor restarts.
- Additional layout regions such as split panes or sidebars.

## Assumptions
- The status bar lives in the root layout and is rendered as a footer row.
- The existing tab bar remains at the top of the tab group.
- The active buffer name should use the editor’s existing unnamed-buffer fallback when no file name is available.
- The mode label can be derived from the same mode state that currently drives cursor style and key handling.
- File progress is calculated from the active cursor line relative to the total number of lines in the buffer.

## Dependencies
- Existing `Layout` and `TabGroup` rendering and action routing.
- Existing buffer cursor, line-count, and file-name accessors.
- Existing mode implementations, cursor-style handling, and mode-to-label mapping.
- Existing `Screen` drawing primitives and terminal resize handling.
