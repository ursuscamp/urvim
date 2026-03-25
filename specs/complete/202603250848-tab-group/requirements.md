# Tab Group

## Summary
urvim should support a tab-group container that can own multiple editor windows, draw a tab bar, and let the user switch between tabs with vim-style bracketed motions. When the editor is started with multiple file paths, each file should open in its own tab.

## Problem Statement
urvim currently starts with a single window and only opens the first file path from the command line. That makes it awkward to work with multiple files at once and leaves no dedicated UI affordance for switching between open documents.

## User Stories
- As a user, I want each file passed on the command line to open in its own tab, so that I can work with multiple files in one editor session.
- As a user, I want a visible tab bar, so that I can tell which files are open and which tab is active.
- As a user, I want to switch tabs with familiar vim-like keys, so that tab navigation feels consistent with the rest of the editor.
- As a user, I want the active tab to keep its own cursor and buffer state, so that switching tabs does not disturb my place in another file.

## Functional Requirements
- [ ] **REQ-001**: The editor must support a tab-group container that can hold more than one window.
- [ ] **REQ-002**: The tab-group container must render a tab bar separate from the active window content.
- [ ] **REQ-003**: The tab bar must indicate which tab is active.
- [ ] **REQ-004**: The tab bar must horizontally scroll only when the active tab would otherwise be offscreen.
- [ ] **REQ-005**: The tab bar must keep the active tab visible when tab selection changes.
- [ ] **REQ-006**: The tab bar must show a left arrow indicator when tabs exist to the left of the visible region.
- [ ] **REQ-007**: The tab bar must show a right arrow indicator when tabs exist to the right of the visible region.
- [ ] **REQ-008**: The editor must open each file path passed on the command line in its own tab.
- [ ] **REQ-009**: When multiple command-line file paths are provided, the first successfully opened file must be the active tab on startup.
- [ ] **REQ-010**: When no file paths are provided, the editor must start with a single empty tab.
- [ ] **REQ-011**: If a command-line file cannot be opened, the editor must report the failure and continue opening the remaining files.
- [ ] **REQ-012**: The editor must support switching to the previous tab with `[b`.
- [ ] **REQ-013**: The editor must support switching to the next tab with `]b`.
- [ ] **REQ-014**: Tab switching actions must support count prefixes so users can move multiple tabs at once.
- [ ] **REQ-015**: Tab switching must preserve the buffer contents and cursor position of each tab.
- [ ] **REQ-016**: The active tab must receive all editing actions and key-driven cursor movement while it is selected.
- [ ] **REQ-017**: The visible editing area must be reduced to account for the tab bar so content does not overlap the bar.
- [ ] **REQ-018**: The editor must display a useful tab label for each tab, such as the file name when available and an untitled label otherwise.

## Non-Functional Requirements
- [ ] **REQ-019**: Tab switching and tab bar rendering must remain responsive in the terminal UI.
- [ ] **REQ-020**: Existing single-buffer editing behavior must remain unchanged for the active tab.
- [ ] **REQ-021**: The tab-group implementation must be compatible with the existing modal editing model and action-processing flow.
- [ ] **REQ-022**: The feature must be covered by unit tests for startup loading, tab navigation, scrolling tab visibility, and rendering layout.

## Acceptance Criteria
- [ ] **AC-001**: Starting urvim with `file1.txt file2.txt` opens two tabs and shows `file1.txt` as active.
- [ ] **AC-002**: Pressing `[b` moves to the previous tab and pressing `]b` moves to the next tab.
- [ ] **AC-003**: Pressing `3]b` moves three tabs to the right, wrapping as needed.
- [ ] **AC-004**: The tab bar is visible and the active tab is distinguishable from inactive tabs.
- [ ] **AC-005**: When there are more tabs than fit on screen, the bar scrolls horizontally as the active tab changes.
- [ ] **AC-006**: Left and right arrow indicators appear when tabs exist outside the visible tab-bar region.
- [ ] **AC-007**: If a tab is already visible, moving to it must not shift the tab bar unnecessarily.
- [ ] **AC-008**: Each tab preserves its own cursor and content after switching away and back.
- [ ] **AC-009**: If one startup file fails to load, the editor still opens the remaining files.
- [ ] **AC-010**: With no startup files, urvim still launches into an editable empty tab.

## Out of Scope
- Tab closing, tab reordering, and tab renaming.
- Splitting a tab into multiple visible panes.
- Drag-and-drop tab movement.
- Persisting tab state across editor restarts.
- A dedicated tab management command palette or menu.
- Auto-scrolling the tab bar in response to hover or pointer interaction.

## Assumptions
- A tab group contains an ordered list of windows and exactly one active window at a time.
- The tab bar lives at the top of the terminal and consumes one row of screen space.
- The tab bar may render arrow indicators at its edges when tabs are hidden off-screen.
- The first implementation only needs previous/next tab navigation, not full tab management.
- Existing window editing semantics should continue to apply inside each tab without change.
- File-load failures should be non-fatal and should not prevent other startup files from opening.

## Dependencies
- Existing `Window` rendering and editing behavior.
- Existing `Buffer` file-loading support.
- Existing normal-mode keymap and action dispatch flow.
- Existing screen rendering and terminal cursor positioning.
