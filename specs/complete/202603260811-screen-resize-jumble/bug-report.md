# 202603260811-screen-resize-jumble: Terminal contents become jumbled after window resize

## Summary

When the terminal window is resized, the editor can leave stale characters and layout fragments visible on screen. The UI still renders, but the physical terminal is not fully repainted for the new dimensions, so the display looks corrupted or "jumbled" until enough later updates happen to overwrite the old cells.

## Severity: High

This affects the core editor view and happens during a common terminal interaction. It does not appear to corrupt buffer data, but it makes the UI unreliable after a resize.

## Environment

- urvim running in a terminal
- Any session where the terminal size changes while the editor is open
- Most visible when resizing wider or narrower by several columns/rows

## Reproduction Steps

1. Start `urvim` in a terminal with a file open.
2. Make the terminal window noticeably larger or smaller.
3. Observe the editor immediately after the resize event is processed.
4. Repeat the resize a few times, including widening the terminal after shrinking it.

## Expected Behavior

After a resize, the editor should repaint the full visible area for the new terminal size so that no characters from the previous size remain on screen.

## Actual Behavior

Some regions of the terminal keep showing stale content from the previous frame. Newly exposed rows or columns can remain uncleared, producing overlapping text, partial gutters/status bars, or other visual artifacts.

## Impact

- The main editor view becomes hard to read after resizing.
- Users may think the buffer or cursor state is broken when the issue is only rendering.
- Repeated resizes can make the display look progressively more distorted until another full repaint happens indirectly.

## Root Cause

The render pipeline is entirely diff-based, but resize handling resets the in-memory `Screen` buffers without forcing the terminal itself to be cleared or forcing every cell to be repainted.

Relevant code paths:

- `src/main.rs:48-52` renders the frame before processing events.
- `src/main.rs:147-151` updates `rows`/`cols` and calls `screen.resize(...)` after a resize event.
- `src/screen.rs:78-89` rebuilds both buffers with blank cells.
- `src/screen.rs:211-255` only writes cells whose current buffer value differs from the previous buffer.

That means newly blank cells created by a resize are often skipped, because both `buffer` and `old_buffer` contain spaces after `Screen::resize()`. The terminal still contains whatever was drawn at those positions before the resize, so stale pixels remain visible.

## Solution Approach

On resize, force a real repaint of the terminal surface instead of relying on the next diff pass alone.

Recommended fix:

- Clear the physical terminal or otherwise mark the next frame as a full redraw immediately after a resize.
- Keep the buffer resize, but ensure the next render emits every visible cell for the new dimensions.

Rejected alternatives:

- Only resizing the in-memory `Screen` buffers. That preserves the current bug because unchanged blank cells are still skipped.
- Hoping the next normal render will overwrite everything. That is unreliable for blank regions and newly exposed area.

## Code Changes

- `src/main.rs`
  - Handle `Event::Resize` by triggering a full repaint path for the next frame.
  - If needed, clear the terminal before the next render so stale cells cannot survive the size change.
- `src/screen.rs`
  - Add a way to force a full diff reset after resize, or expose a helper that makes the next render repaint all cells.
  - Add regression coverage for resize-driven repaint behavior.
- `src/main.rs` or `src/screen.rs` tests
  - Add a test that simulates a larger screen after a resize and verifies blank cells are actively cleared, not just skipped.

## Edge Cases

- Shrinking the window: content that no longer fits should disappear cleanly without leaving remnants at the edge.
- Growing the window: new rows and columns must be cleared, not left as old terminal contents.
- Rapid consecutive resize events: the fix should remain stable even if multiple `Event::Resize` values arrive back-to-back.
- Wide characters near the edge: the repaint logic must continue to clear adjacent cells correctly.
- Zero-sized or near-zero-sized terminal dimensions: resizing should not panic and should still avoid leaving stale display state behind.
