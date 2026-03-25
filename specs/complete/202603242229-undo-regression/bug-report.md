# URVIM-UNDO-001: `u` does not restore text after a delete
## Summary
Undo is not working for normal text edits. A delete such as `dw` removes the word, but pressing `u` afterward does not restore the deleted text.

## Severity: High

## Environment
Current urvim checkout in the local terminal workspace at `/Users/ryan/Dev/urvim`.

## Reproduction Steps
1. Open urvim with any text buffer containing at least one word, for example `hello world`.
2. Move the cursor onto `hello`.
3. Press `dw`.
4. Press `u`.

## Expected Behavior
`dw` should delete the word, and `u` should restore the buffer to its previous state, bringing the deleted word back.

## Actual Behavior
`dw` deletes the word, but `u` has no visible effect and the deleted text is not restored.

## Impact
This breaks the core undo workflow for normal-mode editing. Users cannot safely recover from accidental deletes or other text modifications, which makes the editor much riskier to use for real editing.

## Root Cause
Undo history is recorded at the wrong point in the main event loop. In [`src/main.rs`](../../src/main.rs), the editor calls `buffer.push_snapshot(cursor)` before processing snapshottable actions, so the snapshot captures the pre-edit buffer state. For a delete like `dw`, the buffer is then mutated, but no post-edit snapshot is recorded.

That interacts badly with the deduplication logic in [`src/buffer/undo.rs`](../../src/buffer/undo.rs): if the current snapshot already matches the buffer text, `push_snapshot()` only updates the cursor instead of creating a new history entry. As a result, the undo stack never advances past the original state, so `buffer.undo()` has nothing useful to restore after the delete.

## Solution Approach
Record undo history after a successful text mutation, or otherwise ensure each snapshottable edit leaves behind a history entry representing the new buffer state. Keep undo/redo themselves out of snapshot creation.

Rejected alternative: removing snapshot deduplication. That would still leave undo without a post-edit state and would add redundant history noise.

## Code Changes
- [`src/main.rs`](../../src/main.rs): move snapshot capture so it happens after text-changing actions have been applied, or add an equivalent post-mutation history step for snapshottable actions.
- [`src/buffer/undo.rs`](../../src/buffer/undo.rs): keep the history model aligned with the new snapshot timing and verify deduplication still behaves correctly.
- [`src/editor/tests.rs`](../../src/editor/tests.rs) or buffer integration tests: add coverage for `dw` followed by `u`, plus redo after undo.

## Edge Cases
- Multiple edits in a row should still undo one step at a time.
- `u` on an untouched buffer should remain a no-op.
- Redo should continue to work after undo.
- Cursor-only movement should not create undo entries.
- Counted deletes like `2dw` should be undoable with one `u`.
