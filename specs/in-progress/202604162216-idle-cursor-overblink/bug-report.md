# 202604162216-idle-cursor-overblink: Idle redraws make the cursor appear to over-blink on large screens

## Summary
When urvim is opened in a very large terminal, especially with an empty buffer or a file with only a few lines, the cursor begins to appear to blink too rapidly after a short startup delay. The effect continues indefinitely while the editor is idle, even though the rest of the screen appears stable.

## Severity: Medium

This does not corrupt data or block editing, but it makes the editor feel visually unstable in an idle state and becomes especially distracting in fullscreen or other large terminal layouts.

## Environment

- Workspace: `/Users/ryan/Dev/urvim`
- Affected area: idle render loop, terminal polling wakeups, cursor visibility handling
- Strongest reproduction:
  - very large terminal window
  - empty file, or a file with roughly 20 lines or fewer
  - editor left idle after open
- Reproduced across terminal emulators, including Kitty

## Reproduction Steps

1. Open urvim in a very large terminal window, such as a fullscreen terminal with hundreds of rows and columns.
2. Open an empty file, or a file with only a small number of lines.
3. Wait briefly after startup without typing.
4. Watch the cursor while the editor remains idle.
5. Observe that the cursor begins to appear to blink more rapidly than intended and keeps doing so indefinitely.

## Expected Behavior

- Idle cursor blinking should remain stable and normal regardless of terminal size or file size.
- Leaving the editor idle should not introduce any extra cursor blink cadence beyond the terminal's normal cursor behavior.
- Background wakeups and redraw scheduling should not make the cursor visually flicker.

## Actual Behavior

- After a short delay following startup, the cursor begins to appear to over-blink while the editor is idle.
- The effect is most noticeable on very large terminals and with empty or very small files.
- The rest of the screen does not obviously flicker, so the visible symptom is isolated to the cursor.

## Impact

- Idle editor sessions feel visually noisy and less polished.
- Fullscreen usage becomes noticeably more distracting than small-window usage.
- The issue can make users suspect unnecessary redraw churn or background work even when they are not interacting with the editor.

## Root Cause

The main event loop redraws on every terminal poll wakeup, even when no input, resize, or accepted background result requires a new frame. In `src/terminal/input.rs`, `Terminal::read_event()` returns `Event::Tick` every 50ms when the input poll times out. In `src/main.rs`, the loop renders a full frame before reading each event, so those idle ticks still trigger `screen.clear()`, `layout.render(...)`, and `screen.render(...)`.

`src/screen.rs` then calls `terminal.hide_cursor()` at the start of every render and `terminal.show_cursor()` at the end, even when the diff contains no changed cells. Repeating that hide/show cycle on each idle tick continuously resets the terminal cursor's visible state. On large terminals, the diff walk itself takes longer, which makes the hide/show cadence more noticeable. Empty or very small files further amplify the effect because there is little other screen activity to mask the cursor resets.

## Solution Approach

Stop treating every idle poll timeout as an unconditional redraw. The editor should only render when a real visual invalidation has occurred, such as input that changes state, a resize, or an accepted background result that actually requests a redraw. Cursor visibility handling should also avoid unnecessary hide/show cycles when no frame update is needed.

Chosen fix direction:
- gate rendering behind an explicit redraw-needed signal instead of redrawing on every `Event::Tick`
- preserve idle wakeups for background work without forcing a frame
- keep cursor hide/show scoped to real paint work only

Rejected alternatives:
- increasing the poll timeout, which would only reduce the frequency of the symptom rather than fix the unconditional redraw loop
- disabling cursor blinking entirely, which would hide the symptom but not remove the unnecessary idle redraw behavior
- special-casing only large screens or small files, which would treat the visibility of the bug rather than the underlying redraw policy

## Code Changes

- `src/main.rs`
  - track whether a redraw is actually needed before clearing, laying out, and rendering a frame
  - skip full-frame rendering on idle ticks unless another subsystem has requested a redraw
  - treat bracketed paste as a no-op until raw paste insertion is implemented
- tests
  - add regression coverage for idle tick handling so repeated `Event::Tick` wakeups do not force redraw behavior
  - add coverage that accepted background work can still request a redraw when needed

## Edge Cases

- Background syntax catch-up should still repaint once a fresh result is accepted.
- Resize events must continue to trigger an immediate redraw.
- Idle sessions with no pending work should not repaint repeatedly.
- Small terminals should keep their current behavior without regressing responsiveness.
- The fix must not delay visible updates after user input or after a background result changes rendered content.
