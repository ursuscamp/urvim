# Delete Line Motions - Technical Design

## Architecture Overview

This feature extends urvim's generalized delete-operator flow so `d` can target:

- line-local motions: `$`, `0`, `^`
- file-position motions: `gg`, `G`

The current delete architecture already supports `d{motion}` for word-family boundary motions and `d{text-object}` for word text objects. This design keeps that single operator execution path, expands boundary motions to also cover line-local anchors, and adds an explicit linewise target variant for file-position ranges.

The main distinction in this feature is that `gg` and `G` stop behaving like ordinary counted operator targets once paired with `d`: their counts are interpreted as motion destination line numbers, and the resulting delete is linewise.

## Interface Design

### Action model

Extend the operator-target model so delete commands can distinguish characterwise boundary targets from linewise file-motion targets:

```rust
pub enum OperatorTarget {
    TextObject(TextObject),
    BoundaryMotion(BoundaryMotion),
    LinewiseMotion(LinewiseMotion),
}

pub enum LinewiseMotion {
    FirstLine,
    LastLine,
}
```

This keeps the public interface shallow:

- `BoundaryMotion` expands to cover both word-family traversal and same-line anchors
- `LinewiseMotion` covers whole-line deletes driven by vertical file motions

### Normal mode keymap

Add the following operator-pending sequences in [`src/editor/normal.rs`](/Users/ryan/Dev/urvim/src/editor/normal.rs):

```rust
"d$"  -> Operation(Delete, BoundaryMotion(LineEnd))
"d0"  -> Operation(Delete, BoundaryMotion(LineStart))
"d^"  -> Operation(Delete, BoundaryMotion(LineContentStart))
"dgg" -> Operation(Delete, LinewiseMotion(FirstLine))
"dG"  -> Operation(Delete, LinewiseMotion(LastLine))
```

### Count parsing rule for `d0`

The normal-mode parser currently treats digit sequences as counts before resolving motion keys. `d0` needs one targeted exception: once an operator prefix has consumed `d`, a following `0` must be available as a motion key instead of being absorbed as a count digit.

This is best handled in the count/parser boundary where operator-pending sequences are recognized, rather than by special-casing execution later.

### Buffer/operator resolution API

The existing operator-target API should remain the main entry point:

```rust
impl Buffer {
    pub fn get_operator_target_range(
        &self,
        cursor: Cursor,
        target: OperatorTarget,
    ) -> Option<TextObjectRange>;

    pub fn get_operator_target_range_with_count(
        &self,
        cursor: Cursor,
        target: OperatorTarget,
        count: usize,
    ) -> Option<TextObjectRange>;
}
```

The behavior changes are:

- expanded `BoundaryMotion` resolves both existing word-family motions and the new same-line anchors to a characterwise `TextObjectRange`
- `LinewiseMotion` resolves to a linewise deletion range
- counted `LinewiseMotion` uses the count as a destination line number, not as a multiplicative repeat count

If `TextObjectRange` cannot clearly represent whole-line deletion including newline handling, the buffer layer should introduce a focused public delete-target range type instead of overloading characterwise ranges.

## Data Models

### `OperatorTarget`

- Type: enum
- Purpose: unify operator-pending delete target kinds
- New constraints:
  - line-anchor `BoundaryMotion` variants always resolve within the current line
  - `LinewiseMotion` always resolves to whole-line deletion semantics

### `BoundaryMotion`

- Type: enum
- Purpose: model characterwise delete targets driven by existing motion semantics
- Mapping:
  - existing variants such as `WordForward`, `WordEnd`, and `WordBackward` remain unchanged
  - `LineEnd` -> `$`
  - `LineStart` -> `0`
  - `LineContentStart` -> `^`

### `LinewiseMotion`

- Type: enum
- Purpose: model delete targets that operate on whole lines
- Mapping:
  - `FirstLine` -> `gg`
  - `LastLine` -> `G`

### Range representation

The buffer layer needs to represent two behaviors:

1. Characterwise same-line delete ranges for `d$`, `d0`, and `d^`
2. Whole-line delete ranges for `dgg` and `dG`

Two viable designs:

- keep `TextObjectRange` for characterwise deletes and add a dedicated linewise delete payload
- generalize the existing range type with a `linewise: bool` flag

