# Text Delete Keys - Implementation Tasks

## Overview

Total: 9 tasks
Estimated completion: 1-2 hours
Prerequisites: None (feature builds on existing keybinding and buffer infrastructure)

## Implementation Tasks

### buffer.rs - Buffer Deletion Methods

- [x] **1.** Add `delete_char_before_cursor` method to Buffer
  - [x] **1.1** Handle case: cursor at start of line - join with previous line (test: cursor at line 1, col 0, joins with line 0)
  - [x] **1.2** Handle case: cursor at document start - return None (test: cursor at line 0, col 0, no operation)
  - [x] **1.3** Handle case: cursor in middle of line - remove grapheme before cursor (test: "abc" with cursor at col 2, deletes "b", returns col 1)
  - [x] **1.4** Handle Unicode grapheme clusters correctly (test: "héllo" with cursor at col 2, removes "é" as single unit)
  - [x] **1.5** Handle emoji correctly (test: "a👍b" with cursor at col 2, removes "👍" as single unit)

- [x] **2.** Add `delete_char_at_cursor` method to Buffer
  - [x] **2.1** Handle case: cursor at end of line - join with next line (test: cursor at end of line 0, joins with line 1)
  - [x] **2.2** Handle case: cursor at document end - return None (test: cursor at last line, last col, no operation)
  - [x] **2.3** Handle case: cursor in middle of line - remove grapheme at cursor (test: "abc" with cursor at col 1, deletes "b", cursor stays at col 1)
  - [x] **2.4** Handle Unicode grapheme clusters correctly (test: "héllo" with cursor at col 1, removes "é" as single unit)
  - [x] **2.5** Handle emoji correctly (test: "a👍b" with cursor at col 1, removes "👍" as single unit)

### editor.rs - Action Enum and Keybindings

- [x] **3.** Add new Action variants
  - [x] **3.1** Add `DeleteBackward` variant to Action enum (test: Action::DeleteBackward == Action::DeleteBackward)
  - [x] **3.2** Add `DeleteForward` variant to Action enum (test: Action::DeleteForward == Action::DeleteForward)

- [x] **4.** Add Normal Mode keybindings
  - [x] **4.1** Bind "x" to Action::DeleteForward (test: press x in normal mode triggers DeleteForward)
  - [x] **4.2** Bind "X" to Action::DeleteBackward (test: press X in normal mode triggers DeleteBackward)

- [x] **5.** Add Insert Mode keybindings
  - [x] **5.1** Bind "<Backspace>" to Action::DeleteBackward (test: press Backspace in insert mode triggers DeleteBackward)
  - [x] **5.2** Bind "<Delete>" to Action::DeleteForward (test: press Delete in insert mode triggers DeleteForward)

### window.rs - Action Processing

- [x] **6.** Add Window deletion methods
  - [x] **6.1** Add `delete_char_before_cursor` method (test: calls buffer method and updates cursor correctly)
  - [x] **6.2** Add `delete_char_at_cursor` method (test: calls buffer method and keeps cursor in place)

- [x] **7.** Handle new actions in process_action
  - [x] **7.1** Handle Action::DeleteBackward (test: process_action returns Handled for DeleteBackward)
  - [x] **7.2** Handle Action::DeleteForward (test: process_action returns Handled for DeleteForward)

### Testing

- [x] **8.** Write unit tests for buffer deletion methods
  - [x] **8.1** Test delete_char_before_cursor at line start joins lines (test: verify line join behavior)
  - [x] **8.2** Test delete_char_before_cursor at document start does nothing (test: verify no change)
  - [x] **8.3** Test delete_char_before_cursor with Unicode graphemes (test: verify "é" removed as single unit)
  - [x] **8.4** Test delete_char_before_cursor with emoji (test: verify "👍" removed as single unit)
  - [x] **8.5** Test delete_char_at_cursor at line end joins lines (test: verify line join behavior)
  - [x] **8.6** Test delete_char_at_cursor at document end does nothing (test: verify no change)
  - [x] **8.7** Test delete_char_at_cursor with Unicode graphemes (test: verify "é" removed as single unit)
  - [x] **8.8** Test delete_char_at_cursor with emoji (test: verify "👍" removed as single unit)

- [x] **9.** Manual integration testing
  - [x] **9.1** Test insert mode Backspace (test: press Backspace, character is deleted, cursor moves back)
  - [x] **9.2** Test insert mode Delete (test: press Delete, character is deleted, cursor stays)
  - [x] **9.3** Test normal mode x (test: press x, character is deleted at cursor)
  - [x] **9.4** Test normal mode X (test: press X, character before cursor is deleted)
  - [x] **9.5** Test line joining with Backspace (test: at line start, joins with previous line)
  - [x] **9.6** Test line joining with Delete (test: at line end, joins with next line)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| buffer.rs | 2 | 2 | 100% |
| editor.rs | 3 | 3 | 100% |
| window.rs | 2 | 2 | 100% |
| Testing | 2 | 2 | 100% |
| **Total** | **9** | **9** | **100%** |
