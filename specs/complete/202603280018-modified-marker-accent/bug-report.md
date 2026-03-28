# BUG-202603280018: Modified marker inherits resolved background instead of acting as an accent

## Summary

The modified-buffer marker in the tab bar and status bar is rendered by layering `theme.ui.modified_marker` onto the surrounding UI style. Because `modified_marker` is resolved against the theme default style first, it can carry a background color, and the current `overlay` composition keeps that background when the marker is drawn. The result is that the `*` marker does not behave like a pure accent and can pick up an unintended background from `Style::default()`-derived theme state.

## Severity: Medium

- Affects a visible unsaved-changes indicator in two core UI regions
- Makes the marker less consistent across themes
- Can reduce contrast or introduce a background block where only an accent glyph is expected
- The editor still functions, so this is not blocking, but it is user-facing and persistent

## Environment

| Field | Value |
|-------|-------|
| App Version | Current development build |
| OS | macOS / Linux |
| Terminal | Any ANSI-capable terminal |
| Rust Version | Stable |

## Reproduction Steps

1. Start urvim with a theme that defines a non-default background for the status/tab UI, or one where the theme default background is visually distinct.
2. Open a file and make any edit so the buffer becomes modified.
3. Observe the `*` modified marker in the tab bar and status bar.
4. Compare the marker cell against the surrounding UI background.
5. Notice that the marker is composed using the resolved style background instead of acting as a foreground-only accent.

## Expected Behavior

The modified marker should behave like an accent:

- Preserve the background of the surrounding tab bar or status bar
- Apply only the marker's foreground and text attributes such as bold
- Never introduce a background color of its own

## Actual Behavior

The modified marker is rendered with a style that can include a background color inherited from theme resolution. When that style is overlaid onto the surrounding UI style, the background remains part of the final marker cell, so the `*` can appear with an unintended block of background rather than just an accented glyph.

## Impact

- Unsaved-changes indicators are visually inconsistent with the rest of the UI
- The marker can look heavier or more intrusive than intended
- Theme authors cannot rely on `modified_marker` behaving like a foreground-only highlight
- The same issue affects both the tab bar and status bar, so it is visible in multiple places

## Root Cause

`modified_marker` is resolved in the theme loader against the theme default style:

- `src/theme/loader.rs:129-136` resolves `ui.modified_marker` with `default_style`

Later, the rendering code composes that style with the surrounding UI using `Style::overlay`:

- `src/tab_group.rs:198-224`
- `src/status_bar.rs:75-96`

`Style::overlay` copies background when the overlaid style provides one:

- `src/terminal/style.rs:366-400`

That makes `overlay` the wrong composition primitive for the modified marker, because the marker is supposed to accent the existing background, not replace it.

## Solution Approach

**Chosen**: Add a new `Style::accent` method that behaves like `overlay` for foreground attributes and text decorations, but ignores background entirely. Use it when composing the modified marker in the tab bar and status bar.

**Reasoning**:

- Matches the intended semantics of a modified indicator
- Keeps the surrounding UI background intact
- Avoids special-casing background stripping at each call site

**Rejected alternatives**:

- Keep using `overlay`: this continues to inherit or preserve background in the marker cell
- Strip background manually at each render site: duplicates the same logic in multiple places
- Change theme resolution so `modified_marker` cannot carry a background: this would make the style model less flexible than needed

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/terminal/style.rs` | Modify | Add `Style::accent` alongside `overlay`, with foreground-only composition semantics |
| `src/tab_group.rs` | Modify | Use `accent` when rendering the modified marker in the tab bar |
| `src/status_bar.rs` | Modify | Use `accent` when rendering the modified marker in the status bar |
| `src/terminal/style.rs` or render tests | Modify | Add tests covering that accent preserves the base background while applying foreground attributes |
| `src/tab_group.rs` and `src/status_bar.rs` tests | Modify | Update or add assertions for the modified marker style behavior |

## Edge Cases

- Theme `modified_marker` specifies only foreground and bold: marker should still inherit the surrounding background
- Theme `modified_marker` specifies a background: marker should ignore it
- Default theme fallback path: marker should remain readable even without an active theme
- Narrow tab/status regions: marker composition should not affect truncation or layout
- Existing uses of `overlay` elsewhere should remain unchanged
