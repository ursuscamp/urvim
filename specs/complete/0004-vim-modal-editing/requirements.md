# Vim-Style Modal Editing

## Summary

Implement vim-style modal editing with Insert and Normal modes in the terminal editor. The editor will have two distinct modes: Normal mode for navigation and command execution, and Insert mode for text input. Each mode will have unique cursor styling and keybindings.

## Problem Statement

Currently, the editor lacks modal editing capabilities. Users familiar with vim expect to be able to switch between Normal mode (for navigation and commands) and Insert mode (for text entry). Without this core vim functionality, users cannot efficiently navigate and edit text using vim-style workflows.

## User Stories

- **As a** vim user, **I want** to press `Esc` to switch to Normal mode **so that** I can navigate and execute commands without inserting text.
- **As a** vim user, **I want** to press `i` in Normal mode to switch to Insert mode **so that** I can type and edit text.
- **As a** vim user, **I want** to use `h/j/k/l` for cursor movement in Normal mode **so that** I can navigate efficiently without leaving the home row.
- **As a** vim user, **I want** to see a block cursor in Normal mode and a bar cursor in Insert mode **so that** I can easily identify the current mode visually.

## Functional Requirements

- [ ] **REQ-001**: Create a `Mode` trait that defines handle_key and cursor_style methods
- [ ] **REQ-002**: Define a `KeyAction` enum that represents actions triggered by keypresses
- [ ] **REQ-003**: Normal mode processes keys and returns appropriate actions (movement, mode switch, quit)
- [ ] **REQ-004**: Insert mode processes keys and returns appropriate actions (character insertion, mode switch)
- [ ] **REQ-005**: Implement cursor movement actions: `MoveLeft` (h), `MoveDown` (j), `MoveUp` (k), `MoveRight` (l)
- [ ] **REQ-006**: Implement character insertion action: `InsertChar(c)` for typing characters
- [ ] **REQ-007**: Implement mode switch actions: `SwitchToNormal` (Esc) and `SwitchToInsert` (i)
- [ ] **REQ-008**: Implement quit action: `Quit` (Ctrl-q)
- [ ] **REQ-009**: Render block cursor (full block character) in Normal mode
- [ ] **REQ-010**: Render bar cursor (vertical bar/line) in Insert mode
- [ ] **REQ-011**: Main event loop processes actions and updates editor state accordingly

## Non-Functional Requirements

- **Performance**: Mode switching should be instantaneous (< 1ms)
- **Usability**: Cursor shape change must be visible immediately upon mode change

## Acceptance Criteria

- [ ] **AC-001**: Pressing `Esc` in Insert mode switches to Normal mode
- [ ] **AC-002**: Pressing `i` in Normal mode switches to Insert mode
- [ ] **AC-003**: In Normal mode, pressing `h` moves cursor left
- [ ] **AC-004**: In Normal mode, pressing `j` moves cursor down
- [ ] **AC-005**: In Normal mode, pressing `k` moves cursor up
- [ ] **AC-006**: In Normal mode, pressing `l` moves cursor right
- [ ] **AC-007**: In Insert mode, pressing a character key inserts that character
- [ ] **AC-008**: Pressing Ctrl-q in any mode quits the editor
- [ ] **AC-009**: Normal mode displays a block cursor
- [ ] **AC-010**: Insert mode displays a bar cursor
- [ ] **AC-011**: The main event loop correctly dispatches actions from key inputs

## Out of Scope

- Additional vim modes (Visual, Command-line, etc.)
- Vim commands (d, y, c, etc.) beyond mode switching
- Macros and registers
- Search and replace functionality

## Assumptions

- The editor already has a basic buffer and cursor implementation
- The terminal supports Unicode block and vertical bar characters
- The main event loop can process enum variants as actions

## Dependencies

- Existing buffer implementation for text storage
- Existing cursor/position tracking
- Existing terminal rendering system
