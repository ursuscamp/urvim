# Save Indicators and Save-on-Command
## Summary
Add manual saving with `<C-s>`, show modified-buffer indicators in the tab bar and status bar, and stop refreshing filetype classification on every edit. Filetype classification should be refreshed when a file is loaded or successfully saved, and unnamed buffers should not be saved for now.

## Problem Statement
urvim currently has editing and rendering flows, but it does not present a clear modified-state indicator or a dedicated save action. The buffer also recomputes filetype during many edit operations, which couples filetype state to unrelated text edits and makes filetype changes happen more often than necessary. The editor needs a save-centered workflow that makes unsaved changes visible, saves predictable, and keeps filetype detection tied to save/load events instead of every mutation.

## User Stories
- As a user, I want to save the active buffer with `<C-s>`, so that I can persist my edits without leaving the editor.
- As a user, I want to see which buffers have unsaved changes, so that I know what still needs to be written to disk.
- As a user, I want the active buffer's modified state to be reflected in the status bar, so that I can confirm whether the current file is saved.
- As a user, I want the tab bar to highlight modified tabs, so that I can spot unsaved work even when the buffer is not active.
- As a user, I want filetype detection to happen on save instead of after every edit, so that filetype classification follows the file's saved contents.
- As a user, I want unnamed buffers to remain unsaved for now, so that urvim does not invent file names or prompt for one yet.

## Functional Requirements
- [ ] **REQ-001**: The editor shall track whether each buffer's current contents differ from the last successful save or load state.
- [ ] **REQ-002**: Any buffer mutation that changes text content shall mark the buffer as modified until the buffer is saved or returns to the saved state through undo/redo.
- [ ] **REQ-003**: The editor shall bind `<C-s>` to a save action that can target a specific buffer or default to the active buffer when no buffer is supplied.
- [ ] **REQ-004**: A save action on a buffer with a known filename shall write the current contents to that filename and report success without changing the current editor mode.
- [ ] **REQ-005**: A save action on a buffer without a filename shall not create a file, shall not invent a filename, and shall leave the buffer otherwise unchanged.
- [ ] **REQ-006**: When a save succeeds, the buffer shall no longer be reported as modified until the next content change.
- [ ] **REQ-007**: The tab bar shall display a visible modified indicator for each modified tab with a distinct, theme-provided style from the tab label text.
- [ ] **REQ-008**: The status bar shall display a visible modified indicator for the active buffer alongside the existing buffer metadata with a distinct, theme-provided style from the surrounding text.
- [ ] **REQ-009**: Filetype classification shall be refreshed only when a buffer is loaded, assigned a path, or successfully saved.
- [ ] **REQ-010**: Editing operations shall not recompute filetype classification on every mutation.
- [ ] **REQ-011**: A successful save shall recompute filetype from the buffer's filename and current first line before the modified indicator is cleared.
- [ ] **REQ-012**: Failed save attempts shall preserve the buffer's contents, modified state, and previously resolved filetype.
- [ ] **REQ-013**: The editor shall expose a themed style for modified markers, and the built-in themes shall define a suitable value for that style.

## Non-Functional Requirements
- **Usability**: Modified indicators shall be compact and easy to recognize in both the tab bar and status bar.
- **Reliability**: Save success and save failure shall be distinguishable so the editor never clears the modified state after a failed write.
- **Compatibility**: Existing window, tab, and status rendering behavior shall remain intact apart from the added save and modified-state cues.
- **Performance**: Removing edit-time filetype refreshes shall avoid unnecessary recomputation during ordinary typing and cursor movement.
- **Theming**: The modified marker style shall come from the active theme so custom and built-in themes can render it consistently.

## Acceptance Criteria
- [ ] **AC-001**: Pressing `<C-s>` in a named buffer writes the file and clears the modified marker in both the tab bar and status bar.
- [ ] **AC-002**: Pressing `<C-s>` in an unnamed buffer does not create a file on disk and does not assign a new filename.
- [ ] **AC-003**: Editing a buffer causes the tab bar and status bar to show the modified marker until the buffer is saved or restored to the saved contents.
- [ ] **AC-004**: Saving a buffer after changing its shebang updates the displayed filetype only at save time, not during the edit itself.
- [ ] **AC-005**: Failed saves leave the modified marker visible and do not change the active filetype label.
- [ ] **AC-006**: Existing mode, buffer name, cursor position, and progress information still render in the status bar after the modified marker is added.

## Out of Scope
- Save As, file pickers, or filename prompts
- Autosave or background persistence
- Recovery dialogs after failed saves
- Per-buffer configuration for modified marker symbols
- Syntax highlighting or filetype-specific editor behavior beyond display and classification refresh timing

## Assumptions
- urvim already runs the terminal in raw mode, so `<C-s>` can be delivered to the editor as an input event.
- A concise ASCII marker such as `*` is acceptable for the modified indicator.
- The editor should prefer explicit no-op behavior over guessing a filename for unnamed buffers.
- Undo and redo should continue to work as today, with modified-state tracking reflecting the saved baseline rather than the raw edit history.

## Dependencies
- Existing buffer persistence and buffer pool save paths
- Existing status bar and tab bar rendering paths
- Existing key handling for normal and insert mode
- Existing filetype detection helpers and buffer filename access
