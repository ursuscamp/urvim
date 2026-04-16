# 202604152139: `G` lags before jumping to the bottom of long syntax-highlighted files

## Summary
Pressing `G` in a long syntax-highlighted file should move the cursor and visible viewport to the bottom immediately. Instead, the editor pauses briefly while syntax catch-up work runs, and the jump finishes only after that work starts to settle.

## Severity: High

This regression affects a core navigation motion in ordinary editor use. It does not corrupt text, but it makes a basic Vim motion feel unresponsive in the exact large-file case the background syntax system was meant to improve.

## Environment

- Workspace: `/Users/ryan/Dev/urvim`
- Affected area: normal-mode `G` motion in long files with syntax highlighting enabled
- Relevant concepts:
  - Buffer
  - Window
  - Syntax Highlighting
  - Background Worker Framework
  - Syntax Catch-Up

## Reproduction Steps

1. Open a long file with syntax highlighting enabled.
2. Press `G` to jump to the last line.
3. Observe the time between the keypress and the cursor landing at the bottom of the file.
4. Notice that the editor pauses briefly before completing the jump.
5. Repeat the same jump in other long syntax-highlighted files.
6. Notice that the lag appears consistently across file types.

## Expected Behavior

- `G` should move the cursor and viewport to the last line immediately.
- Syntax highlighting may continue catching up in the background after the jump.
- The first visible frame after the jump should not wait for offscreen syntax work to finish.

## Actual Behavior

- `G` pauses for a moment before the cursor reaches the bottom of the file.
- The delay appears tied to syntax catch-up rather than the motion itself.
- Once the pause ends, the jump completes and syntax continues filling in afterward.

## Impact

- Deep navigation feels sluggish in large files.
- The editor no longer preserves the “jump instantly, refine later” behavior that background syntax catch-up was intended to provide.
- This is especially noticeable when opening large files and using `G` as a quick way to reach the end.

## Root Cause

The render path for a visible window still performs synchronous syntax warmup before painting the target viewport. In `src/window/view.rs`, `build_render_data_with_style` calls `buffer.ensure_syntax_through(visible_end_line)` whenever syntax is enabled. When `G` jumps to the end of a long file, that visible end line is also the file end, so the synchronous warmup ends up computing syntax for the entire prefix of the file before the jump can be painted.

That means the background catch-up system is still in place, but the deep-jump path is reintroducing a blocking foreground syntax pass at exactly the wrong time.

## Solution Approach

- Stop forcing a full synchronous syntax warmup when the viewport has jumped far ahead into an incomplete cache.
- Keep rendering the target viewport immediately using whatever cached spans already exist, plus base styling where syntax data is not yet available.
- Preserve background syntax catch-up so the rest of the file still fills in after the first frame.
- Avoid disabling background catch-up entirely, because that would regress the incremental syntax completion behavior already relied on elsewhere.

## Code Changes

- `src/window/view.rs`
  - keep the synchronous highlight pass needed for the initial visible viewport
  - remove or narrow the deeper `ensure_syntax_through` path that currently blocks long jumps to the bottom
  - keep the background catch-up request in place for the target buffer
- `src/window/tests.rs`
  - add a regression test that renders a large syntax-highlighted buffer after a bottom jump and verifies the first render does not synchronously complete the full cache
- `src/buffer/syntax.rs`
  - preserve the existing generation and background result handling so delayed catch-up still applies safely

## Edge Cases

- Syntax-disabled buffers should continue to skip all syntax warmup and background catch-up.
- The initial visible viewport should still be synchronously highlighted on open so the editor never flashes entirely unhighlighted text at startup.
- Small files and nearby scrolling should still render normally without introducing stale visible syntax.
- The fix must not let stale background results overwrite newer syntax state after another edit or jump.
- Repeated `G` presses in the same buffer should not accumulate redundant catch-up work.
- Multi-window views of the same buffer should continue to share syntax state safely.
