# 202603301738: Markdown fenced closing delimiter renders twice

## Summary
In `fixtures/syntax/markdown.md`, the JavaScript fenced code block's closing
delimiter is rendered incorrectly. The visible result looks like one closing
delimiter is styled as part of the code/fence text, and a second closing
delimiter appears as punctuation-style ghost text even though that second
delimiter is not present in the file.

## Severity: Medium

The issue is user-visible in a core syntax-highlighting path, but it does not
corrupt buffer contents or block editing.

## Environment

- Workspace: `/Users/ryan/Dev/urvim`
- Fixture: `fixtures/syntax/markdown.md`
- Relevant implementation: `src/buffer/syntax.rs`, `src/window/view.rs`,
  `src/syntax_builtin/markdown.toml`

## Reproduction Steps

1. Open `fixtures/syntax/markdown.md` in urvim with Markdown syntax highlighting
   enabled.
2. Scroll to the JavaScript fenced block around lines 17-19.
3. Inspect the closing fence on line 19.
4. Observe that the closing delimiter appears twice:
   - one rendering looks like a quote/string-style fence
   - a second punctuation-style fence appears as ghost text even though the file
     only contains one closing fence

## Expected Behavior

- The closing Markdown fence should render exactly once.
- The closing fence should use the Markdown fence/punctuation style only.
- No extra delimiter should appear outside the source text.

## Actual Behavior

- The closing fence in the JavaScript block appears to be painted twice.
- One rendering looks like a quote/string-style delimiter.
- A second punctuation-style delimiter is drawn as ghost text even though it is
  not present in the fixture.

## Impact

- Markdown fenced blocks look visually corrupted or duplicated at the close of
  the block.
- The user cannot rely on the rendered closing fence to match the source text.
- The issue is especially confusing because the extra delimiter is visually
  present but not actually stored in the buffer.

## Root Cause

The symptom is consistent with a fence-closing span mismatch in the Markdown
injected-region path or in the renderer's span/clamping logic.

`src/buffer/syntax.rs` already emits a closing span for injected regions, and
`src/window/view.rs` then slices and overlays syntax spans to build the visible
line. If the closing fence span is emitted or consumed twice, or if the closing
span is clamped in a way that leaves behind a stale styled chunk, the renderer
can show a duplicate delimiter even though the buffer contains only one.

This report is narrower than the earlier Markdown highlighting bug report: the
previous report covers the general closing-fence styling regression, while this
one captures the duplicated/ghost closing delimiter specifically visible in the
JavaScript fence in `fixtures/syntax/markdown.md`.

## Solution Approach

- Add a regression test that inspects the Markdown fence closing line and
  asserts the delimiter is rendered once.
- Trace the closing-fence path through `src/buffer/syntax.rs` and
  `src/window/view.rs` to ensure the delimiter span is emitted and consumed only
  once.
- Avoid a Markdown-only rendering special case; the fix should stay within the
  existing syntax span model.

## Code Changes

- `src/buffer/tests.rs`
  - add a regression test for the duplicated Markdown closing fence
- `src/buffer/syntax.rs`
  - verify the injected-region closing span is emitted with the correct bounds
  - ensure the post-close state transition does not create an extra visible
    delimiter
- `src/window/view.rs`
  - verify span clamping and chunk emission do not duplicate a closing fence
- `fixtures/syntax/markdown.md`
  - keep the JavaScript fenced block as the reproduction case

## Edge Cases

- Rust, JavaScript, and WAT fenced blocks should all continue to render their
  closing fences once.
- Unknown or missing fence languages should still render unstyled inside the
  fence body without introducing duplicate delimiters.
- Horizontal scrolling should not reintroduce a ghost closing fence when spans
  are partially visible.
- The fix should not affect inline Markdown constructs such as backticks or
  links.