Preferred direction: add a focused operator-range payload if needed, because linewise deletion has distinct semantics for newline consumption and cursor placement.

## Key Components

### `src/editor/action.rs`

Responsibilities:

- extend the public operator-target model
- keep `Action::Operation` countable and snapshottable
- document which `BoundaryMotion` variants are same-line anchors and which targets are linewise

Public API changes:

- extend `BoundaryMotion` with `LineStart`, `LineContentStart`, and `LineEnd`
- add `LinewiseMotion`
- extend `OperatorTarget`

### `src/editor/normal.rs`

Responsibilities:

- register the new `d{motion}` bindings
- preserve prefix waiting for `d` and `dg`
- ensure `d0` is parsed as a motion, not a count prefix
- preserve the existing line-number count behavior for raw `gg` and `G`

### `src/buffer/operator_target.rs`

Responsibilities:

- resolve word-family and line-anchor characterwise delete ranges
- resolve file-motion linewise delete spans
- centralize the special counted behavior for `dgg` and `dG`

Public API additions or changes:

- helper methods for the new `BoundaryMotion` line-anchor variants
- helper methods for `LinewiseMotion`
- a linewise-aware operator target result if the existing range shape is insufficient

### `src/window/commands.rs`

Responsibilities:

- execute delete operations for both characterwise and linewise targets
- preserve one-snapshot-per-operation undo behavior
- place the cursor at the first surviving line or the start of the deleted span, depending on the resolved target type

## User Interaction

### Key sequence behavior

```text
d  -> wait
  $  -> delete to end of line
  0  -> delete to start of line
  ^  -> delete to first non-whitespace on line
  g  -> wait
    g -> delete linewise to first line
  G  -> delete linewise to last line
```

### Count behavior

Counts split into two groups:

1. Same-line line motions
   - `2d$` should keep following urvim's existing count model for line actions and delete targets
   - `d0` must still resolve as motion `0`, not as an operator sub-count prefix

2. File-position linewise motions
   - `dgg` -> delete current line through line 1
   - `d5gg` -> delete current line through line 5
   - `dG` -> delete current line through last line
   - `d5G` -> delete current line through line 5

For `dgg` and `dG`, the delete target is defined by the motion's resolved destination line. The operation must not reinterpret that count as "repeat the delete N times."

### Range semantics

1. `d$`
   - start = original cursor
   - end = exclusive cursor position at end of current line

2. `d0`
   - start = column 0 of current line
   - end = original cursor

3. `d^`
   - start = first non-whitespace column of current line
   - end = original cursor

4. `dgg`
   - target line = 0 when no count is present, otherwise `count - 1`
   - start line = min(current line, target line)
   - end line = max(current line, target line)
   - delete all full lines in that inclusive span

5. `dG`
   - target line = last line when no count is present, otherwise `count - 1`
   - start line = min(current line, target line)
   - end line = max(current line, target line)
   - delete all full lines in that inclusive span

## External Dependencies

No new external dependencies are required.

The feature reuses:

- existing trie keymap and prefix handling
- existing line motions
- existing delete snapshot and undo infrastructure

## Error Handling

Expected cases and behavior:

| Scenario | Behavior |
| --- | --- |
| `d0` at column 0 | Resolve an empty range and leave the buffer unchanged |
| `d^` on a line with no non-whitespace characters | Resolve to the start of the line and delete back to the cursor if applicable |
| `dgg` while already on line 1 | Delete the current first line linewise |
| `d5G` when the buffer has fewer than 5 lines | Clamp the target to the last line |
| Counted operator target resolves to the current line only | Delete that single line linewise for `dgg`/`dG` |

## Security

No new security concerns are introduced. All inputs remain local keyboard events interpreted inside the editor process.

## Configuration

No configuration changes are required.

## Component Interactions

```text
Keypress -> NormalMode keymap/count parser
         -> Action::Operation(Delete, OperatorTarget::{BoundaryMotion|LinewiseMotion})
         -> Window::handle_operation_with_count()
         -> Buffer resolves target span
         -> Buffer deletes text or full lines
         -> Window updates cursor
```

The main design constraint is to keep the "count means destination line number" rule localized to the `gg`/`G` delete target resolution instead of leaking that exception into unrelated operator targets.

## Platform Considerations

The behavior is terminal-platform agnostic. It depends only on key canonicalization already used by normal mode.
