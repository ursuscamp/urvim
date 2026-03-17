# Column Preservation for Vertical Movement - Implementation Tasks

## Overview

Total: 7 tasks
Feature: Column preservation when moving vertically
Dependencies: None (self-contained feature)

## Implementation

- [x] **1.** Add `remembered_visual_col` field to `BufferView` struct
  - [x] **1.1** Add `remembered_visual_col: Option<usize>` field to BufferView (test: cargo check passes)
  - [x] **1.2** Initialize field to `None` in `BufferView::new()` (test: new() creates None)

- [x] **2.** Add helper methods to `BufferView`
  - [x] **2.1** Add `get_or_compute_target_col(&self) -> usize` method (test: returns remembered or current)
  - [x] **2.2** Add `update_remembered_to_current(&mut self)` method (test: sets to current visual col)
  - [x] **2.3** Add `set_remembered_visual_col(&mut self, col: usize)` method (test: sets to specific value)

- [x] **3.** Add methods to `Action` enum
  - [x] **3.1** Add `resets_remembered_column(&self) -> bool` method (test: correct for MoveLeft, MoveRight, etc.)
  - [x] **3.2** Add `uses_remembered_column(&self) -> bool` method (test: correct for MoveUp, MoveDown)

- [x] **4.** Modify `move_cursor_up()` and `move_cursor_down()` in `BufferView` to take target_col parameter
  - [x] **4.1** Update `move_cursor_up(target_col: usize)` signature and implementation (test: moves up with target col)
  - [x] **4.2** Update `move_cursor_down(target_col: usize)` signature and implementation (test: moves down with target col)

- [x] **5.** Modify `Window::process_action()` for centralized column logic
  - [x] **5.1** Update vertical movement handling to get target col, move, then remember (test: vertical moves preserve)
  - [x] **5.2** Add reset logic after horizontal movements: call `update_remembered_to_current()` (test: horizontal resets)
  - [x] **5.3** Pass target_col to move_cursor_up() and move_cursor_down() calls (test: compiles and works)

- [x] **6.** Write unit tests for column preservation
  - [x] **6.1** Test first vertical move uses current column and remembers it (test: test passes)
  - [x] **6.2** Test consecutive vertical moves preserve column (test: test passes)
  - [x] **6.3** Test horizontal movement resets remembered column (test: test passes)
  - [x] **6.4** Test column clamping on shorter lines (test: test passes)

- [x] **7.** Verify with cargo check and tests
  - [x] **7.1** Run `cargo check` to verify compilation (test: no warnings or errors)
  - [x] **7.2** Run existing tests to ensure no regression (test: all pass)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 5 | 5 | 100% |
| Testing | 2 | 2 | 100% |
| **Total** | **7** | **7** | **100%** |
