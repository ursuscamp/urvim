# Fuzzy File Finder

## Summary
Implement a reusable fuzzy picker overlay with a search bar and results window. The first concrete picker is a file picker that searches files under the current working directory and streams matches back asynchronously as the user types.

## Problem Statement
The editor currently lacks a fast, keyboard-driven way to search and open files from within the UI. Without a reusable picker abstraction, future search-based workflows would each need their own custom UI and state handling, which would make them harder to extend and maintain.

## User Stories
- As a user, I want to open a file picker with a single key, so that I can quickly find files without leaving the editor.
- As a user, I want search results to update as I type, so that I can narrow the list interactively.
- As a user, I want the picker to stay responsive while results stream in, so that large directories do not block the UI.
- As a user, I want the picker to be reusable for other result types later, so that the same UI can power additional search workflows.

## Functional Requirements
- [ ] **REQ-001**: The editor must open the picker overlay with `F1`.
- [ ] **REQ-002**: The picker overlay must render a search input area at the top and a results area directly below it.
- [ ] **REQ-003**: The picker must support a generic result type so that future picker implementations can reuse the same UI and interaction model.
- [ ] **REQ-004**: The picker must support a pluggable selection action that depends on the concrete picker type.
- [ ] **REQ-005**: The file picker must search only files, not directories.
- [ ] **REQ-006**: The file picker must search starting from the current working directory.
- [ ] **REQ-007**: The file picker search must match case-insensitively.
- [ ] **REQ-008**: The file picker must begin a background search when the query changes.
- [ ] **REQ-009**: The background search must walk the filesystem with a gitignore-aware recursive iterator and stream picker events back asynchronously.
- [ ] **REQ-010**: The picker event stream must include `PickerSearchStarted`, `PickerChunk`, `PickerSearchStale`, and `PickerSearchComplete` events.
- [ ] **REQ-011**: `PickerChunk` events must carry chunked result data.
- [ ] **REQ-012**: When the query changes, the picker must discard stale results and restart the search for the new query.
- [ ] **REQ-013**: The picker must remain responsive while search results are being produced asynchronously.
- [ ] **REQ-012**: `Esc` and `Ctrl-C` must close the picker without selecting a result.
- [ ] **REQ-013**: `Ctrl-N` and `Ctrl-P` must move the highlighted result down and up, respectively.
- [ ] **REQ-014**: `Enter` and `Ctrl-Y` must select the currently highlighted result.
- [ ] **REQ-015**: When the selected result comes from the file picker, the editor must open the file in a new tab or switch to the existing tab if the file is already open.

## Non-Functional Requirements
- **Performance**: Typing in the picker must not block on a full directory scan before showing the first matching results.
- **Reliability**: Stale background results from older queries must not replace results for the current query.
- **Compatibility**: File discovery must respect repository ignore rules through gitignore-aware traversal.
- **Usability**: The picker must remain keyboard-driven and dismissible without mouse interaction.

## Acceptance Criteria
- [ ] **AC-001**: Pressing `F1` opens a picker with a visible search bar and results region.
- [ ] **AC-002**: Typing in the picker starts an asynchronous file search rooted at the current working directory.
- [ ] **AC-003**: Search progress is reported through the `PickerSearchStarted`, `PickerChunk`, `PickerSearchStale`, and `PickerSearchComplete` event flow.
- [ ] **AC-004**: `PickerChunk` events carry chunked result data.
- [ ] **AC-005**: Updating the query clears previous results and shows only results for the latest query.
- [ ] **AC-006**: File matching is case-insensitive and excludes directories.
- [ ] **AC-007**: Pressing `Esc` or `Ctrl-C` closes the picker without changing the active buffer or tab.
- [ ] **AC-008**: Pressing `Ctrl-N` moves the highlight to the next result and `Ctrl-P` moves it to the previous result.
- [ ] **AC-009**: Pressing `Enter` or `Ctrl-Y` activates the highlighted result.
- [ ] **AC-010**: Selecting a file opens it in a new tab or focuses the existing tab if that file is already open.

## Out of Scope
- Searching non-file result types in the initial implementation.
- Multi-select behavior.
- Persisting picker history or recent searches.
- Custom fuzzy ranking rules beyond the first file picker implementation.
- Searching outside the current working directory.

## Assumptions
- The editor already has a widget or overlay system that can host the picker UI.
- The current working directory is available to the editor at picker open time.
- File selection can reuse the existing open-or-focus tab behavior.
- `walkdir` traversal is sufficient for the first implementation's filesystem discovery needs.

## Dependencies
- **Internal**: Widget rendering, keyboard input handling, tab management, and file-opening logic.
- **External**: The `ignore` crate.
- **Blocked by**: None identified.
