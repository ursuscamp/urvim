# Pane Resizing - Implementation Tasks

## Overview
Implement a dedicated resizing mode for split panes, wire it into normal-mode key handling, and teach the layout tree to resize the focused pane relative to its adjacent split neighbors while clamping at minimum usable sizes.

## Backend
- [x] **1.** Add resizing mode to the editor mode system and key binding flow.
  - [x] **1.1** Extend `ModeKind` and window mode switching so the editor can enter and leave resizing mode.
  - [x] **1.2** Add a new `ResizingMode` implementation that maps `h/j/k/l` to resize actions and `Esc` back to normal mode.
  - [x] **1.3** Bind `<C-w>r` in normal mode to enter resizing mode without disturbing existing `Ctrl-w` bindings.
- [x] **2.** Add layout actions and split-tree support for pane resizing.
  - [x] **2.1** Introduce resize action kinds that express horizontal and vertical growth/shrink intent.
  - [x] **2.2** Implement layout handling that finds the focused pane’s nearest matching split ancestor and updates its split weights.
  - [x] **2.3** Clamp resize operations so repeated shrinking stops at the minimum usable pane size.
  - [x] **2.4** Keep resize handling local to the focused pane subtree so unrelated panes keep their contents and focus state.
- [x] **3.** Update any user-facing labels and mode plumbing affected by the new mode.
  - [x] **3.1** Ensure the status bar and window mode label report resizing mode clearly.
  - [x] **3.2** Preserve the existing action dispatch and mode transition flow for all other modes.

## Testing
- [x] **4.** Add editor-mode tests for resizing mode key handling.
  - [x] **4.1** Verify `Ctrl-w r` enters resizing mode from normal mode.
  - [x] **4.2** Verify `h/j/k/l` return the expected resize actions while resizing mode is active.
  - [x] **4.3** Verify `Esc` exits resizing mode and unrelated keys are ignored.
- [x] **5.** Add layout tests for resizing behavior.
  - [x] **5.1** Verify horizontal and vertical resize actions update split proportions as expected.
  - [x] **5.2** Verify resize clamping prevents panes from shrinking below the minimum usable size.
  - [x] **5.3** Verify resizing works in nested split trees and does not disturb unrelated panes.
  - [x] **5.4** Verify layout redraw continues to derive pane regions from the updated split weights.

## Completion Summary
| Category | Total | Completed | Status |
| --- | ---: | ---: | --- |
| Backend | 3 | 3 | Complete |
| Testing | 2 | 2 | Complete |
| Overall | 5 | 5 | Complete |
