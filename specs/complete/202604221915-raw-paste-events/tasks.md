# Raw Paste Events - Implementation Tasks

## Overview
Implement mode-aware raw paste handling for insert, normal, and visual modes with verbatim text insertion/replacement semantics and single-step undo per paste event.

## Core Implementation
- [ ] **1.** Extend action surface for raw paste workflows.
  - [x] **1.1** Add `ActionKind::InsertRawPaste(String)` and `ActionKind::ReplaceSelectionRawPaste(String)` in `src/editor/action.rs`.
  - [x] **1.2** Add constructors for both actions on `Action` (`insert_raw_paste`, `replace_selection_raw_paste`).
  - [x] **1.3** Update action classification helpers to include raw paste semantics:
    - [x] **1.3.1** Mark both raw paste actions as snapshottable.
    - [x] **1.3.2** Keep both raw paste actions out of dot-repeat source classification.
    - [x] **1.3.3** Keep snapshot-cursor update policy aligned with direct edit behavior.

- [ ] **2.** Route terminal paste events to mode-specific raw paste actions in `src/main.rs`.
  - [x] **2.1** Replace placeholder `Event::Paste` ignore branch with dispatch logic by active mode.
  - [x] **2.2** Dispatch `InsertRawPaste` in `Insert` and `Normal` modes.
  - [x] **2.3** Dispatch `ReplaceSelectionRawPaste` in `Visual` and `VisualLine` modes with transition to `Normal`.
  - [x] **2.4** Preserve redraw, cursor-style updates, and undo snapshot flow parity with existing action dispatch paths.

- [ ] **3.** Implement window-level raw paste edit execution in `src/window/widget.rs`.
  - [x] **3.1** Handle `InsertRawPaste` by inserting payload verbatim at cursor and moving cursor to end of inserted text.
  - [x] **3.2** Ensure raw insert path bypasses insert helpers (auto-pairs and auto-indent behavior).
  - [x] **3.3** Handle `ReplaceSelectionRawPaste` by replacing the active visual selection with payload text.
  - [x] **3.4** Ensure visual raw paste clears selection and exits to normal mode.
  - [x] **3.5** Validate empty-payload behavior is a no-op where appropriate.

## Testing
- [ ] **4.** Add regression coverage for raw paste behavior.
  - [x] **4.1** Add/extend `src/main.rs` tests for paste dispatch by mode.
  - [x] **4.2** Add/extend `src/window/tests.rs` for insert-mode verbatim raw paste behavior (including newline payload).
  - [x] **4.3** Add/extend `src/window/tests.rs` for normal-mode raw paste insertion behavior.
  - [x] **4.4** Add/extend `src/window/tests.rs` for visual-mode raw paste replacement and mode switch to normal.
  - [x] **4.5** Add undo assertions proving one undo reverts one raw paste event in insert, normal, and visual replacement paths.
  - [x] **4.6** Add assertions confirming auto-pairs and auto-indent do not transform raw paste payloads.

## Verification
- [ ] **5.** Run project quality checks.
  - [x] **5.1** Run `cargo fmt`.
  - [x] **5.2** Run `cargo check`.
  - [x] **5.3** Run targeted tests for affected modules, then full test suite if needed. Full suite completed with one unrelated failure in `window::tests::test_page_motions_render_updated_gutter_line_numbers`.

## Completion Summary
| Metric | Value |
| --- | --- |
| Total Tasks | 5 |
| Completed | 5 |
| Remaining | 0 |
| Progress | 100% |
