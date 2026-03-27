# Bracket Text Objects

## Summary

Add Vim-compatible text objects for selecting text inside and around matching bracket pairs. The feature should work through operator-pending mode and support the standard delimiter families used by Vim bracket objects.

## Problem Statement

urvim currently supports only word-based text objects (`iw`, `aw`).

That leaves a gap for common Vim editing workflows where users expect to operate on text inside or around matching delimiters such as parentheses, square brackets, curly braces, and angle brackets.

Without bracket text objects, users must fall back to manual cursor movement or repeated motions, which slows down editing and makes urvim feel incomplete for Vim-style text-object workflows.

## User Stories

- As a Vim user, I want to delete or change text inside matching delimiters, so that I can edit structured code quickly.
- As a Vim user, I want to delete or change text around matching delimiters, so that I can remove both the delimiters and the enclosed content in one command.
- As a urvim user, I want bracket text objects to behave like Vim's bracket objects, so that familiar commands work the way I expect.

## Functional Requirements

- [ ] **REQ-001**: Urvim shall recognize operator-pending text-object sequences for inner and around bracket regions using the standard delimiter families supported by Vim-style bracket objects.
- [ ] **REQ-002**: Inner bracket text objects shall select the text between the matching delimiters and exclude the delimiters themselves.
- [ ] **REQ-003**: Around bracket text objects shall select the matching delimiters together with the enclosed text.
- [ ] **REQ-004**: The supported delimiter families shall include parentheses, square brackets, curly braces, and angle brackets, with Vim-compatible alias keys accepted where applicable.
- [ ] **REQ-005**: Bracket text objects shall resolve the innermost matching pair that encloses the cursor when nested delimiter pairs are present.
- [ ] **REQ-006**: Bracket text objects shall work when the matching delimiters span multiple lines.
- [ ] **REQ-007**: Bracket text objects shall preserve the existing operator-pending flow so they can be used with supported operators such as delete and change.
- [ ] **REQ-008**: Existing count parsing shall continue to apply to bracket text-object operations using the same multiplicative count rules already used for other operator-pending actions.
- [ ] **REQ-009**: If the cursor is not inside a matching delimiter pair, bracket text-object resolution shall try the next matching pair that starts on the current line before failing.
- [ ] **REQ-010**: If no valid matching delimiter pair can be resolved on the current line, the operation shall leave the buffer unchanged.
- [ ] **REQ-011**: Successful bracket text-object operations shall leave the cursor at the start of the affected range, consistent with existing delete and change behavior.

## Non-Functional Requirements

- **Compatibility**: The feature should match Vim's bracket text-object expectations closely enough that common commands feel familiar to experienced Vim users.
- **Reliability**: Matching must be deterministic for nested delimiters, empty delimiter pairs, and multi-line regions.
- **Usability**: Inner and around variants should behave predictably across all supported delimiter families.

## Acceptance Criteria

- [ ] **AC-001**: On `foo(bar)baz`, `di(` removes `bar` and leaves `foo()baz`.
- [ ] **AC-002**: On `foo(bar)baz`, `da(` removes `(bar)` and leaves `foobaz`.
- [ ] **AC-003**: On `foo[bar]baz`, `di[` removes `bar` and leaves `foo[]baz`.
- [ ] **AC-004**: On `foo{bar}baz`, `da{` removes `{bar}` and leaves `foobaz`.
- [ ] **AC-005**: On nested delimiters such as `one (two (three) four) five`, the selected region resolves to the innermost matching pair that encloses the cursor.
- [ ] **AC-006**: On a multi-line bracketed region, the inner and around objects select across line boundaries rather than stopping at the current line.
- [ ] **AC-007**: When the cursor is outside a matching delimiter pair but the current line contains a later valid pair, the selection resolves against that next pair on the current line.
- [ ] **AC-008**: When the cursor is outside a matching delimiter pair and no valid pair exists later on the current line, the operation does not modify the buffer.
- [ ] **AC-009**: Bracket text-object operations participate in the existing undo/redo model as a single logical edit when they succeed.

## Out of Scope

- Sentence, paragraph, or tag text objects
- Visual-mode text object selection
- New operators beyond the existing supported operator-pending commands
- Changing the semantics of non-text-object motions

## Assumptions

- The existing operator-pending architecture will be reused rather than replaced.
- Vim-compatible alias handling is limited to the delimiter families explicitly included in this feature.
- The current supported operators are sufficient for the first release of these text objects.

## Dependencies

- Existing operator-pending action flow
- Existing count parsing behavior
- Existing buffer deletion and undo/redo infrastructure
- Existing cursor and multi-line text navigation primitives
