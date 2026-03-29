# BigWord Text Objects - Implementation Tasks

## Overview

Total: 8 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Extend the text-object model for BigWord variants
  - [x] **1.1** Add `InnerBigWord` and `AroundBigWord` variants to `TextObject` in [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) (test: compile)
  - [x] **1.2** Update any public re-exports or module docs if needed so the new variants remain visible to callers (test: compile)

- [x] **2.** Register `diW` / `daW` sequences in normal mode
  - [x] **2.1** Add `diW` and `ciW` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.2** Add `daW` and `caW` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.3** Preserve partial-sequence waiting behavior for prefixes like `d` and `c` when `W`-family text objects are available (test: unit test)

- [x] **3.** Add BigWord range resolution in the buffer layer
  - [x] **3.1** Implement `get_inner_big_word_range()` in [`src/buffer/text_object.rs`](/Users/ryan/Dev/urvim/src/buffer/text_object.rs) or the existing word text-object module (test: unit test)
  - [x] **3.2** Implement `get_around_big_word_range()` in [`src/buffer/text_object.rs`](/Users/ryan/Dev/urvim/src/buffer/text_object.rs) or the existing word text-object module (test: unit test)
  - [x] **3.3** Ensure BigWord resolution uses `Boundary::BigWord` semantics for contiguous non-whitespace runs (test: unit test)
  - [x] **3.4** Keep whitespace-adjacent cursor behavior aligned with the existing word text-object style (test: unit test)

- [x] **4.** Route operator execution through the new BigWord API
  - [x] **4.1** Update [`src/buffer/operator_target.rs`](/Users/ryan/Dev/urvim/src/buffer/operator_target.rs) to resolve `InnerBigWord` and `AroundBigWord` through the buffer helpers (depends on: 3.1, 3.2)
  - [x] **4.2** Preserve delete/change cursor placement and undo snapshot behavior for BigWord operations (test: unit test)

- [x] **5.** Update documentation and glossary
  - [x] **5.1** Add `iW` and `aW` to [`docs/motions.md`](/Users/ryan/Dev/urvim/docs/motions.md) with examples and edge cases (depends on: 2.1, 2.2, 3.1, 3.2)
  - [x] **5.2** Keep [`specs/glossary.md`](/Users/ryan/Dev/urvim/specs/glossary.md) aligned with the new BigWord text-object terminology if implementation details change

- [x] **6.** Add editor key-sequence tests
  - [x] **6.1** Verify `diW` resolves to `Operation(Delete, InnerBigWord)` (test: unit test)
  - [x] **6.2** Verify `daW` resolves to `Operation(Delete, AroundBigWord)` (test: unit test)
  - [x] **6.3** Verify `d` and `di` still wait for more input when the new bindings are registered (test: unit test)

- [x] **7.** Add buffer-level range tests
  - [x] **7.1** Test cursor-inside-token behavior for `get_inner_big_word_range()` using punctuation-heavy input such as `foo-bar baz` (test: unit test)
  - [x] **7.2** Test cursor-on-whitespace behavior for `get_inner_big_word_range()` and `get_around_big_word_range()` (test: unit test)
  - [x] **7.3** Test trailing-whitespace expansion for `get_around_big_word_range()` (test: unit test)
  - [x] **7.4** Test empty-line and end-of-line edge cases (test: unit test)

- [x] **8.** Verify the implementation
  - [x] **8.1** Run `cargo check` and fix any compile errors or warnings (test: build check)
  - [x] **8.2** Run targeted unit tests for editor keymaps and buffer text-object resolution (test: unit tests)
  - [x] **8.3** Update task completion status as each item is finished

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 5 | 5 | 100% |
| Testing | 3 | 3 | 100% |
| **Total** | **8** | **8** | **100%** |
