# Relative Line Numbering and Active Gutter Highlight - Technical Design

## Architecture Overview
This feature spans three existing editor subsystems: startup configuration, gutter rendering, and theme style resolution.

The window render path will continue to own the gutter lifecycle. On each render, it will determine:
- the current cursor line for the focused window
- whether relative line numbering is enabled
- whether the active-line gutter highlight should be shown for the current render target
- which theme styles should be used for the normal gutter and the active gutter row

The gutter renderer will then format each visible row using one of two labels:
- the cursor line keeps its absolute buffer line number
- all other visible logical lines show their absolute distance from the cursor line

Wrapped continuation rows keep the existing blank-gutter behavior, so the new numbering mode does not change wrapping presentation.

## Interface Design
### Configuration
Add a boolean startup config field:

```toml
relative_number = false
```

- Type: boolean
- Default: `false`
- Scope: all editor modes

The resolved config object should expose the field alongside the other startup toggles.

### Theme Styles
Add a dedicated UI style for the active gutter row. The style should be resolved through the same theme system used for the existing UI chrome styles, using a hierarchical name under the window gutter namespace.

Proposed highlight name:

- `ui.window.gutter.active_line`

The resolved theme model should expose the active gutter row style separately from the base gutter style so the renderer can choose between them without synthesizing colors at runtime.

### Gutter Rendering
The gutter renderer should accept enough render context to decide, per visible row:
- the buffer line number being displayed
- whether the row corresponds to the cursor line
- whether the row is a wrapped continuation that should keep the existing blank-number behavior
- which gutter style to use for the row background and text

Conceptually, the row label rules are:

- if the row is the cursor line, render the absolute buffer line number
- if relative numbering is enabled and the row is not the cursor line, render the positive distance from the cursor line
- if the row is a wrapped continuation row, preserve the existing blank gutter behavior

The active gutter row should render across the full gutter width, not just on the digit cell. The base gutter background and the active gutter background should both fill the row area so the highlight remains visually continuous.

## Data Models
### Config
Add a boolean field to the resolved config model and the partial TOML schema:

- `Config.relative_number: bool`
- `PartialConfig.relative_number: Option<bool>`

### Theme Styles
Extend the resolved UI style set with a dedicated active gutter row field:

- `UiStyles.gutter_active_line: Style`

Add the corresponding raw theme field and UI key entry used by the loader and resolver.

### Gutter Row State
The gutter renderer already works from viewport-derived row data. The new numbering mode only needs:

- `start_line`
- `visible_rows`
- `total_buffer_lines`
- `cursor_line`
- a per-row flag for whether the row is the cursor line

No new buffer-owned data structure is required.

## Key Components
### Config Loader
The config loader resolves the new boolean field from TOML and default values. It should follow the existing boolean-toggle behavior used by the other editor options.

### Theme Loader
The theme loader resolves the new gutter-row style from the active theme and from built-in theme files. It should use the same palette lookup and validation rules as the other predefined UI styles.

### Gutter
The gutter is responsible for:
- calculating width from the total buffer line count
- deciding which label to render on each visible row
- preserving blank continuation rows
- applying the correct style for the active row versus normal rows

### Window Render Path
The window render path is responsible for:
- supplying the gutter with the current cursor line
- passing the current mode and focus state needed for active-line gating
- keeping content width and cursor positioning consistent with the gutter width

The gutter should remain an internal render detail of the window, not a separate user-facing object.

## User Interaction
When the option is disabled, the gutter behaves exactly as it does today and shows absolute line numbers only.

When the option is enabled, the cursor line keeps its absolute number while surrounding visible lines show their distance from the cursor line. This behavior applies in normal mode, insert mode, and visual mode.

The active gutter row highlight should make the cursor line easier to track without changing the text content or selection behavior in the buffer area.

## External Dependencies
No new external dependencies are required. The work stays inside the existing config, theme, and window rendering code.

## Error Handling
The new config field should follow the existing startup config parsing behavior:

- malformed TOML should continue to fail during config loading
- unknown or invalid theme entries should continue to fail theme parsing according to the current validation rules

If the active gutter style is unavailable at runtime for any reason, the renderer should fall back to the normal gutter style rather than panicking.

## Security
This feature only affects local UI rendering and configuration parsing. It does not introduce new security-sensitive behavior.

## Configuration
Add documentation for `relative_number` in `docs/config.md`.

The built-in themes should define the active gutter row style with a background that stays readable against the base gutter while remaining subtle enough to fit each theme's tone.

## Component Interactions
1. Config loading resolves `relative_number` into the global runtime config.
2. Theme loading resolves `ui.window.gutter` and `ui.window.gutter.active_line` into the active theme's UI style set.
3. Window rendering computes the cursor line and viewport rows for the current frame.
4. The gutter renders visible rows using either absolute or relative labels based on the config toggle.
5. The current cursor row uses the active gutter style when active-line emphasis is enabled for that render target.
6. The gutter and buffer content share the same viewport math so the gutter width and cursor placement stay aligned.

## Platform Considerations
The feature should behave consistently across all supported terminals and platforms because it only changes computed labels and resolved styles.

The implementation should avoid any platform-specific color assumptions beyond the existing theme palette model.
