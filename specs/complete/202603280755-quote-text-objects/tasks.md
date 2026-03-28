# Quote Text Objects - Implementation Tasks

## Overview

Total: 7 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Extend the text-object model for quote families
  - [x] **1.1** Add a public `QuoteKind` enum to [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) for single quote, double quote, and backtick families (test: compile)
  - [x] **1.2** Add `InnerQuote(QuoteKind)` and `AroundQuote(QuoteKind)` variants to `TextObject` in [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) (test: compile)
  - [x] **1.3** Re-export any new public types from [`src/editor/mod.rs`](/Users/ryan/Dev/urvim/src/editor/mod.rs) if needed (test: compile)

- [x] **2.** Register quote text-object key sequences in normal mode
  - [x] **2.1** Add `di'`/`da'` and `ci'`/`ca'` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.2** Add `di"`/`da"` and `ci"`/`ca"` bindings in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.3** Add backtick-family bindings for `di\``/`da\`` and `ci\``/`ca\`` in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.4** Preserve partial-sequence waiting behavior for operator prefixes like `d` and `c` (test: unit test)

- [x] **3.** Add quote-aware text-object resolution in the buffer layer
  - [x] **3.1** Introduce focused quote-matching helpers in [`src/buffer/quote_text_object.rs`](/Users/ryan/Dev/urvim/src/buffer/quote_text_object.rs) or a similarly focused helper module (test: compile)
  - [x] **3.2** Add a public API to resolve inner and around quote text-object ranges with count support (test: unit test)
  - [x] **3.3** Reuse the existing `TextObjectRange` contract for all resolved selections (test: unit test)

- [x] **4.** Implement quote matching and selection semantics
  - [x] **4.1** Resolve the innermost valid enclosing quote pair for the requested family (test: unit test)
  - [x] **4.2** Ignore escaped quote delimiters when identifying matching spans (test: unit test)
  - [x] **4.3** Support multi-line quote spans without truncating the selection to a single line (test: unit test)
  - [x] **4.4** Fall back to the next valid pair that starts on the current line when the cursor is outside a pair (test: unit test)
  - [x] **4.5** Return no range for unmatched, malformed, or ambiguous quote structures (test: unit test)

- [x] **5.** Route operator execution through the new quote text-object API
  - [x] **5.1** Update window command handling to resolve quote text-object ranges through the buffer API (depends on: 3.2)
  - [x] **5.2** Preserve delete/change cursor placement and undo snapshot behavior (test: unit test)

- [x] **6.** Update user-facing docs and glossary
  - [x] **6.1** Document the new quote text objects in [`docs/motions.md`](/Users/ryan/Dev/urvim/docs/motions.md) (depends on: 2.1, 2.2, 2.3)
  - [x] **6.2** Add glossary terms in [`specs/glossary.md`](/Users/ryan/Dev/urvim/specs/glossary.md) for quote text objects and related variants

- [x] **7.** Add and run verification coverage
  - [x] **7.1** Add editor key-sequence tests in [`src/editor/tests.rs`](/Users/ryan/Dev/urvim/src/editor/tests.rs) for the supported quote families and prefix waiting
  - [x] **7.2** Add buffer tests in [`src/buffer/tests.rs`](/Users/ryan/Dev/urvim/src/buffer/tests.rs) for inner, around, escaped, nested, multi-line, and unmatched cases
  - [x] **7.3** Add window-level execution tests covering at least one successful inner edit and one no-op unmatched case
  - [x] **7.4** Run `cargo check` and targeted tests; fix regressions before marking the feature complete

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 6 | 6 | 100% |
| Testing | 1 | 1 | 100% |
| **Total** | **7** | **7** | **100%** |
