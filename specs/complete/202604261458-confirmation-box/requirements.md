# Confirmation Box

## Summary
Add a reusable confirmation box for simple yes/no prompts. The confirmation box should be usable anywhere the editor needs to ask the user whether to proceed with a potentially destructive or interrupting action, including quitting while buffers have unsaved changes. The caller must be able to provide the positive intent at construction time so the same component can be reused for different actions.

## Problem Statement
The editor needs a consistent way to ask the user to confirm simple decisions before continuing with an action. Today, flows such as quitting with modified buffers need a reusable confirmation surface that clearly presents a yes/no choice, works entirely from the keyboard, and returns a meaningful intent when the user accepts. Without a reusable component, each caller must implement its own ad hoc prompt behavior, leading to inconsistent interaction and duplicated logic.

## User Stories
- As an urvim user, I want a clear yes/no confirmation prompt before quitting with unsaved changes, so that I do not lose work accidentally.
- As an urvim user, I want to answer confirmation prompts with the keyboard, so that I can keep using the editor without switching to the mouse.
- As an urvim contributor, I want a reusable confirmation component, so that multiple editor flows can share the same interaction pattern.
- As an urvim contributor, I want to specify the positive intent when constructing the confirmation box, so that the component can be reused for different actions without hardcoding behavior.

## Functional Requirements
- [ ] **REQ-001**: The editor shall provide a reusable confirmation box for simple yes/no questions.
- [ ] **REQ-002**: The confirmation box shall display a caller-supplied query message to the user.
- [ ] **REQ-003**: The confirmation box API shall allow the caller to specify the positive intent when the box is constructed.
- [ ] **REQ-004**: When the user confirms with Yes, the confirmation box shall return the caller-supplied positive intent.
- [ ] **REQ-005**: When the user declines with No, the confirmation box shall cancel the pending action and return no intent.
- [ ] **REQ-006**: The confirmation box shall accept explicit `Y` and `N` key inputs as confirmation and cancellation choices.
- [ ] **REQ-007**: The confirmation box shall treat `Enter` as Yes and `Esc` as No.
- [ ] **REQ-008**: The confirmation box shall be reusable by the quit-with-unsaved-files flow and similar future flows that need a simple proceed-or-cancel decision.
- [ ] **REQ-009**: The confirmation box shall not alter the editor state unless the user confirms the action.
- [ ] **REQ-010**: The confirmation box shall remain usable from the keyboard without requiring pointer interaction.

## Non-Functional Requirements
- **Usability**: The prompt should make the user’s choice obvious and keep the yes/no decision simple to understand.
- **Compatibility**: The confirmation box should integrate with existing editor intent dispatch so callers can pass through a positive intent without special-case plumbing.
- **Reliability**: Canceling the prompt must never trigger the associated action.
- **Maintainability**: The confirmation box should support future reuse without requiring duplicate prompt implementations.

## Acceptance Criteria
- [ ] **AC-001**: A caller can construct a confirmation box with a custom query message and a supplied positive intent.
- [ ] **AC-002**: Pressing `Y` or `Enter` while the prompt is active returns the supplied positive intent.
- [ ] **AC-003**: Pressing `N` or `Esc` while the prompt is active cancels the action and returns no intent.
- [ ] **AC-004**: Attempting to quit with modified buffers shows the confirmation box before the quit action proceeds.
- [ ] **AC-005**: Choosing No during a quit-with-modified-buffers prompt leaves the editor open and preserves unsaved changes.
- [ ] **AC-006**: Choosing Yes during a quit-with-modified-buffers prompt allows the quit flow to continue.
- [ ] **AC-007**: The same confirmation box can be reused for at least one non-quit caller without changing its yes/no semantics.

## Out of Scope
- Multi-button dialogs or prompts with more than yes/no choices.
- Mouse interaction for confirmation prompts.
- Persistent prompt history or undoing a canceled confirmation.
- Changing the semantics of modified-buffer tracking itself.

## Assumptions
- The editor already routes user actions and UI commands through an `Intent`-based dispatch path.
- A confirmation box can be represented as a reusable widget or equivalent UI component in the existing UI architecture.
- The quit-with-unsaved-files flow already has access to the information needed to decide whether a confirmation is required.

## Dependencies
- Intent dispatch and UI command routing.
- Widget or overlay infrastructure for presenting modal UI surfaces.
- Existing modified-buffer tracking used by the quit flow.
