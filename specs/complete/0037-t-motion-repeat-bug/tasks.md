# T Motion Repeat Bug - Implementation Tasks

## Overview

- Total: 5 tasks
- Estimated completion: 30 minutes
- Dependencies: None

## Implementation

- [x] **1.** Modify `move_cursor_till_backward` to calculate search_start_col
  - [x] **1.1** Add logic similar to `move_cursor_till_forward` to advance past current grapheme (test: verify cursor moves to next 'h' on repeated T)
  - [x] **1.2** Use `search_start_col` in `find_char_backward` call (test: verify correct character found)
  - [x] **1.3** Ensure landing position is after found character (test: verify TillBackward lands after character)

- [x] **2.** Add test for repeated TillBackward motion
  - [x] **2.1** Test `T` repeated on "hhello" finds subsequent 'h's (test: run unit test)
  - [x] **2.2** Test `T` with no previous occurrence stays in place (test: run unit test)

- [x] **3.** Run cargo check and tests
  - [x] **3.1** Run `cargo check` to verify no compilation errors (test: cargo passes)
  - [x] **3.2** Run relevant unit tests (test: tests pass)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 3 | 0 | 0% |
| Testing | 2 | 0 | 0% |
| **Total** | **5** | **0** | **0%** |
