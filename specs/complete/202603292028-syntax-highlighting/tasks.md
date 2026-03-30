# Syntax Highlighting - Implementation Tasks

## Overview
Implement buffer-owned syntax highlighting with line-range invalidation, built-in filetype tokenizers for the initial core set, and renderer integration that turns highlighted spans into render chunks.

## Backend
- [x] **1.** Add syntax cache data structures and accessors to `Buffer`.
  - [x] **1.1** Define the cached per-line syntax span and continuation state models.
  - [x] **1.2** Add buffer methods for invalidating syntax from a line, checking cache readiness, and retrieving spans for rendering.
  - [x] **1.3** Initialize syntax cache state in all buffer constructors and make sure derived state is not included in save output or undo snapshots.
- [x] **2.** Wire syntax invalidation into all buffer mutation and state-reset paths.
  - [x] **2.1** Mark the cache dirty from the earliest affected line for insert, remove, newline split/merge, join, and line deletion operations.
  - [x] **2.2** Ensure undo/redo and filetype refreshes also invalidate or rebuild the cache as needed.
  - [x] **2.3** Add unit tests that verify invalidation behavior after representative edits and state changes.
- [x] **3.** Add the built-in syntax tokenization module.
  - [x] **3.1** Define the internal syntax category and tokenizer state model.
  - [x] **3.2** Implement tokenizers for Rust, Python, JavaScript, TypeScript, Shell, JSON, TOML, and Markdown.
  - [x] **3.3** Preserve multiline continuation state across lines for the supported tokenizers.
  - [x] **3.4** Fall back to plain-text spans for unsupported filetypes.

## Rendering
- [x] **4.** Update window render-data construction to consume syntax spans.
  - [x] **4.1** Split visible lines into multiple render chunks based on cached syntax spans.
  - [x] **4.2** Keep the existing base-style overlay behavior so theme defaults still apply.
  - [x] **4.3** Preserve cursor positioning, scrolling, gutter rendering, and empty-row filling.
- [x] **5.** Ensure syntax styling maps cleanly onto the existing theme system.
  - [x] **5.1** Map syntax categories onto the existing theme syntax styles.
  - [x] **5.2** Keep unsupported or missing styles safe by falling back to base styling.
  - [x] **5.3** Confirm multi-window rendering uses the same buffer syntax state.

## Testing
- [x] **6.** Add buffer and tokenizer tests.
  - [x] **6.1** Test cache initialization and invalidation for each edit path.
  - [x] **6.2** Test tokenizer output for the core filetypes, including multiline cases.
  - [x] **6.3** Test unsupported filetypes and plain-text fallback.
- [x] **7.** Add renderer integration tests.
  - [x] **7.1** Verify highlighted chunks reach the screen with the expected styles.
  - [x] **7.2** Verify rendering remains correct for gutters, cursor placement, and empty rows.
  - [x] **7.3** Verify multiple windows observing the same buffer share the same syntax state.
- [x] **8.** Run project validation.
  - [x] **8.1** Run `cargo check` and fix any warnings or build errors.
  - [x] **8.2** Run the relevant test subset and confirm the new syntax coverage passes.

## Completion Summary

| Area | Tasks | Status |
| --- | --- | --- |
| Backend | 3 | Complete |
| Rendering | 2 | Complete |
| Testing | 3 | Complete |
| Total | 8 | Complete |
