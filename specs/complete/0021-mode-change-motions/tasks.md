# Mode-Change Motions: a, A, I - Implementation Tasks

## Overview

Total: 7 tasks (6 feature + 1 bug fix)
Implementing vim-style mode-change motions `a`, `A`, and `I`.

## Implementation Tasks

- [x] **1.** Add new Action variants to enum (test: cargo check)
  - [x] **1.1** Add `AppendAfterCursor`, `AppendToLineEnd`, `InsertAtLineStart` to Action enum
  - [x] **1.2** Update `Action::resets_remembered_column()` to include new actions
  - [x] **1.3** Update `Action::is_line_action()` to include `AppendToLineEnd` and `InsertAtLineStart`

- [x] **2.** Add key bindings in NormalMode (test: cargo check)
  - [x] **2.1** Map `a` to `Action::AppendAfterCursor`
  - [x] **2.2** Map `A` to `Action::AppendToLineEnd`
  - [x] **2.3** Map `I` to `Action::InsertAtLineStart`

- [x] **3.** Handle new actions in Window::process_action() (test: cargo check)
  - [x] **3.1** Handle `AppendAfterCursor` - call move_cursor_right()
  - [x] **3.2** Handle `AppendToLineEnd` - set cursor to line_len (after last character)
  - [x] **3.3** Handle `InsertAtLineStart` - call move_cursor_to_line_content_start()
  - [x] **3.4** All return `ActionResult::Handled`

- [x] **4.** Update main.rs to switch to insert mode (test: cargo check)
  - [x] **4.1** After process_action() returns Handled, check for mode-change motions
  - [x] **4.2** Switch to InsertMode for these actions

- [x] **5.** Add unit tests for action properties (test: cargo test)
  - [x] **5.1** Test `is_line_action()` returns true for A and I, false for a
  - [x] **5.2** Test `is_countable()` returns false for all three
  - [x] **5.3** Test `with_count()` works for A and I, not for a

- [x] **6.** Update documentation (test: verify file exists)
  - [x] **6.1** Add a, A, I to docs/motions.md

- [x] **7.** Bug fix: "A" was inserting one character before end of line
  - [x] **7.1** Changed handler to use line_len instead of move_cursor_to_line_end() (test: cargo test)

- [x] **8.** Bug fix: Count actions (3A, 3I) don't switch to insert mode
  - [x] **8.1** Add Action::switches_to_insert_mode() helper that recursively checks Count inner action (test: cargo test)
  - [x] **8.2** Use helper in main.rs instead of matching specific variants (test: cargo test)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 4 | 4 | 100% |
| Testing | 1 | 1 | 100% |
| Documentation | 1 | 1 | 100% |
| Bug Fixes | 2 | 2 | 100% |
| **Total** | **8** | **8** | **100%** |
