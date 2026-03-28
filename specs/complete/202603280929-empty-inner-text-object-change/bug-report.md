# 202603280929-empty-inner-text-object-change: Change on empty inner text objects does not enter insert mode

## Summary
When `c` is used with an inner bracket or quote text object that resolves to an empty region, urvim treats the operation as a no-op instead of entering insert mode at the text-object location.

This shows up with commands like `ci(` on `()` and `ci"` on `""`. The buffer stays unchanged, but the editor does not transition into insert mode, which breaks the expected Vim-style "change inside empty object" workflow.

## Severity: Medium

This is a common editing path for bracketed or quoted placeholders and empty strings. The bug does not corrupt data, but it blocks a core operator-pending interaction.

## Environment

- urvim workspace at `/Users/ryan/Dev/urvim`
- Rust workspace on 2026-03-28
- Reproduced in the current text-object implementation for bracket and quote families

## Reproduction Steps

1. Open a buffer containing a single empty bracket pair, for example `()`.
2. Place the cursor on or inside the pair.
3. Press `c` followed by `i` and `(`.
4. Repeat the same flow with an empty quoted pair such as `""` using `ci"`.

## Expected Behavior

- The inner text object should resolve to the empty region between the delimiters.
- The editor should treat `c` as a successful change operation.
- The cursor should land at the text-object start position.
- Insert mode should begin immediately, even though there was nothing to delete.

## Actual Behavior

- The resolved inner range is zero-length.
- `Window` treats the operation as a no-op because the range start and end are equal.
- `Change` returns `NotHandled`, so the main loop never switches into insert mode.
- The cursor stays in normal mode at its original position.

## Impact

- Empty placeholders inside brackets or quotes cannot be edited with the expected `ci...` workflow.
- This makes common patterns like `()`, `[]`, `{}`, `""`, and `''` awkward to fill in.
- The behavior diverges from the rest of urvim's operator-pending editing model, where `c` is supposed to be the operator that leads into insert mode.

## Root Cause

The characterwise operator path rejects empty resolved ranges before the operator result is finalized.

In [`src/window/commands.rs`](/Users/ryan/Dev/urvim/src/window/commands.rs), `handle_characterwise_operation_with_count` returns `None` when `range.start == range.end`:

- the resolved range from [`src/buffer/bracket_text_object.rs`](/Users/ryan/Dev/urvim/src/buffer/bracket_text_object.rs) or [`src/buffer/quote_text_object.rs`](/Users/ryan/Dev/urvim/src/buffer/quote_text_object.rs) can legitimately be empty for inner empty pairs
- `None` flows into `operation_noop_result`
- `operation_noop_result(Operator::Change)` returns `NotHandled`
- the app-level loop only switches into insert mode when the action result is `Handled`

So the range resolver is working, but the window layer discards the successful empty change before the mode switch can happen.

## Solution Approach

Treat empty inner text-object ranges as a successful `Change` operation instead of a failed operation.

The fix should:

- keep delete behavior unchanged for empty ranges
- keep unmatched or unresolvable text objects as no-ops
- position the cursor at the resolved insertion point for empty `Change` targets
- allow the existing app-level `switches_to_insert_mode()` path to enter insert mode after the window reports success

Rejected alternatives:

- Widening the text-object range in the buffer layer would make `ci(` and `ci"` behave like around-object commands and would delete delimiters unexpectedly.
- Forcing insert mode from the window layer would duplicate logic that already lives in the main event loop.

## Code Changes

- [`src/window/commands.rs`](/Users/ryan/Dev/urvim/src/window/commands.rs): handle zero-length characterwise change targets as a successful operation that leaves the cursor at the text-object start.
- [`src/window/tests.rs`](/Users/ryan/Dev/urvim/src/window/tests.rs): add regression coverage for `ci(` and `ci"` on empty pairs.
- [`src/buffer/tests.rs`](/Users/ryan/Dev/urvim/src/buffer/tests.rs): add or extend range tests if needed to document the empty inner range contract.

## Edge Cases

- `ca(` and `ca"` on empty pairs should continue to delete the delimiters and enter insert mode.
- `di(` and `di"` on empty pairs should remain no-ops, since there is nothing to delete.
- Unmatched bracket or quote targets should still leave the buffer unchanged.
- Counts should not change the behavior of empty inner change targets.
- Nested text objects should still resolve to the innermost valid pair before the empty-range check runs.
