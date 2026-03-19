# Character Scan Motion Grapheme Bugs - Implementation Tasks

## Overview

Total: 6 tasks
Estimated completion: 1-2 hours
Dependencies: None (self-contained bug fixes)

## Implementation Tasks

- [x] **1.** Update `find_char_forward` to use grapheme indices
  - [x] **1.1** Change `char_indices()` to `grapheme_indices(true)` in `src/buffer.rs:2340` (test: cargo test test_find_forward_moves_to_char)
  - [x] **1.2** Verify byte offset is correctly captured from grapheme_indices (test: cargo test)

- [x] **2.** Update `find_char_backward` to use grapheme indices
  - [x] **2.1** Change `char_indices()` to `grapheme_indices(true)` in `src/buffer.rs:2370` (test: cargo test test_find_backward_moves_to_char)
  - [x] **2.2** Handle case when cursor is on target character - search with `idx < cursor.col` instead of `idx <= start_col` (test: "helllo" cursor on 3rd 'l' -> "Fl" -> 2nd 'l')

- [x] **3.** Fix `move_cursor_till_forward` grapheme start position
  - [x] **3.1** Calculate correct start position using grapheme width at new cursor position (test: cargo test test_till_forward_lands_before_char)
  - [x] **3.2** Iterate graphemes to find proper search start position

- [x] **4.** Add test cases for double-letter scenarios
  - [x] **4.1** Test `F` motion on "helllo" finds previous 'l' (test: cargo test test_find_backward_skips_current_char_on_duplicate)
  - [x] **4.2** Test `t` motion repeated on "hello" finds subsequent 'l's (test: cargo test test_till_forward_repeated_finds_next_occurrence)

- [x] **5.** Run existing tests to ensure no regression
  - [x] **5.1** Run all character scan motion tests (test: cargo test char_scan)
  - [x] **5.2** Run all buffer tests (test: cargo test buffer)
  - [x] **5.3** Run all window tests (test: cargo test window)

- [x] **6.** Run full test suite and fix any issues
  - [x] **6.1** Run `cargo test` (test: all tests pass)
  - [x] **6.2** Run `cargo clippy` (test: no relevant warnings)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 3 | 3 | 100% |
| Testing | 3 | 3 | 100% |
| **Total** | **6** | **6** | **100%** |

## Summary of Changes

### Bug Fixes

1. **F motion with double letters**: Fixed `find_char_backward` to skip the current character when the cursor is on the target character. Changed from `idx < start_col` with `start_col = cursor.col - 1` to `idx < cursor.col` which properly excludes the character at the cursor position.

2. **t motion repeated**: Fixed `move_cursor_till_forward` to calculate the correct search start position by iterating through graphemes to find the proper starting point. This ensures repeated `t` motions find the next occurrence.

3. **Grapheme indices**: Both `find_char_forward` and `find_char_backward` now use `grapheme_indices(true)` instead of `char_indices()` to properly handle Unicode grapheme clusters.

### Files Changed

- `src/buffer.rs`: Updated `find_char_forward` and `find_char_backward` to use grapheme indices
- `src/window.rs`: Updated `move_cursor_till_forward` to calculate correct search start position, added test cases
