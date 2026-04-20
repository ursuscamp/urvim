# 202604200824-active-line-background-overlay: Active line background is not applied through tokenized text

## Summary
When `active_line` is enabled and the active-line style has a background that differs from the theme default, the active line looks correct only in regions without token content. Tokenized text on that same line continues to show the theme default background instead of the active-line background overlay.

## Severity: Medium

## Environment
- urvim editor
- `active_line = true`
- Normal mode
- Any theme where `ui.window.active_line` uses a background that is visually distinct from the theme default background

## Reproduction Steps
1. Open any buffer that produces syntax-highlighted token spans, such as a Rust file with keywords and identifiers on the same line.
2. Use a theme where `ui.window.active_line` has a background color different from the theme default background.
3. Enable `active_line` in config.
4. Place the cursor on a line that contains both tokenized text and empty space.
5. Observe the active line.

## Expected Behavior
The active line should use the theme default style overlaid with the active-line style across the whole line.

- Regions with no token-specific background should show the active-line background.
- Token spans with an explicit background should keep their own background.

## Actual Behavior
The non-token portions of the active line show the expected active-line background, but tokenized text on that same line continues to render with the theme default background instead of the active-line overlay.

## Impact
The current line highlight becomes visually inconsistent inside tokenized text, which makes the active line harder to track in syntax-highlighted buffers.

## Root Cause
The active-line style is being applied as a line-level base style, but token chunk styles are still being resolved from the theme default alone. As a result, the active-line background does not become part of the base style used for tokenized spans, so tokenized cells can retain the theme default background instead of inheriting the active-line overlay.

## Solution Approach
Update the style composition path so the active-line style is included in the base style used for rendered token chunks on the active line, while still allowing any token-specific background to override it.

Rejected alternatives:
- Applying the active-line color only to empty cells. That would keep the mismatch between tokenized and non-tokenized regions.
- Forcing the active-line background over all token backgrounds. That would break explicit token background styling.

## Code Changes
- `src/window/mod.rs`: confirm the active-line base style is passed into the render pipeline for the active row.
- `src/window/render.rs`: ensure line base style is composed into chunk rendering before token styles are emitted, while preserving explicit chunk backgrounds.
- `src/window/tests.rs`: add a regression test that asserts tokenized text on the active line receives the active-line overlay unless the token itself defines a background.

## Edge Cases
- Token spans with an explicit background should continue to win over the active-line background.
- Active-line highlighting should remain disabled in insert mode and when `active_line` is `false`.
- Blank trailing regions on the active line should continue to use the active-line background.
