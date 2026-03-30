# Syntax Aliases - Implementation Tasks
## Overview
Add alias metadata to syntax definitions, teach the registry to resolve canonical names and aliases, populate well-known aliases across the built-in syntax set, and verify injected syntax selectors use the new lookup path.

## Backend
- [x] **1.** Extend syntax metadata and registry lookup to support aliases
  - [x] **1.1** Add an `alias` field to syntax metadata parsing and compiled metadata structures.
  - [x] **1.2** Normalize and validate alias labels during load, including trimming and duplicate checks.
  - [x] **1.3** Add alias ownership tracking in the registry and reject collisions with canonical names or other alias labels.
  - [x] **1.4** Keep canonical-name lookup behavior unchanged for existing call sites.
- [x] **2.** Update injected syntax resolution to use alias-aware lookup
  - [x] **2.1** Route injected label resolution through the alias-aware registry path.
  - [x] **2.2** Preserve the existing fallback behavior for unknown injected labels.
  - [x] **2.3** Keep label resolution deterministic for capture-based selectors and fixed selectors.
- [x] **3.** Populate built-in alias lists for the shipped syntax set
  - [x] **3.1** Add alias metadata to each supported built-in syntax definition.
  - [x] **3.2** Ensure the alias table covers the common injected labels used in Markdown code fences and similar content.
  - [x] **3.3** Verify the built-in set does not introduce duplicate alias ownership.

## Testing
- [x] **4.** Add regression coverage for alias resolution
  - [x] **4.1** Add registry tests covering canonical-name lookup, alias lookup, and duplicate-alias rejection.
  - [x] **4.2** Add buffer or syntax tests for Markdown fences using aliases such as `js` and `ts`.
  - [x] **4.3** Add tests for unknown injected labels continuing to use the configured fallback.
- [x] **5.** Validate the full crate
  - [x] **5.1** Run `cargo test` for the syntax and buffer test suites.
  - [x] **5.2** Run `cargo check` to confirm the build and warnings remain clean.

## Completion Summary
| Item | Status |
| --- | --- |
| Backend tasks | Done |
| Testing tasks | Done |
| Total | 5 / 5 completed |
