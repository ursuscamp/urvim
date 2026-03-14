# Hierarchical Action System

## Summary

Implement a hierarchical action system where "widgets" can receive actions and return whether they handled them. The main loop receives an action from the mode handler, passes it to the window to process, and if the window doesn't handle it, the app processes it. This enables a modular, extensible architecture for handling user input across multiple widgets.

## Problem Statement

Currently, all key actions from the mode handler are processed directly in the main event loop (main.rs). This creates a monolithic structure where:
- All action handling logic lives in one place
- Adding new widgets requires modifying the main loop
- There's no standard way for widgets to claim or reject actions

This architecture makes it difficult to add more widgets (e.g., status bar, command palette, file tree) that can handle their own key bindings independently.

## User Stories

- **As a** developer, **I want** widgets to process actions first **so that** each widget can handle its own key bindings independently.

- **As a** developer, **I want** an enum to indicate whether an action was handled **so that** I can easily check if processing succeeded and fall back to app-level handling.

- **As a** developer, **I want** the app to process unhandled actions **so that** global actions like mode switching and quit work regardless of which widget was focused.

- **As a** developer, **I want** a Widget trait **so that** I can easily add new widgets that follow the same action processing pattern.

## Functional Requirements

- [ ] **REQ-001**: Create an `ActionResult` enum with `Handled` and `NotHandled` variants to indicate whether a widget handled an action.

- [ ] **REQ-002**: Create a `Widget` trait that defines a `process_action(&Action) -> ActionResult` method for processing actions.

- [ ] **REQ-003**: Implement the `Widget` trait for `Window`, returning `ActionResult::Handled` for actions that modify the buffer (movement, insert), and `ActionResult::NotHandled` for other actions.

- [ ] **REQ-004**: Update the main event loop to:
  1. Get action from mode handler
  2. Pass action to window via `process_action()`
  3. If window returns `NotHandled`, process the action at app level (mode switching, quit)

- [ ] **REQ-005**: Ensure mode switching (Normal ↔ Insert) and quit actions are handled at the app level when not handled by the window.

## Non-Functional Requirements

- **Extensibility**: The Widget trait should be simple to implement for new widget types (status bar, command palette, etc.)
- **Performance**: Action processing should be O(1) with minimal overhead
- **Code Clarity**: The action flow should be obvious from reading the code

## Acceptance Criteria

- [ ] **AC-001**: Movement keys (h, j, k, l, arrow keys) are handled by the window and move the cursor correctly.

- [ ] **AC-002**: Character insertion in insert mode is handled by the window and inserts characters into the buffer.

- [ ] **AC-003**: Mode switching (Escape to Normal, 'i' to Insert) is handled at the app level and correctly switches modes.

- [ ] **AC-004**: Quit action (Ctrl-q) is handled at the app level and exits the editor.

- [ ] **AC-005**: The `ActionResult` enum correctly indicates whether an action was handled.

- [ ] **AC-006**: The system is extensible - adding a new widget requires implementing the `Widget` trait.

## Out of Scope

- Adding additional widgets (status bar, command palette, file tree)
- Complex widget hierarchies (nested widgets)
- Focus management between widgets

## Assumptions

- The current `Action` enum (renamed from KeyAction) will continue to be used for action representation
- The window is the primary widget that handles buffer-related actions
- Future widgets will be added that handle their own specific actions

## Dependencies

- **Internal**: Uses existing `Action` enum (renamed from KeyAction) from `editor.rs`, `Window` from `window.rs`
- **Blocked by**: None - this is a new feature built on existing code
