# Surround Add Keybind

## Summary

Add `gsa` surround-add keybindings that wrap a resolved text object or active visual selection in a delimiter pair.

## Problem Statement

urvim already supports surround commands for replacing and deleting existing surrounding delimiter pairs, but it does not provide a command for adding a new surrounding pair around text that is not already wrapped.

Without surround-add support, users must manually insert both delimiters around words, text objects, and visual selections. This is slower than the existing Vim-style surround workflow and makes common edits such as quoting a word or wrapping a selected expression more error-prone.

## User Stories

- As a Vim-style editor user, I want to press `gsa{text object}{delimiter}` in normal mode, so that I can wrap a text object without manually moving to both ends.
- As a urvim user, I want to press `gsa{delimiter}` in visual mode, so that I can wrap an active character-wise selection.
- As a urvim user, I want to press `gsa{delimiter}` in visual line mode, so that I can wrap an entire selected line range.
- As a Vim-style editor user, I want surround-add to use the same delimiter selectors as existing surround commands, so that the `gs` surround family feels consistent.

## Functional Requirements

- [ ] **REQ-001**: urvim shall recognize `gsa` in normal mode as the add-surround command prefix.
- [ ] **REQ-002**: After normal-mode `gsa`, urvim shall read a text object sequence followed by exactly one delimiter keystroke.
- [ ] **REQ-003**: Normal-mode `gsa{text object}{delimiter}` shall surround the range resolved by the text object with the selected delimiter pair.
- [ ] **REQ-004**: Normal-mode `gsa` shall support the same text object families currently available to operator-pending commands.
- [ ] **REQ-005**: urvim shall recognize `gsa` in character-wise visual mode as the add-surround command prefix.
- [ ] **REQ-006**: After visual-mode `gsa`, urvim shall read exactly one delimiter keystroke.
- [ ] **REQ-007**: Visual-mode `gsa{delimiter}` shall surround the active character-wise selection with the selected delimiter pair.
- [ ] **REQ-008**: urvim shall recognize `gsa` in visual line mode as the add-surround command prefix.
- [ ] **REQ-009**: After visual-line-mode `gsa`, urvim shall read exactly one delimiter keystroke.
- [ ] **REQ-010**: Visual-line-mode `gsa{delimiter}` shall surround the active selected line range with the selected delimiter pair.
- [ ] **REQ-011**: When visual-line-mode `gsa{delimiter}` succeeds and `auto_indent` is enabled, urvim shall indent only the originally selected lines by one existing indentation step.
- [ ] **REQ-012**: Visual-line-mode auto-indentation shall leave the inserted delimiter lines unindented relative to the selected line range.
- [ ] **REQ-013**: The supported delimiter families shall match existing surround commands: parentheses `()`, square brackets `[]`, curly braces `{}`, angle brackets `<>`, double quotes `"`, single quotes `'`, and backticks `` ` ``.
- [ ] **REQ-014**: Delimiter selection shall be symmetric for bracket families, so either opener or closer keystrokes resolve to the same family.
- [ ] **REQ-015**: For quote families, the quote character shall select its own family.
- [ ] **REQ-016**: Surround-add shall preserve the selected or resolved inner text exactly, inserting only the opening and closing delimiters, except for the specified visual-line auto-indentation behavior.
- [ ] **REQ-017**: If the text object cannot resolve, the command shall leave the buffer unchanged.
- [ ] **REQ-018**: If the delimiter keystroke is unsupported, the command shall leave the buffer unchanged.
- [ ] **REQ-019**: If the pending command sequence is canceled, the command shall leave the buffer unchanged.
- [ ] **REQ-020**: Successful surround-add edits shall participate in undo/redo as a single logical edit.
- [ ] **REQ-021**: Successful visual-mode and visual-line-mode surround-add edits shall return the editor to normal mode.

## Non-Functional Requirements

- **Compatibility**: Behavior should align with the existing `gs` surround command family and current text object semantics.
- **Reliability**: Failed operations must be deterministic no-ops that do not modify buffer text, cursor state, selection state, or undo history.
- **Usability**: The key sequence should feel consistent across normal, visual, and visual-line modes.
- **Maintainability**: Delimiter family parsing should reuse or share existing surround delimiter behavior so future delimiter changes do not diverge across surround commands.

## Acceptance Criteria

- [ ] **AC-001**: Given `hello world` with the cursor on `hello`, pressing `gsaiw"` changes the text to `"hello" world`.
- [ ] **AC-002**: Given `hello world` with the cursor on `hello`, pressing `gsaiw)` changes the text to `(hello) world`.
- [ ] **AC-003**: Given `foo bar baz` with `bar` selected in character-wise visual mode, pressing `gsa"` changes the text to `foo "bar" baz`.
- [ ] **AC-004**: Given `foo bar baz` with `bar` selected in character-wise visual mode, pressing `gsa]` changes the text to `foo [bar] baz`.
- [ ] **AC-005**: Given two whole lines selected in visual line mode, pressing `gsa{` inserts `{` before the selected line range and `}` after the selected line range while preserving the selected text.
- [ ] **AC-006**: Given two whole lines selected in visual line mode and `auto_indent = "neighbor"`, pressing `gsa{` inserts delimiter lines and indents the originally selected lines by one existing indentation step.
- [ ] **AC-007**: Given two whole lines selected in visual line mode and `auto_indent = "off"`, pressing `gsa{` inserts delimiter lines without changing the indentation of the originally selected lines.
- [ ] **AC-008**: Given a supported bracket family selector, opener and closer keys produce the same surround pair.
- [ ] **AC-009**: Given an unsupported delimiter selector after `gsa`, the buffer and undo history remain unchanged.
- [ ] **AC-010**: Given a normal-mode `gsa` command whose text object cannot resolve, the buffer and undo history remain unchanged.
- [ ] **AC-011**: Given a pending `gsa` command canceled with Escape, the buffer and undo history remain unchanged.
- [ ] **AC-012**: Given a successful normal-mode, visual-mode, or visual-line-mode surround-add edit, a single undo restores the exact prior buffer state.
- [ ] **AC-013**: `docs/motions.md` documents the normal-mode, visual-mode, and visual-line-mode `gsa` forms, including visual-line auto-indentation behavior.
- [ ] **AC-014**: `cargo check` passes after implementation.

## Out of Scope

- Changing the behavior of existing `gsr` replace-surround or `gsd` delete-surround commands
- Adding new delimiter families beyond the existing surround delimiter set
- Adding delete or replace surround workflows for visual selections
- Changing text object resolution semantics beyond what is needed to consume existing text objects after `gsa`
- Adding configurable surround keybindings

## Assumptions

- Existing text object resolution can be reused for normal-mode `gsa{text object}{delimiter}`.
- Existing surround delimiter parsing can be reused or shared by the new add-surround command.
- The current undo infrastructure can represent a two-sided insertion as one logical edit.
- Visual line mode has a stable selected line range that can be used as the surround target.
- The existing indentation step used by `>>` is the correct definition of one indentation level for visual-line surround-add.

## Dependencies

- Existing normal-mode keymap and pending-sequence infrastructure for multi-key commands
- Existing visual-mode and visual-line-mode action handling
- Existing text object resolution for operator-style commands
- Existing surround delimiter family support
- Existing indentation step and auto-indent configuration support
- Buffer edit primitives for inserting opening and closing delimiters around a range
- Existing undo/redo tracking infrastructure
