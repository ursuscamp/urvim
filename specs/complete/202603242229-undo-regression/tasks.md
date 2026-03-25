# Undo Regression - Implementation Tasks

## Overview

Total: 6 tasks
Estimated completion: 1 day
Prerequisites: `bug-report.md` is complete

## Backend

- [x] **1.** Rework snapshot timing for undoable edits
  - [x] **1.1** Update the main event loop so snapshots are recorded at the correct point for text-modifying actions
  - [x] **1.2** Ensure undo/redo actions themselves do not create new snapshots
  - [x] **1.3** Preserve cursor updates for movement actions after the snapshot timing change

- [x] **2.** Align operator delete handling with the undo history model
  - [x] **2.1** Review the `Action::Operation(Operator::Delete, ...)` path used by `dw`, `db`, `d$`, and related deletes
  - [x] **2.2** Remove or adjust any duplicate snapshot creation in operator delete handlers so the history reflects the post-edit state
  - [x] **2.3** Verify that counted delete operations still produce one undo step

- [x] **3.** Verify direct delete commands still interact correctly with undo history
  - [x] **3.1** Confirm `x` and other direct delete commands continue to restore the previous buffer state with `u`
  - [x] **3.2** Confirm redo still restores the deleted text after an undo

## Testing

- [x] **4.** Add regression coverage for `x` undo behavior
  - [x] **4.1** Test that deleting a character with `x` can be undone with `u`
  - [x] **4.2** Test that redo after that undo restores the deleted character

- [x] **5.** Add regression coverage for `dw` undo behavior
  - [x] **5.1** Test that deleting a word with `dw` can be undone with `u`
  - [x] **5.2** Test that redo after that undo restores the deleted word
  - [x] **5.3** Test a counted word delete such as `2dw`

- [x] **6.** Run validation
  - [x] **6.1** Run `cargo test` for the relevant buffer, window, and editor paths
  - [x] **6.2** Run `cargo check` and fix any warnings introduced by the change

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Backend | 3 | 3 | 100% |
| Testing | 3 | 3 | 100% |
| **Total** | **6** | **6** | **100%** |
