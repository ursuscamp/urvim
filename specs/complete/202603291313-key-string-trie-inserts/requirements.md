# Key String Trie Inserts

## Summary
Urvim should let developers register trie keymap bindings from a single canonical key string instead of building `Vec<String>` sequences by hand. This will make literal key bindings easier to read and maintain while preserving the existing trie behavior.

## Problem Statement
Trie keymap setup currently mixes `insert("x".to_string(), ...)` calls with `insert_sequence(vec![...], ...)` calls. Multi-key bindings are especially noisy because the intended key sequence is obscured by repeated `to_string()` calls and vector construction. The codebase already has a canonical key string representation for runtime keys, so keymap setup should use the same representation directly.

## User Stories
- As a maintainer, I want to register trie bindings from a single string, so that keymap setup reads like the actual key sequence.
- As a developer, I want canonical key strings to be parsed consistently, so that `gg`, `diw`, and `<C-s>` all map to the expected trie tokens.
- As a tester, I want parser coverage for canonical keys and special tokens, so that regressions in binding registration are caught early.

## Functional Requirements
- [ ] **REQ-001**: Provide a trie keymap helper that accepts a single canonical key string and registers the equivalent binding.
- [ ] **REQ-002**: Parse each non-bracketed character in the string as an individual key token, in order.
- [ ] **REQ-003**: Parse each bracketed canonical token, such as `<Esc>` or `<C-s>`, as a single key token.
- [ ] **REQ-004**: Preserve the behavior of existing vector-based trie insertion for equivalent bindings.
- [ ] **REQ-005**: Support both single-key bindings and multi-key bindings through the string-based helper.
- [ ] **REQ-006**: Migrate literal trie keymap registrations in editor setup to the string-based helper where the binding is already known as a canonical string.
- [ ] **REQ-007**: Keep the vector-based insertion path available for call sites that assemble key sequences dynamically.
- [ ] **REQ-008**: Reject malformed canonical strings rather than silently creating the wrong trie binding.

## Non-Functional Requirements
- [ ] **NFR-001**: Maintainability - keymap declarations should become shorter and easier to scan.
- [ ] **NFR-002**: Compatibility - valid bindings must resolve to the same actions as before.
- [ ] **NFR-003**: Reliability - parsing must be deterministic and unambiguous for canonical key strings.
- [ ] **NFR-004**: Testability - the parser and new helper must have focused unit tests.

## Acceptance Criteria
- [ ] **AC-001**: `gg`, `gJ`, and `diw` can be inserted from a single string and resolve to the same actions as the existing vector-based form.
- [ ] **AC-002**: `<Esc>`, `<C-s>`, `<Left>`, `<Space>`, `<LessThan>`, and `<GreaterThan>` each parse as one key token.
- [ ] **AC-003**: The trie keymap still supports the existing `insert` and `insert_sequence` forms for callers that need them.
- [ ] **AC-004**: Literal keymap registrations in editor mode setup use the new string helper instead of manually building vectors.
- [ ] **AC-005**: Existing keymap behavior for normal and insert mode remains unchanged for valid inputs.
- [ ] **AC-006**: Malformed canonical strings are not accepted as valid bindings.

## Out of Scope
- User-configurable keymap files or runtime remapping.
- Changes to `Key::canonical_string()`.
- Changes to the terminal input parser or key event model.
- Reworking other keymap types beyond the minimal literal-call-site cleanup.

## Assumptions
- The string helper consumes the same canonical notation produced by `Key::canonical_string()`.
- Angle-bracketed tokens are the only multi-character key tokens.
- The new helper is intended primarily for internal code paths and literal editor bindings.

## Dependencies
- Existing `Key::canonical_string()` output.
- Current `TrieKeymap` lookup and insertion behavior.
- Existing editor mode setup and keymap tests.
