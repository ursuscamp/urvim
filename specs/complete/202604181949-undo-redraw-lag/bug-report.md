# 202604181949-undo-redraw-lag: Undo and redo do not trigger an immediate redraw

## Summary
Undo and redo update editor state correctly, but the visible screen does not refresh immediately afterward. The stale content remains on screen until another action, such as moving the cursor, causes a redraw.

## Severity: Medium

This is a user-facing regression in core editing behavior. It does not appear to lose data, but it makes undo and redo look broken because the buffer state changes are not reflected right away.

## Environment
- Workspace: `/Users/ryan/Dev/urvim`
- Affected area: normal-mode undo/redo handling and render invalidation
- Observed in all editor windows
- Observed for both undo and redo

## Reproduction Steps
1. Open a file in urvim.
2. Press `o` to open a new line.
3. Type `hello`.
4. Press `Esc` to return to normal mode.
5. Press `u` to undo.
6. Observe that the visible buffer does not change.
7. Move the cursor or perform another action that forces a redraw.
8. Observe that the undo becomes visible only after that later action.
9. Repeat the sequence with redo and observe the same delayed refresh.

## Expected Behavior
- Undo should immediately repaint the affected editor window after the buffer changes.
- Redo should immediately repaint the affected editor window after the buffer changes.
- The screen should reflect the current buffer state without waiting for a later cursor movement or unrelated action.

## Actual Behavior
- Undo changes editor state but the screen stays on the previous frame.
- Redo behaves the same way.
- The updated buffer content becomes visible only after another interaction triggers a redraw.

## Impact
- Undo and redo feel unreliable because the editor appears not to respond.
- Users may repeat the command or assume it failed.
- Any workflow that relies on visual confirmation after undo/redo is slowed down.

## Root Cause
Undo and redo likely mutate editor state without setting the render path to dirty, or they update the buffer without requesting a fresh frame from the window/layout layer. As a result, the main loop has no reason to repaint until some later input event happens to mark the UI as needing redraw.

## Solution Approach
Make undo and redo explicitly request a redraw when they succeed, and ensure that request reaches every affected editor window. The fix should treat undo/redo like other visible state changes that invalidate the frame.

Rejected alternatives:
- Relying on a later cursor movement to incidentally trigger repaint, which leaves the regression in place.
- Forcing a constant redraw loop, which would be unnecessary work and could reintroduce idle rendering churn.
- Restricting the fix to a single window or a single undo path, which would leave the bug visible in other editors or redo flows.

## Code Changes
- Update the undo/redo action path so a successful state change requests an immediate redraw.
- If redraw invalidation is tracked separately from buffer mutation, wire undo/redo into that invalidation path.
- Add regression tests covering undo and redo both repainting immediately after the command completes.
- Confirm the behavior in multi-window or multi-editor scenarios so the fix applies everywhere the action can run.

## Edge Cases
- Undo or redo with no available history should remain a no-op and should not introduce redraw churn.
- Multiple editor windows should all remain consistent after the action.
- Repeated undo/redo sequences should continue to repaint after each successful step.
- The fix should not trigger extra redraws for unrelated commands that do not change visible state.
