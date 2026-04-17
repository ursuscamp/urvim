# Window-Owned Mode State

## Summary

Move editor mode state from a single shared editor-wide value to per-window state. Each window should remember its own current mode, switch back to that mode when it regains focus, and start in normal mode when it is created.

## Problem Statement

urvim currently treats mode as shared editor state, which means window focus changes do not preserve the mode a user had in each window. This makes multi-window editing feel inconsistent because a window can lose its own modal context when the user switches away and back.

## User Stories

- As a user working in multiple windows, I want each window to remember its own mode, so that switching focus does not change the mode I left in that window.
- As a user, I want a newly created window to start in normal mode, so that fresh windows behave predictably.
- As a user, I want a closed window to forget its prior mode, so that recreated windows do not revive stale modal state.

## Functional Requirements

- [ ] **REQ-001**: Each editor window shall maintain its own current mode state.
- [ ] **REQ-002**: The editor shall not use a single shared mode state for all windows.
- [ ] **REQ-003**: A newly created window shall start in normal mode.
- [ ] **REQ-004**: When the user switches focus to a window, that window shall restore the mode it last had when it was focused.
- [ ] **REQ-005**: Mode changes shall apply only to the currently focused window.
- [ ] **REQ-006**: Closing a window shall discard that window’s mode state.
- [ ] **REQ-007**: If a previously closed window is recreated, it shall start in normal mode rather than restoring old mode state.
- [ ] **REQ-008**: The focused window’s current mode shall be reflected consistently in user-facing mode-dependent UI such as the status bar and mode-sensitive rendering behavior.
- [ ] **REQ-009**: Switching focus between windows shall not modify the hidden mode state stored in non-focused windows.

## Non-Functional Requirements

- **Compatibility**: Single-window editing behavior should remain unchanged.
- **Usability**: Switching between windows should preserve the modal context a user left behind, reducing surprise during multi-window workflows.
- **Reliability**: Mode restoration should be deterministic and should not depend on transient focus history beyond the last mode stored for each live window.
- **Maintainability**: Window mode ownership should be represented in the window lifecycle rather than as a shared editor-wide concern.

## Acceptance Criteria

- [ ] **AC-001**: A newly created window opens in normal mode.
- [ ] **AC-002**: If one window is left in insert mode and the user switches to another window, the first window still returns to insert mode when focused again.
- [ ] **AC-003**: If the user changes mode in one window, other open windows retain their own current mode values.
- [ ] **AC-004**: Closing a window removes its stored mode state.
- [ ] **AC-005**: Recreating a window after it was closed starts it in normal mode.
- [ ] **AC-006**: Existing single-window workflows continue to behave as before.

## Out of Scope

- Changing which actions enter or exit specific modes
- Adding new modes
- Persisting mode state across application restarts
- Changing how buffers, cursor positions, or jumplists are stored

## Assumptions

- urvim already has a window lifecycle that can associate state with each live window.
- The editor already has a defined normal mode that can serve as the default mode for new windows.
- Mode-sensitive rendering and status display can read the active window’s current mode.

## Dependencies

- Existing window creation, focus switching, and teardown behavior
- Existing mode switching actions and mode-specific rendering
- Existing status bar or footer rendering that displays the current mode
