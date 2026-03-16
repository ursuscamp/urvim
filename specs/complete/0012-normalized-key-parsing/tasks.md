# Normalized Key Parsing - Implementation Tasks

## Overview

Total: 12 tasks
Estimated completion: 1-2 days
Prerequisites: canonical_string() implementation (spec 0011 - complete)

## Core Implementation

- [x] **1.** Create `HandleKeyResult` enum in editor module
  - [x] **1.1** Define `HandleKeyResult::Complete(Action)` variant (test: compiles and can be constructed)
  - [x] **1.2** Define `HandleKeyResult::WaitForMore` variant (test: compiles)
  - [x] **1.3** Define `HandleKeyResult::InvalidSequence` variant (test: compiles)

- [x] **2.** Create `Keymap` trait in editor module
  - [x] **2.1** Define `get_action(&self, keys: &[String]) -> Option<Action>` method (test: trait compiles)
  - [x] **2.2** Define `is_prefix(&self, keys: &[String]) -> bool` method (test: trait compiles)

- [x] **3.** Create `SimpleKeymap` struct implementing `Keymap`
  - [x] **3.1** Create struct with `HashMap<String, Action>` bindings field (test: struct instantiates)
  - [x] **3.2** Implement `new()` constructor (test: can create new keymap)
  - [x] **3.3** Implement `insert(&mut self, key: String, action: Action)` (test: can insert bindings)
  - [x] **3.4** Implement `get_action()` using HashMap lookup (test: returns correct action)
  - [x] **3.5** Implement `is_prefix()` - for single-key maps, returns false (test: returns false for single-key)

- [x] **4.** Modify `Mode` trait for stateful key handling
  - [x] **4.1** Change `handle_key(&self, key: &Key) -> Action` to `handle_key(&mut self, key: &Key) -> HandleKeyResult` (test: compiles)
  - [x] **4.2** Add `is_waiting(&self) -> bool` method (test: compiles)
  - [x] **4.3** Add `clear_buffer(&mut self)` method (test: compiles)

- [x] **5.** Refactor `NormalMode` to use normalized keys
  - [x] **5.1** Add `buffer: Vec<String>` field (test: struct compiles)
  - [x] **5.2** Add `waiting: bool` field (test: struct compiles)
  - [x] **5.3** Add `keymap: SimpleKeymap` field (test: struct compiles)
  - [x] **5.4** Implement `handle_key()` to use canonical_string() and keymap lookup (test: single keys map correctly)
  - [x] **5.5** Implement `is_waiting()` returning `waiting` field (test: returns correct value)
  - [x] **5.6** Implement `clear_buffer()` to clear buffer and reset waiting (test: buffer cleared)
  - [x] **5.7** Create default keymap with current keybindings (test: all existing keys map to actions)

- [x] **6.** Refactor `InsertMode` to use normalized keys
  - [x] **6.1** Add `buffer: Vec<String>` field (test: struct compiles)
  - [x] **6.2** Add `waiting: bool` field (test: struct compiles)
  - [x] **6.3** Add `keymap: SimpleKeymap` field (test: struct compiles)
  - [x] **6.4** Implement `handle_key()` to use canonical_string() and keymap lookup (test: characters insert correctly)
  - [x] **6.5** Implement `is_waiting()` and `clear_buffer()` (test: methods work)
  - [x] **6.6** Create default keymap for insert mode (test: all existing keys work)

## Main Loop Integration

- [x] **7.** Update main event loop to handle `HandleKeyResult`
  - [x] **7.1** Update event loop to call `handle_key` on mutable mode reference (test: compiles)
  - [x] **7.2** Handle `HandleKeyResult::Complete(action)` - execute action (test: actions execute)
  - [x] **7.3** Handle `HandleKeyResult::WaitForMore` - continue without action (test: no action taken)
  - [x] **7.4** Handle `HandleKeyResult::InvalidSequence` - treat as no action (test: ignored)

## Testing

- [x] **8.** Write unit tests for `SimpleKeymap`
  - [x] **8.1** Test single key lookup (test: tests pass)
  - [x] **8.2** Test is_prefix returns false for single-key map (test: tests pass)

- [x] **9.** Write unit tests for `NormalMode` key handling
  - [x] **9.1** Test single keys map to correct actions (test: existing behavior preserved)
  - [x] **9.2** Test invalid keys clear buffer and return InvalidSequence (test: buffer cleared)
  - [x] **9.3** Test Escape clears buffer (test: buffer cleared)

- [x] **10.** Write unit tests for `InsertMode` key handling
  - [x] **10.1** Test characters insert correctly (test: existing behavior preserved)
  - [x] **10.2** Test Escape switches to normal mode (test: mode switch works)

- [x] **11.** Run existing tests to verify backward compatibility
  - [x] **11.1** Run all editor tests (test: all pass)
  - [x] **11.2** Run terminal/key tests (test: all pass)

## Cleanup

- [x] **12.** Clean up any clippy warnings or errors
  - [x] **12.1** Run cargo check (test: no errors)
  - [x] **12.2** Run clippy (test: no warnings)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Core Implementation | 6 | 6 | 100% |
| Main Loop Integration | 1 | 1 | 100% |
| Testing | 4 | 4 | 100% |
| Cleanup | 1 | 1 | 100% |
| **Total** | **12** | **12** | **100%** |
