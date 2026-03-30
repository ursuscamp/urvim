# Syntax Config-Driven Regions - Implementation Tasks

## Overview

Total: 7 tasks
Estimated completion: 2-4 days
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Add the syntax config schema, loader, and registry
  - [x] **1.1** Create the public syntax registry module and export the registry, definition, rule, and load error types
  - [x] **1.2** Define the TOML-backed raw schema for syntax files, including filetypes, ordered regions, and optional nested references
  - [x] **1.3** Implement built-in syntax discovery and loading from TOML files into an in-memory registry
  - [x] **1.4** Validate duplicate names, duplicate aliases, malformed region data, and unknown style/category references during load

- [x] **2.** Implement region matching and line-tokenization infrastructure
  - [x] **2.1** Replace the hardcoded tokenizer entry point with a registry-backed syntax engine
  - [x] **2.2** Implement regex-backed token regions and start/end delimited regions
  - [x] **2.3** Implement multiline region state propagation across lines
  - [x] **2.4** Implement `terminator_guard` handling so closing delimiters are recognized only when the guard does not match
  - [x] **2.5** Preserve existing syntax span categories and renderer-facing `SyntaxSpan` output

- [x] **3.** Add nested syntax injection and Markdown code-fence support
  - [x] **3.1** Add nested syntax resolution by name for delimited regions
  - [x] **3.2** Implement code-fence detection in Markdown as a nested-region use case
  - [x] **3.3** Support language lookup from fence info strings and fall back cleanly when the language is unknown
  - [x] **3.4** Ensure nested syntax state is preserved in the cache so embedded code highlights correctly across lines

- [x] **4.** Migrate the currently supported syntax rules into TOML files
  - [x] **4.1** Create TOML syntax definitions for Rust, JavaScript, TypeScript, JSON, TOML, Markdown, Python, and shell-family filetypes
  - [x] **4.2** Encode the current highlight categories for keywords, constants, types, functions, comments, numbers, operators, punctuation, strings, and variables in the new schema
  - [x] **4.3** Preserve the existing multiline behaviors for block comments, triple-quoted strings, template strings, and fenced code blocks
  - [x] **4.4** Ensure filetype aliases map to the same loaded syntax definition where the current highlighter shares behavior

- [x] **5.** Keep incremental invalidation and cache behavior intact
  - [x] **5.1** Update the buffer syntax cache to store and restore the new syntax engine state
  - [x] **5.2** Keep line-based invalidation from the changed line forward for edits, filetype changes, and nested-region transitions
  - [x] **5.3** Verify that unchanged prefixes are not reparsed after an edit that occurs later in the buffer

- [x] **6.** Add and update tests for syntax loading and highlighting behavior
  - [x] **6.1** Test registry loading, duplicate detection, and invalid TOML rejection
  - [x] **6.2** Test delimiter guarding, multiline region continuation, and nested region handoff
  - [x] **6.3** Test Markdown fenced code injection with a known nested syntax
  - [x] **6.4** Test the migrated supported filetypes against representative samples for the existing token classes
  - [x] **6.5** Test that edits still invalidate downstream cached syntax and refresh highlighted spans correctly

- [x] **7.** Verify builds, warnings, and regressions
  - [x] **7.1** Run `cargo check` and fix compile errors or warnings
  - [x] **7.2** Run the focused syntax highlighting test suite
  - [x] **7.3** Run the full test suite before marking the work complete

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 5 | 5 | 100% |
| Testing | 2 | 2 | 100% |
| **Total** | **7** | **7** | **100%** |
