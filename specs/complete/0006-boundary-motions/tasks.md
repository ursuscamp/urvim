# Boundary-Based Vim Motions - Implementation Tasks

## Overview

Total: 16 tasks
Estimated completion: 2-3 hours
Prerequisites: None

This implementation adds boundary-based vim motions (w, b, e, W, B, E) using a flexible Boundary enum and generic ForwardTo/BackTo actions.

## Buffer Module (Boundary Detection)

- [x] **1.** Create `Boundary` enum in buffer.rs
  - [x] **1.1** Define `Word`, `WordEnd`, `BigWord`, `BigWordEnd` variants (test: enum compiles and has 4 variants)
  - [x] **1.2** Add derive macros: Debug, Clone, Copy, PartialEq, Eq

- [x] **2.** Add character classification helper methods to Buffer
  - [x] **2.1** Add `is_word_char(grapheme: &str) -> bool` method (test: verify alphanumeric and underscore return true)
  - [x] **2.2** Add `is_whitespace_char(grapheme: &str) -> bool` method (test: verify space, tab, newline return true)
  - [x] **2.3** Add `is_bigword_char(grapheme: &str) -> bool` method (test: verify non-whitespace returns true)

- [x] **3.** Implement `is_at_boundary(cursor, boundary)` method
  - [x] **3.1** Handle `Boundary::Word` case (test: cursor at start of word returns true)
  - [x] **3.2** Handle `Boundary::WordEnd` case (test: cursor at end of word returns true)
  - [x] **3.3** Handle `Boundary::BigWord` case (test: cursor at start of BigWord returns true)
  - [x] **3.4** Handle `Boundary::BigWordEnd` case (test: cursor at end of BigWord returns true)
  - [x] **3.5** Test edge cases: start/end of line, buffer boundaries (test: cursor at line start returns correct values)

- [x] **4.** Implement `next_boundary(cursor, boundary)` method
  - [x] **4.1** Handle forward navigation for each boundary type (test: verify moves to next boundary)
  - [x] **4.2** Handle line wrapping (test: moving past end of line continues to next line)
  - [x] **4.3** Handle buffer end (test: returns None when no more boundaries)

- [x] **5.** Implement `prev_boundary(cursor, boundary)` method
  - [x] **5.1** Handle backward navigation for each boundary type (test: verify moves to previous boundary)
  - [x] **5.2** Handle line wrapping backward (test: moving past start of line continues from previous line)
  - [x] **5.3** Handle buffer start (test: returns None when no more boundaries)

## Editor Module (Action Integration)

- [x] **6.** Add Boundary enum export to lib.rs
  - [x] **6.1** Ensure Boundary is re-exported from buffer module (test: can import Boundary from urvim)

- [x] **7.** Add new Action variants to editor.rs
  - [x] **7.1** Add `ForwardTo(Boundary)` variant (test: Action enum has the variant)
  - [x] **7.2** Add `BackTo(Boundary)` variant (test: Action enum has the variant)

- [x] **8.** Update NormalMode key mappings
  - [x] **8.1** Map 'w' to `Action::ForwardTo(Boundary::Word)` (test: pressing w returns correct action)
  - [x] **8.2** Map 'b' to `Action::BackTo(Boundary::Word)` (test: pressing b returns correct action)
  - [x] **8.3** Map 'e' to `Action::ForwardTo(Boundary::WordEnd)` (test: pressing e returns correct action)
  - [x] **8.4** Map 'W' to `Action::ForwardTo(Boundary::BigWord)` (test: pressing W returns correct action)
  - [x] **8.5** Map 'B' to `Action::BackTo(Boundary::BigWord)` (test: pressing B returns correct action)
  - [x] **8.6** Map 'E' to `Action::ForwardTo(Boundary::BigWordEnd)` (test: pressing E returns correct action)

- [x] **9.** Implement action execution for boundary motions
  - [x] **9.1** Handle ForwardTo action in editor/action processing (test: pressing w moves cursor)
  - [x] **9.2** Handle BackTo action in editor/action processing (test: pressing b moves cursor)

## Integration

- [x] **9.** Ensure cargo check passes (test: run `cargo check` with no errors)

- [x] **10.** Run existing tests to ensure no regressions (test: `cargo test` passes)

## Testing

- [x] **11.** Add unit tests for character classification
  - [x] **11.1** Test is_word_char with alphanumeric, underscore, special chars
  - [x] **11.2** Test is_whitespace_char with space, tab, newline, non-whitespace

- [x] **12.** Add unit tests for boundary detection
  - [x] **12.1** Test is_at_boundary for Word boundary at various positions
  - [x] **12.2** Test is_at_boundary for WordEnd boundary at various positions
  - [x] **12.3** Test is_at_boundary for BigWord boundary at various positions
  - [x] **12.4** Test is_at_boundary for BigWordEnd boundary at various positions

- [x] **13.** Add unit tests for boundary navigation
  - [x] **13.1** Test next_boundary with Word boundary
  - [x] **13.2** Test prev_boundary with Word boundary
  - [x] **13.3** Test next_boundary with BigWord boundary
  - [x] **13.4** Test prev_boundary with BigWord boundary

- [x] **14.** Add unit tests for edge cases
  - [x] **14.1** Test empty buffer behavior
  - [x] **14.2** Test single character buffer
  - [x] **14.3** Test cursor at buffer boundaries
  - [x] **14.4** Test line wrapping forward and backward

- [x] **15.** Add unit tests for Unicode handling
  - [x] **15.1** Test with Unicode characters (non-ASCII letters)
  - [x] **15.2** Test with emoji
  - [x] **15.3** Test with combining characters

- [x] **16.** Add integration tests for NormalMode key handling
  - [x] **16.1** Test all six motion keys return correct actions

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Buffer Module | 5 | 5 | 100% |
| Editor Module | 3 | 3 | 100% |
| Integration | 2 | 2 | 100% |
| Action Execution | 1 | 1 | 100% |
| Testing | 6 | 6 | 100% |
| **Total** | **17** | **17** | **100%** |
