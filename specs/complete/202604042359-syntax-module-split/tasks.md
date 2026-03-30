# Syntax Module Split - Implementation Tasks

## Overview
Split `src/syntax/mod.rs` into smaller modules with clear ownership boundaries while preserving the public API, behavior, and tests of the syntax subsystem.

## Backend
- [x] **1.** Extract syntax error and data model types into focused modules
  - [x] **1.1** Move `SyntaxLoadError` into its own module and re-export it from the syntax facade.
  - [x] **1.2** Move syntax definition, metadata, rule, and selector types into a dedicated data-model module.
  - [x] **1.3** Preserve existing doc comments and public visibility on moved types.

- [x] **2.** Extract loader and normalization logic
  - [x] **2.1** Move TOML parsing and raw-to-compiled syntax conversion into a loader module.
  - [x] **2.2** Move label and marker normalization helpers into a shared helper module.
  - [x] **2.3** Keep the same parse, validation, and regex compilation semantics after the move.

- [x] **3.** Extract registry and builtin source wiring
  - [x] **3.1** Move registry state, lookup, promotion, and validation into a registry module.
  - [x] **3.2** Move builtin embedded source wiring into a builtin source module.
  - [x] **3.3** Keep lazy promotion and duplicate-mapping checks behaving exactly as before.

- [x] **4.** Reduce `src/syntax/mod.rs` to a facade
  - [x] **4.1** Declare the new submodules from `mod.rs`.
  - [x] **4.2** Re-export the public syntax API from `mod.rs`.
  - [x] **4.3** Remove moved implementation code from `mod.rs` without changing behavior.

## Testing
- [x] **5.** Rehome and preserve syntax unit tests
  - [x] **5.1** Move loader-focused tests next to the loader implementation.
  - [x] **5.2** Move registry-focused tests next to the registry implementation.
  - [x] **5.3** Keep top-level API regression tests covering registry loading and promotion.

- [x] **6.** Verify the refactor
  - [x] **6.1** Run `cargo check` and fix compile errors or warnings introduced by the split.
  - [x] **6.2** Run targeted syntax registry and loader tests.
  - [x] **6.3** Run relevant buffer/syntax regression tests to confirm no behavior changed.

## Completion Summary
| Area | Status | Notes |
| --- | --- | --- |
| Type extraction | Complete | Error and data-model modules |
| Loader/normalization | Complete | TOML parsing and helpers |
| Registry/builtins | Complete | Lookup, promotion, embedded sources |
| Facade cleanup | Complete | `mod.rs` is now a thin re-export layer |
| Testing | Complete | Module-local tests and regression checks |
