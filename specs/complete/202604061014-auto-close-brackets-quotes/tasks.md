# Auto Close Brackets and Quotes - Implementation Tasks

## Overview

Total: 9 tasks
Estimated completion: 1 day
Prerequisites: None

## Implementation Tasks

### Configuration and Documentation

- [x] **1.** Add the `auto_close_pairs` startup config option (depends on: none)
  - [x] **1.1** Extend `Config` and `PartialConfig` with a boolean pairing flag that defaults to `true`
  - [x] **1.2** Wire config parsing so the option loads from TOML and is preserved in the resolved config
  - [x] **1.3** Add config-loading tests for default-on behavior and explicit `false` behavior

- [x] **2.** Document the new config option in `docs/config.md` (depends on: 1)
  - [x] **2.1** Add the option to the canonical schema list and example TOML block
  - [x] **2.2** Describe the supported delimiter pairs and the insert-mode-only behavior

### Insert-Mode Pairing

- [x] **3.** Add a shared delimiter-pair helper in `src/editor/` (depends on: 1)
  - [x] **3.1** Define the six supported pairs: parentheses, square brackets, curly braces, double quotes, single quotes, and backticks
  - [x] **3.2** Provide opener-to-closer and closer-to-opener lookup helpers
  - [x] **3.3** Add unit tests for the supported pair lookups

- [x] **4.** Teach `InsertMode` to auto-close supported openers and skip supported closers (depends on: 1.3, 3)
  - [x] **4.1** Read the resolved config flag when constructing insert mode
  - [x] **4.2** Convert supported opener keypresses into a single `InsertText` action containing both delimiters
  - [x] **4.3** Detect a matching closer at the cursor and emit a cursor-right action instead of inserting a duplicate closer
  - [x] **4.4** Preserve existing special-key behavior for `<Esc>`, `<Backspace>`, `<Delete>`, and configured insert escape bindings
  - [x] **4.5** Add insert-mode tests for opener insertion, closer skipping, and disabled pairing

### Pair-Aware Backspace

- [x] **5.** Implement pair-aware backspace removal in the window edit path (depends on: 3)
  - [x] **5.1** Detect when the cursor sits between a supported opener and closer
  - [x] **5.2** Remove both characters as one buffer edit when pairing is enabled
  - [x] **5.3** Fall back to the existing single-character backspace behavior otherwise
  - [x] **5.4** Add unit tests for pair deletion, non-pair deletion, and disabled pairing

### Undo and Redo

- [x] **6.** Verify paired edits stay atomic for undo/redo (depends on: 4, 5)
  - [x] **6.1** Confirm opener insertion restores the exact pre-edit state with one undo
  - [x] **6.2** Confirm pair deletion restores the exact pre-edit state with one undo
  - [x] **6.3** Confirm redo reapplies the paired edit without producing duplicate delimiters
  - [x] **6.4** Add regression tests around snapshot timing and cursor restoration

### Polish and Validation

- [x] **7.** Update public docs/comments touched by the feature (depends on: 1, 3, 4, 5)
  - [x] **7.1** Add or update doc comments for any new public config or editor API surface
  - [x] **7.2** Keep terminology aligned with the project glossary and existing config wording

- [x] **8.** Run `cargo check` and fix build or warning issues (depends on: 1, 3, 4, 5)
  - [x] **8.1** Resolve any compile errors introduced by the new config field or insert-mode logic
  - [x] **8.2** Address warnings so the feature lands cleanly

- [x] **9.** Perform a focused manual verification pass (depends on: 4, 5, 6, 8)
  - [x] **9.1** Verify typing an opener inserts the matching closer and positions the cursor between them
  - [x] **9.2** Verify typing a supported closer next to an auto-inserted closer skips over it
  - [x] **9.3** Verify backspace between a supported pair removes both characters
  - [x] **9.4** Verify undo and redo behave as a single logical edit for paired insertion and deletion

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Configuration and Documentation | 2 | 2 | 100% |
| Insert-Mode Pairing | 2 | 2 | 100% |
| Pair-Aware Backspace | 1 | 1 | 100% |
| Undo and Redo | 1 | 1 | 100% |
| Polish and Validation | 3 | 3 | 100% |
| **Total** | **9** | **9** | **100%** |
