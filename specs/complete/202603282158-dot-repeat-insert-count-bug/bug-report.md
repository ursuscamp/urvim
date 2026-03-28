# 202603282158-dot-repeat-insert-count-bug: Dot repeat duplicates inserted text for counted changes

## Summary

Dot repeat currently replays inserted text once per stored count when the original change entered insert mode. A sequence like `2cw`, then typing `hello`, then leaving insert mode will replay as `2cw` plus `hellohello` when `.` is pressed. Vim compatibility requires the count to affect the structural change, not to multiply the committed insert text.

## Severity: High

## Environment

- urvim in the `/Users/ryan/Dev/urvim` workspace
- Normal-mode dot repeat with repeatable change commands that enter insert mode
- Current replay path in `src/main.rs`

## Reproduction Steps

1. Open a buffer containing text with at least one word, such as `alpha beta`.
2. In normal mode, run `2cw`.
3. Type `hello`.
4. Press `<Esc>` to leave insert mode.
5. Move to a new word or use `.` immediately after the change.
6. Press `.`.

## Expected Behavior

- The structural change is replayed with the original count.
- The committed insert text is inserted once for the completed repeat.
- The repeated edit behaves like Vim: the count applies to `cw`, not to the inserted payload.

## Actual Behavior

- The replayed structural action uses the stored count.
- The committed insert text is inserted once per count iteration.
- `2cw` followed by `hello` replays as `hellohello` instead of `hello`.

## Impact

- Common change-and-insert workflows do not behave like Vim.
- Users see duplicated text during dot repeat, which makes the feature unsafe for routine editing.
- The bug affects one of the most common repeat scenarios, so it is likely to be noticed quickly.

## Root Cause

The repeat replay loop in `src/main.rs:272-305` applies `replay.count` as an outer loop and inserts `replay.insert_text` inside that loop. For counted changes that enter insert mode, the stored count represents the structural change size, but the replay code treats it as the number of times to replay the whole compound edit. Because the insert payload is emitted inside the loop, it is duplicated by the count.

The repeat record and resolver in `src/editor/action.rs:394-443` currently carry a single `count` field and an optional `insert_text` payload, but the replay path does not distinguish between the structural count and the repeat invocation count.

## Solution Approach

Refactor dot-repeat replay so the structural change count and the repeat invocation count are handled separately.

Preferred fix:

- Apply the stored count to the structural action itself.
- Insert the committed text once for each completed repeat replay, not once per structural count unit.
- Preserve `.` count override behavior for the repeat command.
- Keep the last valid repeat record intact if a replay fails.

Rejected alternative:

- Leaving the current loop structure in place and trying to compensate by post-processing the inserted text. That would keep the count semantics tangled together and would be harder to reason about.

## Code Changes

- `src/main.rs`: update `replay_repeat_action` so inserted text is not multiplied by the structural count, and keep repeat command count handling explicit.
- `src/editor/action.rs`: adjust repeat resolution or replay data if a separate structural count field is needed.
- `src/globals.rs`: update repeat state only if the data model changes.
- `src/main.rs` and `src/editor/tests.rs`: add regression tests for counted change repeat behavior.

## Edge Cases

- A count before `.` should still override the stored repeat request.
- Empty or missing insert text should continue to behave like the existing structural dot-repeat path.
- Multiline insert text should still replay correctly.
- Failed structural replay should not clear the stored repeat record.
- Non-insert repeat sources such as deletes and motions should remain unchanged.
