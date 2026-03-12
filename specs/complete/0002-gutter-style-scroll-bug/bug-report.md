# BUG-002: Gutter style rendering breaks on scroll

## Summary
When rendering the gutter, the initial render displays correctly. However, when scrolling vertically by even one line, some line number digits render with the correct gutter style while others render with the terminal's default style. This is caused by a bug in the screen's diff-based rendering that doesn't properly handle style state for unchanged cells.

## Severity: High

- Affects core editor functionality (line numbers are unreadable after scrolling)
- Every user who scrolls will see this bug
- No workaround available (can't avoid scrolling)
- This is the first feature that uses styles, so this bug likely affects any styled content

## Environment

| Field | Value |
|-------|-------|
| App Version | Unknown (current main) |
| OS | macOS / Linux |
| Terminal | Any terminal with ANSI support |
| Rust Version | Current project version |

## Reproduction Steps

1. Open a file in urvim with enough lines to require scrolling (e.g., 20+ lines)
2. Observe the gutter: line numbers 1-N display with correct gutter style (background color)
3. Scroll down by one line (e.g., press `j` or arrow down)
4. Observe the gutter: some digits now render with terminal default style instead of gutter style

## Expected Behavior
All gutter content (line numbers and background) should always render with the gutter style, regardless of scroll position.

## Actual Behavior
After scrolling:
- Some line number digits render with correct gutter style
- Some line number digits render with terminal default style (usually white/light on black)
- The pattern seems inconsistent - not all digits are affected equally

## Impact
- Gutter becomes partially unreadable after any scroll
- Affects usability of the editor for any file with multiple lines
- This bug likely affects any future styled content rendered with diff-based updates

## Root Cause

The bug is in the screen's `render` method (`src/screen.rs`). When comparing cells between the old buffer and new buffer:

1. **Unchanged cells**: When a cell's text AND style are the same as the old buffer, the code enters the `else` branch and just increments the column counter without writing anything to the terminal
2. **Style state leakage**: After processing each cell in the `if` branch, `terminal.reset_style()` is called. However, when we skip an unchanged cell (else branch), we don't write anything to that position
3. **The terminal retains whatever was last written** at that position from a previous frame, BUT more importantly - if the previous cell had a style applied, that style may still be "active" in some terminals

The key insight: when we skip an unchanged cell, we don't explicitly handle its style. The terminal may have residual style state from previous operations.

**Location**: `src/screen.rs:222-246`

## Solution Approach

**Chosen**: Track the previous cell's style and ensure we always write (or at least reset style for) each cell position

The fix requires:
1. Track what style was last written to the terminal (not just what's in the buffers)
2. For unchanged cells, compare against the style we actually wrote (old_buffer), BUT also ensure we don't leave stale styles
3. Alternatively: Always write each cell's style explicitly - if unchanged, still apply the style (since terminal state may differ from buffer state)

**Reasoning**:
- The current optimization of skipping unchanged cells is sound, but it doesn't account for terminal state
- We need to ensure each cell position is properly handled regardless of whether content changed

**Rejected alternatives**:
- Disable diff-based rendering entirely: Would hurt performance significantly
- Rewrite to always render full frame: Too drastic, performance impact unacceptable

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/screen.rs` | Modify | Fix render method to handle style state for unchanged cells |

The fix should ensure that when we skip writing to a cell (because it's unchanged), we still properly handle the style state - either by:
1. Tracking and resetting to the previous cell's style, or
2. Tracking the "effective" style we last wrote and comparing against that

## Edge Cases

- Test with different gutter widths (1-digit, 2-digit, 3-digit line numbers)
- Test rapid scrolling (multiple scroll events before render)
- Test scroll up vs scroll down
- Test when gutter width changes (e.g., file grows from 99 to 100 lines)
- Test with multiple styled regions (if buffer content also has styles)
- Test with wide characters in gutter (if applicable)
- Ensure the fix doesn't break the existing optimization (should still skip redundant writes)
