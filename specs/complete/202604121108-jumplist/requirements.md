# Vim-Like Jumplist
## Summary
Add a per-window, session-only jumplist that records meaningful cursor locations and supports backward and forward navigation with Vim-style `Ctrl-O` and `Ctrl-I`.

## Problem Statement
Users need a fast way to return to recently visited locations after making large cursor moves, following searches, or jumping between places in the editor. urvim currently lacks a dedicated jump history, which makes it harder to move around a file or between files and then return to the previous context.

## User Stories
- As a user navigating a file, I want the editor to remember meaningful cursor locations, so that I can quickly return to where I was working.
- As a user moving through search results or other large jumps, I want to go backward and forward through my recent positions, so that I can retrace my navigation path.
- As a user jumping to a location I already visited, I want that location to be promoted in history instead of duplicated, so that the jumplist stays useful and compact.

## Functional Requirements
- [ ] **REQ-001**: The editor must maintain a jumplist for each window.
- [ ] **REQ-002**: The jumplist must exist only for the current editor session and must not persist across restarts.
- [ ] **REQ-003**: The jumplist must store cursor locations as window-relevant entries that identify both the buffer and the cursor position.
- [ ] **REQ-004**: If the jumplist head refers to the active file or buffer, ordinary cursor movement within that same file or buffer must refresh that head entry instead of creating a new entry.
- [ ] **REQ-005**: The meaningful-distance threshold must be based on combined line and column distance between the previous location and the new location.
- [ ] **REQ-006**: If a cursor movement within the active file or buffer crosses the meaningful-distance threshold, the editor must create a new jumplist entry for that file or buffer.
- [ ] **REQ-007**: The editor must provide backward jumplist navigation with `Ctrl-O`.
- [ ] **REQ-008**: The editor must provide forward jumplist navigation with `Ctrl-I`.
- [ ] **REQ-009**: Jumping backward and then making a new qualifying jump must discard the forward portion of the jumplist.
- [ ] **REQ-010**: If a qualifying jumplist location already exists in the jumplist, the editor must remove the older occurrence and move that location to the front instead of storing a duplicate.
- [ ] **REQ-011**: The jumplist must have a fixed maximum size and must drop the oldest entries when that size is exceeded.
- [ ] **REQ-012**: Navigating the jumplist must restore the recorded buffer and cursor position for the selected entry.
- [ ] **REQ-013**: Jumping into a different file or buffer must begin tracking that destination in the jumplist head for subsequent cursor movement updates.
- [ ] **REQ-014**: If the user moves backward in the jumplist and then makes a cursor move within the distance threshold, the editor must update the current jumplist entry in place without discarding forward entries.
- [ ] **REQ-015**: Before storing a jumplist cursor position, the editor must sync the cursor to a valid grapheme boundary so the stored position cannot point at an invalid byte offset.
- [ ] **REQ-016**: Before restoring any stored cursor position, including jumplist playback and window restoration, the editor must sync the cursor to a valid grapheme boundary if the target buffer has changed since the position was recorded.

## Non-Functional Requirements
- [ ] **NFR-001**: Jumplist navigation must be deterministic for the same session state and sequence of navigation actions.
- [ ] **NFR-002**: Jumplist bookkeeping must not alter buffer contents or modify unrelated editor state.
- [ ] **NFR-003**: Jumplist behavior must remain predictable when the active window switches between buffers.
- [ ] **NFR-004**: Jumplist operations must remain responsive during normal editing use.

## Acceptance Criteria
- [ ] **AC-001**: After a qualifying jump, `Ctrl-O` returns to the previous recorded location.
- [ ] **AC-002**: After moving backward in the jumplist, `Ctrl-I` moves forward again until the newest entry is reached.
- [ ] **AC-003**: A qualifying jump made after moving backward removes any forward history and starts a new branch from the current position.
- [ ] **AC-004**: Re-visiting an already recorded location moves that location to the front of the jumplist rather than adding a duplicate entry.
- [ ] **AC-005**: When the jumplist reaches its maximum size, the oldest entry is discarded.
- [ ] **AC-006**: Closing and reopening the editor does not restore previous jumplist state.
- [ ] **AC-007**: Jumplist navigation restores the buffer and cursor position associated with the selected entry.
- [ ] **AC-008**: After moving backward in the jumplist, a small cursor move updates the current entry without removing forward history.
- [ ] **AC-009**: Jumplist entries are stored and restored only after cursor positions have been normalized to valid grapheme boundaries.
- [ ] **AC-010**: Any editor action that restores a stored cursor position leaves the cursor on a valid grapheme boundary after restoration.

## Out of Scope
- Persisting jumplist state across restarts.
- Exposing jumplist contents in the UI or through an inspection command.
- A configurable jumplist size or user-facing jumplist settings.
- Visual indicators for the current jumplist position.

## Assumptions
- A meaningful jump is determined by a fixed internal threshold based on combined line and column distance.
- `Ctrl-O` and `Ctrl-I` are reserved for backward and forward jumplist navigation in normal mode.
- Jumplist entries should be considered unique by buffer and cursor position together.
- The exact jumplist size limit will be chosen during implementation and documented in tests if needed.
- Cursor sync should preserve the nearest valid position rather than rejecting storage or restoration.
- Cursor sync should apply to other cursor-restoring flows beyond the jumplist when they reuse stored positions.

## Dependencies
- Normal-mode key handling for `Ctrl-O` and `Ctrl-I`.
- Window state that can store and restore recent cursor locations.
- Cursor movement tracking that can detect qualifying jumps across buffers and within a buffer.
