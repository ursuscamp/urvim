# Nested Splits - Implementation Tasks

## Overview

Implement nested binary splits in the root layout so each pane hosts a `TabGroup`, expose Vim-style `Ctrl-w` split management keys, and collapse panes automatically when their tab groups become empty. The first implementation should store integer child weights on each split node, render from those weights, and leave interactive resizing for a later stage.

## Backend

- [x] **1.** Extend the action and keymap surface for split management
  - [x] **1.1** Add layout-level action variants for vertical split, horizontal split, directional pane focus, and pane close
  - [x] **1.2** Bind `Ctrl-w v`, `Ctrl-w s`, `Ctrl-w h`, `Ctrl-w j`, `Ctrl-w k`, `Ctrl-w l`, and `Ctrl-w q` in normal mode
  - [x] **1.3** Classify the new actions as layout navigation or management actions that are not dot-repeatable edits and do not disturb unrelated editor state
  - [x] **1.4** Add editor-level tests that verify the `Ctrl-w` sequences parse to the correct actions

- [x] **2.** Replace the single-tab-group layout root with a binary split tree
  - [x] **2.1** Introduce `LayoutNode`, `PaneNode`, `SplitNode`, `SplitAxis`, and `SplitSize` in [`src/layout.rs`](/Users/ryan/Dev/urvim/src/layout.rs)
  - [x] **2.2** Store stable pane identity and focused-pane tracking in `Layout`
  - [x] **2.3** Initialize the root layout as a single pane containing the startup `TabGroup`
  - [x] **2.4** Add public doc comments for the new public layout types and methods

- [x] **3.** Implement split creation, focus movement, and collapse behavior in `Layout`
  - [x] **3.1** Split the focused pane into a new binary node with an initial `1:1` `first_weight` and `second_weight`
  - [x] **3.2** Route directional pane focus using computed pane geometry so movement works across nested split levels
  - [x] **3.3** Remove panes cleanly on explicit close and collapse parent split nodes to their surviving child
  - [x] **3.4** Signal the application to exit when removing the final pane leaves the layout empty

- [x] **4.** Rewire layout rendering and active-pane access around the split tree
  - [x] **4.1** Render binary split children from stored integer weights for both horizontal and vertical splits
  - [x] **4.2** Preserve deterministic remainder handling so the full assigned region is consumed without gaps or overlap
  - [x] **4.3** Replace single-tab-group accessors with focused-pane accessors used by the main loop and status bar
  - [x] **4.4** Keep the footer status bar driven by the focused pane’s active window metadata

- [x] **5.** Update `TabGroup` and the main loop for pane lifecycle
  - [x] **5.1** Allow a `TabGroup` to report that it became empty instead of silently recreating an empty tab
  - [x] **5.2** Teach [`src/main.rs`](/Users/ryan/Dev/urvim/src/main.rs) to dispatch layout actions through `Layout` while preserving undo, redo, save, snapshot, and mode-transition behavior for the focused pane
  - [x] **5.3** Ensure last-window closure removes the hosting pane and exits the editor when no panes remain
  - [x] **5.4** Keep non-layout editing behavior unchanged when the editor stays in a single-pane state

## Testing

- [x] **6.** Add regression coverage for split-tree behavior
  - [x] **6.1** Add layout tests for splitting the root pane vertically and horizontally
  - [x] **6.2** Add layout tests for nested mixed-axis splits and stable focused-pane tracking
  - [x] **6.3** Add layout tests that verify new splits start with even `first_weight` and `second_weight`
  - [x] **6.4** Add layout tests for pane collapse after explicit close and after tab-group empty-state removal

- [x] **7.** Add navigation and rendering tests for nested panes
  - [x] **7.1** Verify `Ctrl-w h/j/k/l` moves focus to the expected pane across nested layouts
  - [x] **7.2** Verify split rendering divides rows or columns according to stored weights
  - [x] **7.3** Verify uneven terminal sizes assign remainder cells deterministically
  - [x] **7.4** Verify cursor placement and status-bar metadata continue to reflect the focused pane

- [x] **8.** Add lifecycle coverage for pane-hosted tab groups
  - [x] **8.1** Verify unrelated panes preserve their buffer, cursor, and mode state when another pane is split or closed
  - [x] **8.2** Verify a pane closes when its last window disappears
  - [x] **8.3** Verify the editor exits when the final pane’s final window closes
  - [x] **8.4** Update existing tests that assume the layout owns exactly one tab group

## Verification

- [x] **9.** Verify the nested split implementation and clean up touched surfaces
  - [x] **9.1** Run `cargo fmt` after the refactor
  - [x] **9.2** Run `cargo check` and fix any build or warning regressions
  - [x] **9.3** Run the focused layout, tab-group, and editor test suites that cover split actions, rendering, and pane lifecycle
  - [x] **9.4** Update [docs/motions.md](/Users/ryan/Dev/urvim/docs/motions.md) with the new Vim-style window split and pane-navigation bindings

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Backend | 5 | 5 | 100% |
| Testing | 3 | 3 | 100% |
| Verification | 1 | 1 | 100% |
| **Total** | **9** | **9** | **100%** |
