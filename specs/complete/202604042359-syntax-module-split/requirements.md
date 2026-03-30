# Syntax Module Split

## Summary
Split `src/syntax/mod.rs` into smaller focused modules so the syntax subsystem is easier to navigate, test, and extend, without changing the public syntax API or runtime behavior.

## Problem Statement
`src/syntax/mod.rs` currently owns multiple responsibilities at once: load errors, syntax data models, TOML parsing, builtin source registration, metadata normalization, registry lookup, promotion, validation, and a large chunk of tests. That makes the file harder to understand and increases the risk of unrelated changes colliding. The syntax subsystem needs to be organized into smaller modules with clearer ownership boundaries while keeping the existing syntax format and behavior intact.

## User Stories
- As a maintainer, I want syntax loading and registry logic split into smaller files, so that I can understand and change one concern at a time.
- As a maintainer, I want the public syntax API to stay stable, so that existing callers do not need to change.
- As a contributor, I want syntax tests grouped by responsibility, so that failures point to the right layer of the subsystem.
- As a user, I want no visible behavior changes from this refactor, so that syntax highlighting continues to work exactly as before.

## Functional Requirements
- [ ] **REQ-001**: The syntax subsystem shall be split into smaller internal modules with focused responsibilities instead of keeping all loader and registry logic in a single large `mod.rs` file.
- [ ] **REQ-002**: The refactor shall preserve the existing public syntax API surface exposed to the rest of the editor.
- [ ] **REQ-003**: The syntax loader shall continue to parse the same TOML format and produce the same raw and compiled syntax behavior as before.
- [ ] **REQ-004**: The syntax registry shall continue to resolve canonical names, aliases, filename matches, and shebang matches exactly as before.
- [ ] **REQ-005**: Built-in syntax sources shall continue to load from the same embedded TOML files.
- [ ] **REQ-006**: Validation errors for invalid syntax data shall continue to be reported with the same semantics as before.
- [ ] **REQ-007**: The syntax module split shall not change buffer syntax classification, highlighting output, or nested syntax resolution behavior.
- [ ] **REQ-008**: Syntax-related unit tests shall continue to pass after the module split, with tests grouped by the responsibility of the module they cover.

## Non-Functional Requirements
- **Maintainability**: Each new module should have a single primary responsibility and a clear public boundary.
- **Compatibility**: Existing callers of `crate::syntax::*` should not need changes unless the refactor exposes an actual bug.
- **Reliability**: The refactor must not introduce behavior changes in syntax loading, promotion, or validation.
- **Testability**: The new module boundaries should make it easier to test loader and registry behavior independently.

## Acceptance Criteria
- [ ] **AC-001**: `src/syntax/mod.rs` is reduced to a small facade that wires the syntax modules together and re-exports the public API.
- [ ] **AC-002**: Loader, registry, definition, and normalization responsibilities are separated into focused modules.
- [ ] **AC-003**: `cargo check` succeeds after the split.
- [ ] **AC-004**: Syntax registry and loader tests still pass after the split.
- [ ] **AC-005**: Opening and highlighting a representative set of files still produces the same syntax labels and spans as before the refactor.

## Out of Scope
- Changing the TOML syntax grammar format
- Changing syntax highlighting behavior
- Renaming public syntax types or functions unless required by the split
- Adding new syntax features
- Changing builtin grammar contents

## Assumptions
- The current syntax module boundaries can be separated without changing the on-disk TOML shape.
- The public API can remain stable by re-exporting types and functions from the new internal modules.
- The current loader and registry semantics are already correct and only need organizational changes.

## Dependencies
- Existing syntax loader and registry implementation in `src/syntax/mod.rs`
- Existing built-in syntax TOML files under `src/syntax/builtins`
- Existing syntax and buffer regression tests
