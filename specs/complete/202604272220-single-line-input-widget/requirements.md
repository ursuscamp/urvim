# Single-Line Input

## Summary
Create a reusable single-line input widget for shell-style text entry. The widget must work for both command-line and picker UIs, provide common single-line editing keys out of the box, and allow consumers to override any key handling behavior.

## Problem Statement
Command-line and picker UIs need the same core line-editing behavior, but that behavior should live in one reusable widget rather than being duplicated per consumer. The widget should feel like normal shell line editing, not Vim editing, and it should allow callers to customize key handling without losing default text insertion for unhandled keys.

## User Stories
- As a user, I want shell-style line editing keys to work in the command line and picker inputs, so that text entry feels familiar and efficient.
- As a maintainer, I want one shared widget for single-line input, so that command-line and picker behavior stays consistent.
- As a consumer, I want to override any key handling I need, so that the widget can adapt to different overlays and workflows.

## Functional Requirements
- [ ] **REQ-001**: The widget must support editing a single line of text.
- [ ] **REQ-002**: The widget must be reusable by both command-line and picker UIs.
- [ ] **REQ-003**: The widget must support normal text input for keys that are not overridden by the consumer and are not handled by built-in editing logic.
- [ ] **REQ-004**: The widget must allow consumers to override handling for any key press.
- [ ] **REQ-005**: The widget must provide shell-style built-in editing keys for deleting backward by word.
- [ ] **REQ-006**: The widget must provide shell-style built-in editing keys for deleting from the cursor to the start of the line.
- [ ] **REQ-007**: The widget must provide shell-style built-in movement by word in both directions.
- [ ] **REQ-008**: The widget must provide built-in `Home` and `End` navigation.
- [ ] **REQ-009**: The widget must provide built-in `Backspace` and `Delete` behavior.
- [ ] **REQ-010**: The widget must provide built-in `Ctrl-U` behavior for clearing from cursor to line start.
- [ ] **REQ-011**: `Enter` and `Esc` must do nothing unless a consumer overrides them.
- [ ] **REQ-012**: The widget must preserve cursor and text state correctly across insertions, deletions, and navigation.
- [ ] **REQ-013**: The widget must expose a consumer-facing API that supports live text updates and custom key interception.
- [ ] **REQ-014**: The widget must be suitable for use as the shared editing core for command-line and picker widgets without requiring caller-specific editing logic.

## Non-Functional Requirements
- **Maintainability**: Shared line editing behavior must live in one widget implementation.
- **Consistency**: Command-line and picker input should behave identically for the same key sequence unless a consumer explicitly overrides a key.
- **Usability**: Default key bindings should feel like standard shell line editing.

## Acceptance Criteria
- [ ] **AC-001**: Typing into the widget inserts text normally when no override handles the key.
- [ ] **AC-002**: Word-wise movement and deletion work with the widget’s built-in shell-style bindings.
- [ ] **AC-003**: `Home`, `End`, `Backspace`, `Delete`, and `Ctrl-U` work by default.
- [ ] **AC-004**: `Enter` and `Esc` have no default effect unless overridden by the consumer.
- [ ] **AC-005**: A consumer can override any key and replace the widget’s default handling for that key.
- [ ] **AC-006**: The same widget implementation can be embedded in both command-line and picker UIs.

## Out of Scope
- Vim-style line editing bindings.
- Multiline text editing.
- History navigation or submission semantics for command-line or picker flows.
- Search/filter logic for picker result sets.

## Assumptions
- The existing UI architecture can host a reusable widget with consumer-supplied callbacks.
- Command-line and picker overlays can both delegate text-entry behavior to the same widget instance or shared component.
- Key handling already has a way to represent raw key presses before they become editor actions.

## Dependencies
- Widget trait and overlay composition infrastructure.
- Key input representation and dispatch plumbing.
- Existing command-line and picker UI implementations.
