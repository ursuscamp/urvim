# Markdown Fence Ghost Text - Implementation Tasks
## Overview
Fix the duplicated closing delimiter rendering in Markdown fenced code blocks so
the close fence appears exactly once and does not produce ghost punctuation in
`fixtures/syntax/markdown.md`.

## Backend
- [x] **1.** Trace and fix the closing-fence span path
  - [x] **1.1** Inspect the injected-region closing span emitted from `src/buffer/syntax.rs` and confirm the span bounds match the actual closing delimiter bytes
  - [x] **1.2** Verify `src/window/view.rs` does not duplicate or preserve a stale chunk when adjacent syntax spans touch the closing fence
  - [x] **1.3** Apply the smallest fix that makes the closing fence render once without changing non-Markdown injected-region behavior

- [x] **2.** Keep the Markdown fence definition aligned with the regression case
  - [x] **2.1** Confirm `src/syntax_builtin/markdown.toml` still represents fenced code blocks as a delimited injected region with punctuation styling
  - [x] **2.2** Preserve the existing known-alias and unstyled fallback behavior for `js` and unknown fence languages

## Testing
- [x] **3.** Add regression coverage for the duplicate closing fence
  - [x] **3.1** Add or update a buffer syntax test that loads a Markdown fence and asserts the closing delimiter is highlighted once with the expected fence style
  - [x] **3.2** Add a fixture-focused assertion for `fixtures/syntax/markdown.md` so the `js` fence reproduces the bug case
  - [x] **3.3** Add a guard test to ensure the fix does not change the Rust and WAT fenced block behavior already covered by existing tests

- [x] **4.** Validate the fix
  - [x] **4.1** Run the targeted buffer and window tests covering Markdown syntax spans and rendering
  - [x] **4.2** Run `cargo check` to verify the build and catch warnings

## Completion Summary
| Area | Tasks | Done |
| --- | --- | --- |
| Backend | 2 | 2 |
| Testing | 2 | 2 |
| Total | 4 | 4 |
