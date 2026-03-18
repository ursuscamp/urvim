# BUG-0023: L key motion crashes when count exceeds viewport height

## Summary
The L key motion (MoveToScreenBottom) causes a panic/crash when used with a count greater than the viewport height. This is due to an arithmetic underflow when calculating the target line position.

## Severity: High

- Editor crashes completely, requiring restart
- No workaround available for large counts
- Affects any user trying to navigate with large counts

## Environment

| Field | Value |
|-------|-------|
| App Version | Latest (development) |
| OS | macOS / Linux / Windows |
| Terminal | Any |

## Reproduction Steps

1. Open a file in urvim with enough lines to fill more than the viewport
2. Press a number larger than the viewport height (e.g., `100L` if viewport is 24 rows)
3. Observe: Editor crashes with panic

## Expected Behavior
The L key with count should move the cursor to N lines from the bottom of the viewport, clamped to valid bounds (like H does).

## Actual Behavior
The editor panics with an arithmetic underflow error:
```
thread 'main' panicked at 'attempt to subtract with overflow', src/window.rs:864
```

## Impact
- Editor becomes unusable until restarted
- Loss of unsaved work if any
- Blocks productivity for users who need to navigate large distances

## Root Cause

The L motion with count calculates target line as:
```rust
let target_line = (end_line - offset).max(start_line);
```

When `offset` (which is `count - 1`) is greater than `end_line`, the subtraction `end_line - offset` underflows on unsigned integer (`usize`), causing a panic.

Location: `src/window.rs:864`

The H motion (MoveToScreenTop) handles this correctly by using `.min()` to clamp:
```rust
let target_line = (start_line + offset)
    .min(start_line + viewport_rows - 1)
    .min(line_count - 1);
```

## Solution Approach

**Chosen**: Use `saturating_sub` to prevent underflow, then clamp with `.max(start_line)`

**Reasoning**: 
- Matches the pattern used for H motion but inverted
- Minimal code change
- Handles all edge cases (count = 0, count = 1, count > viewport)

**Rejected alternatives**:
- Return early if count > viewport: Would change behavior from vim (vim clamps, doesn't error)
- Use checked_sub with error handling: More complex, unnecessary for this case

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/window.rs` | Modify | Fix line 864 to use saturating_sub |

**Before:**
```rust
let target_line = (end_line - offset).max(start_line);
```

**After:**
```rust
let target_line = end_line.saturating_sub(offset).max(start_line);
```

## Edge Cases
- count = 0: Should behave like L without count (go to bottom of viewport)
- count = 1: Should go to bottom line of viewport (end_line)
- count = viewport_height: Should go to top line of viewport (start_line)
- count > viewport_height: Should clamp to start_line (like vim)
- Empty buffer: Should return early (already handled)
- viewport_rows = 0: Should return early (already handled)
