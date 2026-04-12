# Line Indent Controls
## Summary
Add line-wise indentation controls for normal mode and insert mode. Normal mode should support shifting the current line range left and right with `<<` and `>>`, while insert mode should support quick dedent behavior from `Shift-Tab` and backspace when the cursor is at the start of a line's indentation.

## Problem Statement
Users often need to adjust indentation after writing or restructuring code. urvim can already infer indentation for newly created lines, but it still lacks a direct way to shift existing lines left or right, or to quickly back out of indentation while staying in insert mode. Without these controls, users must leave the line and retype whitespace manually.

## User Stories
- As a user editing code, I want `<<` and `>>` in normal mode, so that I can shift line indentation left or right without retyping whitespace.
- As a user adjusting several lines at once, I want those normal-mode indentation commands to respect a count, so that I can reindent a block efficiently.
- As a user typing in insert mode, I want `Shift-Tab` to dedent the current line, so that I can correct indentation without leaving insert mode.
- As a user typing in insert mode, I want backspace at the start of indentation to remove indentation one step at a time, so that repeated backspace presses can walk a line back to its leftmost alignment.

## Functional Requirements
- [ ] **REQ-001**: The editor must support a normal-mode command that decreases the indentation of one or more lines.
- [ ] **REQ-002**: The editor must support a normal-mode command that increases the indentation of one or more lines.
- [ ] **REQ-003**: The normal-mode decrease and increase indentation commands must be triggered by `<<` and `>>`.
- [ ] **REQ-004**: The normal-mode indentation commands must support a count that applies the shift to multiple lines.
- [ ] **REQ-005**: When a normal-mode indentation command cannot remove more indentation from a line, it must leave that line at its current leftmost alignment rather than failing.
- [ ] **REQ-006**: Normal-mode indentation commands must preserve the buffer contents of the selected lines except for the leading indentation that is added or removed.
- [ ] **REQ-007**: In insert mode, pressing `Shift-Tab` on a line must decrease that line's indentation.
- [ ] **REQ-008**: Insert-mode `Shift-Tab` must work even when the cursor is not positioned at the first character of the line.
- [ ] **REQ-009**: In insert mode, pressing backspace while the cursor is within the leading indentation of a line must decrease indentation one step at a time.
- [ ] **REQ-010**: Repeated insert-mode backspace presses at the start of indentation must continue to remove indentation until the line is no longer indented, after which backspace must resume normal character deletion behavior.
- [ ] **REQ-011**: Insert-mode indentation reduction commands must remain within insert mode and must not switch the editor back to normal mode.
- [ ] **REQ-012**: Indentation shifting must use the editor's current indentation step or detected indentation width rather than deleting or inserting arbitrary amounts of whitespace.

## Non-Functional Requirements
- [ ] **NFR-001**: The feature must behave deterministically for the same buffer contents, cursor position, and count.
- [ ] **NFR-002**: The feature must preserve unrelated text and avoid altering non-indentation content.
- [ ] **NFR-003**: The feature must remain compatible with buffers that use spaces, tabs, or mixed indentation.
- [ ] **NFR-004**: The behavior must remain predictable in insert mode, even when the cursor is inside the line rather than at its first non-whitespace character.

## Acceptance Criteria
- [ ] **AC-001**: In normal mode, `<<` reduces the indentation of the current line.
- [ ] **AC-002**: In normal mode, `>>` increases the indentation of the current line.
- [ ] **AC-003**: In normal mode, a count shifts that many lines using the same command.
- [ ] **AC-004**: When a line has less indentation than the requested left shift, the command removes only the available indentation.
- [ ] **AC-005**: In insert mode, `Shift-Tab` dedents the current line without leaving insert mode.
- [ ] **AC-006**: In insert mode, backspace at the start of indentation removes one indent step and repeated presses continue dedenting the line.
- [ ] **AC-007**: Insert-mode backspace outside the leading indentation continues to behave like ordinary text deletion.
- [ ] **AC-008**: None of these commands alter the line's non-indentation text.

## Out of Scope
- Visual block or range selection indentation commands.
- Filetype-specific or syntax-aware indentation heuristics beyond the editor's current indent detection.
- Reformatting an entire file automatically.
- Changing the buffer's underlying tab or indentation configuration.

## Assumptions
- The editor already has a way to detect or infer the indentation unit used by the current buffer.
- `<<` and `>>` act on line indentation rather than on arbitrary text columns.
- Insert-mode `Shift-Tab` should behave like a reverse-indent action instead of inserting a literal tab character.
- Backspace-based dedenting should apply only when the cursor is in the leading indentation region of the line.

## Dependencies
- Existing buffer mutation helpers that can adjust the leading whitespace of one or more lines.
- Normal-mode key handling for `<<` and `>>`.
- Insert-mode key handling for `Shift-Tab` and backspace.
- The editor's current indentation detection logic.
