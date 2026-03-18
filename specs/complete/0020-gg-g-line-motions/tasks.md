# gg and G Line Motions - Implementation Tasks

## Overview

Total: 10 tasks
Estimated completion: 1-2 hours
Prerequisites: None

## Implementation Tasks

- [x] **1.** Add MoveToFirstLine and MoveToLastLine to Action enum
  - [x] **1.1** Add Action::MoveToFirstLine variant (test: cargo check passes)
  - [x] **1.2** Add Action::MoveToLastLine variant (test: cargo check passes)

- [x] **2.** Update Action trait methods
  - [x] **2.1** Update resets_remembered_column() to return false for new actions (test: unit test)
  - [x] **2.2** Update uses_remembered_column() to return true for new actions (test: unit test)
  - [x] **2.3** Update is_countable() to return true for new actions (test: unit test)
  - [x] **2.4** Update is_line_action() to return true for new actions (test: unit test)

- [x] **3.** Extend SimpleKeymap for multi-key sequence support
  - [x] **3.1** Add Vec bindings to SimpleKeymap struct (test: cargo check)
  - [x] **3.2** Add insert_sequence method (test: cargo check)
  - [x] **3.3** Update get_action to search Vec (test: unit test)
  - [x] **3.4** Update is_prefix to check Vec prefixes (test: unit test)

- [x] **4.** Register keybindings in NormalMode
  - [x] **4.1** Add "g" prefix key binding (test: cargo check)
  - [x] **4.2** Add "gg" sequence binding (test: cargo check)
  - [x] **4.3** Add "G" key binding (test: cargo check)

- [x] **5.** Handle 'g' prefix in NormalMode::handle_key
  - [x] **5.1** Detect when 'g' is pressed and waiting is true (test: cargo check)
  - [x] **5.2** Execute MoveToFirstLine on second 'g' with pending count (test: manual test)

- [x] **6.** Add action handlers in Window::process_action
  - [x] **6.1** Handle Action::MoveToFirstLine (test: manual test gg)
  - [x] **6.2** Handle Action::MoveToLastLine (test: manual test G)

- [x] **7.** Handle Count wrapper for new motions
  - [x] **7.1** Update Window::process_action to handle Count with line motions (test: test 5gg, 5G)

- [x] **8.** Add unit tests
  - [x] **8.1** Test gg motion (test: unit test)
  - [x] **8.2** Test G motion (test: unit test)
  - [x] **8.3** Test count prefix with gg and G (test: test 5gg, 5G)

- [x] **9.** Update documentation
  - [x] **9.1** Update docs/motions.md (test: verify file exists and is updated)

- [x] **10.** Final verification
  - [x] **10.1** Run cargo check (test: no warnings)
  - [x] **10.2** Run cargo test (test: all tests pass)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Action enum | 2 | 2 | 100% |
| Action methods | 4 | 4 | 100% |
| Keymap support | 4 | 4 | 100% |
| NormalMode bindings | 3 | 3 | 100% |
| Window handlers | 3 | 3 | 100% |
| Documentation | 1 | 1 | 100% |
| Testing | 3 | 3 | 100% |
| **Total** | **10** | **10** | **100%** |
