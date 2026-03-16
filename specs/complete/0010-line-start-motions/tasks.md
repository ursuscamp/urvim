# Line Start Motions - Implementation Tasks

## Overview

Total: 9 tasks
Add `0` key for absolute line start and `^` key for first non-whitespace navigation.

## Implementation

- [x] **1.** Add Action variants to editor.rs
  - [x] **1.1** Add `MoveToLineStart` variant to Action enum (test: cargo check)
  - [x] **1.2** Add `MoveToLineContentStart` variant to Action enum (test: cargo check)
  - [x] **1.3** Verify it compiles (test: cargo check)

- [x] **2.** Add key handling in NormalMode
  - [x] **2.1** Add case for `KeyCode::Char('0')` → Action::MoveToLineStart (test: cargo check)
  - [x] **2.2** Add case for `KeyCode::Char('^')` → Action::MoveToLineContentStart (test: cargo check)
  - [x] **2.3** Verify it compiles (test: cargo check)

- [x] **3.** Add `cursor_start_of_line` method in Buffer
  - [x] **3.1** Implement method in src/buffer.rs (test: cargo check)
  - [x] **3.2** Handle edge cases: cursor at column 0, empty buffer (test: cargo check)

- [x] **4.** Add `cursor_content_start_of_line` method in Buffer
  - [x] **4.1** Implement method in src/buffer.rs (test: cargo check)
  - [x] **4.2** Handle edge cases: at first non-whitespace, first line, no leading whitespace (test: cargo check)

- [x] **5.** Add Window movement methods
  - [x] **5.1** Add `move_cursor_to_line_start` method in window.rs (test: cargo check)
  - [x] **5.2** Add `move_cursor_to_line_content_start` method in window.rs (test: cargo check)

- [x] **6.** Add Action handlers in Window::process_action
  - [x] **6.1** Handle Action::MoveToLineStart (test: cargo check)
  - [x] **6.2** Handle Action::MoveToLineContentStart (test: cargo check)

- [x] **7.** Add unit tests for Buffer methods
  - [x] **7.1** Test cursor_start_of_line (test: cargo test)
  - [x] **7.2** Test cursor_content_start_of_line (test: cargo test)
  - [x] **7.3** Test edge cases (test: cargo test)

- [x] **8.** Add unit tests for NormalMode key handling
  - [x] **8.1** Test `0` key produces MoveToLineStart action (test: cargo test)
  - [x] **8.2** Test `^` key produces MoveToLineContentStart action (test: cargo test)

- [x] **9.** Run full test suite
  - [x] **9.1** Run cargo test to verify no regressions (test: cargo test)
  - [x] **9.2** Run cargo check for warnings (test: cargo check)

- [ ] **10.** Update `0` key behavior to wrap to previous line (like `^`)
  - [x] **10.1** Update cursor_start_of_line to wrap backwards (test: cargo test)
  - [x] **10.2** Update tests (test: cargo test)
  - [x] **10.3** Run full test suite (test: cargo test)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 6 | 6 | 100% |
| Testing | 3 | 3 | 100% |
| Bug fix | 3 | 3 | 100% |
| **Total** | **12** | **12** | **100%** |
