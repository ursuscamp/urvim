# Text Objects: Inner Word and Around Word - Implementation Tasks

## Overview

Total: 10 tasks
Estimated completion: 1-2 days
Prerequisites: None

## Implementation

- [x] **1.** Add Operator and TextObject enums to `src/editor.rs`
  - [x] **1.1** Add `Operator` enum with `Delete` variant (test: enum compiles)
  - [x] **1.2** Add `TextObject` enum with `InnerWord` and `AroundWord` variants (test: enum compiles)

- [x] **2.** Add `Action::Operation(Operator, TextObject)` variant to `src/editor.rs`
  - [x] **2.1** Add variant to Action enum (test: compile)
  - [x] **2.2** Add to `is_snapshottable()` - returns true for delete (test: unit test)
  - [x] **2.3** Add to `updates_snapshot_cursor()` - returns true (test: unit test)
  - [x] **2.4** Add to `switches_to_insert_mode()` - returns false (test: unit test)

- [x] **3.** Register "diw" and "daw" sequences in TrieKeymap
  - [x] **3.1** Insert `["d", "i", "w"]` → `Action::Operation(Delete, InnerWord)` (test: unit test)
  - [x] **3.2** Insert `["d", "a", "w"]` → `Action::Operation(Delete, AroundWord)` (test: unit test)
  - [x] **3.3** Verify "d" alone returns `WaitForMore` (has children) (test: unit test)
  - [x] **3.4** Verify "di" alone returns `WaitForMore` (has children) (test: unit test)

- [x] **4.** Add `TextObjectRange` struct to `src/buffer.rs`
  - [x] **4.1** Define struct with `start: Cursor` and `end: Cursor` fields (test: compiles)
  - [x] **4.2** Add `delete_range()` method to Buffer (test: unit test)

- [x] **5.** Implement `get_inner_word_range()` in `src/buffer.rs`
  - [x] **5.1** Cursor inside word: return word boundaries (test: "hello world" at 'h' → (0,0)-(0,5))
  - [x] **5.2** Cursor inside whitespace: return whitespace region (test: "  hello" at ' ' → (0,0)-(0,2))
  - [x] **5.3** Cursor at line boundaries: handle edge cases (test: empty line, line end)

- [x] **6.** Implement `get_around_word_range()` in `src/buffer.rs`
  - [x] **6.1** Cursor inside word: return word + ALL trailing whitespace (test: "hello   world" at 'h' → (0,0)-(0,8))
  - [x] **6.2** Cursor inside whitespace: return whitespace + trailing word (test: "   world" at ' ' → (0,0)-(0,7))
  - [x] **6.3** No trailing whitespace: behave like inner word (test: "hello" at 'h' → (0,0)-(0,5))

- [x] **7.** Handle `Action::Operation` in `src/window.rs` `process_action()`
  - [x] **7.1** Match on Operator::Delete (test: unit test end-to-end)
  - [x] **7.2** Call appropriate buffer method based on TextObject (test: unit test)
  - [x] **7.3** Save undo snapshot before delete (test: undo works)
  - [x] **7.4** Set cursor to start of deleted region (test: cursor position check)

- [x] **8.** Handle `Count` with `Operation` in `src/window.rs` `handle_count()`
  - [x] **8.1** Add case for `Action::Count(count, Operation(...))` (test: unit test)
  - [x] **8.2** Execute operation count times (test: 3diw deletes 3 words)

## Testing

- [x] **9.** Write unit tests in `src/editor/tests.rs`
  - [x] **9.1** Test "diw" sequence → `Operation(Delete, InnerWord)` (test: unit test)
  - [x] **9.2** Test "daw" sequence → `Operation(Delete, AroundWord)` (test: unit test)
  - [x] **9.3** Test "d" alone → WaitForMore (test: unit test)
  - [x] **9.4** Test Escape during sequence → InvalidSequence (test: unit test)
  - [x] **9.5** Test counts: 3diw, d3iw, 3d3iw (test: unit test)

- [ ] **10.** Integration tests for text object behavior
  - [ ] **10.1** `diw` with cursor on word (test: word deleted)
  - [ ] **10.2** `diw` with cursor in whitespace (test: whitespace deleted)
  - [ ] **10.3** `daw` with cursor on word (test: word + trailing whitespace deleted)
  - [ ] **10.4** `daw` with cursor in whitespace (test: whitespace + trailing word deleted)
  - [ ] **10.5** `d3iw` deletes 3 words (test: count works)
  - [ ] **10.6** Undo after `diw` restores text (test: undo works)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 8 | 8 | 100% |
| Testing | 2 | 1 | 50% |
| **Total** | **10** | **9** | **90%** |
