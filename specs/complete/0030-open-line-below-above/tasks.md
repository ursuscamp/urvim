# Open Line Below and Above - Implementation Tasks

## Overview

Total: 8 tasks
Estimated completion: 1-2 hours
Prerequisites: None

## Implementation

- [x] **1.** Add OpenLineBelow and OpenLineAbove actions to Action enum
  - [x] **1.1** Add Action::OpenLineBelow variant in editor.rs (test: cargo check)
  - [x] **1.2** Add Action::OpenLineAbove variant in editor.rs (test: cargo check)
  - [x] **1.3** Add both actions to resets_remembered_column() (test: cargo check)
  - [x] **1.4** Add both actions to switches_to_insert_mode() (test: cargo check)
  - [x] **1.5** Add OpenLineBelow and OpenLineAbove to is_countable() for count support (test: cargo check)

- [x] **2.** Add keybindings in NormalMode
  - [x] **2.1** Add "o" keymap for Action::OpenLineBelow (test: cargo check)
  - [x] **2.2** Add "O" keymap for Action::OpenLineAbove (test: cargo check)

- [x] **3.** Implement Buffer.insert_lines_after() method
  - [x] **3.1** Create insert_lines_after() method in buffer.rs (test: cargo check)
  - [x] **3.2** Handle edge case: empty buffer (test: unit test)
  - [x] **3.3** Handle edge case: insert at beginning (test: unit test)
  - [x] **3.4** Handle edge case: insert at end (test: unit test)
  - [x] **3.5** Handle count > 1 (test: unit test)

- [x] **4.** Handle actions in Window.process_action()
  - [x] **4.1** Handle Action::OpenLineBelow (test: cargo check)
  - [x] **4.2** Handle Action::OpenLineAbove (test: cargo check)

- [x] **5.** Handle count prefix for OpenLineBelow and OpenLineAbove
  - [x] **5.1** Add count handling for OpenLineBelow in Action::Count branch (test: cargo check)
  - [x] **5.2** Add count handling for OpenLineAbove in Action::Count branch (test: cargo check)

- [x] **6.** Write unit tests for Buffer.insert_lines_after()
  - [x] **6.1** Test insert after first line (test: cargo test)
  - [x] **6.2** Test insert after middle line (test: cargo test)
  - [x] **6.3** Test insert after last line (test: cargo test)
  - [x] **6.4** Test insert into empty buffer (test: cargo test)
  - [x] **6.5** Test insert multiple lines (test: cargo test)

- [x] **7.** Write integration tests for NormalMode
  - [x] **7.1** Test o key creates line below (test: cargo test)
  - [x] **7.2** Test O key creates line above (test: cargo test)
  - [x] **7.3** Test count prefix with o (test: cargo test)

- [x] **8.** Run cargo check and fix any warnings
  - [x] **8.1** Run cargo check (test: verify no errors)
  - [x] **8.2** Run cargo test (test: verify all tests pass)
  - [x] **8.3** Run clippy (test: verify no lints)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 5 | 5 | 100% |
| Testing | 3 | 3 | 100% |
| **Total** | **8** | **8** | **100%** |
