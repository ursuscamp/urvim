# BUG-0029: dd and cc ignore count prefix

## Summary

The "dd" and "cc" commands ignore the count prefix entirely. When a count is provided (e.g., "2dd" or "2cc"), the commands only delete/change a single line at the count-th line position, rather than deleting/changing N lines starting from the cursor position. This breaks Vim-compatible behavior.

## Severity: High

- Core editing functionality is broken
- Count prefix is completely ignored for line operations
- Workaround exists: manually repeat dd/cc multiple times
- Affects all users who expect Vim-compatible behavior

## Environment

| Field | Value |
|-------|-------|
| App Version | Latest (development) |
| OS | macOS / Linux |
| Rust Version | Stable |

## Reproduction Steps

Same issue with "dd":
1. Open a buffer with "a\nb\nc\nd\ne" (5 lines)
2. Position cursor on line 1 (which contains "a")
3. Press "2dd" to delete 2 lines starting from the cursor
4. Expected: Lines 1 and 2 are deleted ("a" and "b"), leaving "c\nd\ne"
5. Actual: Only line 2 ("b") is deleted, leaving "a\nc\nd\ne"

Same issue with "cc":
1. Open a buffer with "line1\nline2\nline3"
2. Position cursor on line 1
3. Press "2cc" to change 2 lines
4. Expected: Lines 1 and 2 are replaced with a blank line, leaving "\nline3"
5. Actual: Only line 2 is replaced with a blank line, leaving "line1\n\nline3"

## Expected Behavior

In Vim:
- `2dd` from line 1 should delete lines 1 and 2 (2 lines starting from cursor)
- `2cc` from line 1 should replace lines 1 and 2 with a blank line

## Actual Behavior

In urvim:
- `2dd` from line 1 deletes only line 2 (the 2nd line in the buffer)
- `2cc` from line 1 replaces only line 2 (the 2nd line in the buffer)

The count is being used to go to that line number (like "2G"), but then dd/cc only operates on a single line regardless of the count.

## Impact

- Users cannot rely on count prefixes with dd/cc
- Muscle memory from Vim fails
- Makes bulk line operations tedious (must manually repeat dd/cc)

## Root Cause

The count prefix handling for `Action::DeleteLine` and `Action::ChangeLine` goes to the specified line (using the count as an absolute line number like "G"), then performs a single delete/change operation instead of repeating it count times.

Location: `src/window.rs` - the Count handler for DeleteLine and ChangeLine

## Solution Approach

**Chosen**: Fix the Count handler for DeleteLine and ChangeLine to:
1. Stay at the current cursor position (don't jump to the count-th line)
2. Delete/change N lines starting from the current cursor position

**Reasoning**: 
- Matches Vim behavior where "2dd" means "delete 2 lines starting from cursor"
- The current implementation incorrectly goes to line N then does a single dd/cc

**Rejected alternatives**:
- Leave as-is: This is clearly incorrect behavior

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/window.rs` | Modify | Fix Count handler for DeleteLine to delete count lines from cursor |
| `src/window.rs` | Modify | Fix Count handler for ChangeLine to change count lines from cursor |

## Edge Cases

- Count exceeds available lines: Should delete/change all remaining lines (this already works)
- Count = 1: Should behave same as without count
- Cursor at last line with count > 1: Should handle gracefully
- Empty buffer: Should be a no-op
