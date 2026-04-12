# Tab Group Owned Jumplist
## Summary
Move jumplist ownership from individual windows to the tab group so jump navigation can activate the correct tab for the target buffer. When a jumplist entry points at a buffer that is not currently open in the tab group, the editor must reopen that buffer and then restore the recorded cursor position.

## Problem Statement
urvim currently keeps jumplist state inside each `Window`, which means history is tied to whichever window happened to record it. That makes jump navigation awkward once the user moves across tabs because the jumplist does not naturally select the tab that owns the destination buffer. It also makes it impossible for jump navigation to reopen a buffer that is no longer represented by an open tab.

The editor needs the tab group to own jumplist state so the tab group can treat jump history as a navigation concern for the whole set of open tabs, not just one window instance.

## User Stories
- As a user switching between tabs, I want jumplist navigation to take me back to the correct tab, so that I do not have to hunt for the destination buffer manually.
- As a user jumping to a buffer that is not currently open in the tab group, I want the editor to reopen that buffer for me, so that jump history remains usable even after tabs have been closed.
- As a user moving through history, I want the recorded cursor position to be restored for the selected buffer, so that the tab switch lands me where I left off.

## Functional Requirements
- [ ] **REQ-001**: The tab group must own the active jumplist state for the editor session.
- [ ] **REQ-002**: Jumplist entries must continue to identify both the destination buffer and the cursor position within that buffer.
- [ ] **REQ-003**: Recording cursor history must update the tab group jumplist rather than per-window jumplist state.
- [ ] **REQ-004**: Jumping backward or forward must select the tab whose buffer matches the target jumplist entry when that buffer is already open in the tab group.
- [ ] **REQ-005**: If the target jumplist buffer is not currently open in the tab group, jump navigation must open that buffer in the tab group before restoring the cursor.
- [ ] **REQ-006**: If a jumplist destination buffer was previously represented by a tab and is now absent from the tab group, jump navigation must reopen it instead of failing just because no tab is active for it.
- [ ] **REQ-007**: Jump navigation must restore the recorded cursor position after the tab group has selected or opened the target buffer.
- [ ] **REQ-008**: Jump navigation must preserve the existing jumplist behaviors for deduplication, bounded history, and forward-history branching.
- [ ] **REQ-009**: The jumplist must remain session-local and must not persist across editor restarts.
- [ ] **REQ-010**: Ordinary tab switching must not clear or reset the tab group jumplist.
- [ ] **REQ-011**: If the recorded target buffer no longer exists in the live buffer pool, jumplist navigation must fail safely without corrupting the active tab group state.

## Non-Functional Requirements
- [ ] **NFR-001**: Jump navigation must remain deterministic for the same session history and tab-group state.
- [ ] **NFR-002**: Reopening a jumplist target must not duplicate an already-open buffer in the same tab group.
- [ ] **NFR-003**: Jumplist ownership changes must not alter buffer contents or unrelated editor state.
- [ ] **NFR-004**: Tab selection and buffer reopening during jumplist playback must remain responsive during normal editing use.

## Acceptance Criteria
- [ ] **AC-001**: After recording jump history across multiple tabs, pressing `Ctrl-O` or `Ctrl-I` activates the tab that owns the target buffer.
- [ ] **AC-002**: If the target buffer is already open in the tab group, jumplist navigation selects that existing tab instead of opening a duplicate.
- [ ] **AC-003**: If the target buffer is not open in the tab group, jumplist navigation opens it and restores the recorded cursor position.
- [ ] **AC-004**: If a buffer was previously open and later removed from the tab group, jumplist navigation reopens that buffer from history.
- [ ] **AC-005**: Moving between tabs does not discard or reset jumplist history.
- [ ] **AC-006**: Existing jumplist behavior for deduplication and forward-history branching still works after the ownership change.

## Out of Scope
- Persisting jumplist state across editor restarts.
- Exposing jumplist contents in the UI or through a new inspection command.
- Changing the user-facing jumplist key bindings.
- Allowing multiple independent jumplists per tab group.
- Introducing a tab closure UI or buffer deletion lifecycle.

## Assumptions
- A tab group may need to reopen a jumplist destination by reusing an already live buffer from the global buffer pool.
- The editor continues to keep buffers alive in the pool even when a tab is no longer open for them, so reopening can be done from the stored buffer identity.
- Existing jumplist semantics from the completed jumplist work remain the source of truth unless this spec explicitly changes them.
- The editor still uses a single active tab group at the top level, so jumplist ownership moves one level up without introducing multiple tab groups.

## Dependencies
- Existing window, buffer view, and tab group infrastructure.
- Existing jumplist recording and restore behavior.
- Normal-mode `Ctrl-O` and `Ctrl-I` action handling.
