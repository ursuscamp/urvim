# Intent-First Input Pipeline Refactor

## Summary
Refactor the editor input architecture from action-first dispatch to intent-first dispatch so UI orchestration flows through `Command` and editing flows through `Action`, with no UI concepts represented as `ActionKind` variants.

## Problem Statement
The current input system is action-centric from keymap to dispatch. This makes UI concerns (such as opening overlays) awkward to model, encourages bridge actions for non-editing behavior, and blurs the distinction between editing semantics and UI orchestration. The architecture should align with the existing `Intent` model and route UI interactions as commands at the source.

## User Stories
- As a maintainer, I want key handling to emit intents directly, so that editing and UI orchestration are clearly separated.
- As a maintainer, I want UI overlays (for example command-line) to be opened via commands, so that UI behavior is not encoded in editing action enums.
- As a contributor, I want dispatch interfaces to reflect app semantics, so that adding future overlays and widgets is straightforward and consistent.

## Functional Requirements
- [ ] **REQ-001**: The input handling pipeline must become intent-first, with key processing able to emit either `Intent::Action` or `Intent::Command` directly.
- [ ] **REQ-002**: No UI-only behaviors may rely on `ActionKind` bridge variants after migration.
- [ ] **REQ-003**: Opening the command-line overlay from `:` must be represented as a UI command intent.
- [ ] **REQ-004**: Split, pane, wrap, and quit behaviors that orchestrate the UI must be represented as commands rather than action bridge variants.
- [ ] **REQ-005**: Existing editing behavior must remain functionally unchanged for normal, insert, visual, visual-line, and resizing modes.
- [ ] **REQ-006**: Existing UI command behavior (for example notification enqueue) must remain functionally unchanged.
- [ ] **REQ-007**: Mode/keymap abstractions must support emitting command intents without bypassing existing key-sequence resolution features (counts, prefixes, multi-key sequences where applicable).
- [ ] **REQ-008**: Routing and dispatch must preserve overlay-first UI event precedence.
- [ ] **REQ-009**: Tests must cover command intent emission from key handling and verify no regression in core editing flows.
- [ ] **REQ-010**: Public API and module documentation must describe intent-first dispatch responsibilities and separation of concerns.
- [ ] **REQ-011**: `Intent`, `HandleKeyResult`, and related constructors must be ergonomic to build via `From<Action>` / `From<Command>` and generic `Into`-based constructors where appropriate.

## Non-Functional Requirements
- **Performance**: Input-to-dispatch latency must remain effectively unchanged during interactive editing.
- **Reliability**: Refactor must not introduce crashes or panics for invalid sequences, partial sequences, or rapid key entry.
- **Compatibility**: Existing keybindings and user-visible behavior must remain compatible except for internal architectural cleanup.
- **Maintainability**: New architecture must make future UI command additions possible without introducing action-level bridge variants.

## Acceptance Criteria
- [ ] **AC-001**: `:` in normal mode yields a command intent path (not an action bridge) that opens the command-line overlay.
- [ ] **AC-002**: `ActionKind::OpenCommandLine`, split/pane bridge actions, quit bridge actions, and any equivalent UI-only bridge action are removed.
- [ ] **AC-003**: Key handling interfaces and dispatch path compile and pass tests with intent-first contracts.
- [ ] **AC-004**: Representative editing sequences (movement, insert, delete/change/yank, tab/split controls) continue to pass regression tests.
- [ ] **AC-005**: Notification command dispatch, overlay routing, pane/split orchestration, and quit behavior continue to pass regression tests.
- [ ] **AC-006**: Architecture documentation is updated to explain intent-first input flow.

## Out of Scope
- Adding new editor commands or changing command-line command semantics.
- Reworking rendering systems unrelated to input/dispatch contracts.
- Introducing plugin APIs or external scripting interfaces.

## Assumptions
- Existing `Intent` and `Command` types remain the canonical dispatch envelope for app-level orchestration.
- Keymap and mode abstractions can be extended without changing user-facing keybinding syntax.
- Existing tests provide sufficient baseline coverage to detect regressions during migration.

## Dependencies
- Editor mode key handling traits and result types.
- Keymap resolution infrastructure.
- Main event loop intent/action dispatch pipeline.
- Layout command dispatch and overlay routing subsystems.