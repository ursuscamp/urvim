# Empty Inner Text Object Change - Implementation Tasks

## Overview

Total: 3 tasks
Estimated completion: 2-4 hours
Prerequisites: Approved bug report

## Implementation

- [x] **1.** Update characterwise change handling for empty text-object ranges
  - [x] **1.1** In [`src/window/commands.rs`](/Users/ryan/Dev/urvim/src/window/commands.rs), distinguish a resolved empty range from an unresolved text object
  - [x] **1.2** Keep `Operator::Change` successful for empty inner bracket and quote ranges so the app-level mode switch can enter insert mode
  - [x] **1.3** Preserve current delete no-op behavior and unmatched-target behavior

- [x] **2.** Add regression coverage for empty bracket and quote changes
  - [x] **2.1** Add a window test for `ci(` on `()` that verifies the operation is handled and the cursor lands at the inner insertion point
  - [x] **2.2** Add a window test for `ci"` on `""` that verifies the same insert-point behavior
  - [x] **2.3** Add or extend a buffer test if needed to lock in the zero-length inner range contract for empty pairs

- [x] **3.** Verify the fix
  - [x] **3.1** Run `cargo check` and targeted tests covering the new regression cases
  - [x] **3.2** Fix any clippy or test regressions uncovered by the change

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 2 | 2 | 100% |
| Testing | 1 | 1 | 100% |
| **Total** | **3** | **3** | **100%** |
