# BUG-0037: T motion (TillBackward) stops at first character when repeated

## Summary

The `T` motion (TillBackward) cannot be pressed multiple times to find subsequent occurrences of the same character. After the first `T` motion lands after a character, pressing `T` again finds the same character instead of the next one. This is because `move_cursor_till_backward` lacks the search start position logic that `move_cursor_till_forward` has for handling repeated motions.

## Severity: Medium

- Character scan motions are core navigation features
- Workaround: manually type `T` with the target character each time
- Affects users navigating to repeated characters backward (e.g., "hhello" or any text with duplicate characters)

## Environment

| Field | Value |
|-------|-------|
| App Version | Latest |
| OS | macOS |
| Terminal | Any |

## Reproduction Steps

1. Open a buffer with text containing duplicate characters (e.g., "hhello")
2. Place cursor at the end of the line (after the last character)
3. Press `T h` to search backward for 'h'
4. Observe: cursor lands after the first 'h' found
5. Press `T h` again (or press `;` to repeat)
6. Expected: cursor should find the next 'h' (if any) and land after it
7. Actual: cursor stays in place, not moving to find the next 'h'

## Expected Behavior

When pressing `T h` multiple times on "hhello" with cursor at 'o':
- First `T h`: finds 'h' at col 1, lands after it at col 2
- Second `T h`: finds 'h' at col 0, lands after it at col 1
- Third `T h`: no more 'h' before col 0, cursor stays

## Actual Behavior

When pressing `T h` multiple times on "hhello" with cursor at 'o':
- First `T h`: finds 'h' at col 1, lands after it at col 2
- Second `T h`: finds 'h' at col 1 again (because search looks at idx < 2, finds 'h' at 1), lands after it at col 2
- Cursor stays at col 2 - no forward progress

## Impact

- Users cannot efficiently navigate to repeated characters using `T` motion
- Forces users to use alternative navigation (e.g., `Fh`, `,`, or h key)
- The bug mirrors Bug 3 from BUG-0034 which affected the `t` motion (fixed)

## Root Cause

**Location**: `src/window.rs:661-675` in `move_cursor_till_backward`

The issue is that `move_cursor_till_backward` does not calculate a proper search start position for repeated searches. When `T` lands after finding a character, subsequent `T` presses should search from a position BEFORE where we landed, not from the cursor position itself.

**Current code**:
```rust
pub fn move_cursor_till_backward(&mut self, target: char, count: usize) {
    let cursor = self.buffer_view.cursor();
    if let Some(new_cursor) = self
        .buffer_view
        .buffer()
        .find_char_backward(cursor, target, count)
    {
        // Land one position after the found character
        let line_len = self.buffer_view.buffer.line_len(new_cursor.line);
        let new_col = (new_cursor.col + 1).min(line_len);
        self.buffer_view
            .set_cursor(crate::buffer::Cursor::new(new_cursor.line, new_col));
    }
    // If not found, cursor stays in place (do nothing)
}
```

**Problem**: When the second `T` is pressed, `find_char_backward(cursor=2, ...)` looks for characters with `idx < 2`. It finds the same 'h' at col 1 that we just found. This creates an infinite loop where we keep finding the same character.

**Why `t` works**: `move_cursor_till_forward` has special logic (lines 633-646) to calculate `search_start_col`:
```rust
let search_start_col = if cursor.col == 0 {
    0
} else {
    // Find the grapheme at or after cursor position and get its width
    let mut col = cursor.col;
    for (byte_offset, grapheme) in line.grapheme_indices(true) {
        if byte_offset >= cursor.col {
            col = byte_offset + grapheme.len();
            break;
        }
    }
    col
};
```

This advances the search past the current grapheme, ensuring repeated `t` finds the next occurrence.

## Solution Approach

**Chosen**: Modify `move_cursor_till_backward` to calculate the correct search start position for repeated searches

**Reasoning**:
- Follows the same pattern as `move_cursor_till_forward`
- Minimal code change with targeted fix
- Maintains consistency between `t` and `T` behaviors

**The fix**:
When `T` lands at position `col` (after finding the character at `col-1`), the next search should start from `col-1` (or earlier for count > 1). This allows `find_char_backward` to find the same or previous character.

**Alternative approaches rejected**:
- Reusing `find_char_forward` with reversed logic: Would require significant refactoring
- Creating a new `find_char_backward_from` function: Overkill for this single use case

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/window.rs` | Modify | Update `move_cursor_till_backward` to calculate search_start_col similar to `move_cursor_till_forward` |

## Edge Cases

- **Cursor at start of line**: Should handle gracefully (search finds nothing, cursor stays)
- **No occurrence found**: Cursor stays in place (existing behavior)
- **Count > 1**: Should find the count-th occurrence from the adjusted search position
- **Multi-byte characters**: Should work correctly with grapheme indices
- **Line boundaries**: Should clamp at line start/end when landing