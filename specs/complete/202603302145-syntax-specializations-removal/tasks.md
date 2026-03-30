# Syntax Specializations Removal - Implementation Tasks
## Overview
Remove the syntax schema specializations for block comments, keywords, types, and constants, then update builtin grammars, tests, and fixtures so the remaining highlight behavior is driven by standard delimiter and regex rules. Keep line comments untouched.

## Backend
- [x] **1.** Simplify the syntax schema and loader to remove unsupported special-case fields.
  - [x] **1.1** Update `src/syntax.rs` data models and deserialization structs to drop block comment, keyword, type, and constant fields.
  - [x] **1.2** Remove loader and validation logic that compiles or references the deleted fields.
  - [x] **1.3** Add or update tests that ensure removed fields are rejected by the syntax parser.
- [x] **2.** Remove tokenizer branches that depend on deleted syntax specializations.
  - [x] **2.1** Update `src/buffer/syntax.rs` so block comments are matched only through general delimited regions.
  - [x] **2.2** Remove identifier classification against keyword, type, and constant lists.
  - [x] **2.3** Keep line comment handling unchanged and covered by tests.

## Syntax Data
- [x] **3.** Rewrite builtin syntax definitions to use generic rules only.
  - [x] **3.1** Update `src/syntax_builtin/rust.toml` to replace removed categories with delimiter, regex, keyword, and identifier rules that fit Rust syntax.
  - [x] **3.2** Update `src/syntax_builtin/javascript.toml` to replace removed categories with delimiter, regex, keyword, and identifier rules that fit JavaScript and TypeScript syntax.
  - [x] **3.3** Update `src/syntax_builtin/python.toml` to replace removed categories with delimiter, regex, keyword, and identifier rules that fit Python syntax.
  - [x] **3.4** Update `src/syntax_builtin/shell.toml` to replace removed categories with delimiter, regex, keyword, and identifier rules that fit shell-family syntax.
  - [x] **3.5** Update `src/syntax_builtin/json.toml` and `src/syntax_builtin/toml.toml` so they only use the remaining schema fields and any needed identifier rules.

## Testing
- [x] **4.** Refresh syntax regression coverage.
  - [x] **4.1** Update fixture files under `fixtures/syntax/` if needed to reflect the new rule-driven highlighting behavior.
  - [x] **4.2** Add or update syntax tests for builtin highlighting paths that previously relied on the removed categories.
  - [x] **4.3** Run `cargo check` and the relevant syntax/window tests to confirm the new schema and highlighting behavior compile cleanly.

## Completion Summary
| Section | Status | Notes |
| --- | --- | --- |
| Backend | Complete | Schema and tokenizer updated |
| Syntax Data | Complete | Builtin TOML files rewritten |
| Testing | Complete | `cargo check` and `cargo test` passed |
