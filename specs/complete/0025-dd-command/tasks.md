# dd Command - Implementation Tasks

## Overview

Total: 7 tasks
Estimated completion: 1-2 hours
Prerequisites: requirements.md and design.md completed

## Implementation Tasks

- [x] **1.** Add Action::DeleteLine variant to Action enum
  - [x] **1.1** Add `DeleteLine` variant to Action enum in src/editor.rs (test: cargo check passes)
  - [x] **1.2** Add DeleteLine to `is_countable()` → returns true (test: verify is_countable returns true)
  - [x] **1.3** Add DeleteLine to `resets_remembered_column()` → returns true (test: verify method returns true)

- [x] **2.** Register "dd" sequence in keymap
  - [x] **2.1** Ensure "d" is set as prefix key (if not already) (test: pressing d then another key waits for sequence)
  - [x] **2.2** Add "dd" → Action::DeleteLine mapping in normal mode keymap (test: cargo check passes)

- [x] **3.** Implement Buffer::delete_lines method
  - [x] **3.1** Create `delete_lines(start_line, count)` method in src/buffer.rs (test: unit test for single/multiple line deletion)
  - [x] **3.2** Handle edge cases: empty buffer, count exceeds lines (test: edge case tests pass)

- [x] **4.** Add Window action handler for DeleteLine
  - [x] **4.1** Add match arm for Action::DeleteLine in Window::process_action (test: cargo check passes)
  - [x] **4.2** Add match arm for Count(DeleteLine) in process_action (test: cargo check passes)
  - [x] **4.3** Implement cursor repositioning after deletion (test: manual testing)

- [x] **5.** Add unit tests for Buffer::delete_lines
  - [x] **5.1** Test single line deletion (test: verify line removed, cursor position)
  - [x] **5.2** Test multiple line deletion (test: verify correct lines removed)
  - [x] **5.3** Test edge case: delete from last line (test: cursor moves to previous)
  - [x] **5.4** Test edge case: delete only line (test: buffer has 1 empty line)

- [x] **6.** Add unit tests for Action::DeleteLine
  - [x] **6.1** Test is_countable returns true (test: test passes)
  - [x] **6.2** Test resets_remembered_column returns true (test: test passes)

- [x] **7.** Integration testing
  - [x] **7.1** Test "dd" in normal mode (test: verify line deleted)
  - [x] **7.2** Test "2dd" (test: verify 2 lines deleted)
  - [x] **7.3** Test "3dd" with insufficient lines (test: deletes remaining lines)
  - [x] **7.4** Test cursor position after dd on last line (test: cursor on previous line)
  - [x] **7.5** Run cargo test to verify all tests pass (test: cargo test passes)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Action Definition | 3 | 3 | 100% |
| Keymap | 2 | 2 | 100% |
| Buffer Method | 2 | 2 | 100% |
| Window Handler | 3 | 3 | 100% |
| Unit Tests | 4 | 4 | 100% |
| Integration Tests | 2 | 2 | 100% |
| **Total** | **16** | **16** | **100%** |
