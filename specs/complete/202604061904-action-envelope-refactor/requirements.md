# Action Envelope Refactor

## Summary
Rework the editor action model so each action carries both its intent and its mode context. This should let the editor handle mode-sensitive behavior, mode transitions, and repeatable edits without hidden mode globals or a separate family of insert-only action variants.

## Problem Statement
Urvim currently treats `Action` as a single enum of editor intents. That makes it awkward to express when an action originated in a specific mode, when it should switch modes after it runs, and when behavior should differ based on the active mode. The current approach has already pushed mode-aware logic into special cases and global state, which is harder to reason about and test.

## User Stories
- As a user, I want mode-sensitive actions to behave consistently, so that insert-mode features still work without special hidden plumbing.
- As a maintainer, I want mode transitions to be explicit in the action model, so that mode changes are easier to trace and debug.
- As a user, I want undo, redo, and repeat to keep working after the action model changes, so that common editing flows remain reliable.
- As a contributor, I want normal-mode and insert-mode behavior to remain clearly separated at the action level, so that future features do not need extra globals or mode-specific action variants.

## Functional Requirements
- [ ] **REQ-001**: The editor must represent actions in a way that preserves both the action intent and the mode context associated with that action.
- [ ] **REQ-002**: The editor must record the mode that created an action and use that recorded source mode when deciding whether the action may be executed.
- [ ] **REQ-003**: The editor must support actions that transition the editor into a new mode as part of the same action record.
- [ ] **REQ-004**: The editor must support mode changes without requiring dedicated switch-only action variants for normal-to-insert or insert-to-normal transitions.
- [ ] **REQ-005**: The editor must continue to support insert-mode auto-pairing behavior for supported brackets and quotes after the action model change.
- [ ] **REQ-006**: The editor must continue to support closer skipping in insert mode when the cursor is immediately before a matching closer.
- [ ] **REQ-007**: The editor must continue to support pair-aware backspace behavior in insert mode when the cursor is between a matching opening and closing delimiter.
- [ ] **REQ-008**: The editor must continue to preserve normal-mode deletion behavior, including commands that delete backward outside insert mode.
- [ ] **REQ-009**: The editor must preserve undo and redo semantics for edits produced through the new action model.
- [ ] **REQ-010**: The editor must preserve dot-repeat behavior for repeatable edits created through the new action model.
- [ ] **REQ-011**: The editor must not rely on a global current-mode variable to decide whether an action is allowed or how it should be handled.
- [ ] **REQ-012**: The editor must keep unsupported mode/action combinations from mutating the buffer or moving the cursor in an unexpected way.

## Non-Functional Requirements
- **Compatibility**: Existing modal editing workflows must continue to feel the same to users after the refactor.
- **Reliability**: Mode-sensitive edits must remain deterministic across undo, redo, and repeat.
- **Maintainability**: The action model should make mode intent visible in one place instead of scattering mode checks across the editor.
- **Usability**: The refactor must not add new user-facing commands or configuration just to preserve existing editing behavior.

## Acceptance Criteria
- [ ] **AC-001**: Typing an opening bracket or quote in insert mode still produces the expected auto-paired insertion with the cursor placed between the pair.
- [ ] **AC-002**: Typing a matching closer immediately before an existing closer still skips over the closer instead of inserting a duplicate character.
- [ ] **AC-003**: Pressing backspace between an opening delimiter and its matching closer still removes both characters as a single logical edit.
- [ ] **AC-004**: Entering insert mode and returning to normal mode still works without dedicated switch-only action variants.
- [ ] **AC-005**: Undo after a paired insertion or paired deletion restores the exact pre-edit buffer and cursor state.
- [ ] **AC-006**: Redo after undo reapplies the same edit without introducing extra delimiter characters or mode drift.
- [ ] **AC-007**: An action created for one mode does not execute as a valid edit when dispatched from an incompatible mode.
- [ ] **AC-008**: Normal-mode delete commands continue to delete only the intended characters and do not use insert-mode pair behavior.

## Out of Scope
- Adding new delimiter families or changing the supported auto-pair set.
- Introducing a user-facing action inspector or replay history UI.
- Reworking the existing configuration system beyond what is needed to keep current behavior intact.
- Changing the editor’s visible mode labels or status bar wording.

## Assumptions
- `ActionKind` will remain the payload that describes the edit intent, while the action envelope stores mode metadata.
- A single originating mode is sufficient for each action.
- Mode transitions can be represented by an action with no payload plus a destination mode.
- The current supported auto-pair behavior should remain unchanged from the user’s point of view.

## Dependencies
- Existing mode-handling infrastructure.
- Existing buffer mutation, undo/redo, and repeat machinery.
- Existing insert-mode key handling and auto-pair behavior.
- Existing window dispatch and layout mode-label updates.
