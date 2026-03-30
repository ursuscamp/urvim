# Syntax Filetype Metadata - Implementation Tasks

## Overview
Refactor filetype handling so syntax grammar metadata becomes the single source of truth for canonical names, display labels, filename matching, and shebang matching. Remove the filetype enum, migrate all built-in syntax definitions, and update buffer/UI consumers to read syntax metadata directly.

## Backend
- [x] **1.** Replace the filetype enum with syntax metadata and registry lookups
  - [x] **1.1** Add a metadata model to syntax definitions for `name`, `display_name`, `filename`, and `shebang`.
  - [x] **1.2** Update the TOML loader to parse `[metadata]` and compile the regex lists.
  - [x] **1.3** Add deterministic filename and shebang resolution helpers to the syntax registry.
  - [x] **1.4** Preserve canonical syntax-name lookup for tokenizer and nested syntax references.

- [x] **2.** Migrate all built-in syntax files to the new metadata layout
  - [x] **2.1** Update each built-in syntax TOML file to use lowercase canonical names under `[metadata]`.
  - [x] **2.2** Add `display_name` values that match the old user-facing labels.
  - [x] **2.3** Move existing filename and shebang detection rules into `filename` and `shebang` regex lists.
  - [x] **2.4** Add built-in syntax files for every filetype previously covered by the enum, using empty rule sets where no highlighting rules are needed yet.
  - [x] **2.5** Update any internal syntax references to use canonical syntax names.

- [x] **3.** Update buffer state and syntax cache handling to use syntax metadata
  - [x] **3.1** Remove filetype storage from `Buffer` and replace it with canonical syntax identity or equivalent metadata-backed state.
  - [x] **3.2** Refresh resolved syntax when buffer path or shebang content changes.
  - [x] **3.3** Update syntax cache invalidation so it keys off the new syntax identity.
  - [x] **3.4** Keep plain-text fallback behavior stable when no syntax matches.

- [x] **4.** Update UI consumers to display syntax metadata labels
  - [x] **4.1** Replace filetype label accessors with syntax display-name accessors in buffer and window views.
  - [x] **4.2** Update layout/status bar rendering to use the syntax metadata display name.
  - [x] **4.3** Preserve the existing footer order and modified-marker behavior.

## Testing
- [x] **5.** Add regression coverage for metadata-based filetype resolution and display
  - [x] **5.1** Add tests for filename matching, including extensions and special filenames such as `Dockerfile`, `Makefile`, and `Justfile`.
  - [x] **5.2** Add tests for shebang matching, including `env -S` style wrappers.
  - [x] **5.3** Add tests that the status bar shows the syntax display name rather than a removed enum label.
  - [x] **5.4** Add tests for empty-rule-set syntaxes to ensure metadata-only syntax files still resolve and display correctly.
  - [x] **5.5** Add tests for fallback behavior when no syntax metadata matches.

- [x] **6.** Verify the refactor with project checks
  - [x] **6.1** Run `cargo check` and fix resulting compile errors or warnings.
  - [x] **6.2** Run the relevant unit test suite for buffer, syntax, window, and layout behavior.
  - [x] **6.3** Fix any clippy issues surfaced by the refactor.

## Completion Summary
| Area | Status |
| --- | --- |
| Backend | Complete |
| Testing | Complete |
| Documentation | Complete |
| Overall | Complete |
