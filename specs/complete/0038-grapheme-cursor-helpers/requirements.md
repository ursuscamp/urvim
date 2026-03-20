# Grapheme-Based Cursor Navigation Helpers

## Summary

Refactor cursor navigation in the buffer to use proper Unicode grapheme cluster handling instead of ad-hoc byte/character arithmetic. This will fix cursor navigation bugs with multi-byte characters like emojis and CJK characters.

## Problem Statement

The codebase has multiple places where cursor column is incremented or decremented by 1 (`col + 1`, `col - 1`) to move between characters. This approach is incorrect because:

1. **Multi-byte characters**: Unicode grapheme clusters can be multiple bytes. A cursor column represents a byte offset, not a character count. Adding/subtracting 1 moves by bytes, not by graphemes.

2. **Example**: The emoji "👨‍👩‍👧‍👦" (family with 4 people) is composed of multiple Unicode code points and is ~25 bytes. Using `col + 1` would move only 1 byte, landing in the middle of the emoji.

3. **Affected code**: The `bracket_matcher.rs` module uses character-based iteration (`chars().nth()`) instead of grapheme-based iteration, causing incorrect bracket matching with multi-byte characters.

The existing `cursor_right()` and `cursor_left()` methods already properly use `grapheme_indices(true)` for grapheme-aware navigation. However, the bracket matcher and other locations do not use these helpers.

## User Stories

- **As a user**, I want bracket matching (`%` motion) to work correctly with emojis and CJK characters, so I can navigate code containing international characters.

- **As a user**, I want cursor movement to work correctly with multi-byte characters, so I don't land in the middle of a character when pressing arrow keys.

- **As a developer**, I want clear helper methods for grapheme-based cursor navigation, so I don't accidentally use incorrect byte arithmetic.

## Functional Requirements

- [ ] **REQ-001**: Create `Buffer::next_cursor_line(cursor)` method that returns the cursor position of the next grapheme cluster within the same line, or `None` if at end of line.

- [ ] **REQ-002**: Create `Buffer::prev_cursor_line(cursor)` method that returns the cursor position of the previous grapheme cluster within the same line, or `None` if at start of line.

- [ ] **REQ-003**: Create `Buffer::next_cursor(cursor)` method that returns the cursor for the next grapheme, wrapping to the start of the next line if at end of current line. Returns `None` only at end of last line.

- [ ] **REQ-004**: Create `Buffer::prev_cursor(cursor)` method that returns the cursor for the previous grapheme, wrapping to the end of the previous line if at start of current line. Returns `None` only at start of first line.

- [ ] **REQ-005**: Refactor `bracket_matcher.rs` to use grapheme-based iteration instead of character-based iteration, using the new helper methods.

- [ ] **REQ-006**: Find and fix any other locations in the codebase using incorrect `col + 1` or `col - 1` cursor arithmetic for grapheme navigation (excluding terminal coordinate conversion which uses 1-indexing).

- [ ] **REQ-007**: Rename `cursor_right` to `next_cursor` and `cursor_left` to `prev_cursor`. Update all call sites in the codebase.

- [ ] **REQ-008**: Remove the old `cursor_right` and `cursor_left` method names after renaming.

## Non-Functional Requirements

- **Performance**: New methods should use existing `grapheme_indices(true)` pattern already used in buffer.rs, adding minimal overhead.

- **Backward Compatibility**: The existing `cursor_right()` and `cursor_left()` methods should continue to work as before (they already use proper grapheme handling).

- **Code Clarity**: New helper methods should have clear documentation explaining their behavior and edge cases.

## Acceptance Criteria

- [ ] **AC-001**: `next_cursor_line` on line "a👨‍👩‍👧‍👦b" at byte 0 returns cursor at byte position of 'a' (~1 byte).

- [ ] **AC-002**: `next_cursor_line` on line "👨‍👩‍👧‍👦b" at start of emoji returns cursor at byte position after the complete emoji cluster.

- [ ] **AC-003**: `prev_cursor_line` on line "a👨‍👩‍👧‍👦b" at position after emoji returns cursor at byte position of 'a'.

- [ ] **AC-004**: `next_cursor` at end of line "hello" returns cursor at position 0 of next line.

- [ ] **AC-005**: `prev_cursor` at start of line "hello" (where previous line is "world") returns cursor at end of "world" line.

- [ ] **AC-006**: `find_matching_bracket` in bracket_matcher correctly matches brackets when multi-byte characters (emojis, CJK) are present.

- [ ] **AC-007**: All existing cursor movement tests continue to pass.

- [ ] **AC-008**: `cursor_right` has been renamed to `next_cursor` and `cursor_left` renamed to `prev_cursor`. All call sites updated.

## Out of Scope

- Changes to terminal coordinate handling (the `row + 1`, `col + 1` for 1-indexed terminal coordinates is intentional and correct).
- Modifying the Cursor struct itself.

## Assumptions

- The `unicode_segmentation::UnicodeSegmentation` crate's `grapheme_indices(true)` correctly handles all Unicode grapheme clusters including emojis, CJK characters, and combining characters.
- Cursor column positions are stored as byte offsets (as documented in the Cursor struct).

## Dependencies

- **Internal**: 
  - Buffer struct and Cursor struct
  - `unicode_segmentation` crate (already in use)
- **External**: None
- **Blocked by**: None

## Related Terms

- **Grapheme Cluster**: A user-perceived character, which may be composed of multiple Unicode code points (e.g., 'é' as single grapheme but 2 bytes in UTF-8, or emoji families as multiple code points).
- **Cursor**: A position in the buffer specified by line index and byte offset within the line.
