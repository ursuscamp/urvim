# GS Surround Operations

## Summary

Add normal-mode surround manipulation keybindings under the `gs` prefix for replacing and deleting surrounding delimiter pairs.

## Problem Statement

urvim currently lacks direct Vim-style surround editing commands for existing paired delimiters.

Without these commands, users must manually navigate to delimiters and edit each side independently, which is slower and error-prone for common structured edits.

## User Stories

- As a Vim-style editor user, I want to replace surrounding delimiters with a different pair, so that I can quickly reshape surrounding structure without manually editing both sides.
- As a Vim-style editor user, I want to delete surrounding delimiters while preserving inner text, so that I can unwrap quoted or bracketed content in one command.
- As a urvim user, I want surround operations to support standard bracket and quote families across lines, so that I can use the same workflow in multi-line code and prose.

## Functional Requirements

- [ ] **REQ-001**: urvim shall recognize `gsr` in normal mode as the replace-surround command prefix.
- [ ] **REQ-002**: After `gsr`, urvim shall read exactly two delimiter keystrokes: the first selects the target surrounding pair family and the second selects the replacement pair family.
- [ ] **REQ-003**: urvim shall recognize `gsd` in normal mode as the delete-surround command prefix.
- [ ] **REQ-004**: After `gsd`, urvim shall read exactly one delimiter keystroke to select the target surrounding pair family.
- [ ] **REQ-005**: The supported delimiter families for `gsr` and `gsd` shall be parentheses `()`, square brackets `[]`, curly braces `{}`, angle brackets `<>`, double quotes `"`, single quotes `'`, and backticks `` ` ``.
- [ ] **REQ-006**: Delimiter selection shall be symmetric for bracket families, so either opener or closer keystrokes resolve to the same family (for example, `(` and `)` both select parentheses).
- [ ] **REQ-007**: For quote families, the quote character shall select its own family.
- [ ] **REQ-008**: Surround pair resolution shall search across line boundaries and shall not be limited to the current line.
- [ ] **REQ-009**: `gsr` shall replace only the two delimiters of the resolved surrounding pair while preserving enclosed text unchanged.
- [ ] **REQ-010**: `gsd` shall delete only the two delimiters of the resolved surrounding pair while preserving enclosed text unchanged.
- [ ] **REQ-011**: If no matching surrounding pair can be resolved for the selected family, the command shall leave the buffer unchanged.
- [ ] **REQ-012**: If a required delimiter keystroke after `gsr` or `gsd` is not a supported delimiter selector, the command shall leave the buffer unchanged.
- [ ] **REQ-013**: If `gsr` selects the same source and replacement family, the command shall leave the buffer unchanged.
- [ ] **REQ-014**: Successful surround edits shall participate in undo/redo as a single logical edit.

## Non-Functional Requirements

- **Compatibility**: Behavior should align with expected Vim-style surround workflows for delimiter-family selection and nearest surrounding-pair edits.
- **Reliability**: Pair resolution and edits must be deterministic for nested delimiters, empty pairs, and multi-line regions.
- **Usability**: Failed operations should be safe no-ops and must not modify buffer state.

## Acceptance Criteria

- [ ] **AC-001**: Given `foo{bar}baz` with the cursor inside `bar`, `gsr{[` changes text to `foo[bar]baz`.
- [ ] **AC-002**: Given `foo{bar}baz` with the cursor inside `bar`, `gsr}[` produces the same result as `gsr{[`.
- [ ] **AC-003**: Given `foo(bar)baz` with the cursor inside `bar`, `gsr)"` changes text to `foo"bar"baz`.
- [ ] **AC-004**: Given `foo"bar"baz` with the cursor inside `bar`, `gsd"` changes text to `foobarbaz`.
- [ ] **AC-005**: Given multi-line content where an opening delimiter is on one line and its matching closing delimiter is on a later line, `gsr` and `gsd` resolve and edit that surrounding pair.
- [ ] **AC-006**: Given nested same-family delimiters and cursor inside the innermost region, operations resolve the nearest enclosing pair.
- [ ] **AC-007**: Given no resolvable surrounding pair for the selected family, `gsr` and `gsd` do not modify the buffer.
- [ ] **AC-008**: Given `gsr` with an unsupported selector key in either selector position, the command does not modify the buffer.
- [ ] **AC-009**: Given `gsd` with an unsupported selector key, the command does not modify the buffer.
- [ ] **AC-010**: Given a successful `gsr` or `gsd`, a single undo restores the exact prior buffer state.

## Out of Scope

- Adding surround-add operations (for example, adding new surrounding delimiters where none exist)
- Adding visual-mode-specific surround workflows
- Supporting delimiter families beyond the listed bracket and quote families
- Changing unrelated normal-mode keybinding semantics outside the `gs` prefix

## Assumptions

- Existing nearest-enclosing pair resolution semantics used by text objects can be reused conceptually for surround operations.
- `gs` keybindings are introduced in normal mode only for this feature slice.
- The editor's current undo infrastructure can represent delimiter-only edits as one logical change.

## Dependencies

- Normal-mode keymap/trie infrastructure for multi-key command sequences
- Delimiter-pair resolution primitives that can operate across lines and nested structures
- Buffer edit primitives for targeted single-character replacement/deletion
- Existing undo/redo tracking infrastructure
