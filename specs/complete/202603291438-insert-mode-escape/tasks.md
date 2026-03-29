# Insert Mode Escape Binding - Implementation Tasks
## Overview
Total: 6 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Extend startup config with an optional insert-mode escape binding
  - [x] **1.1** Add `insert_escape: Option<String>` to `PartialConfig` and resolved `Config` with documentation comments.
  - [x] **1.2** Update config resolution so the new field is loaded from TOML and carried into the resolved config alongside `theme`.
  - [x] **1.3** Validate the field during config loading so empty, whitespace-only, or malformed canonical key strings fail startup with a clear error.
  - [x] **1.4** Add config tests for default `None`, valid values, and invalid values.

- [x] **2.** Teach insert mode to read the alternate escape binding from globals
  - [x] **2.1** Update `InsertMode::new()` so it reads the resolved config through `globals::with_config`.
  - [x] **2.2** Register the configured alternate escape binding in the insert-mode trie keymap when present, while keeping `<Esc>` bound unconditionally.
  - [x] **2.3** Preserve existing insert-mode bindings, repeat capture behavior, and mode-switch behavior for the built-in escape key.
  - [x] **2.4** Add insert-mode tests for the default escape path and a configured alternate escape path.

- [x] **3.** Add reusable key-string validation for config-driven bindings
  - [x] **3.1** Introduce a fallible canonical key-string validation path that can surface parse errors instead of panicking.
  - [x] **3.2** Reuse the same validation logic for the insert escape config field so config errors and keymap parsing stay aligned.
  - [x] **3.3** Add unit tests covering valid multi-key sequences, invalid unterminated tokens, and empty strings.

- [x] **4.** Keep the config and glossary docs synchronized
  - [x] **4.1** Update `docs/config.md` to document the new `insert_escape` option, its canonical-string format, and its additive relationship to `<Esc>`.
  - [x] **4.2** Keep `specs/glossary.md` aligned with the new insert-mode escape binding terminology if the wording changes during implementation.

- [x] **5.** Verify editor behavior end to end
  - [x] **5.1** Add or update integration-style tests to confirm `<Esc>` still exits insert mode when a custom binding is configured.
  - [x] **5.2** Add tests that confirm a configured escape sequence exits insert mode without inserting literal text.
  - [x] **5.3** Confirm the custom binding has no effect in normal mode.

- [x] **6.** Run checks and fix regressions
  - [x] **6.1** Run `cargo check` and fix compile errors or warnings.
  - [x] **6.2** Run the targeted test suite for config, keymap parsing, globals, and editor mode handling.
  - [x] **6.3** Run the full test suite before marking the work complete.

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 4 | 4 | 100% |
| Docs | 1 | 1 | 100% |
| Testing | 1 | 1 | 100% |
| **Total** | **6** | **6** | **100%** |
