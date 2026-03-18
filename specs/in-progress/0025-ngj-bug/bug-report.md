# BUG-024: NgJ key sequence (count + gJ) returns wrong action

## Summary

When pressing a count prefix followed by gJ (e.g., `3gJ` to join 4 lines without space), the editor incorrectly executes `JoinWithSpace` instead of `JoinWithoutSpace`. This prevents users from using count-prefixed gJ joins without spaces.

## Severity: Medium

- Affects users who want to join multiple lines without spaces using a count prefix
- Workaround: Use J for joins with space, or manually invoke gJ multiple times
- The individual keys work correctly (J and gJ separately work fine)

## Environment

| Field | Value |
|-------|-------|
| App Version | 0.1.0 |
| OS | macOS / Linux |
| Branch | feature/join-line-motions |

## Reproduction Steps

1. Open urvim with a multi-line buffer (e.g., "a\nb\nc\nd")
2. Press `3` (count prefix)
3. Press `g`
4. Press `J`
5. Observe: The lines are joined WITH spaces ("a b c")
6. Expected: The lines should be joined WITHOUT spaces ("abcd")

## Expected Behavior

`3gJ` on "a\nb\nc\nd" should produce "abcd" (join 4 lines without spaces)

## Actual Behavior

`3gJ` on "a\nb\nc\nd" produces "a b c" (join 4 lines WITH spaces)

Note: `gJ` without a count prefix works correctly and produces "abcd".

## Impact

- Users cannot efficiently join multiple lines without spaces using a count
- Forces workaround: either use J with spaces, or manually repeat gJ
- Low frequency: most users use J (with space) rather than gJ (without space)

## Root Cause

The bug is in the keymap parsing logic in `NormalMode::handle_key()`. When there's a pending count and a key is pressed that could start both a single-key sequence (J -> JoinWithSpace) and a multi-key sequence (gJ -> JoinWithoutSpace), the single-key binding takes precedence.

Location: `src/editor.rs` - in the section that handles keys with pending counts (lines ~394-426)

The logic checks if the key alone matches a single-key binding first, and if so, applies the count to that action rather than waiting to see if it could be part of a multi-key sequence.

Debug output shows:
- After pressing `3g`: buffer = ["g"], pending_count = Some(3)
- After pressing `J`: result = Complete(Count(3, JoinWithSpace)) - should be JoinWithoutSpace

## Solution Approach

**Chosen**: Modify keymap matching to prefer multi-key sequences when there's a pending count and the buffer could be a prefix of a multi-key sequence.

**Reasoning**:
- Matches expected Vim behavior where counts apply to the complete sequence
- Minimal impact on existing behavior
- Could also benefit other multi-key sequences with counts

**Rejected alternatives**:
- Add explicit special handling for gJ in the count branch: Too specific, doesn't fix the general problem
- Change keymap to not include J as single-key when gJ is registered: Would break standalone J functionality

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/editor.rs` | Modify | In `handle_key()`, when there's a pending count, check if buffer could be prefix of multi-key sequence before applying count to single-key match |

## Edge Cases

- Verify `gJ` without count still works (confirmed working)
- Verify `NJ` (e.g., `3J`) still works (confirmed working)
- Test other multi-key sequences with counts (e.g., gg, if there are others)
- Ensure the fix doesn't break any existing single-key bindings
