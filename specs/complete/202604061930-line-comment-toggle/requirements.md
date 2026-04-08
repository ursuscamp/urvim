# Line Comment Toggle
## Summary
Add a normal-mode action and key binding for toggling line comments on the current line. The command should use syntax metadata to determine the correct line comment prefix for the active filetype, and it should support count prefixes so multiple consecutive lines can be toggled in one command.

## Problem Statement
Urvim has editing actions for deleting, changing, and joining lines, but it does not yet have a built-in way to comment out the current line with a familiar Vim-style `gcc` binding. Commenting is highly workflow-specific and depends on the active syntax, so the editor needs filetype metadata to know which prefix to insert or remove.

## User Stories
- As a normal-mode user, I want to press `gcc` to toggle comment state on the current line, so that I can quickly disable or re-enable code while editing.
- As a normal-mode user, I want to prefix `gcc` with a count, so that I can toggle comments across multiple consecutive lines at once.
- As a user editing a supported filetype, I want the editor to use that filetype's line comment prefix, so that the result matches the language's comment syntax.

## Functional Requirements
- [ ] **REQ-001**: The editor shall expose a normal-mode action that toggles the line comment state of the current line.
- [ ] **REQ-002**: The editor shall bind `gcc` in normal mode to the line-comment toggle action.
- [ ] **REQ-003**: The line-comment toggle action shall support count prefixes and apply to the requested number of consecutive lines starting at the current line.
- [ ] **REQ-004**: The editor shall use syntax metadata to determine the active filetype's canonical `comment_prefix`.
- [ ] **REQ-005**: When a target line is not commented, the action shall insert the filetype's `comment_prefix` at the appropriate comment position for that line.
- [ ] **REQ-006**: When a target line is already commented with the filetype's canonical `comment_prefix`, the action shall remove that prefix instead of inserting a second copy.
- [ ] **REQ-007**: The action shall preserve the line's existing indentation when inserting or removing the line comment prefix.
- [ ] **REQ-008**: If the active syntax does not define a `comment_prefix`, the action shall leave the buffer unchanged and report the command as unhandled or unavailable.
- [ ] **REQ-009**: The editor shall keep existing syntax highlighting behavior unchanged for all filetypes not involved in comment toggling.

## Non-Functional Requirements
- [ ] **NFR-001**: The action shall run in time proportional to the number of affected lines.
- [ ] **NFR-002**: The feature shall behave consistently across all supported platforms and terminal sizes.
- [ ] **NFR-003**: The implementation shall avoid corrupting buffer contents when the current line is empty, whitespace-only, or already commented.
- [ ] **NFR-004**: The change shall remain compatible with existing normal-mode key handling, count parsing, and dot-repeat behavior.

## Acceptance Criteria
- [ ] **AC-001**: Pressing `gcc` on a supported filetype comments or uncomments the current line using that filetype's canonical line comment prefix.
- [ ] **AC-002**: Pressing `3gcc` toggles the current line and the next two lines.
- [ ] **AC-003**: A supported line that is already commented is restored to its previous content after a second toggle.
- [ ] **AC-004**: A filetype without a line comment prefix does not gain any text changes when `gcc` is pressed.
- [ ] **AC-005**: Existing syntax highlighting and other normal-mode commands continue to work after the change.

## Out of Scope
- Visual-mode comment toggling.
- Block-comment toggling.
- Commenting arbitrary selections outside the current line range.
- Custom user configuration for comment prefixes beyond syntax metadata.

## Assumptions
- The comment command should operate on whole lines, not on selected character ranges.
- The editor will treat the syntax metadata value as the canonical prefix to add and remove for that filetype.
- If a line is already indented, the comment prefix should be inserted after the indentation rather than before it.
- Existing repeat and count semantics for line-oriented normal-mode actions should be reused where possible.

## Dependencies
- Syntax metadata support for a line comment prefix on each built-in filetype that supports line comments.
- Normal-mode keymap plumbing for the new `gcc` binding.
- Buffer or window editing support that can inspect and rewrite a line while preserving indentation.
