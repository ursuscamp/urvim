# BUG-0039: ^ (Caret) motion does not wrap to previous line when cursor is on blank line

## Summary

The `^` motion (MoveToLineContentStart) fails to wrap to the previous line's first non-whitespace character when the cursor is on a blank (whitespace-only) line at column 0. This prevents efficient backward navigation when crossing blank lines.

## Severity: Medium

- Core navigation motion impaired
- Workaround: use `k^` or manually navigate around blank lines
- Affects workflow when editing code with blank lines between sections

## Environment

| Field | Value |
|-------|-------|
| App Version | Latest |
| OS | macOS |
| Terminal | Any |

## Reproduction Steps

1. Open a buffer with multiple lines where a blank line separates content:
   ```
   Line 1: "  hello"
   Line 2: "  " (blank/whitespace-only line)
   Line 3: "  world"
   ```
2. Place cursor on the blank line (Line 2) at column 0
3. Press `^` to move to first non-whitespace
4. Observe: cursor stays at column 0 (no movement)
5. Expected: cursor should wrap to Line 1's first non-whitespace ('h' at col 2)

## Expected Behavior

When cursor is on a blank line at column 0 and `^` is pressed:
- If cursor is not at column 0, move to column 0
- If cursor is already at column 0 on a blank line, wrap to previous line's first non-whitespace

## Actual Behavior

When cursor is on a blank line at column 0:
- Code returns `None` immediately (no movement)
- Does not attempt to wrap to previous line

## Impact

- Users cannot efficiently navigate past blank lines using `^` motion
- Forces users to use alternative navigation (e.g., `k` then `^`, or `kk^`)
- Inconsistent behavior compared to when cursor is on a non-blank line

## Root Cause

**Location**: `src/buffer.rs:1488-1496` in `cursor_content_start_of_line`

**Current code**:
```rust
let content_start = match first_non_ws {
    Some(pos) => pos,
    None => {
        // Line has no non-whitespace - return at column 0
        if cursor.col > 0 {
            return Some(Cursor::new(cursor.line, 0));
        }
        return None;  // <-- BUG: returns None when cursor.col == 0
    }
};
```

**Problem**: When a line has no non-whitespace (blank line) and `cursor.col == 0`, the code returns `None` instead of continuing to the wrap logic. The wrap logic at lines 1505-1525 should handle moving to the previous line's first non-whitespace.

**What should happen**:
1. When on a blank line at col 0, we should continue to the wrap logic
2. The wrap logic should find the previous line and its first non-whitespace
3. If previous line is also blank, continue wrapping until finding content

## Solution Approach

**Chosen**: Modify the `None` branch to continue to wrap logic instead of returning `None`

**Reasoning**:
- Follows vim's behavior where `^` on blank line at col 0 wraps to previous line
- Minimal code change
- Maintains existing behavior for non-blank lines

**The fix**:
When `first_non_ws` is `None` and `cursor.col == 0`, don't return `None`. Instead, continue to the wrap logic to find the previous line's first non-whitespace.

**Alternative approaches rejected**:
- Returning `Some(Cursor::new(cursor.line, 0))` when `cursor.col == 0` on blank line: This would move to col 0 but not wrap, which is incorrect
- Special-casing blank lines at the wrap logic: Would be more complex and error-prone

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/buffer.rs` | Modify | Update `cursor_content_start_of_line` to continue to wrap logic when on blank line at col 0 |

## Edge Cases

- **Cursor at start of buffer on blank line**: Should stay in place (no previous line)
- **Multiple consecutive blank lines**: Should skip all blank lines to find previous non-blank
- **First line is blank**: Should stay at col 0 (no previous line to wrap to)
- **Cursor at col > 0 on blank line**: Should move to col 0 (existing behavior, correct)
