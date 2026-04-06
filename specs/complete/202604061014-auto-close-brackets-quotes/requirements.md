# Auto Close Brackets and Quotes

## Summary
Add a startup config option that enables automatic bracket and quote pairing while editing. When enabled by default, typing an opening delimiter inserts its matching closer and places the cursor between them, typing a matching closer next to an auto-inserted closer skips over it instead of inserting a duplicate character, and backspacing an opening delimiter deletes its paired closer when the cursor is between the pair.

## Problem Statement
Users expect fast, low-friction delimiter entry when editing code and prose. Urvim currently requires manual insertion and cleanup of matching brackets and quotes, which is slower and more error-prone. The editor should provide the common auto-pairing workflow without breaking undo and redo behavior.

## User Stories
- As a user, I want opening brackets and quotes to auto-insert their matching closers, so that I can type balanced text faster.
- As a user, I want typing a closing delimiter next to its matching auto-inserted closer to move past the closer, so that I do not create duplicates.
- As a user, I want backspace to remove both sides of an auto-inserted pair when the cursor is between them, so that cleanup feels natural.
- As a user, I want the feature to be configurable and enabled by default, so that I can turn it off if I prefer plain insertion behavior.
- As a user, I want undo and redo to restore delimiter edits cleanly, so that paired insertion and deletion behave like a single edit.

## Functional Requirements
- [ ] **REQ-001**: The editor must expose a user-facing startup config option that controls automatic bracket and quote pairing.
- [ ] **REQ-002**: The automatic pairing option must default to enabled.
- [ ] **REQ-003**: When automatic pairing is enabled and the user types one of the supported opening delimiters in insert mode, the editor must insert the matching closing delimiter and place the cursor between the two delimiters.
- [ ] **REQ-004**: The supported delimiter pairs must be parentheses, square brackets, curly braces, double quotes, single quotes, and backticks.
- [ ] **REQ-005**: When automatic pairing is enabled and the cursor is immediately before an auto-inserted matching closer, typing that closer must move the cursor past the closer instead of inserting another copy.
- [ ] **REQ-006**: When automatic pairing is enabled and the cursor is between an opening delimiter and its matching auto-inserted closer, backspace must delete both delimiters as a single user-visible edit.
- [ ] **REQ-007**: The editor must treat the six supported pairs independently so that each opening delimiter inserts and skips only its own matching closer.
- [ ] **REQ-008**: Paired delimiter insertion, closer skipping, and paired backspace deletion must preserve undo and redo behavior so that a single undo step restores the pre-edit buffer state and a single redo step reapplies the edit.
- [ ] **REQ-009**: When automatic pairing is disabled, opening and closing brackets and quotes must behave as plain inserted characters and backspace must only delete the character immediately before the cursor.
- [ ] **REQ-010**: The pairing behavior must be limited to insert mode and must not alter normal-mode command handling.
- [ ] **REQ-011**: The pairing behavior must apply only to the supported bracket and quote delimiters and must not rewrite unrelated characters.
- [ ] **REQ-012**: The feature must not leave stray duplicate closers in the buffer when a matching closer is skipped.

## Non-Functional Requirements
- [ ] **NFR-001**: The feature must remain responsive during normal text entry and should not introduce noticeable input lag.
- [ ] **NFR-002**: The implementation must remain compatible with the editor’s existing undo/redo model.
- [ ] **NFR-003**: The change must not introduce unsafe code.
- [ ] **NFR-004**: The configuration option must be documented alongside the other user-facing startup settings.

## Acceptance Criteria
- [ ] **AC-001**: With the feature enabled, typing `(` in insert mode produces `(|)` with the cursor between the delimiters.
- [ ] **AC-002**: With the feature enabled, typing `[` in insert mode produces a square-bracket pair with the cursor between the delimiters.
- [ ] **AC-003**: With the feature enabled, typing `{` in insert mode produces `{|}` with the cursor between the delimiters.
- [ ] **AC-004**: With the feature enabled, typing `"` in insert mode produces `"|"` with the cursor between the delimiters.
- [ ] **AC-005**: With the feature enabled, typing `'` in insert mode produces `'|'` with the cursor between the delimiters.
- [ ] **AC-006**: With the feature enabled, typing <code>`</code> in insert mode produces <code>`|`</code> with the cursor between the delimiters.
- [ ] **AC-007**: With the feature enabled, typing `)` when the cursor is immediately before an auto-inserted `)` moves the cursor past the closer without inserting a second `)`.
- [ ] **AC-008**: With the feature enabled, pressing backspace between an opening delimiter and its auto-inserted closer removes both characters.
- [ ] **AC-009**: Undo after paired insertion restores the exact pre-insertion buffer and cursor state.
- [ ] **AC-010**: Redo after undo reapplies the paired insertion or paired deletion without producing extra delimiter characters.
- [ ] **AC-011**: With the feature disabled, typing brackets or quotes inserts only the typed character and backspace removes only one character at a time.

## Out of Scope
- Context-sensitive suppression rules for strings, comments, or language-specific delimiter heuristics.
- Smart indentation or newline-aware auto-pairing behavior.
- Visual-mode or normal-mode delimiter transforms.
- User-configurable custom delimiter maps in the initial version.

## Assumptions
- The feature will be configured through the existing TOML startup config.
- The implementation will use the editor’s existing insert-mode key handling and undo/redo infrastructure.
- The initial delimiter set will cover the six explicit pairs listed in REQ-004.
- The default enabled behavior is intended to match common editor expectations unless the user explicitly disables it.

## Dependencies
- Startup configuration parsing and documentation.
- Insert-mode key handling.
- Buffer edit and cursor movement primitives.
- Existing undo/redo machinery.
