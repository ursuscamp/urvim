# Flash on Yank - Technical Design

## Architecture Overview
The yank flash is a transient, window-local visual state that is recorded when a yank succeeds in Normal Mode and expires automatically after a short interval. The implementation should reuse the existing selection rendering path so the flashed region looks identical to a Visual Mode selection.

The design keeps the feature inside the window/rendering layer rather than introducing a new global subsystem. That matches the current ownership model: a `Window` owns a `BufferView`, and `BufferView` already owns cursor and visual selection state used by rendering.

The high-level flow is:
1. A Normal Mode yank resolves a range and stores text into a register as it does today.
2. The same code path records a transient highlight for that range with a fixed 200ms lifetime.
3. Rendering overlays the selection style on the flashed range.
4. On later input ticks, expired flashes are cleared and the window is redrawn.
5. A new yank replaces any existing flash immediately.

## Interface Design
### New transient selection API
Add a small transient selection model alongside the existing visual selection model:

```rust
pub enum YankFlashSelection {
    Character(crate::buffer::TextObjectRange),
    Line { start_line: usize, count: usize },
}

pub struct YankFlash {
    pub selection: YankFlashSelection,
    pub expires_at: std::time::Instant,
}
```

### BufferView flash methods
Extend `BufferView` with transient highlight management methods:

```rust
impl BufferView {
    pub fn begin_yank_flash(&mut self, selection: YankFlashSelection, duration: std::time::Duration);
    pub fn clear_yank_flash(&mut self);
    pub fn yank_flash(&self) -> Option<YankFlash>;
    pub fn prune_yank_flash(&mut self, now: std::time::Instant) -> bool;
}
```

Method behavior:
- `begin_yank_flash` replaces any existing flash and starts a new 200ms timer.
- `clear_yank_flash` removes the transient highlight immediately.
- `yank_flash` returns the current flash state, if any.
- `prune_yank_flash` clears the flash when it is expired and returns whether state changed.

### Window yank hooks
Normal-mode yank handlers should call the flash API immediately after the register is populated and before returning success. This includes:
- linewise yank actions
- operator-based yank actions resolved from motions or text objects

Visual-mode yank actions should not call the flash API because the selection is already visible and the requirement limits the behavior to Normal Mode.

## Data Models
### YankFlashSelection
Represents the yanked region in a form the renderer can apply without recomputing the range.

Fields and constraints:
- `Character(TextObjectRange)` for characterwise yanks.
- `Line { start_line, count }` for linewise yanks.
- The selection is always stored in normalized form at the moment the yank succeeds.

### YankFlash
Stores the transient highlight and its expiration.

Fields:
- `selection: YankFlashSelection`
- `expires_at: Instant`

Constraints:
- The expiration is fixed at 200ms from the moment the yank completes.
- A newer yank always overwrites an older flash.

## Key Components
### BufferView
Owns the flash state because it already owns the cursor and visual selection state that drive rendering.

Responsibilities:
- store the current transient flash state
- clear expired flash state
- expose the flash for rendering
- keep flash state independent from modal state

### Window command handlers
The yank-related handlers are responsible for creating the flash record when a yank succeeds in Normal Mode.

Responsibilities:
- resolve the yanked region
- store register content exactly as today
- start the transient flash with the resolved region
- avoid flashing for Visual Mode yanks

### Render path
The buffer rendering path should apply the flash using the same `ui.selection` theme style already used for Visual Mode.

Responsibilities:
- preserve syntax highlighting underneath the selection overlay
- render linewise flashes across entire selected lines
- render characterwise flashes across only the yanked span
- avoid changing the current cursor mode or cursor style

### Event loop / redraw coordination
The main loop already wakes up on terminal tick events. Those ticks should be used to prune expired flash state for the active window tree and trigger a redraw when the flash disappears.

Responsibilities:
- clear expired flash state without user input
- ensure the highlight disappears even if the user pauses after the yank
- keep the expiration cadence bounded by the existing terminal poll interval

## User Interaction
1. The user performs a yank in Normal Mode.
2. The editor copies the selected text into the appropriate register.
3. The yanked region briefly flashes with the existing Visual Mode selection colors.
4. The user stays in Normal Mode throughout.
5. If the user performs another yank before the flash ends, the display switches to the newest yanked region and the timer restarts.

The flash should feel like confirmation, not a mode change. It should not create an actual visual selection state that the user can interact with.

## External Dependencies
No new external crates are required.

The feature relies on:
- `std::time::Instant` and `std::time::Duration`
- the existing terminal tick events used by the main loop
- the existing theme highlight for `ui.selection`

## Error Handling
This feature should fail open and never block the yank itself.

Expected recovery behavior:
- If the flash state cannot be rendered because the buffer is unavailable, clear the flash silently.
- If the selected range can no longer be applied safely, skip the overlay instead of panicking.
- If a yank happens while another flash is active, replace the old flash immediately.
- If the event loop misses a tick, the flash should still disappear on the next render or input cycle.

## Security
No new security concerns are introduced.

The feature only adds transient UI state and does not affect register contents, file contents, or external process interaction.

## Configuration
There is no user-facing configuration for this first version.

The flash duration is fixed at 200ms and the style is fixed to the existing Visual Mode selection colors.

## Component Interactions
1. **Normal-mode yank handler** resolves the range and stores register text.
2. The handler calls `begin_yank_flash(...)` with the resolved selection.
3. `BufferView` records the flash and its expiration time.
4. The render path reads the flash state and overlays the `ui.selection` style.
5. On terminal tick, the active window tree prunes expired flashes.
6. If the flash is expired, the next redraw omits it.

The rendering overlay should reuse the same selection application logic as Visual Mode so characterwise and linewise yanks are highlighted consistently.

## Platform Considerations
The expiration timing is driven by the existing TTY poll timeout, which is currently 50ms. That means the visible duration is approximate rather than frame-accurate, but it should still read as a short flash.

This behavior is appropriate for terminal environments and should work consistently across supported platforms because it uses the editor's normal event loop and rendering path.
