# Change Operator - Implementation Tasks

## Overview

Total: 6 tasks
Estimated completion: 1 day
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Extend the operator model for `Change`
  - [x] **1.1** Add `Operator::Change` in [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) and keep the type public (test: compile)
  - [x] **1.2** Update `Action::is_snapshottable()` so `Operation(Change, _)` snapshots like delete (test: unit test)
  - [x] **1.3** Update `Action::switches_to_insert_mode()` so `Operation(Change, _)` and counted change actions switch modes on success (test: unit test)
  - [x] **1.4** Confirm `Action::is_countable()` still treats `Operation(_, _)` as countable (test: unit test)

- [x] **2.** Register `c`-prefixed operator sequences in normal mode
  - [x] **2.1** Add `cw`, `ce`, `cb`, `cW`, `cE`, and `cB` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.2** Add `ciw` and `caw` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.3** Add `c$`, `c0`, `c^`, `cgg`, and `cG` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.4** Verify `c` still waits for more input when it is only a prefix (test: unit test)

- [x] **3.** Route `Operator::Change` through the existing window execution path
  - [x] **3.1** Update [`src/window/commands.rs`](/Users/ryan/Dev/urvim/src/window/commands.rs) to treat `Operator::Change` like delete for range resolution and deletion (depends on: 1.1) (test: unit test)
  - [x] **3.2** Return `ActionResult::NotHandled` for empty or impossible change ranges so the main loop does not enter insert mode (depends on: 3.1) (test: unit test)
  - [x] **3.3** Ensure successful change operations leave the cursor at the start of the changed region (depends on: 3.1) (test: unit test)

- [x] **4.** Preserve linewise change behavior
  - [x] **4.1** Reuse the existing linewise operator-target helpers for `c$`, `c0`, `c^`, `cgg`, and `cG` (depends on: 3.1) (test: unit test)
  - [x] **4.2** Verify linewise change commands remain compatible with existing `cc` and `C` actions (test: unit test)

- [x] **5.** Update user-facing docs
  - [x] **5.1** Update [`docs/motions.md`](/Users/ryan/Dev/urvim/docs/motions.md) to document the new `c` operator forms and their relation to the existing delete behavior (depends on: 2.1, 2.2, 2.3)

## Testing

- [x] **6.** Add and run coverage for parsing, execution, and mode switching
  - [x] **6.1** Add editor key-sequence tests in [`src/editor/tests.rs`](/Users/ryan/Dev/urvim/src/editor/tests.rs) for `cw`, `ciw`, `caw`, linewise `c` commands, and `c` prefix waiting
  - [x] **6.2** Add window-level tests in [`src/window/tests.rs`](/Users/ryan/Dev/urvim/src/window/tests.rs) covering successful change commands, no-op edge cases, and cursor placement
  - [x] **6.3** Add undo-focused tests proving a successful change records a single logical edit and can be undone cleanly
  - [x] **6.4** Run `cargo check` and the targeted test set; fix any regressions before marking complete

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 5 | 5 | 100% |
| Testing | 1 | 1 | 100% |
| **Total** | **6** | **6** | **100%** |
