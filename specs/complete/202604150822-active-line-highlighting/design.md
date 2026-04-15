# Active Line Highlighting - Technical Design
## Architecture Overview
Active-line highlighting will be added as a small cross-cutting UI feature that touches configuration, theme schema/resolution, and window rendering.

The editor will resolve a boolean startup setting that controls whether the focused window's cursor line receives an active-line style. Rendering will consult that setting together with the current mode and the focused-window state. The theme system will expose a dedicated UI style for the active line so themes can define the appearance independently from the main window background and selection style.

## Interface Design
### Configuration
Add a boolean startup config field:

```toml
active_line = false
```

- Type: boolean
- Default: `false`
- Scope: focused window only, normal mode only

The config loader should expose the value through the resolved `Config` object alongside the existing startup options.

### Theme Schema
Add a new predefined UI style named `active_line` to the closed theme schema.

The new style should be available in:
- raw theme parsing
- resolved `UiStyles`
- theme lookup and built-in theme definitions

### Rendering
Window rendering should treat the active-line style as an overlay applied to the line containing the cursor when all of the following are true:
- the config toggle is enabled
- the window being rendered is the focused window
- the editor is in normal mode

The active-line overlay should preserve the existing syntax and UI layering model, and it should not affect inactive windows.

## Data Models
### Config
Add a boolean field to the resolved config model and the partial TOML schema:

- `Config.active_line: bool`
- `PartialConfig.active_line: Option<bool>`

The field should participate in normal config resolution like the other boolean editor toggles.

### UI Styles
Extend the closed UI style set with:

- `UiStyles.active_line: Style`

Add the corresponding raw theme field:

- `RawUiStyles.active_line: RawStyle`

Add the corresponding UI key:

- `UiStyleKey::ActiveLine`

## Key Components
### Config Loader
The config loader is responsible for resolving the new boolean field from file and default values. It does not need special validation beyond normal TOML boolean parsing.

### Theme Loader
The theme loader must resolve the new UI style from the theme's `[ui]` section using the same palette-reference rules as the other predefined UI styles.

### Window Render Path
The window view/render pipeline must determine whether the focused window should receive the active-line style and then overlay that style on the current line's rendered chunks.

The renderer should rely on the current cursor position rather than duplicating cursor-tracking logic.

## User Interaction
When the option is disabled, the editor behaves exactly as it does today.

When the option is enabled, only the focused window in normal mode highlights the cursor line. Switching focus, switching windows, or leaving normal mode should immediately remove the highlight from that window.

The highlight should remain subtle enough to support reading the line content and should not obscure syntax highlighting or selection styling.

## External Dependencies
No new external dependencies are required. The work is entirely within the existing config, theme, and rendering subsystems.

## Error Handling
The new config and theme fields should follow the existing failure behavior:

- malformed TOML should continue to fail during startup config loading
- unknown theme fields should continue to fail theme parsing
- missing `active_line` style entries in built-in themes should be treated as theme authoring errors during theme loading

If a theme does not provide the active-line style, rendering should still have a safe fallback path through the resolved style model rather than panicking at runtime.

## Security
This feature does not introduce new security-sensitive behavior. It only affects local UI rendering and config parsing.

## Configuration
Add documentation for the new `active_line` config field in `docs/config.md`.

The built-in themes should be updated so their active-line styles use a slightly lighter background than each theme's normal background. For themes that already rely on named palette colors, this can be expressed with the existing palette and style resolution system rather than introducing a new color manipulation primitive.

## Component Interactions
1. Config loading resolves `active_line` into the global runtime config.
2. Theme loading resolves `ui.active_line` into the active theme's UI style set.
3. The main loop already tracks the current mode and focused window state.
4. Window rendering checks the config toggle, the render target window, and the current mode.
5. If all conditions match, the renderer overlays the active-line style on the line containing the cursor.
6. The active-line overlay composes with syntax highlighting and other UI overlays using the existing style layering model.

## Platform Considerations
The feature should behave identically across terminals and platforms supported by urvim because it only changes resolved styles, not terminal control behavior.

On ANSI-only themes, the active-line styling should still be expressible using the existing palette/index-based theme format. On true-color themes, the same UI style should continue to work through RGB palette resolution.
