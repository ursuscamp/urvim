# Visual Mode - Implementation Tasks

## Overview

Total: 5 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Extend the editor model and key handling for visual mode
  - [x] **1.1** Add `Visual` to `ModeKind` and update its display label in [`src/editor/mode.rs`](/Users/ryan/Dev/urvim/src/editor/mode.rs)
  - [x] **1.2** Create a new [`src/editor/visual.rs`](/Users/ryan/Dev/urvim/src/editor/visual.rs) mode implementation that starts from `v`, exits on `Esc` and `v`, and exposes motion/delete/change bindings
  - [x] **1.3** Re-export `VisualMode` from [`src/editor/mod.rs`](/Users/ryan/Dev/urvim/src/editor/mod.rs) and wire it into [`src/main.rs`](/Users/ryan/Dev/urvim/src/main.rs)
  - [x] **1.4** Add editor tests for mode transitions, supported bindings, and unsupported-key behavior in [`src/editor/tests.rs`](/Users/ryan/Dev/urvim/src/editor/tests.rs)

- [x] **2.** Add window-local visual selection state and selection-aware edit helpers
  - [x] **2.1** Store a visual selection anchor and live cursor in [`src/window/view.rs`](/Users/ryan/Dev/urvim/src/window/view.rs) or the most appropriate window-local view type
  - [x] **2.2** Add helpers on [`src/window/mod.rs`](/Users/ryan/Dev/urvim/src/window/mod.rs) and/or [`src/window/motions.rs`](/Users/ryan/Dev/urvim/src/window/motions.rs) to start, update, clear, and normalize the active visual selection
  - [x] **2.3** Reuse the existing buffer range-edit APIs in [`src/buffer/edit.rs`](/Users/ryan/Dev/urvim/src/buffer/edit.rs) so visual delete and change leave the cursor at the start of the selected range
  - [x] **2.4** Keep visual-selection cursor syncing consistent with existing cursor normalization behavior

- [x] **3.** Render the active selection in the editor viewport
  - [x] **3.1** Add a dedicated selection style to the UI theme model in [`src/theme/model.rs`](/Users/ryan/Dev/urvim/src/theme/model.rs) and wire it through any theme-loading code that constructs `UiStyles`
  - [x] **3.2** Add a visual selection render path to [`src/window/view.rs`](/Users/ryan/Dev/urvim/src/window/view.rs) that overlays the active range without changing syntax styling
  - [x] **3.3** Ensure the rendered selection works across single-line and multi-line ranges and respects the existing gutter/layout calculations
  - [x] **3.4** Keep the normal and insert cursors unchanged outside visual mode

- [x] **4.** Hook visual mode into command execution and mode switching
  - [x] **4.1** Teach [`src/main.rs`](/Users/ryan/Dev/urvim/src/main.rs) to transition into and out of visual mode using the existing mode loop
  - [x] **4.2** Route visual delete and change back through the window action pipeline so undo, redo, and cursor updates remain consistent
  - [x] **4.3** Make sure visual change switches immediately to insert mode after the selected range is removed

- [x] **5.** Add regression coverage and run verification
  - [x] **5.1** Add window tests for entering visual mode, moving the selection, deleting, changing, and exiting with `Esc`/`v` in [`src/window/tests.rs`](/Users/ryan/Dev/urvim/src/window/tests.rs)
  - [x] **5.2** Add rendering tests for the active selection highlight and cursor placement in visual mode
  - [x] **5.3** Update user-facing docs if needed, especially any mode or command reference that now lists visual mode behavior
  - [x] **5.4** Run `cargo check` and the targeted editor/window test suites, then fix any regressions

## Completion Summary

| Item | Status | Notes |
| --- | --- | --- |
| 1. Editor model and key handling | Complete | Add visual mode and its keymap |
| 2. Selection state and edit helpers | Complete | Anchor/cursor tracking plus range edits |
| 3. Selection rendering | Complete | Highlight active range in the viewport |
| 4. Mode switching and execution flow | Complete | Wire visual mode into the main loop |
| 5. Regression coverage | Complete | Tests, docs, and verification |
