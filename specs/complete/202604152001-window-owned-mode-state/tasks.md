# Window-Owned Mode State - Implementation Tasks

## Overview

Total: 5 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Backend

- [x] **1.** Move live mode ownership into `Window` and make it the source of truth for mode transitions
  - [x] **1.1** Add a mode field to [`src/window/mod.rs`](/Users/ryan/Dev/urvim/src/window/mod.rs) and initialize new and recreated windows in normal mode
  - [x] **1.2** Add window-level helpers for reading the current mode kind, label, cursor style, and switching modes
  - [x] **1.3** Update [`src/window/widget.rs`](/Users/ryan/Dev/urvim/src/window/widget.rs) and related window action paths so key interpretation uses the window-owned mode instead of a main-loop-owned mode
  - [x] **1.4** Preserve existing mode-entry and mode-exit side effects, including visual selection initialization/clearing and insert-mode repeat capture (depends on: 1.2)

- [x] **2.** Thread active-window mode state through `TabGroup`, `Layout`, and the main loop
  - [x] **2.1** Update [`src/tab_group.rs`](/Users/ryan/Dev/urvim/src/tab_group.rs) to expose the active window’s mode kind, mode label, and cursor style
  - [x] **2.2** Remove the layout-level mode cache in [`src/layout.rs`](/Users/ryan/Dev/urvim/src/layout.rs) and make the footer read the active window’s mode label directly
  - [x] **2.3** Refactor [`src/main.rs`](/Users/ryan/Dev/urvim/src/main.rs) so the event loop no longer owns a single `Box<dyn Mode>` and instead queries/switches the active window’s mode
  - [x] **2.4** Keep tab switching behavior stable so each window restores its own stored mode when it becomes active again

- [x] **3.** Remove the old global mode dependency from render-time and editor-state code paths
  - [x] **3.1** Replace `globals::with_mode_kind` lookups in [`src/window/mod.rs`](/Users/ryan/Dev/urvim/src/window/mod.rs) with the focused window’s own mode kind
  - [x] **3.2** Remove or simplify the global mode storage in [`src/globals.rs`](/Users/ryan/Dev/urvim/src/globals.rs) if it is no longer needed after the refactor
  - [x] **3.3** Update any remaining mode-sensitive callers so they read from the active window instead of the process-global slot

## Testing

- [x] **4.** Add regression coverage for per-window mode ownership and restoration
  - [x] **4.1** Add window tests in [`src/window/tests.rs`](/Users/ryan/Dev/urvim/src/window/tests.rs) that verify new windows start in normal mode and that window-local mode switches do not leak across windows
  - [x] **4.2** Add tab-group or layout tests that verify switching away from a window and back restores its previous mode
  - [x] **4.3** Add integration-style coverage for the main event loop path if needed to confirm mode transitions still dispatch correctly
  - [x] **4.4** Update any existing tests that currently assume a single global mode kind

## Verification

- [x] **5.** Clean up public documentation comments and verify the workspace builds cleanly
  - [x] **5.1** Add or update doc comments for any new public mode accessors or window mode helpers
  - [x] **5.2** Run `cargo fmt` after the refactor to keep the touched files consistent with project style
  - [x] **5.3** Run `cargo check` and the relevant test suites, then fix any regressions or clippy warnings surfaced by the change

## Completion Summary

| Item | Status | Notes |
| --- | --- | --- |
| 1. Window-owned mode | Done | Move live mode state into each window |
| 2. Tab/layout/main wiring | Done | Route mode access through the active window |
| 3. Global mode cleanup | Done | Remove the shared mode dependency where possible |
| 4. Regression coverage | Done | Confirm per-window restoration behavior |
| 5. Verification and docs | Done | Update comments, format, and run checks |
