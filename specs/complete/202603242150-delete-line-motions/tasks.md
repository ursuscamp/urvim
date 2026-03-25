# Delete Line Motions - Implementation Tasks

## Overview

Total: 8 tasks
Estimated completion: 1 day
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Extend the operator-target model for line-boundary delete motions
  - [x] **1.1** Extend public `BoundaryMotion` in [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) with `LineStart`, `LineContentStart`, and `LineEnd`, and add a public `LinewiseMotion` enum with documentation comments for each public type and variant (test: `cargo check`)
  - [x] **1.2** Extend public `OperatorTarget` in [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) to include `LinewiseMotion` while preserving current boundary-motion delete behavior (test: unit test)
  - [x] **1.3** Re-export any new public types from [`src/editor/mod.rs`](/Users/ryan/Dev/urvim/src/editor/mod.rs) (test: `cargo check`)

- [x] **2.** Register the new delete sequences in normal mode
  - [x] **2.1** Add `d$`, `d0`, and `d^` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) using `BoundaryMotion::LineEnd`, `BoundaryMotion::LineStart`, and `BoundaryMotion::LineContentStart` (test: unit test)
  - [x] **2.2** Add `dgg` and `dG` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) using `LinewiseMotion::FirstLine` and `LinewiseMotion::LastLine` while preserving prefix waiting for `d` and `dg` (test: unit test)
  - [x] **2.3** Update count parsing in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) or nearby parser helpers so `d0` resolves as a motion, not as the start of a sub-count (test: unit test)

- [x] **3.** Teach the buffer layer to resolve same-line delete targets
  - [x] **3.1** Add focused helpers in [`src/buffer/operator_target.rs`](/Users/ryan/Dev/urvim/src/buffer/operator_target.rs) for the new `BoundaryMotion` line-anchor variants used by `d$`, `d0`, and `d^` (test: unit test)
  - [x] **3.2** Keep empty-range cases as no-ops without deleting text or creating malformed ranges (test: unit test)

- [x] **4.** Add linewise target resolution for `dgg` and `dG`
  - [x] **4.1** Introduce or adapt a buffer-level operator range representation that can express whole-line deletes cleanly (test: compile)
  - [x] **4.2** Resolve `dgg` to an inclusive line span from the current line to either line 1 or the counted destination line (test: unit test)
  - [x] **4.3** Resolve `dG` to an inclusive line span from the current line to either the last line or the counted destination line (test: unit test)
  - [x] **4.4** Clamp counted destinations to the buffer range and preserve linewise semantics when the target is above or below the current line (test: unit test)

- [x] **5.** Route execution through the generalized delete-target path
  - [x] **5.1** Update [`src/window/commands.rs`](/Users/ryan/Dev/urvim/src/window/commands.rs) so `Action::Operation` can execute both characterwise and linewise delete targets through one snapshot flow (depends on: 4.1)
  - [x] **5.2** Ensure counted `dgg` and `dG` treat the count as a destination line number instead of multiplying a repeat count during execution (test: unit test)
  - [x] **5.3** Preserve cursor placement at the start of the surviving text after both same-line and linewise deletes (test: unit test)

- [x] **6.** Document the new motion/operator combinations
  - [x] **6.1** Update [`docs/motions.md`](/Users/ryan/Dev/urvim/docs/motions.md) to describe `d$`, `d0`, `d^`, `dgg`, and `dG`
  - [x] **6.2** Call out that counted `dgg` and `dG` use counts as destination line numbers and remain linewise

## Testing

- [x] **7.** Add parsing and target-resolution coverage
  - [x] **7.1** Add editor key-sequence tests in [`src/editor/tests.rs`](/Users/ryan/Dev/urvim/src/editor/tests.rs) for `d$`, `d0`, `d^`, `dgg`, `dG`, `d5gg`, `d5G`, and the `d`/`dg` prefix wait states
  - [x] **7.2** Add buffer tests in [`src/buffer/tests.rs`](/Users/ryan/Dev/urvim/src/buffer/tests.rs) for same-line ranges, empty-range no-ops, linewise spans, and clamped counted destinations

- [x] **8.** Add execution coverage and verify the build
  - [x] **8.1** Add window-level tests in [`src/window/tests.rs`](/Users/ryan/Dev/urvim/src/window/tests.rs) covering cursor placement, undo snapshots, and linewise deletion outcomes for `dgg` and `dG`
  - [x] **8.2** Run `cargo check` and targeted tests; fix regressions before marking the spec complete

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 6 | 6 | 100% |
| Testing | 2 | 2 | 100% |
| **Total** | **8** | **8** | **100%** |
