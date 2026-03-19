# Open Line Below and Above

## Summary

Implement the Vim-style `o` and `O` keys in normal mode to open a new line below or above the current line and enter insert mode.

## Problem Statement

Users familiar with Vim expect to be able to quickly insert new lines using `o` (below) and `O` (above). Currently, urvim lacks these fundamental editing commands that are essential for efficient text editing workflows.

## User Stories

- As a Vim user, I want to press `o` to create a new line below my current line and start typing, so I can quickly add content without manually navigating to the end of the line and pressing Enter.

- As a Vim user, I want to press `O` to create a new line above my current line and start typing, so I can add comments or headers above existing content efficiently.

- As a user, I want the cursor to be positioned at the start of the newly created line in insert mode, so I can begin typing immediately.

## Functional Requirements

- [ ] **REQ-001**: Pressing `o` in normal mode creates a new empty line below the current line
- [ ] **REQ-002**: Pressing `O` in normal mode creates a new empty line above the current line
- [ ] **REQ-003**: After creating the new line, the editor enters insert mode
- [ ] **REQ-004**: The cursor is positioned at the beginning of the new line (column 0)
- [ ] **REQ-005**: The new line is inserted into the buffer and persisted when the file is saved
- [ ] **REQ-006**: The count prefix (e.g., `3o` or `3O`) creates multiple lines (3 lines below or above)

## Non-Functional Requirements

- **Performance**: The operation should complete in under 1ms, providing instant feedback

## Acceptance Criteria

- [ ] **AC-001**: Pressing `o` on line 3 of a 5-line file creates a new line 4, shifting lines 4-5 down, and enters insert mode at column 0
- [ ] **AC-002**: Pressing `O` on line 3 of a 5-line file creates a new line 3, shifting the original line 3 and below down, and enters insert mode at column 0
- [ ] **AC-003**: The editor remains in insert mode after the operation
- [ ] **AC-004**: Pressing Escape cancels the operation (returns to normal mode without creating a line - Vim creates the line first then enters insert mode, so Escape goes back to normal with the line remaining)
- [ ] **AC-005**: If on the last line (`o`) or first line (`O`), the command still works correctly

## Out of Scope

- Visual mode `o` and `O` behavior (different in Vim, swap lines)

## Assumptions

- The editor has a functioning buffer and document model
- Insert mode is already implemented
- Normal mode key handling infrastructure exists

## Dependencies

- Buffer insertion/deletion functionality
- Mode switching (normal to insert mode)
- Cursor position management
