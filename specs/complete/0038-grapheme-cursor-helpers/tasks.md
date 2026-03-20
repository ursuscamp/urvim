# Grapheme-Based Cursor Navigation Helpers - Implementation Tasks

## Overview

Total: 10 tasks
Estimated completion: 1 day
Prerequisites: None

## Implementation

- [x] **1.** Add `next_cursor_line` and `prev_cursor_line` methods to Buffer (test: cargo check, run existing cursor tests)
  - [x] **1.1** Implement `next_cursor_line` using substring iteration `line[cursor.col..]` (test: unit test with ASCII, emoji, CJK)
  - [x] **1.2** Implement `prev_cursor_line` using `.rev().next()` pattern (test: unit test with ASCII, emoji, CJK)
  - [x] **1.3** Add doc comments and doctests (test: cargo doc)
  - [x] **1.4** Verify existing cursor_right/cursor_left tests still pass with new logic (test: cargo test cursor_right cursor_left)

- [x] **2.** Rename `cursor_right` to `next_cursor` and `cursor_left` to `prev_cursor` (test: cargo check after each rename)
  - [x] **2.1** Rename `cursor_right` → `next_cursor` in buffer.rs (test: cargo check)
  - [x] **2.2** Rename `cursor_left` → `prev_cursor` in buffer.rs (test: cargo check)
  - [x] **2.3** Update doc examples that reference old method names (test: cargo test)

- [x] **3.** Update call sites in window.rs (test: cargo check, cargo test)
  - [x] **3.1** Update `move_cursor_left()` to call `prev_cursor` instead of `cursor_left` (test: cargo check)
  - [x] **3.2** Update `move_cursor_right()` to call `next_cursor` instead of `cursor_right` (test: cargo check)
  - [x] **3.3** Verify all other usages in window.rs work correctly (test: cargo test)

- [x] **4.** Update test calls in buffer.rs (test: cargo test)
  - [x] **4.1** Rename test functions from `test_cursor_right_*` to `test_next_cursor_*` (test: cargo test)
  - [x] **4.2** Rename test functions from `test_cursor_left_*` to `test_prev_cursor_*` (test: cargo test)
  - [x] **4.3** Update all `cursor_right(` calls in tests to `next_cursor(` (test: cargo test)
  - [x] **4.4** Update all `cursor_left(` calls in tests to `prev_cursor(` (test: cargo test)
  - [x] **4.5** Update other test calls that use cursor_right (insert tests around line 3375-3414) (test: cargo test)

- [x] **5.** Refactor `bracket_matcher.rs` to use grapheme iteration (test: cargo test, manual test with emoji)
  - [x] **5.1** Refactor `find_matching_forward` to use `line[search_start..].grapheme_indices(true)` (test: cargo test)
  - [x] **5.2** Refactor `find_matching_backward` to use `line[..search_end].grapheme_indices(true).rev()` (test: cargo test)
  - [x] **5.3** Fix `find_matching_bracket` to use grapheme iteration instead of `chars().nth(cursor.col)` (test: cargo test)
  - [x] **5.4** Verify existing bracket matching tests pass (test: cargo test bracket_matcher)

- [x] **6.** Find and fix any remaining incorrect cursor arithmetic (test: cargo test)
  - [x] **6.1** Review `window.rs` `insert_char` method for correct grapheme handling (test: manual test with emoji insertion)
  - [x] **6.2** Check for any other `col + 1` or `col - 1` patterns that should use new helpers (test: grep and review)

- [x] **7.** Run full test suite to verify no regressions (test: cargo test --lib)
  - [x] **7.1** Run buffer tests (test: cargo test buffer)
  - [x] **7.2** Run window tests (test: cargo test window)
  - [x] **7.3** Run bracket_matcher tests (test: cargo test bracket_matcher)
  - [x] **7.4** Run all tests (test: cargo test)

## Testing

- [x] **8.** Add unit tests for new grapheme cursor methods (test: cargo test)
  - [x] **8.1** Test `next_cursor_line` with emoji sequences (test: cargo test)
  - [x] **8.2** Test `prev_cursor_line` with emoji sequences (test: cargo test)
  - [x] **8.3** Test `next_cursor` line boundary crossing with emojis (test: cargo test)
  - [x] **8.4** Test `prev_cursor` line boundary crossing with emojis (test: cargo test)
  - [x] **8.5** Test CJK characters (test: cargo test)

- [x] **9.** Add bracket matching tests with multi-byte characters (test: cargo test)
  - [x] **9.1** Test bracket matching with emoji: `foo(👨‍👩‍👧‍👦bar)` (test: cargo test)
  - [x] **9.2** Test bracket matching with CJK: `函数(参数)` (test: cargo test)
  - [x] **9.3** Test nested brackets with multi-byte: `(a👨‍👩‍👧‍👦(b)c)` (test: cargo test)

- [x] **10.** Run clippy and verify no lints (test: cargo clippy)
  - [x] **10.1** Run cargo clippy on buffer.rs (test: cargo clippy --lib)
  - [x] **10.2** Run cargo clippy on window.rs (test: cargo clippy --lib)
  - [x] **10.3** Run cargo clippy on bracket_matcher.rs (test: cargo clippy --lib)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 7 | 7 | 100% |
| Testing | 3 | 3 | 100% |
| **Total** | **10** | **10** | **100%** |

## Files Modified

| File | Changes |
|------|---------|
| `src/buffer.rs` | Added `next_cursor_line`, `prev_cursor_line`. Renamed `cursor_right`→`next_cursor`, `cursor_left`→`prev_cursor`. Updated tests. |
| `src/motion/bracket_matcher.rs` | Refactored to use grapheme iteration. Fixed `chars().nth()` issue. Added UnicodeSegmentation import. |
| `src/window.rs` | Updated call sites from `cursor_left/cursor_right` to `prev_cursor/next_cursor`. |

## Test Results

- All 497 tests pass
- 11 bracket_matcher tests pass
- 181 buffer tests pass
- 44 window tests pass
