# Paging Keys - Implementation Tasks

## Overview

Implement paging motions for `PageUp`, `PageDown`, `Ctrl-U`, and `Ctrl-D` in both normal mode and insert mode. The work should extend the existing action/keymap/window pipeline, preserve remembered column behavior, and include regression coverage for key handling and cursor movement.

## Backend

- [x] **1.** Extend `ActionKind` with dedicated paging variants and update the action classification helpers so the new motions behave like vertical cursor moves, not insert/edit actions.
  - [x] **1.1** Add action variants for full-page up/down and half-page up/down.
  - [x] **1.2** Update remembered-column, snapshot-cursor, and countability helpers to match the desired paging semantics. (depends on: 1.1)
  - [x] **1.3** Ensure the new actions do not become dot-repeat sources or edit snapshots. (depends on: 1.1)

- [x] **2.** Bind the paging keys in normal mode and insert mode.
  - [x] **2.1** Add `<PageUp>`, `<PageDown>`, `<C-u>`, and `<C-d>` to `NormalMode::new()`. (depends on: 1.1)
  - [x] **2.2** Add the same four bindings to `InsertMode::new()` without changing insert-mode exit behavior. (depends on: 1.1)
  - [x] **2.3** Confirm the bindings do not conflict with existing multi-key sequences or canonical key parsing. (depends on: 2.1, 2.2)

- [x] **3.** Implement cursor movement helpers for full-page and half-page paging in the window layer.
  - [x] **3.1** Add window helper methods that move up and down by a computed viewport delta. (depends on: 1.1)
  - [x] **3.2** Use the current viewport height for full-page motions and half of that height for half-page motions. (depends on: 3.1)
  - [x] **3.3** Clamp movement cleanly at the start and end of the buffer. (depends on: 3.1)
  - [x] **3.4** Route the new action variants through `Window::process_action()`. (depends on: 3.1, 3.2)

## Testing

- [x] **4.** Add regression tests for mode key handling and action classification.
  - [x] **4.1** Verify normal mode maps `PageUp`, `PageDown`, `Ctrl-U`, and `Ctrl-D` to the correct actions. (depends on: 2.1)
  - [x] **4.2** Verify insert mode maps the same keys to the same actions without leaving insert mode. (depends on: 2.2)
  - [x] **4.3** Verify the action helper methods classify paging motions consistently with vertical cursor motions. (depends on: 1.2)

- [x] **5.** Add window-level regression tests for paging movement.
  - [x] **5.1** Verify `PageUp` and `PageDown` move by one viewport height. (depends on: 3.4)
  - [x] **5.2** Verify `Ctrl-U` and `Ctrl-D` move by half a viewport height. (depends on: 3.4)
  - [x] **5.3** Verify paging clamps at buffer boundaries and preserves the visual column when possible. (depends on: 3.4)

## Completion Summary

| Section | Total | Done | Remaining |
|---------|-------|------|-----------|
| Backend | 3 | 3 | 0 |
| Testing | 2 | 2 | 0 |
| Total | 5 | 5 | 0 |
