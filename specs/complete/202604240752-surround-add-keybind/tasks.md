# Surround Add Keybind - Implementation Tasks

## Overview

Add `gsa` surround-add support for normal-mode text objects, character-wise Visual selections, and Visual Line selections. Visual Line surround-add should optionally indent only the originally selected lines by one existing indentation step when `auto_indent` is enabled.

## Action Model

- [x] **1.** Extend editor action types for surround-add
  - [x] **1.1** Add `ActionKind::SurroundAdd { target: TextObject, delimiter: DelimiterFamily }` with documentation comments
  - [x] **1.2** Add `ActionKind::SurroundAddSelection { delimiter: DelimiterFamily }` with documentation comments
  - [x] **1.3** Update action metadata helpers for remembered-column reset, motion/mutation classification, snapshot eligibility, and repeat policy
  - [x] **1.4** Add or update action trait unit tests for the new action variants

## Key Bindings

- [x] **2.** Register normal-mode `gsa{text object}{delimiter}` bindings
  - [x] **2.1** Reuse the existing surround delimiter selector list for `gsa` suffixes
  - [x] **2.2** Register word and BigWord text object combinations (`iw`, `aw`, `iW`, `aW`)
  - [x] **2.3** Register bracket text object combinations for all supported bracket selector aliases
  - [x] **2.4** Register quote text object combinations for single quote, double quote, and backtick
  - [x] **2.5** Add normal-mode keymap tests for representative `gsa` text object and delimiter combinations

- [x] **3.** Register visual-mode `gsa{delimiter}` bindings
  - [x] **3.1** Add shared Visual and Visual Line bindings through `VisualModeState`
  - [x] **3.2** Ensure visual surround-add actions carry `to_mode: Some(ModeKind::Normal)`
  - [x] **3.3** Treat visual surround-add as a visual edit command for count handling
  - [x] **3.4** Add visual and visual-line keymap tests for representative delimiter selectors

## Buffer And Window Behavior

- [x] **4.** Add buffer-level surround-add primitives
  - [x] **4.1** Implement character-wise `Buffer::add_surround(range, delimiter)` using later-then-earlier insertion ordering
  - [x] **4.2** Implement linewise `Buffer::add_linewise_surround(start_line, count, delimiter)` with delimiter lines surrounding the selected line range
  - [x] **4.3** Validate empty or invalid ranges as no-ops
  - [x] **4.4** Document linewise insertion ordering and cursor-return semantics
  - [x] **4.5** Add buffer regression tests for character-wise and linewise surround insertion

- [x] **5.** Wire surround-add into window command handling
  - [x] **5.1** Resolve normal-mode text object targets and call `Buffer::add_surround`
  - [x] **5.2** Resolve character-wise Visual selections and call `Buffer::add_surround`
  - [x] **5.3** Resolve Visual Line selections and call `Buffer::add_linewise_surround`
  - [x] **5.4** When `auto_indent` is enabled, indent only the originally selected Visual Line lines by one existing indentation step after successful insertion
  - [x] **5.5** Leave buffer, cursor, selection, and undo history unchanged for unresolved targets or unsupported actions
  - [x] **5.6** Route new action variants from `Window::process_action`

## Testing

- [x] **6.** Add surround-add behavior regression tests
  - [x] **6.1** Normal-mode `gsaiw"` surrounds an inner word with double quotes
  - [x] **6.2** Normal-mode bracket selectors accept both opener and closer keys
  - [x] **6.3** Character-wise Visual `gsa"` surrounds the active selection and exits to Normal
  - [x] **6.4** Character-wise Visual `gsa]` surrounds with square brackets
  - [x] **6.5** Visual Line `gsa{` inserts delimiter lines around the selected line range
  - [x] **6.6** Visual Line `gsa{` with `auto_indent = "neighbor"` indents only the originally selected lines
  - [x] **6.7** Visual Line `gsa{` with `auto_indent = "off"` preserves original selected-line indentation
  - [x] **6.8** Unresolvable normal-mode text objects leave buffer and undo history unchanged
  - [x] **6.9** Successful normal, visual, and visual-line surround-add edits undo in a single step

## Documentation And Verification

- [x] **7.** Update user-facing motion documentation
  - [x] **7.1** Add normal-mode `gsa{text object}{delimiter}` to `docs/motions.md`
  - [x] **7.2** Add Visual and Visual Line `gsa{delimiter}` behavior to `docs/motions.md`
  - [x] **7.3** Document supported delimiter selectors and Visual Line auto-indent behavior

- [x] **8.** Run final project checks
  - [x] **8.1** Format project code after edits
  - [x] **8.2** Run targeted tests for edited modules
  - [x] **8.3** Run `cargo check` and fix build errors or warnings

## Completion Summary

| Area | Tasks | Complete |
| --- | ---: | ---: |
| Action Model | 4 | 4 |
| Key Bindings | 9 | 9 |
| Buffer And Window Behavior | 11 | 11 |
| Testing | 9 | 9 |
| Documentation And Verification | 6 | 6 |
| **Total** | **39** | **39** |
