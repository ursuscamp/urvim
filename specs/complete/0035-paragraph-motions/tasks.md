# Paragraph Motions - Implementation Tasks

## Overview

Total: 18 tasks
Estimated phases: Buffer methods, Action enum, Window integration, Mode integration, Testing, Documentation

## Buffer Methods

- [x] **1.** Add `is_blank_line()` helper method in `src/buffer.rs`
  - [x] **1.1** Add method that checks if a line is empty or whitespace-only (test: unit tests)
  - [x] **1.2** Add unit tests for `is_blank_line()`

- [x] **2.** Add `cursor_paragraph_backward()` method in `src/buffer.rs`
  - [x] **2.1** Add method signature `pub fn cursor_paragraph_backward(&self, cursor: Cursor) -> Option<Cursor>` (test: compiles)
  - [x] **2.2** Implement blank-line detection logic (test: behavior matches vim)
  - [x] **2.3** Write unit tests for `cursor_paragraph_backward()`

- [x] **3.** Add `cursor_paragraph_forward()` method in `src/buffer.rs`
  - [x] **3.1** Add method signature `pub fn cursor_paragraph_forward(&self, cursor: Cursor) -> Option<Cursor>` (test: compiles)
  - [x] **3.2** Implement blank-line detection logic (test: behavior matches vim)
  - [x] **3.3** Write unit tests for `cursor_paragraph_forward()`

## Action Enum

- [x] **4.** Add new Action variants in `src/editor.rs`
  - [x] **4.1** Add `MoveToPreviousParagraph` variant (test: compiles)
  - [x] **4.2** Add `MoveToNextParagraph` variant (test: compiles)

- [x] **5.** Update `is_countable()` to include paragraph motions
  - [x] **5.1** Add `MoveToPreviousParagraph` to countable matches (test: is_countable returns true)
  - [x] **5.2** Add `MoveToNextParagraph` to countable matches (test: is_countable returns true)

- [x] **6.** Update `resets_remembered_column()` to include paragraph motions
  - [x] **6.1** Add paragraph motions to vertical motion matches (test: resets_remembered_column returns true)

- [x] **7.** Update `uses_remembered_column()` to include paragraph motions
  - [x] **7.1** Add paragraph motions to vertical motion matches (test: uses_remembered_column returns true)

## Mode Integration

- [x] **8.** Add keybindings in `src/editor.rs`
  - [x] **8.1** Add `{` → `MoveToPreviousParagraph` binding in `NormalMode::new()` (test: key produces correct action)
  - [x] **8.2** Add `}` → `MoveToNextParagraph` binding in `NormalMode::new()` (test: key produces correct action)

## Window Integration

- [x] **9.** Add `move_cursor_to_previous_paragraph()` method in `src/window.rs`
  - [x] **9.1** Add method signature and implementation (test: compiles)
  - [x] **9.2** Call `buffer.cursor_paragraph_backward()` (test: moves cursor correctly)
  - [x] **9.3** Handle `None` return (cursor stays in place) (test: stays in place when no previous paragraph)

- [x] **10.** Add `move_cursor_to_next_paragraph()` method in `src/window.rs`
  - [x] **10.1** Add method signature and implementation (test: compiles)
  - [x] **10.2** Call `buffer.cursor_paragraph_forward()` (test: moves cursor correctly)
  - [x] **10.3** Handle `None` return (cursor stays in place) (test: stays in place when no next paragraph)

- [x] **11.** Add `process_action` handlers in `src/window.rs`
  - [x] **11.1** Handle `Action::MoveToPreviousParagraph` (test: executes motion correctly)
  - [x] **11.2** Handle `Action::MoveToNextParagraph` (test: executes motion correctly)

## Testing

- [x] **12.** Write Buffer unit tests for `is_blank_line()`
  - [x] **12.1** Empty line returns true
  - [x] **12.2** Line with only spaces returns true
  - [x] **12.3** Line with only tabs returns true
  - [x] **12.4** Line with content returns false
  - [x] **12.5** Line with content and trailing whitespace returns false

- [x] **13.** Write Buffer unit tests for `cursor_paragraph_backward()`
  - [x] **13.1** From middle of paragraph, moves to blank line before it
  - [x] **13.2** From blank line, moves to blank line before previous paragraph
  - [x] **13.3** From first paragraph at file start, returns None
  - [x] **13.4** Multiple consecutive blank lines treated as single boundary

- [x] **14.** Write Buffer unit tests for `cursor_paragraph_forward()`
  - [x] **14.1** From middle of paragraph, moves to blank line after it
  - [x] **14.2** From blank line, moves to blank line after next paragraph
  - [x] **14.3** From last paragraph at file end, returns None
  - [x] **14.4** Multiple consecutive blank lines treated as single boundary

- [x] **15.** Write Window/editor integration tests
  - [x] **15.1** `{` key produces correct action
  - [x] **15.2** `}` key produces correct action
  - [x] **15.3** Count prefix works (e.g., `5{` → `Count(5, MoveToPreviousParagraph)`)
  - [x] **15.4** Column preservation works for paragraph motions

## Documentation

- [x] **16.** Update `docs/motions.md`
  - [x] **16.1** Add `}` to Supported Motions Cheat Sheet (test: documentation exists)
  - [x] **16.2** Add `{` to Supported Motions Cheat Sheet (test: documentation exists)
  - [x] **16.3** Add Paragraph Motions section with description
  - [x] **16.4** Add examples for `{` and `}` usage
  - [x] **16.5** Document count support

## Final Verification

- [x] **17.** Run full test suite
  - [x] **17.1** `cargo test` passes
  - [x] **17.2** `cargo check` passes with no warnings

- [x] **18.** Clippy and code review
  - [x] **18.1** `cargo clippy` passes with no warnings
  - [x] **18.2** Code follows project conventions (method ordering, comments)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Buffer Methods | 3 | 3 | 100% |
| Action Enum | 4 | 4 | 100% |
| Mode Integration | 1 | 1 | 100% |
| Window Integration | 3 | 3 | 100% |
| Testing | 4 | 4 | 100% |
| Documentation | 1 | 1 | 100% |
| Final Verification | 2 | 2 | 100% |
| **Total** | **18** | **18** | **100%** |