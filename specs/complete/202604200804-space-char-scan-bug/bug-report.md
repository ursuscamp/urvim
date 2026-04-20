# 202604200804-space-char-scan-bug: Character scan motions do not match literal space

## Summary
Character scan motions and their operator-pending range equivalents do not treat `<Space>` as a literal target character. `f<Space>`, `t<Space>`, `F<Space>`, and `T<Space>` fail to move or resolve a range when the target is a space, even though repeated spaces should behave like repeated occurrences of any other character.

## Severity: Medium

This is a core navigation bug. It does not corrupt data, but it breaks a common Vim motion and its operator-pending form.

## Environment
- Workspace: `/Users/ryan/Dev/urvim`
- Affected area: normal-mode character scan motions and operator-pending character scan range motions
- Observed with literal space targets only

## Reproduction Steps
1. Open a buffer containing `hello world`.
2. Place the cursor on the `h`.
3. Press `f<Space>`.
4. Observe that the cursor does not move.
5. Repeat with `t<Space>`, `F<Space>`, and `T<Space>`.
6. Repeat with an operator-pending form such as `d<Space>` after the same setup.
7. Observe that the motion or range resolution fails in the same way.

## Expected Behavior
- `f<Space>` should move to the next literal space on the line.
- `t<Space>` should move to just before the next literal space on the line.
- `F<Space>` should move backward to the previous literal space on the line.
- `T<Space>` should move backward to just after the previous literal space on the line.
- Operator-pending character scan motions should resolve ranges using the same literal-space behavior.
- Repeated spaces should behave like repeated occurrences of any other repeated character.

## Actual Behavior
- `f<Space>` does not move the cursor.
- `t<Space>`, `F<Space>`, and `T<Space>` fail in the same way.
- Operator-pending versions of the same motions also fail to resolve against a literal space.

## Impact
- Users cannot navigate to spaces with standard character scan motions.
- Common editing commands like delete/change/yank over a space target do not work as expected.
- The bug makes space-target motions feel inconsistent with all other literal character targets.

## Root Cause
The character scan path likely treats `<Space>` as a special input token instead of a literal character target somewhere in key parsing, motion dispatch, or range resolution. The failing behavior suggests the literal space never reaches the motion logic intact.

## Solution Approach
Add explicit bindings for space-target character scan motions and operator-pending range motions so `<Space>` is handled directly as a keymap case instead of being converted back into a parsed character target. The fix should cover `f<Space>`, `F<Space>`, `t<Space>`, `T<Space>`, and the matching operator-pending motions.

Rejected alternatives:
- Converting every `<Space>` key back into a parsed `char(' ')`, which would require broader parser changes than needed.
- Treating tabs as equivalent to spaces, which is not part of the bug and would change unrelated behavior.
- Broadly redefining whitespace search semantics, which would exceed the scope of this regression.

## Code Changes
- Add explicit keymap entries for the space-target character scan motions.
- Ensure normal-mode and operator-pending character scan motions both route those entries to the existing motion logic.
- Add regression tests covering `f<Space>`, `t<Space>`, `F<Space>`, `T<Space>`, and at least one operator-pending space-target case.

## Edge Cases
- Consecutive spaces should be handled the same way as repeated occurrences of any other character.
- Only the literal space character should be affected; tabs should remain unchanged.
- No-occurrence cases should continue to leave the cursor or range unchanged.
