# BUG-202603262215: Buffer viewport blank space ignores theme default style
## Summary
The buffer content area does not apply the active theme's default style to empty cells. When a line is shorter than the content viewport, or when the viewport includes rows past the end of the buffer, those blank cells remain styled with the screen's default style instead of the theme default.

## Severity: Medium
## Environment
- urvim workspace: `/Users/ryan/Dev/urvim`
- A themed window render path with the active theme set through `Window::set_theme`
- Any terminal, because the issue is in render data generation rather than terminal capability detection

## Reproduction Steps
1. Load a theme whose `default` section sets a visible background color.
2. Open a buffer with at least one short line, for example `line1`, in a window wider than the text.
3. Render the window with a visible content area wider than the line text.
4. Inspect cells to the right of the text on that row, and rows below the end of the buffer when the viewport is taller than the file.

## Expected Behavior
The entire buffer viewport, including blank cells after short lines and empty rows within the content area, should use the theme's default style.

## Actual Behavior
Only the explicit text cells receive the theme default style. Blank cells in the buffer area remain at `Style::default()`, so the theme background does not extend across the full content viewport.

## Impact
This creates visible unstyled gaps in themed editor windows. Themes with custom background colors look incorrect because the empty area to the right of text and below the last buffer line falls back to the terminal default styling.

## Root Cause
`Window::render` passes the theme default style into `BufferView::build_render_data_with_style`, but `build_render_data_with_style` only creates `RenderChunk`s for visible text and never emits chunks for trailing blank cells or empty viewport rows. `RenderData::render_with_base_style` then writes only those chunks to the screen, so untouched cells keep whatever style the screen buffer already had. The current screen buffer starts from `Style::default()`, which leaves the buffer area's empty space unthemed. Relevant code paths are `src/window/mod.rs:143-171`, `src/window/view.rs:146-178`, and `src/window/render.rs:41-60`.

## Solution Approach
Add a `Screen` helper that clears or fills a region with an explicit style, then use it from the window render path to paint the full buffer viewport with the theme default style before drawing line text. The fix should stay local to the window/content render path and should not change the global screen defaults, because that would affect non-buffer UI regions and non-themed rendering paths.

## Code Changes
- `src/screen.rs`: add a style-aware clear or fill helper for painting blank regions.
- `src/window/mod.rs`: ensure the window render path continues to pass the active theme default style into content rendering and uses the screen helper for the buffer viewport.
- `src/window/render.rs`: keep text chunk rendering layered on top of the themed base style.
- `src/window/tests.rs`: add regression coverage for short lines and empty viewport rows using a themed default background.

## Edge Cases
- Short last lines should still leave the remaining columns in that row themed.
- Completely empty buffers should still render the full content area using the theme default style.
- Horizontal scrolling should preserve the themed background in the newly exposed blank area.
- The gutter should keep its own style and not inherit the buffer area's default fill.
