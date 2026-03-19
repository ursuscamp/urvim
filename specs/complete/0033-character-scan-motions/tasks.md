# Character Scan Motions - Implementation Tasks

## Overview

Total: 24 tasks (all completed)
Estimated phases: Keymap modules, Action enum, Mode integration, Motion execution, Testing

## Keymap Modules

- [x] **1.** Create CharScanKeymap in `src/motion/char_scan_keymap.rs`
  - [x] **1.1** Create CharScanKeymap struct (test: instantiates without error)
  - [x] **1.2** Implement `Keymap` trait for CharScanKeymap (test: see below)
  - [x] **1.3** Write unit tests for CharScanKeymap (test: all assertions pass)

- [x] **2.** Create ChainedKeymap in `src/motion/chained_keymap.rs`
  - [x] **2.1** Create ChainedKeymap struct with Vec<Box<dyn Keymap>> (test: instantiates without error)
  - [x] **2.2** Implement `Keymap` trait for ChainedKeymap (test: see below)
  - [x] **2.3** Write unit tests for ChainedKeymap (test: all assertions pass)

## Action Enum

- [x] **3.** Add new Action variants in `src/editor.rs`
  - [x] **3.1** Add `FindForward(char)` variant (test: compiles)
  - [x] **3.2** Add `FindBackward(char)` variant (test: compiled)
  - [x] **3.3** Add `TillForward(char)` variant (test: compiled)
  - [x] **3.4** Add `TillBackward(char)` variant (test: compiled)

- [x] **4.** Implement `with_count()` for new Action variants
  - [x] **4.1** Implement for `FindForward(char)` (test: with_count(3) returns Count(3, FindForward))
  - [x] **4.2** Implement for `FindBackward(char)` (test: with_count(3) returns Count(3, FindBackward))
  - [x] **4.3** Implement for `TillForward(char)` (test: with_count(3) returns Count(3, TillForward))
  - [x] **4.4** Implement for `TillBackward(char)` (test: with_count(3) returns Count(3, TillBackward))

## Mode Integration

- [x] **5.** Update NormalMode to use ChainedKeymap in `src/editor.rs`
  - [x] **5.1** Change `keymap: TrieKeymap` to `keymap: ChainedKeymap` (test: compiles)
  - [x] **5.2** Update `NormalMode::new()` to construct ChainedKeymap with TrieKeymap and CharScanKeymap (test: mode initializes correctly)
  - [x] **5.3** Verify escape handling still works (test: press Esc during char scan wait cancels)

## Motion Execution

- [x] **6.** Add motion helper functions in `src/window.rs`
  - [x] **6.1** Add `move_cursor_to_char_forward(target: char, count: usize)` method (test: unit tests)
  - [x] **6.2** Add `move_cursor_to_char_backward(target: char, count: usize)` method (test: unit tests)

- [x] **7.** Add `process_action` handlers in `src/window.rs`
  - [x] **7.1** Handle `Action::FindForward(char)` (test: executes motion correctly)
  - [x] **7.2** Handle `Action::FindBackward(char)` (test: executes motion correctly)
  - [x] **7.3** Handle `Action::TillForward(char)` (test: lands one position before)
  - [x] **7.4** Handle `Action::TillBackward(char)` (test: lands one position after)

## Module Exports

- [x] **8.** Update `src/motion/mod.rs` exports
  - [x] **8.1** Add `pub mod char_scan_keymap;` (test: module accessible)
  - [x] **8.2** Add `pub mod chained_keymap;` (test: module accessible)

## Testing

- [x] **9.** Write CharScanKeymap unit tests
  - [x] **9.1** `get_action(["f", "x"])` returns `FindForward('x')`
  - [x] **9.2** `get_action(["F", "x"])` returns `FindBackward('x')`
  - [x] **9.3** `get_action(["t", "x"])` returns `TillForward('x')`
  - [x] **9.4** `get_action(["T", "x"])` returns `TillBackward('x')`
  - [x] **9.5** `get_action(["f"])` returns `None`
  - [x] **9.6** `get_action(["g", "g"])` returns `None`
  - [x] **9.7** `is_prefix(["f"])` returns `true`
  - [x] **9.8** `is_prefix(["F"])` returns `true`
  - [x] **9.9** `is_prefix(["t"])` returns `true`
  - [x] **9.10** `is_prefix(["T"])` returns `true`
  - [x] **9.11** `is_prefix(["f", "x"])` returns `false`
  - [x] **9.12** `is_prefix(["g"])` returns `false`

- [x] **10.** Write ChainedKeymap unit tests
  - [x] **10.1** `get_action` returns trie result before char scan (test: `gg` goes to trie, `fx` goes to char scan)
  - [x] **10.2** `is_prefix` returns true if any keymap returns true (test: `g` trie says prefix, `f` char scan says prefix)

- [x] **11.** Write Window motion integration tests
  - [x] **11.1** `FindForward` moves cursor to target character
  - [x] **11.2** `FindBackward` moves cursor to target character
  - [x] **11.3** `TillForward` lands one position before target
  - [x] **11.4** `TillBackward` lands one position after target
  - [x] **11.5** Count prefix finds Nth occurrence
  - [x] **11.6** Target not found leaves cursor in place
  - [x] **11.7** Till offset clamps at line boundaries

- [x] **12.** Run full test suite
  - [x] **12.1** `cargo test` passes (464 tests)
  - [x] **12.2** `cargo check` passes with no warnings

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Keymap Modules | 5 | 5 | 100% |
| Action Enum | 5 | 5 | 100% |
| Mode Integration | 3 | 3 | 100% |
| Motion Execution | 4 | 4 | 100% |
| Module Exports | 2 | 2 | 100% |
| Testing | 5 | 5 | 100% |
| **Total** | **24** | **24** | **100%** |
