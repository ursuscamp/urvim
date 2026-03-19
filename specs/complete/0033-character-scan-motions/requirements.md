# Character Scan Motions

## Summary

Implement f/F/t/T character scan motions that allow quick navigation to or past a specified character in the current line, with support for count prefixes to find the Nth occurrence.

## Problem Statement

Users need efficient single-line character navigation similar to vim's f, F, t, and T motions. These motions allow finding a character in the current line and landing on it (f/F) or just before/after it (t/T), enabling rapid movement without word-based navigation.

Currently, the keymap system only supports fixed key sequences. Character scan motions require a two-key sequence where the second key is a runtime parameter (the target character), not a fixed binding.

## User Stories

- **As a** user, **I want to** press `f` followed by a character to **find and land on** the next occurrence of that character in the current line.

- **As a** user, **I want to** press `F` followed by a character to **find and land on** the previous occurrence of that character in the current line.

- **As a** user, **I want to** press `t` followed by a character to **land just before** the next occurrence of that character in the current line.

- **As a** user, **I want to** press `T` followed by a character to **land just after** the previous occurrence of that character in the current line.

- **As a** user, **I want to** prefix any character scan motion with a count (e.g., `3f x`) to **find the Nth occurrence** of the target character.

- **As a** user, **I want** character scan motions to **stay in place** when the target character is not found.

## Functional Requirements

- [ ] **REQ-001**: `f{char}` searches forward for `{char}` in the current line and lands ON the found character. Cursor is positioned at the character's column.

- [ ] **REQ-002**: `F{char}` searches backward for `{char}` in the current line and lands ON the found character. Cursor is positioned at the character's column.

- [ ] **REQ-003**: `t{char}` searches forward for `{char}` in the current line and lands BEFORE the found character (one column to the left).

- [ ] **REQ-004**: `T{char}` searches backward for `{char}` in the current line and lands AFTER the found character (one column to the right).

- [ ] **REQ-005**: All character scan motions search from the character immediately after/before the cursor position (not including current position).

- [ ] **REQ-006**: Count prefixes work with character scan motions (e.g., `3f x` finds the 3rd occurrence of `x`).

- [ ] **REQ-007**: If the target character is not found when searching forward, the cursor does not move.

- [ ] **REQ-008**: If the target character is not found when searching backward, the cursor does not move.

- [ ] **REQ-009**: Character scan motions are case-sensitive (e.g., `fX` searches for uppercase `X`, not lowercase `x`).

- [ ] **REQ-010**: Character scan motions search across the entire line, not stopping at punctuation or other boundaries.

- [ ] **REQ-011**: The keymap system supports character scan triggers (f, F, t, T) that require a second character as a runtime parameter.

- [ ] **REQ-012**: Multiple keymaps can be chained so that trie-based sequences are checked first, then character scan keymap as fallback.

## Non-Functional Requirements

- **Performance**: Character scan search should be O(n) where n is line length. No significant slowdown for long lines.

- **Usability**: Motion should feel immediate - target character entry and cursor movement happen in the same keypress flow as other motions.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `f x` on "hello world" with cursor at `h` moves cursor to `o` (column 4).

- [ ] **AC-002**: Pressing `F h` on "hello world" with cursor at `d` moves cursor to first `h` (column 0).

- [ ] **AC-003**: Pressing `t o` on "hello world" with cursor at `h` moves cursor to `l` (column 3, one before `o`).

- [ ] **AC-004**: Pressing `T h` on "hello world" with cursor at `d` moves cursor to `e` (column 1, one after `h` when going backward).

- [ ] **AC-005**: Pressing `3f l` on "hello l hello l" finds the 3rd `l` occurrence.

- [ ] **AC-006**: Pressing `f z` on "hello" (no `z` exists) cursor stays in place (does not move).

- [ ] **AC-007**: Pressing `F h` on "hello" (cursor at `h`) cursor stays in place (does not move).

- [ ] **AC-008**: Pressing `2f x` with only one `x` in the line finds that single `x`.

- [ ] **AC-009**: `ChainedKeymap.get_action()` returns trie results before checking character scan keymap.

- [ ] **AC-010**: `ChainedKeymap.is_prefix()` returns true if either sub-keymap reports true.

## Out of Scope

- Repeatability with `;` and `,` (will be implemented separately)
- Cross-line character search (character scan limited to current line only - vim behavior)
- Cross-line search with line continuation (vim's `\` option)
- Integration with text objects (e.g., `dt)` - delete until `)`)

## Assumptions

- The target character is entered as a literal keypress (no escaping needed).
- Search always stays within the current line (does not wrap to next/previous line).
- The character scan keymap is stateless - it only inspects the key buffer to determine if a valid trigger+target pair is present.

## Dependencies

- **Internal**:
  - Existing `TrieKeymap` implementation
  - Existing `Action` enum and `with_count` trait method
  - Existing `NormalMode` key handling flow
  - Existing `Window::process_action` method
- **Blocked by**: None - this feature is self-contained with new keymap types

## Glossary Terms

**Character Scan Motion**: A motion that takes a target character as a runtime parameter and navigates to or past that character in the current line. The four variants are `f` (find forward), `F` (find backward), `t` (till forward), and `T` (till backward).

**Chained Keymap**: A keymap wrapper that delegates `get_action` and `is_prefix` calls to multiple sub-keymaps in sequence, trying each until one returns a non-None result.

**Character Scan Keymap**: A stateless keymap that matches two-key sequences where the first key is a character scan trigger (f/F/t/T) and the second key is any character, returning the corresponding action with the character as a parameter.

**Keymap**: A data structure that maps key sequences to actions. Implementations include Trie-based keymaps for fixed sequences and character scan keymaps for parameter-based motions.
