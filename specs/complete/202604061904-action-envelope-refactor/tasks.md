# Action Envelope Refactor - Implementation Tasks

## Overview

Total: 7 tasks
Refactor the action model so actions carry optional source and destination mode metadata while preserving insert behavior, undo/redo, and dot-repeat.

## Implementation

### Action Model
- [x] **1.** Split the current action enum into an action envelope plus an `ActionKind` payload enum
  - [x] **1.1** Introduce the new `Action` struct with `kind`, `from_mode`, and `to_mode` fields in `src/editor/action.rs` (test: compile)
  - [x] **1.2** Move existing action variants into `ActionKind` and keep the existing payload shapes stable where possible (test: compile)
  - [x] **1.3** Update helper methods such as snapshot, repeat, and cursor bookkeeping to inspect the new envelope shape (test: unit tests pass)

### Mode Construction
- [x] **2.** Update normal-mode and insert-mode key handling to construct the new action envelope
  - [x] **2.1** Teach normal mode to emit mode-agnostic actions for ordinary commands and mode-changing actions with `to_mode` set (test: unit test)
  - [x] **2.2** Teach insert mode to emit ordinary `InsertChar` and `DeleteBackward` payloads for auto-pair behavior using `from_mode` when the action should be mode-specific (test: unit test)
  - [x] **2.3** Remove the remaining insert-only action variants and the mode global plumbing once the new envelope covers the same behavior (test: compile)

### Dispatch and Mode Switching
- [x] **3.** Update the main event loop and layout dispatch to use the new action metadata
  - [x] **3.1** Dispatch only actions whose `from_mode` is compatible with the current mode, allowing mode-agnostic actions through (test: unit test)
  - [x] **3.2** Apply `to_mode` after a handled action instead of matching dedicated switch variants (test: unit test)
  - [x] **3.3** Keep snapshot and repeat bookkeeping aligned with the action that actually executed (test: unit test)

### Window Editing
- [x] **4.** Update window-level edit handling to interpret insert behavior from the action envelope
  - [x] **4.1** Preserve auto-pair insertion by treating insert-mode `InsertChar` opener edits as paired insertion when the envelope indicates insert-mode origin (test: unit test)
  - [x] **4.2** Preserve closer skipping by treating insert-mode `InsertChar` closer edits as cursor advancement when appropriate (test: unit test)
  - [x] **4.3** Preserve insert-mode pair-aware backspace by interpreting insert-mode `DeleteBackward` through the action envelope (test: unit test)
  - [x] **4.4** Keep normal-mode `DeleteBackward` behavior unchanged (test: unit test)

### Repeat Handling
- [x] **5.** Keep dot-repeat and repeat-state behavior compatible with the action envelope
  - [x] **5.1** Store repeat state using the new action envelope semantics without depending on removed insert-only variants (test: unit test)
  - [x] **5.2** Ensure repeat replay preserves source mode and destination mode behavior where applicable (test: unit test)

### Test Coverage
- [x] **6.** Add and adjust regression coverage for the refactor
  - [x] **6.1** Update editor tests to cover the new action envelope and mode metadata rules (test: unit tests pass)
  - [x] **6.2** Update window tests for paired insertion, closer skipping, paired backspace, and normal-mode delete behavior (test: unit tests pass)
  - [x] **6.3** Add regression tests for mode transition handling, undo, redo, and dot-repeat under the new model (test: unit tests pass)

### Verification
- [x] **7.** Validate the refactor end to end
  - [x] **7.1** Run `cargo check` and fix any compilation or lint regressions (test: passes cleanly)
  - [x] **7.2** Run `cargo test` and fix any behavioral regressions uncovered by the new action model (test: all pass)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Action Model | 1 | 1 | 100% |
| Mode Construction | 1 | 1 | 100% |
| Dispatch and Mode Switching | 1 | 1 | 100% |
| Window Editing | 1 | 1 | 100% |
| Repeat Handling | 1 | 1 | 100% |
| Test Coverage | 1 | 1 | 100% |
| Verification | 1 | 1 | 100% |
| **Total** | **7** | **7** | **100%** |
