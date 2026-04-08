# Line Comment Toggle - Implementation Tasks
## Overview
Implement a Vim-style `gcc` line-comment toggle command, wire it through normal-mode input handling, and extend syntax metadata so each supported filetype can declare its canonical line comment prefix. Add regression coverage for supported, unsupported, and multi-line count cases.

## Backend
- [x] **1.** Extend syntax metadata to carry an optional `comment_prefix`. (depends on: none)
  - [x] **1.1** Add the new field to raw and compiled syntax metadata structures.
  - [x] **1.2** Parse and validate the field in the syntax loader.
  - [x] **1.3** Update built-in syntax TOML files with the correct prefix for supported filetypes.
  - [x] **1.4** Add or update syntax-loading tests that cover the new metadata field.

- [x] **2.** Implement the line-comment toggle editing path. (depends on: 1)
  - [x] **2.1** Add a new action kind and constructor for line comment toggling.
  - [x] **2.2** Bind `gcc` in the normal-mode keymap.
  - [x] **2.3** Add window or buffer helpers that toggle a line using the active syntax prefix.
  - [x] **2.4** Ensure count prefixes apply the toggle across consecutive lines.
  - [x] **2.5** Preserve indentation and remove an existing prefix cleanly when toggling off.

## Testing
- [x] **3.** Add regression coverage for the new command and metadata. (depends on: 1, 2)
  - [x] **3.1** Add unit tests for line-toggle behavior on commented and uncommented lines.
  - [x] **3.2** Add tests that verify `gcc` and count-prefixed `gcc` dispatch through normal mode.
  - [x] **3.3** Add tests for syntaxes with and without line comment prefixes.
  - [x] **3.4** Run `cargo check` and the relevant test targets to confirm the change is clean.

## Documentation
- [x] **4.** Update project documentation for the new command and syntax metadata. (depends on: 1, 2)
  - [x] **4.1** Document `gcc` and its count behavior in `docs/motions.md`.
  - [x] **4.2** Document the `comment_prefix` syntax metadata field in `docs/syntax/grammar.md`.
  - [x] **4.3** Update any relevant syntax docs or examples that mention the metadata layout.

## Completion Summary
| Item | Status | Notes |
| --- | --- | --- |
| 1. Syntax metadata support | Done | Optional line comment prefix field and builtin updates |
| 2. Toggle action and keybind | Done | New `gcc` action path |
| 3. Regression tests | Done | Metadata, command, and fallback coverage |
| 4. Documentation | Done | Motions and syntax grammar docs |
