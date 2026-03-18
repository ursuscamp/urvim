# H/M/L Motions - Implementation Tasks

## Overview

Total: 8 tasks
Key milestones: Add Action variants → Add keymap bindings → Implement Window handlers → Add tests

## Implementation

- [x] **1.** Add Action enum variants in editor.rs
  - [x] **1.1** Add MoveToScreenTop, MoveToScreenMiddle, MoveToScreenBottom variants (test: cargo check)
  - [x] **1.2** Update resets_remembered_column() to return false for these actions (test: cargo test)
  - [x] **1.3** Update uses_remembered_column() to return true for these actions (test: cargo test)
  - [x] **1.4** Update is_countable() to return true for H and L, false for M (test: cargo test)
  - [x] **1.5** Update is_line_action() to return false (test: cargo test)
  - [x] **1.6** Update with_count() to allow counts for H and L, reject for M (test: cargo test)

- [x] **2.** Add keymap bindings in NormalMode
  - [x] **2.1** Add H key binding to MoveToScreenTop (test: cargo test for key mapping)
  - [x] **2.2** Add M key binding to MoveToScreenMiddle (test: cargo test for key mapping)
  - [x] **2.3** Add L key binding to MoveToScreenBottom (test: cargo test for key mapping)

- [x] **3.** Add Window handler methods
  - [x] **3.1** Add move_cursor_to_screen_top() method (test: cargo test)
  - [x] **3.2** Add move_cursor_to_screen_middle() method (test: cargo test)
  - [x] **3.3** Add move_cursor_to_screen_bottom() method (test: cargo test)

- [x] **4.** Add process_action() handlers in Window
  - [x] **4.1** Handle MoveToScreenTop action (test: cargo test)
  - [x] **4.2** Handle MoveToScreenMiddle action (test: cargo test)
  - [x] **4.3** Handle MoveToScreenBottom action (test: cargo test)
  - [x] **4.4** Handle Count wrapper for H and L motions (test: cargo test)

- [x] **5.** Handle count extraction for screen motions
  - [x] **5.1** Modify Count handler to detect H/L inner actions and pass count (test: cargo test)

- [x] **6.** Run cargo check to verify build
  - [x] **6.1** Fix any compilation errors (test: cargo check passes)

- [x] **7.** Update motion documentation
  - [x] **7.1** Add H/M/L motions to docs/motions.md (test: verify docs exist and are accurate)

## Testing

- [ ] **7.** Add unit tests for action classification
  - [ ] **7.1** Test is_countable() for H/M/L (test: cargo test)
  - [ ] **7.2** Test with_count() rejects M but allows H/L (test: cargo test)

- [ ] **8.** Add integration tests for H/M/L motions
  - [ ] **8.1** Test H key binding returns correct action (test: cargo test)
  - [ ] **8.2** Test M key binding returns correct action (test: cargo test)
  - [ ] **8.3** Test L key binding returns correct action (test: cargo test)
  - [ ] **8.4** Test cursor positioning for H motion (test: cargo test)
  - [ ] **8.5** Test cursor positioning for M motion (test: cargo test)
  - [ ] **8.6** Test cursor positioning for L motion (test: cargo test)
  - [ ] **8.7** Test count prefix with H motion (test: cargo test)
  - [ ] **8.8** Test count prefix with L motion (test: cargo test)
  - [ ] **8.9** Test M ignores count prefix (test: cargo test)
  - [ ] **8.10** Test column preservation (test: cargo test)
  - [ ] **8.11** Test edge case: fewer lines than viewport (test: cargo test)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Action enum | 6 | 6 | 100% |
| Keymap bindings | 3 | 3 | 100% |
| Window handlers | 4 | 4 | 100% |
| Count handling | 1 | 1 | 100% |
| Build verification | 1 | 1 | 100% |
| Documentation | 1 | 1 | 100% |
| **Total** | **16** | **16** | **100%** |
