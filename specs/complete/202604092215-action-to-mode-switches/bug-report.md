# 202604092215-action-to-mode-switches: Insert-mode transitions should come from `Action.to_mode`
## Summary
`Action::switches_to_insert_mode()` currently hardcodes a list of action kinds that should enter insert mode after they succeed. That duplicates information that already belongs on the action envelope itself and makes the mode transition depend on a second source of truth instead of `to_mode`.

## Severity: Medium
This is a correctness and maintainability bug. It does not appear to crash the editor, but it creates inconsistent behavior risk whenever a new action should transition into insert mode, and it forces unrelated code to keep duplicating mode-switch knowledge.

## Environment
- Repository: `urvim`
- Relevant files:
  - `src/editor/action.rs`
  - `src/main.rs`
  - `src/editor/normal.rs`

## Reproduction Steps
1. Open the editor and trigger any action that is meant to leave normal mode and enter insert mode, such as `i`, `a`, `A`, `I`, `o`, `O`, `cc`, or `C`.
2. Observe that some actions enter insert mode because their keymap entry uses `Action::mode_transition(ModeKind::Insert)`, while others rely on `Action::switches_to_insert_mode()` recognizing the `ActionKind`.
3. Add a new action that should behave like an existing insert-entering action but does not match the hardcoded list, then dispatch it through the normal event loop.
4. The new action will not switch modes unless it is also added to the hardcoded helper, even if it is otherwise identical to an existing action.

## Expected Behavior
Whether an action switches to insert mode should be encoded entirely in the action’s `to_mode` metadata.

If two actions differ only by whether they enter insert mode afterward, they should be represented as a single action kind with different `to_mode` values rather than as separate hardcoded cases.

## Actual Behavior
`Action::switches_to_insert_mode()` in `src/editor/action.rs:442-457` checks `to_mode == Some(ModeKind::Insert)` first, but then falls back to a hardcoded match on specific `ActionKind` values and recursive special cases for `Count` and `Operation(Operator::Change, _)`.

`src/main.rs:132-157` still decides whether to enter insert mode by calling that helper after an action is handled, so the event loop is coupled to the helper’s special cases rather than to the action metadata itself.

## Impact
- New insert-entering actions can be missed unless the helper is updated.
- Action behavior is split across keymap construction and a separate mode-switch list.
- Related actions can drift apart even when they should share the same underlying action kind.
- Tests and callers have to account for both the hardcoded list and the `to_mode` field.

## Root Cause
The codebase currently treats “enters insert mode” as a property that can be inferred from `ActionKind` instead of as explicit metadata on `Action`.

That is why the helper has to special-case `AppendAfterCursor`, `AppendToLineEnd`, `InsertAtLineStart`, `ChangeLine`, `ChangeToLineEnd`, `OpenLineBelow`, `OpenLineAbove`, `Count`, and `Operation(Operator::Change, _)` even though the `Action` type already has `to_mode` for this purpose.

## Solution Approach
Set `to_mode` directly on the actions that should enter insert mode and remove the `ActionKind`-based fallback from `switches_to_insert_mode()`.

Where actions are currently duplicated only to differ by mode transition behavior, consolidate them into a single action definition and vary only the `to_mode` metadata.

Rejected alternative: keep the hardcoded helper and add more special cases for newly discovered actions. That preserves the inconsistency and makes the mode logic harder to reason about over time.

## Code Changes
- `src/editor/action.rs`
  - Remove the hardcoded `ActionKind` match from `switches_to_insert_mode()`.
  - Ensure nested actions such as `Count` continue to preserve `to_mode` metadata correctly.
- `src/editor/normal.rs`
  - Set `to_mode = Some(ModeKind::Insert)` for the actions that should enter insert mode.
  - Consolidate any paired actions that only differ by this mode transition behavior.
- `src/main.rs`
  - Keep the event loop using `to_mode` as the single source of truth for mode transitions.
- Tests
  - Update existing action trait tests to assert insert-mode transitions through `to_mode` instead of hardcoded `ActionKind` recognition.
  - Add regression coverage for wrapped/count actions so `to_mode` survives the wrapper.

## Edge Cases
- `Count`-wrapped actions should preserve their inner action’s `to_mode`.
- Actions with `kind == None` and `to_mode == Some(ModeKind::Insert)` should still switch modes correctly.
- Actions that previously relied on the helper but do not explicitly set `to_mode` should fail visibly in tests so they can be corrected rather than silently inferred.
- The change should not affect actions that already switch to normal mode via `to_mode`.
