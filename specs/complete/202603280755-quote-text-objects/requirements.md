# Quote Text Objects

## Summary

Add Vim-style text objects for selecting text inside and around matching quote pairs. The feature should work through operator-pending mode and support the standard quote delimiters used by Vim-like quote objects.

## Problem Statement

urvim currently supports word text objects and bracket text objects, but it still lacks quote text objects.

That leaves a gap for common editing workflows where users expect to operate on text inside or around quoted strings and quoted fragments using familiar Vim commands such as `di"`, `da"`, `di'`, and `da'`.

Without quote text objects, users must manually move the cursor to quote boundaries or rely on repeated motions, which slows down editing and makes urvim feel less complete for Vim-style text-object workflows.

## User Stories

- As a Vim user, I want to delete text inside quotes without manually selecting the region, so that I can edit quoted strings quickly.
- As a Vim user, I want to delete text around quotes including the delimiters, so that I can remove quoted fragments in one command.
- As a urvim user, I want quote text objects to behave like Vim-style quote objects, so that familiar commands work the way I expect.

## Functional Requirements

- [ ] **REQ-001**: Urvim shall recognize operator-pending text-object sequences for inner and around quote regions using the standard quote delimiters supported by Vim-style quote objects.
- [ ] **REQ-002**: The supported quote delimiters shall include single quote, double quote, and backtick.
- [ ] **REQ-003**: Inner quote text objects shall select only the text between matching quote delimiters and exclude the delimiters themselves.
- [ ] **REQ-004**: Around quote text objects shall select the matching quote delimiters together with the enclosed text.
- [ ] **REQ-005**: Quote text objects shall resolve the innermost valid matching quote pair that encloses the cursor when nested or overlapping valid pairs are present.
- [ ] **REQ-006**: Quote text objects shall respect escaped quote characters so that escaped delimiters inside the quoted region do not terminate the match.
- [ ] **REQ-007**: Quote text objects shall work when the matching quote pair spans multiple lines.
- [ ] **REQ-008**: If the cursor is not inside a valid quote pair, quote text-object resolution shall try the next valid pair that starts on the current line before failing.
- [ ] **REQ-009**: If no valid quote pair can be resolved, the operation shall leave the buffer unchanged.
- [ ] **REQ-010**: Existing count parsing shall continue to apply to quote text-object operations using the same multiplicative count rules already used for other operator-pending actions.
- [ ] **REQ-011**: Successful quote text-object operations shall leave the cursor at the start of the affected range, consistent with existing delete and change behavior.

## Non-Functional Requirements

- **Compatibility**: The feature should match Vim-style quote text-object expectations closely enough that common commands feel familiar to experienced Vim users.
- **Reliability**: Matching must be deterministic for nested quotes, escaped delimiters, empty quoted regions, and multi-line regions.
- **Usability**: Inner and around variants should behave predictably across all supported quote delimiters.

## Acceptance Criteria

- [ ] **AC-001**: On `foo "bar" baz`, `di"` removes `bar` and leaves `foo "" baz`.
- [ ] **AC-002**: On `foo "bar" baz`, `da"` removes `"bar"` and leaves `foo  baz`.
- [ ] **AC-003**: On `foo 'bar' baz`, `di'` removes `bar` and leaves `foo '' baz`.
- [ ] **AC-004**: On ``foo `bar` baz``, `di\`` removes `bar` and leaves ``foo `` baz``.
- [ ] **AC-005**: On `foo "say \\\"hi\\\"" baz`, `di"` treats the escaped quotes as part of the quoted text instead of ending the match early.
- [ ] **AC-006**: On nested or overlapping valid quote regions, the selected region resolves to the innermost valid pair that encloses the cursor.
- [ ] **AC-007**: When the cursor is outside a valid quote pair but the current line contains a later valid pair, the selection resolves against that next pair on the current line.
- [ ] **AC-008**: When the cursor is outside a valid quote pair and no valid pair exists later on the current line, the operation does not modify the buffer.
- [ ] **AC-009**: Quote text-object operations participate in the existing undo/redo model as a single logical edit when they succeed.

## Out of Scope

- Visual-mode text object selection
- Sentence, paragraph, or tag text objects
- New operators beyond the existing supported operator-pending commands
- Changing the semantics of non-text-object motions

## Assumptions

- The existing operator-pending architecture will be reused rather than replaced.
- Quote delimiters are limited to the standard single-quote, double-quote, and backtick families for the first release.
- Escaped delimiters are treated as literal content inside the quoted region.
- The current supported operators are sufficient for the first release of these text objects.

## Dependencies

- Existing operator-pending action flow
- Existing count parsing behavior
- Existing buffer deletion and undo/redo infrastructure
- Existing cursor and multi-line text navigation primitives
