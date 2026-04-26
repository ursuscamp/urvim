# Command Line for Save and Edit

## Summary
Add a normal-mode command line that lets users execute editor commands from a centered floating UI. The first supported commands are `save` and `edit`, including quoted path arguments and session command history.

## Problem Statement
Users currently lack a command-driven workflow for core file actions. They need a fast, keyboard-native way to run file commands (`save`, `edit`) and to receive clear feedback when commands fail. The editor also has floating bordered UI logic tied to notifications, which prevents reuse for new overlays.

## User Stories
- As a normal-mode user, I want to press `:` and open a command line, so that I can execute file commands quickly.
- As a user, I want `save` and `save <path>` commands, so that I can save the current buffer or save it under a new file path.
- As a user, I want `edit` and `edit <path>` commands, so that I can open unnamed buffers and open or switch to file buffers.
- As a user, I want command errors to appear in notifications, so that feedback is consistent with existing editor messaging.
- As a developer, I want a reusable floating window abstraction, so that command line and notification UI can share common behavior.

## Functional Requirements
- [ ] **REQ-001**: In normal mode, pressing `:` must open a command-line overlay.
- [ ] **REQ-002**: The command-line overlay must be displayed as a bordered floating window centered on screen.
- [ ] **REQ-003**: The command-line overlay must accept text input and support in-line editing with backspace.
- [ ] **REQ-004**: While command-line overlay is focused, `Enter` must execute the current command line.
- [ ] **REQ-005**: While command-line overlay is focused, `Esc` must cancel command entry and close the overlay without executing.
- [ ] **REQ-006**: The command-line overlay must maintain command history for the current editor session only.
- [ ] **REQ-007**: Command history navigation must support Up/Down arrow keys and `Ctrl-p`/`Ctrl-n`.
- [ ] **REQ-008**: Command parsing must support quoted arguments so file paths containing spaces can be passed.
- [ ] **REQ-009**: Command `save` with no arguments must save the current buffer when it has an associated file path.
- [ ] **REQ-010**: Command `save` with no arguments must show an error notification when the current buffer has no associated file path.
- [ ] **REQ-011**: Command `save <path>` must save the current buffer to a new path when the target path does not already exist.
- [ ] **REQ-012**: Command `save <path>` must show an error notification and must not overwrite when the target path already exists.
- [ ] **REQ-013**: Command `edit` with no arguments must open a new unnamed buffer.
- [ ] **REQ-014**: Command `edit <path>` must switch to an already open buffer for that path when one exists.
- [ ] **REQ-015**: Command `edit <path>` must open a new buffer for that path when no existing open buffer matches.
- [ ] **REQ-016**: Unknown commands or invalid command arguments must produce an error through the existing notification system.
- [ ] **REQ-017**: After command execution is attempted (success or error), the command-line overlay must close.
- [ ] **REQ-018**: The notification banner must use a generic floating window abstraction rather than notification-specific floating logic.
- [ ] **REQ-019**: A shared floating window abstraction must support both notification banner and command-line overlay use cases.

## Non-Functional Requirements
- **Performance**: Opening the command-line overlay and executing supported commands should feel immediate during interactive editing.
- **Reliability**: Command execution failures (invalid syntax, invalid state, filesystem conflicts) must not crash the editor.
- **Compatibility**: Existing notification behavior and key handling outside command-line mode must remain unchanged.
- **Usability**: Command feedback must remain consistent by using the existing notification system for errors.

## Acceptance Criteria
- [ ] **AC-001**: Pressing `:` in normal mode opens a centered bordered command-line overlay.
- [ ] **AC-002**: Typing, backspace editing, `Esc` cancel, and `Enter` execution all work in the command-line overlay.
- [ ] **AC-003**: Up/Down and `Ctrl-p`/`Ctrl-n` both navigate command history within a session.
- [ ] **AC-004**: `save` saves path-backed buffers and reports an error for unnamed buffers.
- [ ] **AC-005**: `save <path>` succeeds when path does not exist and fails with error notification when path exists.
- [ ] **AC-006**: `edit` opens a new unnamed buffer.
- [ ] **AC-007**: `edit <path>` switches to existing open buffer when available; otherwise opens a new buffer.
- [ ] **AC-008**: Quoted path arguments are parsed correctly for `save` and `edit`.
- [ ] **AC-009**: Unknown commands and invalid arguments are reported via notification banner.
- [ ] **AC-010**: Command-line overlay closes after each execute attempt regardless of success or error.
- [ ] **AC-011**: Notification banner still functions and is implemented via the shared floating window abstraction.

## Out of Scope
- Persistent command history across editor restarts.
- Additional Ex-style commands beyond `save` and `edit`.
- Command auto-completion, inline suggestions, or syntax highlighting in the command line.
- Save-overwrite confirmation prompts or force-save modes.

## Assumptions
- Existing buffer pool/path tracking can determine whether a path is already open.
- Existing notification pipeline is available for command errors.
- The editor can represent unnamed buffers independently from file-backed buffers.

## Dependencies
- Existing normal-mode key handling for binding `:`.
- Existing notification queue/banner infrastructure for error messages.
- Existing buffer management and file persistence behavior.