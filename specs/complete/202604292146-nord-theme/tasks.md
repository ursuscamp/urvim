# Nord Theme - Implementation Tasks

## Overview

Total: 5 tasks
Estimated completion: 1 day
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Add the built-in Nord theme asset
  - [x] **1.1** Create `src/theme/builtin/nord.toml`
  - [x] **1.2** Define the Nord palette, default style, UI styles, and syntax styles
  - [x] **1.3** Keep the theme faithful to the upstream Nord palette where urvim has equivalent surfaces

- [x] **2.** Register Nord in the built-in theme registry
  - [x] **2.1** Add Nord to the built-in theme source list in `src/theme/model.rs`
  - [x] **2.2** Ensure the registry still loads existing built-in themes unchanged
  - [x] **2.3** Preserve the current default theme and unknown-theme error behavior

- [x] **3.** Update user-facing documentation
  - [x] **3.1** Update `docs/config.md` to mention Nord as an available built-in theme
  - [x] **3.2** Scan for any other built-in-theme lists or examples that should mention Nord

- [x] **4.** Add regression tests
  - [x] **4.1** Test that `ThemeRegistry::load_builtin()` includes Nord
  - [x] **4.2** Test that `--theme Nord` / exact-name selection resolves to the Nord theme
  - [x] **4.3** Test a representative Nord style resolution path for one or more UI/syntax surfaces
  - [x] **4.4** Test that the default built-in theme remains `Friday Night`

- [x] **5.** Verify the change set
  - [x] **5.1** Run `cargo fmt`
  - [x] **5.2** Run `cargo check`
  - [x] **5.3** Run the relevant targeted tests

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 5 | 5 | 100% |
| **Total** | **5** | **5** | **100%** |
