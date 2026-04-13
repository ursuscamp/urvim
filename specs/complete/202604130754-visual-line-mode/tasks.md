# Visual Line Mode - Implementation Tasks

## Overview

Total: 5 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Backend

- [x] **1.** Add linewise visual mode and wire it into mode switching
  - [x] **1.1** Add a dedicated linewise visual mode variant to [`src/editor/mode.rs`](/Users/ryan/Dev/urvim/src/editor/mode.rs) and update the displayed mode label
  - [x] **1.2** Create [`src/editor/visual_line.rs`](/Users/ryan/Dev/urvim/src/editor/visual_line.rs) with `V` entry, `Esc`/`V` exit, and motion/delete/change bindings for whole-line selection
  - [x] **1.3** Re-export `VisualLineMode` from [`src/editor/mod.rs`](/Users/ryan/Dev/urvim/src/editor/mod.rs) and wire it into the mode transition flow in [`src/main.rs`](/Users/ryan/Dev/urvim/src/main.rs)
  - [x] **1.4** Add editor tests for entering linewise visual mode, leaving it, and rejecting unsupported keys in [`src/editor/tests.rs`](/Users/ryan/Dev/urvim/src/editor/tests.rs)

- [x] **2.** Extend window-local selection state to support linewise ranges
  - [x] **2.1** Update [`src/window/view.rs`](/Users/ryan/Dev/urvim/src/window/view.rs) or the owning window-local view type to store the active selection kind alongside the anchor cursor
  - [x] **2.2** Update the visual selection helpers so they can seed, clear, and normalize both character-wise and linewise selections
  - [x] **2.3** Normalize linewise selections to whole-line boundaries before render or edit code consumes them
  - [x] **2.4** Keep cursor syncing behavior consistent when the buffer changes underneath an active linewise selection

- [x] **3.** Route linewise delete and change through the existing buffer line helpers
  - [x] **3.1** Teach [`src/window/commands.rs`](/Users/ryan/Dev/urvim/src/window/commands.rs) or the owning window action layer to delete selected lines with `Buffer::delete_lines`
  - [x] **3.2** Teach the same path to change selected lines with `Buffer::change_lines`, preserving the blank-line replacement behavior
  - [x] **3.3** Ensure linewise change returns the cursor to the start of the replaced range and switches to insert mode immediately
  - [x] **3.4** Preserve the existing character-wise visual delete/change behavior while adding the linewise branch

## Testing

- [x] **4.** Add regression coverage for linewise visual behavior
  - [x] **4.1** Add window tests in [`src/window/tests.rs`](/Users/ryan/Dev/urvim/src/window/tests.rs) for entering linewise visual mode, expanding with motions, deleting lines, changing lines, and exiting with `Esc`/`V`
  - [x] **4.2** Add tests that verify linewise change leaves a single blank line and positions the cursor at the replacement site
  - [x] **4.3** Add tests that confirm linewise delete removes lines entirely and leaves the cursor at the deleted range start
  - [x] **4.4** Add or update rendering coverage so the visual overlay still highlights whole-line ranges correctly

## Docs

- [x] **5.** Update user-facing docs and verify the editor still builds cleanly
  - [x] **5.1** Update [`docs/motions.md`](/Users/ryan/Dev/urvim/docs/motions.md) to document linewise visual mode and its supported keys
  - [x] **5.2** Update any mode or command reference that lists visual mode behavior so it mentions both character-wise and linewise visual mode
  - [x] **5.3** Run `cargo check` and the targeted editor/window test suites, then fix any regressions

## Completion Summary

| Item | Status | Notes |
| --- | --- | --- |
| 1. Mode wiring | Complete | Add `VisualLineMode` and route `V` |
| 2. Selection state | Complete | Store selection kind and normalize linewise ranges |
| 3. Linewise edit flow | Complete | Reuse `delete_lines` and `change_lines` |
| 4. Regression coverage | Complete | Add editor/window/rendering tests |
| 5. Docs and verification | Complete | Update motions docs and run `cargo check` |
