# Delete Motion Text Objects - Implementation Tasks

## Overview

Total: 8 tasks
Estimated completion: 1 day
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Generalize operator targets in the editor action model
  - [x] **1.1** Add a public `OperatorTarget` enum for operator-pending targets in [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) (test: compile)
  - [x] **1.2** Add a public `BoundaryMotion` enum covering `w`, `e`, `b`, `W`, `E`, and `B` in [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) (test: compile)
  - [x] **1.3** Update `Action::Operation` to store `OperatorTarget` instead of `TextObject` and preserve count/snapshot behavior (test: unit test)
  - [x] **1.4** Re-export any new public types from [`src/editor/mod.rs`](/Users/ryan/Dev/urvim/src/editor/mod.rs) (test: compile)

- [x] **2.** Register delete-motion operator sequences in normal mode
  - [x] **2.1** Update existing `diw` and `daw` bindings to use `OperatorTarget::TextObject` in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (depends on: 1.3)
  - [x] **2.2** Add `dw`, `de`, and `db` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.3** Add `dW`, `dE`, and `dB` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.4** Verify `d` remains a prefix that waits for more input while preserving existing invalid-sequence behavior (test: unit test)

- [x] **3.** Add focused buffer helpers for operator-target range resolution
  - [x] **3.1** Introduce a focused operator-target helper module if needed to keep text-object logic separated by concern in [`src/buffer/mod.rs`](/Users/ryan/Dev/urvim/src/buffer/mod.rs) and a new helper file (test: compile)
  - [x] **3.2** Add public `Buffer` APIs to resolve operator-target ranges with and without counts (test: unit test)
  - [x] **3.3** Reuse the existing `iw` and `aw` helpers through the generalized operator-target API (test: unit test)

- [x] **4.** Implement forward delete-target range resolution
  - [x] **4.1** Resolve `dw` and `dW` as `[cursor, next boundary)` using existing `Boundary::Word` and `Boundary::BigWord` traversal (test: unit test)
  - [x] **4.2** Resolve `de` and `dE` as delete-through-end by converting the resolved end boundary into an exclusive end cursor (test: unit test)
  - [x] **4.3** Support multiplied counts for forward targets by resolving the final range once from the total count (test: `d2w`, `2de`)

- [x] **5.** Implement backward delete-target range resolution
  - [x] **5.1** Resolve `db` and `dB` as `[previous boundary, cursor)` using existing backward boundary traversal (test: unit test)
  - [x] **5.2** Return no range when no backward movement is possible at the buffer edge (test: unit test)
  - [x] **5.3** Support multiplied counts for backward targets by resolving the final range once from the total count (test: `3d2B`)

- [x] **6.** Route window command execution through the generalized operator-target API
  - [x] **6.1** Update uncounted `Action::Operation` handling in [`src/window/commands.rs`](/Users/ryan/Dev/urvim/src/window/commands.rs) to resolve ranges via the new buffer API (depends on: 3.2)
  - [x] **6.2** Update counted operation handling in [`src/window/commands.rs`](/Users/ryan/Dev/urvim/src/window/commands.rs) to use the same generalized path (depends on: 3.2)
  - [x] **6.3** Preserve one-snapshot-per-operation undo behavior and cursor placement at deleted range start (test: unit test)

- [x] **7.** Document the new delete commands
  - [x] **7.1** Update [`docs/motions.md`](/Users/ryan/Dev/urvim/docs/motions.md) to list and describe `dw`, `de`, `db`, `dW`, `dE`, and `dB` (depends on: 4.1, 4.2, 5.1)
  - [x] **7.2** Call out that these commands follow urvim's existing word and BigWord motion semantics where they differ from Vim (test: doc review)

## Testing

- [x] **8.** Add and run coverage for parsing, range resolution, and execution
  - [x] **8.1** Add editor key-sequence tests in [`src/editor/tests.rs`](/Users/ryan/Dev/urvim/src/editor/tests.rs) for `dw`, `de`, `db`, `dW`, `dE`, `dB`, counts, and `d` prefix waiting
  - [x] **8.2** Add buffer tests in [`src/buffer/tests.rs`](/Users/ryan/Dev/urvim/src/buffer/tests.rs) for forward, backward, BigWord, count, and edge no-op cases
  - [x] **8.3** Add window-level execution tests covering cursor placement and undo behavior for at least one forward and one backward delete target
  - [x] **8.4** Run `cargo check` and targeted tests; fix any regressions before marking complete

---

## Completion Summary

| Phase          | Tasks | Completed | Progress |
| -------------- | ----- | --------- | -------- |
| Implementation | 7     | 7         | 100%     |
| Testing        | 1     | 1         | 100%     |
| **Total**      | **8** | **8**     | **100%** |
