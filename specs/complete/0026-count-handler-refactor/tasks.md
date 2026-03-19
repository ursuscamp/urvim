# Count Action Handler Refactoring - Implementation Tasks

## Overview

Total: 10 tasks
This refactoring extracts the Count handler's conditional chain into separate private methods on Window.

## Implementation

- [x] **1.** Create main `handle_count` dispatcher method
  - [x] **1.1** Extract current conditional chain to dispatcher (test: cargo check passes)
  - [x] **1.2** Replace match arm in `process_action` to call `handle_count` (test: existing tests pass)

- [x] **2.** Extract line motion handler (gg, G)
  - [x] **2.1** Create `handle_count_line_motion` method (test: 5gg, 5G work)
  - [x] **2.2** Handle empty buffer case (test: edge case)
  - [x] **2.3** Handle out-of-bounds count (test: 100G in 10-line file)

- [x] **3.** Extract screen motion handler (H, L)
  - [x] **3.1** Create `handle_count_screen_motion` method (test: 3H, 3L work)
  - [x] **3.2** Handle viewport calculations correctly (test: verify cursor position)

- [x] **4.** Extract line action handler (0, $, ^, A, I)
  - [x] **4.1** Create `handle_count_line_action` method (test: 3$, 3^, 3A, 3I work)
  - [x] **4.2** Verify recursive call to inner action (test: action executes on target line)

- [x] **5.** Extract join handler (J, gJ)
  - [x] **5.1** Create `handle_count_join` method (test: 2J joins 3 lines)

- [x] **6.** Extract delete line handler (dd)
  - [x] **6.1** Create `handle_count_delete_line` method (test: 3dd deletes 3 lines)

- [x] **7.** Extract change line handler (cc)
  - [x] **7.1** Create `handle_count_change_line` method (test: 3cc changes 3 lines)

- [x] **8.** Extract open line below handler (o)
  - [x] **8.1** Create `handle_count_open_line_below` method (test: 3o creates 3 lines)

- [x] **9.** Extract open line above handler (O)
  - [x] **9.1** Create `handle_count_open_line_above` method (test: 3O creates 3 lines above)

- [x] **10.** Extract repeatable handler (default case)
  - [x] **10.1** Create `handle_count_repeatable` method (test: 5j, 10k work)

## Verification

- [x] **11.** Run existing test suite
  - [x] **11.1** Run `cargo test` (test: all tests pass)
  - [x] **11.2** Verify acceptance criteria manually:
    - [x] **11.2.1** `5j`, `10k` work (test: repeatable motions)
    - [x] **11.2.2** `5G`, `5gg` work (test: line motions)
    - [x] **11.2.3** `3H`, `3L` work (test: screen-relative)
    - [x] **11.2.4** `3$`, `3^` work (test: line actions)
    - [x] **11.2.5** `3J`, `3gJ` work (test: join motions)
    - [x] **11.2.6** `3dd` works (test: delete lines)
    - [x] **11.2.7** `3cc` works (test: change lines)
    - [x] **11.2.8** `3o` works (test: open lines below)
    - [x] **11.2.9** `3O` works (test: open lines above)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 10 | 10 | 100% |
| Verification | 1 | 1 | 100% |
| **Total** | **11** | **11** | **100%** |
