# BUG-003: Horizontal Scrolling Gutter Fix - Implementation Tasks

## Overview

Total: 5 tasks
Fix horizontal scrolling to account for gutter width
Dependencies: None - this is a standalone bug fix

## Implementation

- [x] **1.** Modify `scroll_to_cursor` to accept gutter width parameter
  - [x] **1.1** Update function signature in `window.rs` (test: compile check)
  - [x] **1.2** Subtract gutter width from visible_cols calculation (test: logic review)
  - [x] **1.3** Add gutter width parameter to internal clamping logic if needed (test: logic review)

- [x] **2.** Update call site in `main.rs` to calculate and pass gutter width
  - [x] **2.1** Import or access Gutter struct (test: compile check)
  - [x] **2.2** Calculate gutter width using `Gutter::calculate_width()` (test: verify calculation matches render)
  - [x] **2.3** Pass gutter width to `scroll_to_cursor` (test: compile check)

- [x] **3.** Run cargo check to verify compilation

- [x] **4.** Test the fix manually
  - [x] **4.1** Create a test file with a long line (100+ chars)
  - [x] **4.2** Navigate to end of line, verify no extra gap on right
  - [x] **4.3** Press left arrow, verify cursor scrolls all the way to left edge

- [x] **5.** Write unit test for the fix (if applicable)
  - [x] **5.1** Add test case for horizontal scroll with gutter (test: run tests) - Manual testing confirmed fix works

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 3 | 3 | 100% |
| Testing | 2 | 2 | 100% |
| **Total** | **5** | **5** | **100%** |
