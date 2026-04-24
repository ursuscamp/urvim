# Generic UI Components and Notification Infrastructure

## Summary
Refactor urvim's UI architecture so it can support a wider set of UI components beyond windows and buffers, while preserving current editing behavior. The first delivered example of the new architecture is a top-right notification banner with level-based styling and dual-write notification macros that write to both debug logging and a UI notification queue.

## Problem Statement
urvim's current UI flow is window-centric: action handling, rendering, and focus are tightly coupled to `Layout -> WindowGroup -> Window`. This makes it harder to add non-window components like floating UI, command-line surfaces, or transient overlays without invasive changes. The project needs a more generic UI component model that can host and route events to different component types while keeping existing editor workflows stable.

## User Stories
- As an urvim user, I want my existing editing and pane workflows to keep working after the UI refactor, so that new UI capabilities do not regress current behavior.
- As an urvim user, I want save confirmations and important errors to appear as brief on-screen notifications, so that I receive immediate feedback without checking logs.
- As an urvim contributor, I want a generic widget/component contract, so that I can add new UI pieces (for example command line and floating surfaces) without rewriting core layout plumbing.
- As an urvim contributor, I want notification macros that also log to `debug.log`, so that UI feedback and diagnostics stay consistent.

## Functional Requirements
- [ ] **REQ-001**: The `Widget` contract shall be expanded to support generic UI component behavior required for routing, layout, and rendering, while preserving compatibility with existing window-based behavior.
- [ ] **REQ-002**: The root UI flow shall support layered composition so non-pane UI components can be rendered independently of the split-pane content.
- [ ] **REQ-003**: The UI flow shall support focus/event routing that can target non-window components when they are active.
- [ ] **REQ-004**: A notification queue shall be added to UI state and shall display notifications sequentially in enqueue order.
- [ ] **REQ-005**: A notification banner component shall render the currently active queued notification at the top-right of the screen.
- [ ] **REQ-006**: Notification display duration shall be adaptive: 3 seconds when no backlog exists, and 1 second per message while one or more additional queued notifications are waiting.
- [ ] **REQ-007**: Notification rendering shall support level-specific visual styling for info, warn, and error levels.
- [ ] **REQ-008**: Notification macros `notify!`, `notify_info!`, `notify_warn!`, and `notify_error!` shall be added.
- [ ] **REQ-009**: Notification macros shall dual-write each notification event to both `debug.log` and the UI notification queue.
- [ ] **REQ-010**: Successful file-save events shall emit user-facing info notifications via the notification system.
- [ ] **REQ-011**: User-impacting runtime failures (curated in this phase) shall emit error notifications via the notification system.
- [ ] **REQ-012**: Existing non-notification logging behavior shall remain available for call sites that are intentionally log-only.

## Non-Functional Requirements
- **Reliability**: Existing editing operations, mode transitions, pane split/focus behavior, and rendering stability must remain functionally equivalent after refactor.
- **Performance**: Notification enqueue/render and layered routing must not introduce noticeable interaction latency in normal editing usage.
- **Compatibility**: Existing keymaps and action handling semantics must continue to work unless explicitly changed by this spec.
- **Usability**: Notification messages must be visible, readable, and non-blocking; they should not take focus away from editing workflows.
- **Maintainability**: The resulting UI architecture should make adding future components (floating windows, completion, dialogs) straightforward without duplicating layout/event code.

## Acceptance Criteria
- [ ] **AC-001**: A full normal editing session (navigation, insert, delete/change/yank, pane split/focus, save, quit) behaves the same as before the refactor.
- [ ] **AC-002**: The codebase contains an expanded `Widget`-based UI contract that is used by existing root/window UI paths.
- [ ] **AC-003**: The codebase contains layered UI rendering/routing support and at least one non-window layer participant.
- [ ] **AC-004**: Triggering a save success results in a top-right info notification visible for approximately 3 seconds.
- [ ] **AC-005**: When multiple notifications are emitted quickly, they are shown sequentially in enqueue order rather than replacing one another.
- [ ] **AC-006**: Info/warn/error notifications are visually distinguishable by style.
- [ ] **AC-007**: `notify!`, `notify_info!`, `notify_warn!`, and `notify_error!` are available and verified to write to both log output and notification queue.
- [ ] **AC-008**: Curated user-impacting errors produce error notifications.
- [ ] **AC-009**: During notification backlog periods, each message remains visible for approximately 1 second until the queue is drained, after which new notifications return to approximately 3 seconds visibility.

## Out of Scope
- Completion popup implementation.
- Command-line widget scaffold and full command-line command ecosystem.
- Rich notification-history UI controls (for example persistent history browser, manual per-item dismissal controls, and filtering).
- Mouse interaction and pointer-driven notification dismissal.

## Assumptions
- `specs/` remains the active specs base directory.
- `debug.log` continues to be the normal application log sink.
- Notification queue lifetime is process-scoped runtime state and does not need persistence.
- Notification queue is unbounded for this phase.
- A curated set of user-impacting errors is sufficient for phase 1 and can be expanded later.

## Dependencies
- Existing `Layout`, `WindowGroup`, `Window`, and `Widget` modules.
- Existing rendering primitives (`Screen`, theme styles, terminal output).
- Existing logging pipeline (`tracing` + logger setup writing to `debug.log`).
- Existing action/event loop in `main.rs` for integration of new routing and notification updates.
