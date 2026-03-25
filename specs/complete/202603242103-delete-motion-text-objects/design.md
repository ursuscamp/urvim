# Delete Motion Text Objects - Technical Design

## Architecture Overview

This feature extends urvim's existing operator-pending delete flow so `d` can target either:

- existing word text objects (`iw`, `aw`)
- new motion-matched delete targets (`w`, `e`, `b`, `W`, `E`, `B`)

The current architecture already routes `diw`/`daw` through `Action::Operation(Operator, TextObject)` and executes the resolved range in `Window`. The new design keeps that execution path, but generalizes the operator target so boundary-based delete targets can be resolved without pretending they are word text objects.

### Proposed flow

```text
Keypress -> NormalMode keymap
         -> Action::Operation(Operator::Delete, OperatorTarget::BoundaryMotion(...))
         -> Window::handle_operation()
         -> Buffer resolves a deletion range from cursor + target + count
         -> Buffer::delete_range()
         -> Cursor moves to deleted range start
```

### Key decisions

1. Keep a single operator execution path for both existing and new delete targets.
2. Introduce a dedicated operator-target model instead of overloading `TextObject` with motion-like variants.
3. Resolve delete ranges in the buffer layer, where boundary semantics already live.
4. Reuse existing `Boundary`, `next_boundary`, and `prev_boundary` behavior so delete targets stay aligned with urvim's documented motion semantics.

## Interface Design

### Action model

Replace the current `Action::Operation(Operator, TextObject)` shape with a generalized target type:

```rust
pub enum OperatorTarget {
    TextObject(TextObject),
    BoundaryMotion(BoundaryMotion),
}

pub enum BoundaryMotion {
    WordForward,
    WordEnd,
    WordBackward,
    BigWordForward,
    BigWordEnd,
    BigWordBackward,
}

pub enum Action {
    // ...
    Operation(Operator, OperatorTarget),
}
```

This keeps interface intent clear:

- `TextObject` remains for objects like `iw` and `aw`
- `BoundaryMotion` represents delete targets that mirror a motion family
- future operators can reuse the same target system

### Normal mode keymap

Add direct operator-pending sequences:

```rust
"dw" -> Operation(Delete, BoundaryMotion(WordForward))
"de" -> Operation(Delete, BoundaryMotion(WordEnd))
"db" -> Operation(Delete, BoundaryMotion(WordBackward))
"dW" -> Operation(Delete, BoundaryMotion(BigWordForward))
"dE" -> Operation(Delete, BoundaryMotion(BigWordEnd))
"dB" -> Operation(Delete, BoundaryMotion(BigWordBackward))
```

Existing `diw` and `daw` bindings become:

```rust
Operation(Delete, OperatorTarget::TextObject(TextObject::InnerWord))
Operation(Delete, OperatorTarget::TextObject(TextObject::AroundWord))
```

### Buffer range-resolution API

Add a buffer-level API dedicated to operator-target resolution:

```rust
impl Buffer {
    /// Resolves the range affected by an operator target at the given cursor.
    pub fn get_operator_target_range(
        &self,
        cursor: Cursor,
        target: OperatorTarget,
    ) -> Option<TextObjectRange>;

    /// Resolves the range affected by an operator target with count expansion.
    pub fn get_operator_target_range_with_count(
        &self,
        cursor: Cursor,
        target: OperatorTarget,
        count: usize,
    ) -> Option<TextObjectRange>;
}
```

The name keeps the interface shallow while allowing the implementation to delegate internally to existing text-object helpers and new boundary-motion helpers.

## Data Models

### `OperatorTarget`

- Type: enum
- Purpose: unified target model for operator-pending actions
- Constraints:
  - `TextObject` variants are range-like selections
  - `BoundaryMotion` variants are cursor-to-boundary delete targets

### `BoundaryMotion`

- Type: enum
- Purpose: identify which existing boundary family and direction should be used for delete-range resolution
- Mapping:
  - `WordForward` -> `w`
  - `WordEnd` -> `e`
  - `WordBackward` -> `b`
  - `BigWordForward` -> `W`
  - `BigWordEnd` -> `E`
  - `BigWordBackward` -> `B`

### `TextObjectRange`

No schema change. It remains the buffer deletion payload:

- `start: Cursor`
- `end: Cursor`

The range remains start-inclusive and end-exclusive, which is important for expressing:

- `dw`: `[cursor, target_start)`
- `de`: `[cursor, target_end_plus_one_grapheme)`
- `db`: `[target_start, cursor)`

## Key Components

### `src/editor/action.rs`

Responsibilities:

- define the generalized operator target model
- keep `Action::Operation` countable and snapshottable

Public API changes:

- add `OperatorTarget`
- add `BoundaryMotion`
- change `Action::Operation` to store `OperatorTarget`

### `src/editor/normal.rs`

Responsibilities:

- register the new `d{motion}` operator-pending sequences
- preserve existing waiting behavior for partial prefixes like `d`

Dependencies:

- `TrieKeymap`
- `OperatorTarget`
- `BoundaryMotion`

### `src/buffer/text_object.rs`

Responsibilities:

- continue resolving `iw` and `aw`
- become the home for generalized operator-target range resolution, or delegate to a focused helper module if the logic grows

Public API additions:

- `get_operator_target_range`
- `get_operator_target_range_with_count`
- helper methods for boundary-motion range resolution

### `src/window/commands.rs`

Responsibilities:

- execute delete operations against generalized operator targets
- share one snapshot/delete/cursor-placement flow for both single-count and counted operations

Dependencies:

- buffer operator-target range resolution
- existing `delete_range`

## User Interaction

### Key sequence behavior

`d` already enters a wait state because the trie has child sequences. After this change:

