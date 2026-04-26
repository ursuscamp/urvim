# Overlay Components as Widgets

## Summary
Refactor overlay UI components so both the command-line overlay and notification banner are implemented as widgets, and establish a consistent architecture rule that contained UI components are widget-based.

## Problem Statement
Current overlay behavior is implemented through layout-managed logic and module-level render helpers rather than widget encapsulation. This creates mixed responsibilities in layout code, makes component reuse harder, and diverges from the desired UI architecture direction.

## User Stories
- As a maintainer, I want overlays like command-line and notification banner to be widgets, so that UI behavior is modular and consistent.
- As a contributor, I want contained UI components to follow one architectural pattern, so that future additions are easier to design and review.
- As a user, I want command-line and notifications to behave exactly as before after the refactor, so that architecture improvements do not regress UX.

## Functional Requirements
- [ ] **REQ-001**: The command-line overlay must be implemented as a widget using the `Widget` trait.
- [ ] **REQ-002**: The notification banner must be implemented as a widget using the `Widget` trait.
- [ ] **REQ-003**: Layout/root orchestration must compose and route events to these widgets rather than owning their internal UI logic.
- [ ] **REQ-004**: Existing command-line behavior must remain unchanged (open/close, input editing, history, execution, error notifications).
- [ ] **REQ-005**: Existing notification behavior must remain unchanged (queue progression, TTL handling, placement, wrapping, styling).
- [ ] **REQ-006**: Shared floating window rendering must continue to be reused by both widgets.
- [ ] **REQ-007**: Widget focus/event precedence must preserve overlay-first handling semantics.
- [ ] **REQ-008**: Project guidance must codify that contained UI components should be widgets.
- [ ] **REQ-009**: Tests must validate widgetized command-line and notification behavior with no user-visible regressions.

## Non-Functional Requirements
- **Performance**: Overlay rendering and event handling must remain responsive with no perceptible latency increase.
- **Reliability**: Refactor must not introduce crashes or stuck-focus states during overlay interactions.
- **Maintainability**: Overlay logic should be cohesive within widget modules and reduce mixed responsibilities in layout.
- **Compatibility**: Existing keybindings and notification UX remain intact.

## Acceptance Criteria
- [ ] **AC-001**: Command-line overlay is represented by a widget implementation with its own event and render lifecycle.
- [ ] **AC-002**: Notification banner is represented by a widget implementation with its own event and render lifecycle.
- [ ] **AC-003**: Layout no longer directly contains command-line/notification rendering internals.
- [ ] **AC-004**: Existing command-line interaction tests pass without behavior regressions.
- [ ] **AC-005**: Existing notification queue/render tests pass without behavior regressions.
- [ ] **AC-006**: New/updated architecture documentation and project guidance state that contained UI components should be widgets.

## Out of Scope
- Adding new command-line features or new notification types.
- Reworking unrelated window, status bar, or split rendering behavior.
- Broad redesign of all existing UI modules beyond command-line and notification overlays.

## Assumptions
- The current `Widget` trait is sufficient for overlay component extraction with minor extension if needed.
- Existing intent/command pipeline remains the integration point for overlay side effects.
- Floating window abstraction remains the common frame primitive.

## Dependencies
- `Widget` trait and UI event routing infrastructure.
- Layout root orchestration and overlay routing paths.
- Command-line state/parse/execute module.
- Notification queue/state and rendering module.
