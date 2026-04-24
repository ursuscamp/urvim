# GS Surround Operations - Implementation Tasks

## Overview

Implement normal-mode surround manipulation under `gs` with:

- `gsr` (replace surrounding pair)
- `gsd` (delete surrounding pair)

The implementation should reuse existing pair-resolution behavior where possible, preserve no-op safety on invalid inputs, and include regression tests for cross-line and nested-pair behavior.

## Core Implementation

- [x] **1.** Add surround operation action and delimiter-family data model wiring
  - [x] **1.1** Add public delimiter-family enum and selector parsing helpers for `() [] {} <> " ' \`` with opener/closer symmetry for brackets
  - [x] **1.2** Add new `ActionKind` variants for surround replace/delete payloads
  - [x] **1.3** Add action-constructor helpers (if needed by current style) and ensure new variants participate in any existing action metadata behavior

- [x] **2.** Bind normal-mode key sequences for `gsr` and `gsd`
  - [x] **2.1** Register `gsd` + one selector key to dispatch delete-surround action
  - [x] **2.2** Register `gsr` + two selector keys to dispatch replace-surround action
  - [x] **2.3** Ensure unsupported selector keys resolve to no-op behavior rather than partial/invalid mutations

- [x] **3.** Implement buffer surround execution APIs
  - [x] **3.1** Add public buffer methods for replace/delete surround around cursor
  - [x] **3.2** Resolve nearest enclosing target-family pair across line boundaries using existing bracket/quote matching semantics
  - [x] **3.3** Apply delimiter-only mutations while preserving enclosed text and cursor validity
  - [x] **3.4** Handle no-op cases: missing pair, invalid selectors, same-family replacement

- [x] **4.** Integrate action dispatch with buffer mutation flow
  - [x] **4.1** Add dispatcher handling for surround replace/delete actions in main event processing
  - [x] **4.2** Ensure successful surround edits produce a single logical undo step
  - [x] **4.3** Ensure failed surround actions do not alter buffer, cursor, or undo history

## Testing

- [x] **5.** Add/extend unit tests for selector parsing and keymap decoding
  - [x] **5.1** Verify opener/closer symmetry for bracket selector parsing
  - [x] **5.2** Verify quote selector parsing and unsupported-key rejection
  - [x] **5.3** Verify `gsr`/`gsd` key sequences map to expected action payloads

- [x] **6.** Add/extend buffer/action regression tests for surround behavior
  - [x] **6.1** Replace surround for bracket-to-bracket and bracket-to-quote cases
  - [x] **6.2** Delete surround for quote and bracket cases
  - [x] **6.3** Nested pair resolution picks nearest enclosing pair
  - [x] **6.4** Cross-line pair resolution works for replace/delete
  - [x] **6.5** No-op behavior for missing surrounding pair and invalid selectors
  - [x] **6.6** Single-undo restoration after successful surround mutation

## Documentation and Validation

- [x] **7.** Update motion/keybinding docs for new `gs` surround commands
  - [x] **7.1** Update `docs/motions.md` with `gsr` and `gsd` usage, selectors, and examples
  - [x] **7.2** Document cross-line behavior and no-op failure semantics

- [x] **8.** Run project checks and formatting
  - [x] **8.1** Format edited Rust files
  - [x] **8.2** Run targeted tests for modified modules
  - [x] **8.3** Run `cargo check` and address warnings/regressions

## Completion Summary

| Metric | Value |
|---|---:|
| Total Tasks | 8 |
| Completed | 8 |
| In Progress | 0 |
| Blocked | 0 |
| Not Started | 0 |
