# BUG-0034: Character scan motions have multiple grapheme-related bugs

## Summary

The character scan motions (f/F/t/T) have three related bugs involving cursor positioning and duplicate characters. The fundamental issue is that `find_char_forward` and `find_char_backward` use `char_indices()` which operates on byte positions of individual characters, not grapheme clusters. Additionally, the search starting positions for F and t motions don't properly handle duplicate characters.

## Severity: Medium

- Character scan motions are core navigation features
- Workaround: use h/l arrow keys for character navigation
- Affects users working with text containing repeated characters

## Environment

| Field | Value |
|-------|-------|
| App Version | Latest |
| OS | macOS |
| Terminal | Any |

## Reproduction Steps

### Bug 1: Cursor should advance by grapheme

1. Open buffer with multi-byte characters (e.g., "hällo")
2. Place cursor at position 0
3. Press `fl` to find 'l'
4. Observe cursor position

### Bug 2: Double letters break F motion

1. Open buffer with "hello"
2. Place cursor at the second 'l': "hel|lo" (cursor at col=3)
3. Press `Fl` to find 'l' backward
4. Expected: cursor moves to first 'l': "he|llo"
5. Actual: cursor does not move

### Bug 3: Double letters break t motion

1. Open buffer with "hello"
2. Place cursor at start: "|hello"
3. Press `tl` - cursor moves to "h|ello" (correct)
4. Press `tl` again - should move to "he|llo" (second 'l')
5. Actual: cursor does not move (stays at "h|ello")

## Expected Behavior

- **Bug 1**: Cursor should land on the correct grapheme cluster
- **Bug 2**: `F` motion should find the previous occurrence when on a duplicate character
- **Bug 3**: Repeated `t` motion should find subsequent occurrences

## Actual Behavior

- **Bug 1**: Cursor may land on wrong position with multi-byte characters
- **Bug 2**: Cursor doesn't move when it should
- **Bug 3**: Cursor doesn't move on repeated t motion

## Impact

- Users cannot efficiently navigate to repeated characters
- Multi-byte character support is broken for character scan motions
- Forces users to use less efficient h/l navigation

## Root Cause

**Bug 1 (Grapheme advancement)**

The `find_char_forward` and `find_char_backward` methods in `src/buffer.rs:2325-2381` use `char_indices()` which returns byte positions of individual characters, not grapheme cluster positions. The cursor's `col` field is used as both a byte position (in some contexts) and potentially a grapheme position (in others), causing mismatches.

Location: `src/buffer.rs:2340-2343`, `src/buffer.rs:2370-2373`

```rust
// Current code uses char_indices():
for (char_idx, ch) in line_str.char_indices() {
    if char_idx >= start_col && ch == target {
        occurrences.push(char_idx);
    }
}
```

**Bug 2 (F motion with duplicates)**

When the cursor is on a character that matches the search target, `find_char_backward` finds that same character (if it's the first occurrence before cursor) and returns its position, making it appear the cursor didn't move. The search should start from `cursor.col - 1` but if that position contains the target character, we need to search from `cursor.col - 2`.

Location: `src/buffer.rs:2367`

```rust
let start_col = cursor.col.saturating_sub(1);
```

**Bug 3 (t motion with duplicates)**

After `t` lands before a character, the cursor is ON that character (at its start). When `t` is pressed again, `find_char_forward` searches from `cursor.col + 1` which is the byte immediately after the character we just found - but that's still within the same character if it has width > 1, or points to the same character if the grapheme is single-byte.

The issue is that after landing on a character, we should search from `cursor.col + grapheme_width(current_char)` to skip past the character we're standing on.

Location: `src/window.rs:628-630`

```rust
// Land one position before the found character
let new_col = new_cursor.col.saturating_sub(1);
```

## Solution Approach

**Chosen**: Fix grapheme handling in find_char functions and adjust search positions for F and t motions

**Reasoning**:
- Grapheme indices should be used consistently throughout
- For F motion: when on the target character, skip it and find the previous
- For t motion: after landing, advance by grapheme width to find next occurrence

**Code Changes Required**:

1. `src/buffer.rs`: Modify `find_char_forward` and `find_char_backward` to use `grapheme_indices(true)` instead of `char_indices()`
2. `src/buffer.rs`: Modify `find_char_backward` to handle case when cursor is on target character by searching from `cursor.col - 2`
3. `src/window.rs`: Modify `move_cursor_till_forward` to calculate correct start position using grapheme width

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/buffer.rs` | Modify | Update `find_char_forward` to use grapheme_indices |
| `src/buffer.rs` | Modify | Update `find_char_backward` to use grapheme_indices and handle duplicate target |
| `src/window.rs` | Modify | Update `move_cursor_till_forward` to use grapheme-aware start position |

## Edge Cases

- Empty lines: should not crash, return None
- Multi-byte characters: should be treated as single graphemes
- Cursor at start/end of line: should handle gracefully
- Repeated motions with count > 1: should find correct occurrence
