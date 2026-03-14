# BUG-002: "E" key at end of line doesn't wrap to next line

## Summary

Pressing the "E" key (BigWordEnd motion) at the end of a line should wrap to the next line and move to the end of the next word, but it stays on the current position instead. This is inconsistent with vim behavior where "E" at the end of a line moves to the end of the next word on the following line.

## Severity: Medium

- Affects text navigation productivity
- Workaround: use j+k or down-arrow to move to next line, then press E
- Not data-corrupting, but significant usability impact

## Environment

| Field | Value |
|-------|-------|
| App Version | Latest |
| OS | macOS / Linux |
| Terminal | Any |

## Reproduction Steps

1. Open a buffer with multiple lines, e.g., "hello\nworld"
2. Place cursor at the end of line 0 (position 4, after 'hello')
3. Press "E" to move to end of next word
4. Observe the cursor position

## Expected Behavior

For text "hello\nworld":
- At position 4 (after 'hello'), pressing "E" should move to position 4 (end of "world" on line 1)

## Actual Behavior

For text "hello\nworld":
- At position 4, pressing "E" stays at position 4 (or returns to position 4)
- The cursor does NOT wrap to the next line

## Impact

- Users cannot efficiently navigate across line boundaries with "E"
- Inconsistent with vim behavior where E wraps to next line
- Forces users to use alternative navigation (j+E or down+E)

## Root Cause

The `next_boundary` function in `src/buffer.rs` handles BigWordEnd boundary incorrectly when at the end of a line. 

Location: `src/buffer.rs:1367-1413` (Boundary::BigWordEnd handling)

The issue is two-fold:
1. The early wrap logic at lines 918-922 only triggers when `col >= line_len` (cursor past last char), not when at the last character position
2. The BigWordEnd logic finds the end of the current word (which we're already at), and returns that same position without wrapping to the next line

When cursor is at position 4 (after 'hello' in "hello\nworld"):
- col (4) >= line_len (5)? No (4 < 5), so no early wrap
- BigWordEnd logic: finds end of current word (position 4), returns position 3 (check_col - 1)
- Result: cursor stays at essentially the same position

## Solution Approach

**Chosen**: Modify Boundary::BigWordEnd handling to wrap to next line when at end of current line

**Reasoning**:
- Matches vim's behavior exactly
- Minimal code change - only need to add wrap logic when current line has no more words
- Consistent with Boundary::BigWord which already has wrap tests

**Rejected alternatives**:
- Modify the early wrap at line 918-922: This would affect all boundary types, not just E, and could break other behaviors

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/buffer.rs` | Modify | Update BigWordEnd handling to wrap to next line when at end of line |

## Edge Cases

- Empty lines: should continue to next line (existing behavior should work)
- Last line: should stay at end of last word or return None
- Multiple empty lines: should skip empty lines and find next word
- Line with only whitespace after current word: should wrap and skip whitespace on next line
- Next line first word is one character: should treat that character as a word end