```text
d -> wait
  w -> delete to next word start
  e -> delete through word end
  b -> delete backward to previous word start
  W -> delete to next BigWord start
  E -> delete through BigWord end
  B -> delete backward to previous BigWord start
  i -> wait for text object completion
  a -> wait for text object completion
```

### Count behavior

The existing count parser remains unchanged. Counts still wrap the final action:

- `2dw` -> `Count(2, Operation(Delete, BoundaryMotion(WordForward)))`
- `d3e` -> `Count(3, Operation(Delete, BoundaryMotion(WordEnd)))`
- `3d2B` -> `Count(6, Operation(Delete, BoundaryMotion(BigWordBackward)))`

Count execution should resolve the final range once using the multiplied count, matching the current counted text-object implementation.

### Range semantics

Boundary-motion delete targets are resolved as follows:

1. `dw` / `dW`
   - start = original cursor
   - target = `next_boundary(cursor, Word|BigWord)` repeated by count
   - end = target

2. `de` / `dE`
   - start = original cursor
   - target = `next_boundary(cursor, WordEnd|BigWordEnd)` repeated by count
   - end = one grapheme after the resolved end boundary
   - if the boundary is the end of a line or buffer, clamp to the valid exclusive cursor position

3. `db` / `dB`
   - target = `prev_boundary(cursor, Word|BigWord)` repeated by count
   - start = target
   - end = original cursor

These rules keep the delete region aligned with how the matching cursor motion resolves its destination, while still honoring exclusive-end deletion ranges.

## External Dependencies

No new external dependencies are needed.

The feature reuses:

- existing trie keymap support
- existing boundary traversal logic
- existing buffer deletion and undo infrastructure

## Error Handling

Expected cases and behavior:

| Scenario                                                   | Behavior                                    |
| ---------------------------------------------------------- | ------------------------------------------- |
| `d` followed by unsupported key sequence                   | existing invalid-sequence behavior; no edit |
| `dw/de/dW/dE` at a forward edge with no reachable boundary | return `None`; no edit                      |
| `db/dB` at the earliest reachable backward boundary        | return `None`; no edit                      |
| Empty buffer                                               | no range resolves; no edit                  |
| Counted delete where traversal stops before the first step | no edit                                     |

The buffer range resolver should return `None` for "no movement possible" so the window layer can treat it as a handled no-op.

## Security

Not applicable. This is an in-memory local editing feature with no auth, secrets, or privilege boundaries.

## Configuration

No new configuration or user settings are required.

## Component Interactions

```text
NormalMode
  -> TrieKeymap
  -> Action::Operation(Delete, OperatorTarget)

Window::handle_operation / handle_count_operation
  -> Buffer::get_operator_target_range(_with_count)
  -> Buffer::push_snapshot(cursor)
  -> Buffer::delete_range(range)
  -> BufferView::set_cursor(range.start)
```

This design intentionally removes the current split between text-object execution and future motion-target execution. The window only cares that it received a delete operator and a resolvable target range.

## Platform Considerations

No platform-specific behavior is expected.

Relevant existing behavior that must remain intact across platforms:

- grapheme-aware cursor movement and deletion
- Unicode-safe exclusive range handling
- newline handling when boundary traversal crosses lines

## Implementation Notes

### Range resolution helpers

To keep `src/buffer/text_object.rs` from turning into a mixed-responsibility file, add a focused helper submodule if needed, such as `src/buffer/operator_target.rs`, and re-export only the public buffer methods from `src/buffer/mod.rs`.

Recommended helper structure:

```text
src/buffer/text_object.rs        // iw/aw-specific logic
src/buffer/operator_target.rs    // generalized operator target resolution
```

### Inclusive end conversion for `e` / `E`

`next_boundary(..., WordEnd|BigWordEnd)` returns the cursor positioned on the end boundary. `delete_range` expects an exclusive end cursor, so the implementation should advance by one grapheme from the resolved boundary before building the range.

That conversion should live in the buffer layer so the window code stays simple and all cursor-edge handling remains close to boundary traversal.

## Test Plan

### Unit tests

1. Keymap parsing
   - `dw`, `de`, `db`, `dW`, `dE`, `dB` resolve to the expected `Action::Operation` targets
   - `d` remains a prefix that waits for more input

2. Buffer range resolution
   - `dw` from a word start deletes through the next `w` target
   - `de` deletes through the end grapheme of the resolved word
   - `db` resolves a backward range ending at the original cursor
   - `dW/dE/dB` use BigWord semantics on punctuation runs
   - no-op cases at document edges return `None`

3. Counted range resolution
   - `d2w`
   - `2de`
   - `3d2B`
   - counted ranges resolve once from the multiplied count

4. Window execution
   - successful delete pushes one snapshot
   - cursor lands at deleted range start
   - undo restores the deleted content as one logical edit

## File Changes Summary

| File                                                           | Change                                                                              |
| -------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `src/editor/action.rs`                                         | Add `OperatorTarget` and `BoundaryMotion`; update `Action::Operation`               |
| `src/editor/mod.rs`                                            | Re-export any new public operator-target types                                      |
| `src/editor/normal.rs`                                         | Register `dw/de/db/dW/dE/dB` sequences                                              |
| `src/buffer/mod.rs`                                            | Add module export wiring if a focused operator-target module is introduced          |
| `src/buffer/operator_target.rs` or `src/buffer/text_object.rs` | Implement operator-target range resolution                                          |
| `src/window/commands.rs`                                       | Route both counted and uncounted operations through the generalized target resolver |
| `src/editor/tests.rs`                                          | Add key-sequence and count parsing tests                                            |
| `src/buffer/tests.rs`                                          | Add range-resolution and edge-case tests                                            |
| `docs/motions.md`                                              | Document the new delete commands after implementation                               |
