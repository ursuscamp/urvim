# Floating Notification Banners - Technical Design

## Architecture Overview
This feature keeps the existing notification queue and top-right placement, but changes the renderer from a single-line text banner into a framed floating window. The new renderer will:

1. Read the active notification from the existing notification state.
2. Measure the available top-right viewport region.
3. Wrap the message into one or more lines that fit the chosen width.
4. Render a bordered notification window using a level-specific style.
5. Continue to respect the existing queue order, TTL behavior, and redraw requests.

The design should remain compatible with the current layout pipeline, so notification rendering continues to happen as part of the normal frame render path.

## Interface Design
### Existing notification rendering entry point
The layout layer should continue to call the notification renderer during frame composition, using the existing render entry point or an equivalent replacement.

- Input: screen surface, viewport origin, viewport size, current time
- Output: draws the active notification if one exists
- Constraints: must not modify focus state or editor mode

### Theme style resolution
Notification level styling should use the existing theme lookup convention with the following highlight names:

- `ui.notification.info`
- `ui.notification.warn`
- `ui.notification.error`

The renderer should use these styles for the notification frame border, and built-in themes should define them explicitly.

### Temporary debug keybindings
Temporary keybindings should be added in the normal input path so each existing notification level can be triggered on demand.

- One keybinding per notification level
- Each binding enqueues a generic notification text for that level
- Bindings must use unused keys and avoid collisions with existing editor actions
- These bindings are temporary and should be easy to remove after validation

## Data Models
### NotificationMessage
The existing notification message model remains the source of truth for:

- `level`: `Info | Warn | Error`
- `text`: rendered notification message
- `created_at`: message timestamp
- `expires_at`: message expiration timestamp

No new persistent notification data model is required.

### Theme highlight overlays
Built-in themes will add overlay definitions for the notification level styles listed above. These styles should be stored in the existing theme highlight tables rather than introducing a new theme subsystem.

## Key Components
### Notification renderer
Responsibilities:
- Read the active notification from global notification state
- Compute a top-right popup rectangle that fits within the viewport
- Wrap message text to the computed width
- Draw the frame and content using the selected styles
- Preserve queue-driven sequential display

Dependencies:
- `Screen` for drawing
- `Position` and `Size` for popup geometry
- `unicode-segmentation` and `unicode-width` for safe wrapping
- existing theme resolution helpers
- existing notification queue state

### Border and frame drawing
Responsibilities:
- Reuse the current border glyph strategy already used by layout split borders
- Render the notification border with the level-specific style
- Render the popup interior with the notification surface style or the active theme's normal background style

Dependencies:
- Existing terminal style and glyph handling
- Existing advanced glyph capability for Unicode borders

### Theme updates
Responsibilities:
- Add notification-level styles to every built-in theme
- Preserve readable contrast across all bundled palettes
- Provide sensible defaults when a custom theme omits notification styles

Dependencies:
- Built-in theme TOML files
- Theme loader regression tests

### Temporary notification test bindings
Responsibilities:
- Enqueue a generic notification for info, warn, or error when the binding is pressed
- Keep the bindings isolated so they can be removed cleanly
- Avoid interfering with save, mode switching, or pane navigation behavior

Dependencies:
- Existing key event dispatch
- Existing command or action routing for notification enqueueing

## User Interaction
1. A notification is emitted.
2. The active message appears in the top-right corner inside a framed popup.
3. If the message is long, it wraps to additional lines inside the same popup.
4. The border color indicates the notification level.
5. Pressing a temporary test key spawns a sample notification of the corresponding level.
6. When notifications are queued, the next item appears after the current one expires, preserving FIFO order.

## External Dependencies
No new external crates are required. The feature reuses:

- existing terminal drawing primitives
- existing theme loading and highlight resolution
- existing Unicode width and grapheme segmentation crates already used by notification rendering

## Error Handling
- If notification rendering is requested while the screen has zero rows or columns, the renderer should return without drawing.
- If the computed popup cannot fit, the renderer should clamp to the available space rather than panicking.
- If a notification style is missing from a custom theme, the renderer should fall back to a safe default level style.
- If a temporary debug binding is unavailable due to input conflicts, it should be reassigned before release rather than overlapping with a user command.

## Security
- Notification text must remain plain text and must not introduce terminal escape-sequence injection beyond existing rendering behavior.
- Temporary debug keys should not expose privileged operations or hidden configuration changes.
- Theme styling data should remain local to bundled theme assets and not depend on external services.

## Configuration
No new end-user configuration options are required.

The feature should respect the existing advanced glyph capability for Unicode borders when drawing notification frames.

## Component Interactions
- The input layer maps temporary test keys to notification enqueue actions.
- Notification enqueueing updates the shared notification state and requests a redraw.
- The layout render path continues to call notification rendering during frame composition.
- The notification renderer consults theme state to resolve level-specific styles.
- The queue advances only when active notifications expire, preserving current sequencing behavior.

## Platform Considerations
- The window frame must use glyphs and widths that remain valid in terminal environments with Unicode and ASCII border modes.
- Wrapping must respect grapheme clusters so emoji and composed characters do not split incorrectly.
- Rendering should remain stable across narrow terminals by clipping or clamping within visible bounds.
