# BUG-003: Horizontal scrolling doesn't account for gutter width

## Summary
When scrolling horizontally on long lines, the scroll position calculation uses the full terminal width instead of the available content width (terminal width minus gutter width). This causes the editor to stop scrolling prematurely and leaves a gap on the right side of the screen when the cursor is near the end of a long line.

## Severity: High

- Affects core editing functionality when editing files with long lines
- Every user who enables the gutter and edits long lines will experience this
- Workaround: disable gutter (if possible), but no ideal workaround exists

## Environment

| Field | Value |
|-------|-------|
| App Version | Current main |
| OS | macOS / Linux |
| Terminal | Any terminal with ANSI support |
| Rust Version | Current project version |

## Reproduction Steps

1. Open a file in urvim with a long line (longer than terminal width, e.g., 100+ characters)
2. Ensure gutter is enabled (default)
3. Navigate to the end of the long line using arrow keys or `$`
4. Observe: cursor is at the right edge of the visible area, but there's still empty space to the right of the cursor
5. Press Left arrow multiple times to scroll back
6. Observe: scrolling stops before the cursor reaches the left edge - there's a gap on the right

## Expected Behavior
When scrolling horizontally, the available content width should be `terminal_columns - gutter_width`. The cursor should be able to scroll all the way to the left edge (column 0 of content), and the right edge should properly show the end of the line.

## Actual Behavior
The scroll calculation uses the full terminal width. This means:
- When at the end of a long line, there's unused space on the right (gutter width worth)
- When scrolling left, it stops prematurely, not letting the cursor reach the true left edge
- The behavior is worse with more digits in the gutter (e.g., line 100 vs line 1)

## Impact
- Users cannot fully utilize screen real estate when gutter is enabled
- Editing long lines is frustrating as cursor doesn't position correctly
- The bug is proportional to gutter width - worse with more line digits

## Root Cause

The bug is in `src/window.rs` in the `scroll_to_cursor` method.

When rendering buffer content, the gutter width IS correctly accounted for:
```rust
// Line 495-496 in render():
let content_origin = Position::new(origin.row, origin.col + gutter_width);
let content_size = Size::new(size.rows, size.cols.saturating_sub(gutter_width));
```

However, in `scroll_to_cursor` (lines 320-321):
```rust
let visible_rows = viewport_size.rows as usize;
let visible_cols = viewport_size.cols as usize;  // BUG: doesn't subtract gutter
```

The `viewport_size.cols` is the full terminal width, but it should be `viewport_size.cols - gutter_width` to match what's used during rendering.

**Location**: `src/window.rs:321` and `src/main.rs:137`

## Solution Approach

**Chosen**: Pass gutter width to `scroll_to_cursor` and subtract it from visible columns

**Reasoning**:
- Minimal code change with clear intent
- Aligns scroll calculation with render calculation
- Maintains backward compatibility (gutter can be 0 if not enabled)

**Implementation**:
1. Calculate gutter width in the calling code (main.rs) using the same logic as window rendering
2. Pass gutter width to `scroll_to_cursor` 
3. Subtract gutter width from `visible_cols` in `scroll_to_cursor`

**Alternative**: Calculate gutter width inside `scroll_to_cursor` using `Gutter::calculate_width()`
- Rejected because it would duplicate gutter calculation logic and require access to buffer line count

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/window.rs` | Modify | Update `scroll_to_cursor` to accept gutter width parameter and subtract it from visible_cols |
| `src/main.rs` | Modify | Calculate and pass gutter width to `scroll_to_cursor` |

## Edge Cases

- Test with gutter disabled (width = 0)
- Test with single-digit line numbers (gutter width = 3)
- Test with 2-digit, 3-digit, 4-digit line numbers
- Test when gutter width changes mid-session (file grows from 99 to 100 lines)
- Test with very narrow terminals (e.g., 80 cols with 5-digit line number)
- Test horizontal scrolling on short lines (should still work correctly)
- Test vertical scrolling still works correctly
