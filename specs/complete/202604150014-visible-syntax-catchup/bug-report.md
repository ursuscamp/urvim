# 202604150014: Visible syntax highlighting lags behind buffer edits

## Summary
Syntax highlighting updates arrive noticeably late after buffer edits. The visible portion of the edited buffer should be re-highlighted synchronously right away in every visible window, while the remainder of the buffer can continue catching up in the background.

## Severity: Medium

This is a user-visible responsiveness issue in a core editor feature. It does not corrupt buffer contents, but it makes edits feel delayed and undermines trust in the highlighted view.

## Environment

- Workspace: `/Users/ryan/Dev/urvim`
- Affected area: syntax highlighting after buffer mutation
- Relevant concepts:
  - Buffer
  - Window
  - Syntax Highlighting
  - Job Framework / background catch-up work

## Reproduction Steps

1. Open a syntax-highlighted file that is large enough for highlighting work to be noticeable.
2. Split the editor so the same buffer is visible in more than one window.
3. Move the cursor to a highlighted region in the buffer.
4. Enter insert mode and type a character, or make another buffer edit such as delete, change, or join.
5. Observe the highlighted styling in the visible area immediately after the edit.
6. Notice that the visible lines do not update their syntax styling right away and instead catch up after a short delay.
7. Repeat the edit in another visible window showing the same buffer.
8. Notice that the lag affects every visible window rather than only the active one.

## Expected Behavior

- Any buffer edit should invalidate syntax state and immediately re-highlight the visible lines in every visible window.
- Offscreen lines may continue to be re-highlighted asynchronously after the visible viewport is corrected.
- The editor should never show stale syntax styling for the currently visible portion of an edited buffer once the edit has completed.

## Actual Behavior

- After edits, syntax highlighting updates on a noticeable lag.
- The visible part of the buffer can remain stale for a moment even though the text change itself has already been applied.
- Background catch-up eventually corrects the styling, but the first visible frame after the edit is not fully up to date.

## Impact

- Typing and other edits feel sluggish because the editor appears to lag behind the user’s action.
- Visible syntax state can be temporarily misleading, especially in side-by-side windows showing the same buffer.
- The issue is most noticeable in larger files or any file where syntax recomputation is expensive enough for the delay to be visible.

## Root Cause

The syntax invalidation path appears to prioritize deferred catch-up work without forcing an immediate recompute for the currently visible region. As a result, edits mark syntax data stale, but the first visible repaint can still depend on asynchronous work that completes later. The problem is likely in the interaction between buffer mutation, syntax invalidation, and the background job path rather than in the grammar definitions themselves.

## Solution Approach

- Recompute syntax synchronously for the visible range of every window that is currently displaying the edited buffer as part of the edit path.
- Keep the remaining syntax catch-up work asynchronous so offscreen lines are still updated without blocking the editor longer than needed.
- Make the visible-range synchronous pass the source of truth for the next redraw, then let background work extend from that state.
- Avoid a renderer-only fix, because the stale data is coming from syntax state that is already wrong by the time the screen is redrawn.

## Code Changes

- `src/buffer/syntax.rs`
  - add or adjust synchronous visible-range recomputation after buffer edits
  - preserve deferred catch-up for offscreen lines
- `src/window/view.rs`
  - ensure each visible window requests immediate syntax refresh for the edited buffer’s on-screen lines
- `src/job.rs` or the existing background worker entry point
  - keep asynchronous catch-up work for the remaining syntax state
- `src/buffer/tests/syntax/*.rs`
  - add regression coverage for edits that should refresh visible highlighting immediately
- `src/buffer/tests/syntax/fixtures/*`
  - extend or reuse fixtures that make stale visible syntax styling easy to detect

## Edge Cases

- Multiple visible windows showing the same buffer should all refresh their visible syntax immediately after the edit.
- Edits near the top or bottom of the viewport should not leave partial stale spans visible until background work completes.
- Rapid repeated edits should not cause the synchronous visible-range refresh to fall behind or flash inconsistent styling.
- A background catch-up job must not overwrite newer visible syntax state after another edit has already occurred.
- Buffers with syntax highlighting disabled should continue to skip syntax work entirely.
