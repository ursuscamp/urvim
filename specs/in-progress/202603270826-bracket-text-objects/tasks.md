# Bracket Text Objects - Implementation Tasks

## Overview

Total: 7 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Extend the text-object model for bracket families
  - [x] **1.1** Add bracket-aware variants to `TextObject` in [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) (test: compile)
  - [x] **1.2** Add a small public enum or equivalent type for the supported delimiter families in [`src/editor/action.rs`](/Users/ryan/Dev/urvim/src/editor/action.rs) (test: compile)
  - [x] **1.3** Re-export any new public types from [`src/editor/mod.rs`](/Users/ryan/Dev/urvim/src/editor/mod.rs) if needed (test: compile)

- [x] **2.** Register bracket text-object key sequences in normal mode
  - [x] **2.1** Add `di(`/`da(` and Vim-compatible aliases for the parenthesis family in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.2** Add `di[`/`da[` and Vim-compatible aliases for the square-bracket family in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.3** Add `di{`/`da{` and Vim-compatible aliases for the curly-brace family in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.4** Add `di<`/`da<` and Vim-compatible aliases for the angle-bracket family in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs) (test: unit test)
  - [x] **2.5** Preserve prefix-wait behavior for partial bracket sequences and operator prefixes (test: unit test)

- [x] **3.** Add bracket-aware text-object resolution in the buffer layer
  - [x] **3.1** Introduce focused delimiter matching helpers in [`src/buffer/text_object.rs`](/Users/ryan/Dev/urvim/src/buffer/text_object.rs) or a dedicated helper module if needed (test: compile)
  - [x] **3.2** Add public APIs to resolve inner and around bracket text-object ranges with count support (test: unit test)
  - [x] **3.3** Reuse the existing `TextObjectRange` contract for all resolved selections (test: unit test)

- [x] **4.** Implement delimiter matching and selection semantics
  - [x] **4.1** Resolve innermost enclosing pairs for nested delimiters (test: unit test)
  - [x] **4.2** Support multi-line delimiter spans without truncating the selection to a single line (test: unit test)
  - [x] **4.3** Handle empty pairs and adjacent pairs without overlapping the wrong region (test: unit test)
  - [x] **4.4** Return no range for unmatched or malformed delimiter structures (test: unit test)

- [x] **5.** Route operator execution through the new bracket text-object API
  - [x] **5.1** Update window command handling to resolve bracket text-object ranges through the buffer API (depends on: 3.2)
  - [x] **5.2** Preserve delete/change cursor placement and undo snapshot behavior (test: unit test)

- [x] **6.** Update user-facing docs and glossary
  - [x] **6.1** Document the new bracket text objects in [`docs/motions.md`](/Users/ryan/Dev/urvim/docs/motions.md) (depends on: 2.1, 2.2, 2.3, 2.4)
  - [x] **6.2** Add or update glossary terms in [`specs/glossary.md`](/Users/ryan/Dev/urvim/specs/glossary.md) to describe bracket text objects and related aliases (test: doc review)

- [x] **7.** Add and run verification coverage
  - [x] **7.1** Add editor key-sequence tests in [`src/editor/tests.rs`](/Users/ryan/Dev/urvim/src/editor/tests.rs) for the supported bracket families and prefix waiting
  - [x] **7.2** Add buffer tests in [`src/buffer/tests.rs`](/Users/ryan/Dev/urvim/src/buffer/tests.rs) for inner, around, nested, multi-line, and unmatched cases
  - [x] **7.3** Add window-level execution tests covering at least one successful inner edit and one no-op unmatched case
  - [x] **7.4** Run `cargo check` and targeted tests; fix regressions before marking the feature complete

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 6 | 6 | 100% |
| Testing | 1 | 1 | 100% |
| **Total** | **7** | **7** | **100%** |
