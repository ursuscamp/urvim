# Window Gutter

## Summary

Add a line number gutter to the Window struct in urvim. The gutter displays line numbers on the left side of the editor window, similar to vim and other terminal-based text editors. It should be its own struct with its own render method, owned by Window.

## Problem Statement

Currently, urvim displays buffer content without any line numbers or gutter. Users of terminal-based text editors like vim expect a gutter showing line numbers for navigation and orientation. This feature improves usability by providing visual line numbering.

## User Stories

- As a user, I want to see line numbers in the gutter so that I can quickly navigate to specific lines
- As a user, I want the gutter to dynamically resize based on the longest visible line number so that alignment is always correct
- As a user, I want the gutter to have distinct styling so that it's visually separated from the buffer content

## Functional Requirements

- [ ] **REQ-001**: Create a `Gutter` struct in the window module with its own render method
- [ ] **REQ-002**: Window should own a Gutter instance (not store permanently, can create on render)
- [ ] **REQ-003**: Gutter should NOT hold a reference to Buffer - only need: start_line, visible_rows, and total_buffer_lines (passed at construction)
- [ ] **REQ-004**: Gutter should calculate width based on: `digits(total_buffer_lines) + 2` (1 space buffer on each side)
- [ ] **REQ-004a**: Width calculation must NOT convert numbers to strings - use mathematical digit counting to avoid allocation
- [ ] **REQ-005**: Gutter should render line numbers right-aligned with a trailing space
- [ ] **REQ-006**: Gutter should use sane default colors (e.g., dark background, light text)
- [ ] **REQ-007**: Gutter should render background for the entire window height (all visible rows), not just rows with content
- [ ] **REQ-008**: Gutter should track the last rendered buffer line number and skip rendering if the same line number would repeat (prepares for line wrapping)
- [ ] **REQ-009**: Window's render method should call gutter render at the correct position
- [ ] **REQ-010**: Buffer content origin should be offset by gutter width (content starts after gutter)
- [ ] **REQ-011**: Buffer render size should be reduced by gutter width (gutter consumes screen real estate)
- [ ] **REQ-012**: Visual cursor position calculation must account for gutter width (subtract gutter width from cursor's visual column)

## Non-Functional Requirements

- **Performance**: Gutter calculation should be O(n) where n is the number of visible lines
- **Compatibility**: Gutter should work with the existing Screen and Style types

## Acceptance Criteria

- [ ] **AC-001**: A new `Gutter` struct exists with a `render` method
- [ ] **AC-002**: Gutter does NOT hold a Buffer reference - receives start_line, visible_rows, and total_buffer_lines at construction
- [ ] **AC-003**: Window owns the gutter and calls its render method during rendering
- [ ] **AC-004**: Gutter width is calculated as `digits(total_buffer_lines) + 2` using mathematical calculation (no string conversion/allocation)
- [ ] **AC-005**: Gutter renders with a dark background color (e.g., ANSI 236) and light foreground (e.g., ANSI 245)
- [ ] **AC-006**: Line numbers are right-aligned within the gutter space
- [ ] **AC-007**: Gutter renders background for ALL visible rows (not just rows with content)
- [ ] **AC-008**: When the same buffer line would be rendered consecutively, the second gutter cell is left blank (prepares for wrapping)
- [ ] **AC-009**: Buffer content is rendered at origin offset by gutter width
- [ ] **AC-010**: Buffer render area is reduced by gutter width
- [ ] **AC-011**: Visual cursor position accounts for gutter width (cursor column shifted left by gutter width)
- [ ] **AC-012**: All existing Window tests pass

## Out of Scope

- Scrolling the gutter independently from buffer content (they scroll together)
- Clickable gutter (future enhancement)
- Git gutter marks or other annotations (future enhancement)
- Customizable gutter colors via configuration (future enhancement)

## Assumptions

- The gutter is always visible when a window is rendered
- The buffer always has at least one line (empty buffer shows "1" for first line)
- Window size is provided correctly to the render method

## Dependencies

- No external dependencies required
- Uses existing `Screen`, `Style`, `Position`, and `Size` types from the window module
