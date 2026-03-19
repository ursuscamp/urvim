# cc Motion - Implementation Tasks

## Overview

Total: 9 tasks
Estimated completion: 1-2 hours
Prerequisites: requirements.md and design.md completed

## Implementation Tasks

- [x] **1.** Add Action::ChangeLine variant to Action enum
  - [x] **1.1** Add `ChangeLine` variant to Action enum in src/editor.rs (test: cargo check passes)
  - [x] **1.2** Add ChangeLine to `is_countable()` → returns true (test: verify is_countable returns true)
  - [x] **1.3** Add ChangeLine to `resets_remembered_column()` → returns true (test: verify method returns true)
  - [x] **1.4** Add ChangeLine to `switches_to_insert_mode()` → returns true (test: verify method returns true)

- [x] **2.** Register "cc" sequence in keymap
  - [x] **2.1** Ensure "c" is set as prefix key (test: pressing c then another key waits for sequence)
  - [x] **2.2** Add "cc" → Action::ChangeLine mapping in normal mode keymap (test: cargo check passes)

- [x] **3.** Implement Buffer::change_lines method
  - [x] **3.1** Create `change_lines(start_line, count)` method in src/buffer.rs (test: unit test for single/multiple line change)
  - [x] **3.2** Handle edge cases: empty buffer, count exceeds lines (test: edge case tests pass)

- [x] **4.** Add Window action handler for ChangeLine
  - [x] **4.1** Add match arm for Action::ChangeLine in Window::process_action (test: cargo check passes)
  - [x] **4.2** Add match arm for Count(ChangeLine) in process_action (test: cargo check passes)
  - [x] **4.3** Implement cursor repositioning after change (test: manual testing)

- [x] **5.** Add Window helper method
  - [x] **5.1** Create `change_line(count)` method in src/window.rs (test: cargo check passes)

- [x] **6.** Add unit tests for Buffer::change_lines
  - [x] **6.1** Test single line change (test: verify line becomes empty, cursor at start)
  - [x] **6.2** Test multiple line change (test: verify N lines replaced with 1 blank)
  - [x] **6.3** Test edge case: change from last line (test: blank line on previous)
  - [x] **6.4** Test edge case: change only line (test: buffer has 1 empty line)

- [x] **7.** Add unit tests for Action::ChangeLine
  - [x] **7.1** Test is_countable returns true (test: test passes)
  - [x] **7.2** Test resets_remembered_column returns true (test: test passes)
  - [x] **7.3** Test switches_to_insert_mode returns true (test: test passes)

- [x] **8.** Update documentation
  - [x] **8.1** Add cc motion to docs/motions.md (test: documentation file updated)

- [x] **9.** Integration testing
  - [x] **9.1** Test "cc" in normal mode (test: verify line becomes empty, enter insert mode)
  - [x] **9.2** Test "2cc" (test: verify 2 lines replaced with 1 blank)
  - [x] **9.3** Test "3cc" with insufficient lines (test: replaces remaining lines with 1 blank)
  - [x] **9.4** Test cursor position after cc on last line (test: cursor on blank previous line)
  - [x] **9.5** Test typing in insert mode after cc (test: characters appear at start of line)
  - [x] **9.6** Run cargo test to verify all tests pass (test: cargo test passes)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Action Definition | 4 | 4 | 100% |
| Keymap | 2 | 2 | 100% |
| Buffer Method | 2 | 2 | 100% |
| Window Handler | 3 | 3 | 100% |
| Window Helper | 1 | 1 | 100% |
| Unit Tests | 4 | 4 | 100% |
| Action Tests | 3 | 3 | 100% |
| Documentation | 1 | 1 | 100% |
| Integration Tests | 6 | 6 | 100% |
| **Total** | **26** | **26** | **100%** |
