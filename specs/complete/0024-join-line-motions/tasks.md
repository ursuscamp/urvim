# Join Line Motions (J and gJ) - Implementation Tasks

## Overview

Total: 10 tasks
Estimated completion: 1-2 hours
Prerequisites: Understanding of existing motion implementation patterns

## Implementation Tasks

### editor.rs Changes

- [x] **1.** Add JoinWithSpace and JoinWithoutSpace Action variants
  - [x] **1.1** Add `JoinWithSpace` to Action enum (test: cargo check compiles)
  - [x] **1.2** Add `JoinWithoutSpace` to Action enum (test: cargo check compiles)
- [x] **2.** Add key bindings for J and gJ
  - [x] **2.1** Add "J" -> Action::JoinWithSpace to keymap (test: cargo check compiles)
  - [x] **2.2** Add "gJ" -> Action::JoinWithoutSpace to keymap (test: cargo check compiles)
- [x] **3.** Update Action trait implementations
  - [x] **3.1** Add JoinWithSpace and JoinWithoutSpace to is_countable() (test: cargo check compiles)
  - [x] **3.2** Add JoinWithSpace and JoinWithoutSpace to resets_remembered_column() (test: cargo check compiles)

### buffer.rs Changes

- [x] **4.** Implement Buffer::join_lines() method
  - [x] **4.1** Create join_lines(start_line, line_count, with_space) method (test: cargo check compiles)
  - [x] **4.2** Handle edge cases (last line, insufficient lines) (test: unit tests)
  - [x] **4.3** Implement space insertion between lines when with_space=true (test: unit tests)
  - [x] **4.4** Return cursor position at end of joined content (test: unit tests)

### window.rs Changes

- [x] **5.** Add action handlers for join motions
  - [x] **5.1** Add JoinWithSpace handling in process_action match (test: cargo check compiles)
  - [x] **5.2** Add JoinWithoutSpace handling in process_action match (test: cargo check compiles)
- [x] **6.** Implement helper methods in Window
  - [x] **6.1** Implement join_lines_with_space() method (test: cargo check compiles)
  - [x] **6.2** Implement join_lines_without_space() method (test: cargo check compiles)
- [x] **7.** Handle count prefix for join actions
  - [x] **7.1** Add special handling in Action::Count match for join actions (test: cargo check compiles)
  - [x] **7.2** Join count+1 lines when count prefix is provided (test: unit tests for 2J, 3gJ)

### Testing

- [x] **8.** Write unit tests for Buffer::join_lines()
  - [x] **8.1** Test join with space between two lines (test: cargo test passes)
  - [x] **8.2** Test join without space between two lines (test: cargo test passes)
  - [x] **8.3** Test join on last line returns None (test: cargo test passes)
  - [x] **8.4** Test join with insufficient lines (test: cargo test passes)
  - [x] **8.5** Test join with empty lines (test: cargo test passes)
- [x] **9.** Write integration tests for window actions
  - [x] **9.1** Test J key produces expected text (test: cargo test passes)
  - [x] **9.2** Test gJ key produces expected text (test: cargo test passes)
  - [x] **9.3** Test 2J joins 3 lines with space (test: cargo test passes)
  - [x] **9.4** Test cursor positioning after join (test: cargo test passes)

### Documentation

- [x] **10.** Update motions documentation
  - [x] **10.1** Add J and gJ to docs/motions.md (test: documentation file updated)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| editor.rs | 3 | 3 | 100% |
| buffer.rs | 1 | 1 | 100% |
| window.rs | 3 | 3 | 100% |
| Testing | 2 | 2 | 100% |
| Documentation | 1 | 1 | 100% |
| **Total** | **10** | **10** | **100%** |
