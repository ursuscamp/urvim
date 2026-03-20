# Repeat Character Search Motions - Implementation Tasks

## Overview

Total: 12 tasks
Estimated completion: 1-2 hours
Prerequisites: None

## Implementation

- [x] **1.** Create `src/globals.rs` module
  - [x] **1.1** Add `Direction` enum (Forward, Backward) (test: cargo check)
  - [x] **1.2** Add `FindKind` enum (Find, Till) (test: cargo check)
  - [x] **1.3** Add `FindState` struct (test: cargo check)
  - [x] **1.4** Add `LAST_FIND` global static with Mutex (test: cargo check)
  - [x] **1.5** Add `set_last_find()` and `get_last_find()` functions (test: unit tests)

- [x] **2.** Update `src/lib.rs`
  - [x] **2.1** Add `pub mod globals;` (test: cargo check)

- [x] **3.** Update `src/editor.rs`
  - [x] **3.1** Add `RepeatLastFind` and `RepeatLastFindReverse` to `Action` enum (test: cargo check)
  - [x] **3.2** Add new actions to `resets_remembered_column()` (test: cargo check)
  - [x] **3.3** Add new actions to `is_countable()` (test: cargo check)
  - [x] **3.4** Add keybindings for `;` and `,` in `NormalMode::new()` (test: cargo check)

- [x] **4.** Update `src/terminal/keys.rs`
  - [x] **4.1** Remove `';' => Some(':')` from shift mapping (test: cargo check)
  - [x] **4.2** Remove `',' => Some('<')` from shift mapping (test: cargo check)

- [x] **5.** Update `src/window.rs`
  - [x] **5.1** Add import for `globals` module (test: cargo check)
  - [x] **5.2** Update `FindForward` handler to call `globals::set_last_find()` (test: unit test)
  - [x] **5.3** Update `FindBackward` handler to call `globals::set_last_find()` (test: unit test)
  - [x] **5.4** Update `TillForward` handler to call `globals::set_last_find()` (test: unit test)
  - [x] **5.5** Update `TillBackward` handler to call `globals::set_last_find()` (test: unit test)
  - [x] **5.6** Add `RepeatLastFind` handler in `process_action()` (test: unit test)
  - [x] **5.7** Add `RepeatLastFindReverse` handler in `process_action()` (test: unit test)

## Testing

- [x] **6.** Add unit tests for `globals.rs`
  - [x] **6.1** Test `set_last_find()` then `get_last_find()` returns `Some(state)` (test: cargo test)
  - [x] **6.2** Test `get_last_find()` on empty state returns `None` (test: cargo test)

- [ ] **7.** Add unit tests for `src/window.rs` (repeat character search)
  - [ ] **7.1** Test `FindForward` updates global state (test: cargo test)
  - [ ] **7.2** Test `RepeatLastFind` with stored Forward state calls correct motion (test: cargo test)
  - [ ] **7.3** Test `RepeatLastFindReverse` with stored Forward state calls opposite motion (test: cargo test)
  - [ ] **7.4** Test `;` with no previous search doesn't move cursor (test: cargo test)
  - [ ] **7.5** Test count prefix with `3 RepeatLastFind` (test: cargo test)

## Documentation

- [x] **8.** Update `docs/motions.md`
  - [x] **8.1** Add documentation for `;` (RepeatLastFind) (test: review doc)
  - [x] **8.2** Add documentation for `,` (RepeatLastFindReverse) (test: review doc)

## Completion

- [x] **9.** Run `cargo check` to verify no warnings or errors
- [x] **10.** Run `cargo test` to verify all tests pass

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 5 | 5 | 100% |
| Testing | 2 | 0 | 0% |
| Documentation | 1 | 1 | 100% |
| Completion | 2 | 2 | 100% |
| **Total** | **10** | **8** | **80%** |
