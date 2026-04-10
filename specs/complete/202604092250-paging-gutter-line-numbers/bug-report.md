# BUG-20260409-01: Paging renders stale gutter line numbers until the next cursor movement

## Summary

When a `Window` pages up or down, the buffer content scrolls to the correct page, but the `Gutter` continues to show the previous page's line numbers for that render. A subsequent single-line cursor movement causes another render pass, after which the gutter line numbers become correct. This makes paging appear visually inconsistent and briefly desynchronizes the `Gutter` from the visible `Buffer View`.

## Severity: Medium

## Environment

- Repository: `urvim`
- Area: window rendering and paging motions
- Relevant files:
  - `src/window/mod.rs`
  - `src/window/gutter.rs`
  - `src/window/motions.rs`
  - `src/window/tests.rs`
- Rendering context: terminal `Screen` rendering with a `Window` that owns a `Buffer View`

## Reproduction Steps

1. Open a buffer with more lines than fit in the viewport.
2. Render a `Window` with a small viewport, for example `Size::new(3, 40)`, so paging changes the visible top line.
3. Trigger `ActionKind::MovePageDown` or `ActionKind::MovePageUp`.
4. Observe the screen immediately after the page motion render.
5. Trigger a single-line cursor movement such as up or down.
6. Observe the gutter again.

## Expected Behavior

After a page motion, the `Gutter` should render line numbers for the same first visible buffer line as the content area during that same render pass.

## Actual Behavior

After a page motion, the content area reflects the new page, but the `Gutter` renders line numbers derived from the previous `scroll_offset`. The line numbers do not update until a later render, such as one caused by moving the cursor up or down by a single line.

## Impact

- The editor presents mismatched content and line numbers during paging.
- Users can momentarily lose trust in line-oriented navigation because the visible line numbers are stale.
- Paging behavior feels unstable even though the cursor and content positions are otherwise correct.

## Root Cause

`Window::render` calculates `start_line` from `self.buffer_view.scroll_offset()` and constructs the `Gutter` before calling `self.buffer_view.scroll_to_cursor(size, gutter_width)`. Page motions update the cursor position, and `scroll_to_cursor` then updates the `Buffer View` scroll offset to keep that cursor visible. However, the already-constructed `Gutter` still holds the old `start_line`, so it renders line numbers for the previous page while the content render uses the updated scroll state. The next render builds a new `Gutter` from the new offset, which is why a later single-line movement appears to fix the problem.

## Solution Approach

Chosen fix:
- Reorder `Window::render` so the `Buffer View` scroll state is finalized before the `Gutter` is constructed from `start_line`.
- Keep gutter width calculation compatible with scroll-to-cursor logic so the content width and gutter width remain consistent during the render pass.
- Add a regression test covering page motion followed by render, asserting that the gutter line numbers match the newly visible page immediately.

Rejected alternatives:
- Forcing an extra render after page motions would mask the ordering problem and add unnecessary work.
- Mutating the `Gutter` after construction would treat the symptom instead of fixing the render pipeline ordering.

## Code Changes

- `src/window/mod.rs`
  - Update `Window::render` so gutter state is derived from the final scroll position used by content rendering.
- `src/window/tests.rs`
  - Add a regression test that reproduces stale line numbers after page up/down and verifies they are correct in the same render pass.

## Edge Cases

- Paging near the start of the buffer where the first visible line clamps to `0`.
- Paging near the end of the buffer where the visible range clamps to the last buffer line.
- Viewports with very small heights, including one-row and two-row windows.
- Buffers whose line count changes gutter width across digit boundaries, such as `99` to `100` lines.
- Half-page motions if they share the same render path and stale gutter symptoms.
