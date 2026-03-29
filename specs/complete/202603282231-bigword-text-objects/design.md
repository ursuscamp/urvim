# BigWord Text Objects - Technical Design

## Architecture Overview

This feature extends urvim's existing operator-pending text-object path so whitespace-delimited selections are resolved through the same `Action::Operation(Operator, TextObject)` flow already used for `iw`, `aw`, bracket text objects, and quote text objects.

The runtime path stays the same:

```text
Keypress -> NormalMode keymap
         -> Action::Operation(Operator::Delete, TextObject::InnerBigWord)
         -> Window command handling
         -> Buffer resolves a BigWord range
         -> Buffer deletes or changes the resolved range
         -> Cursor lands at the start of the affected region
```

The main design goal is to reuse the current operator and text-object architecture rather than creating a separate BigWord-specific command path.

## Interface Design

### Action Model

Extend `TextObject` with BigWord variants:

```rust
pub enum TextObject {
    InnerWord,
    AroundWord,
    InnerBigWord,
    AroundBigWord,
    InnerBracket(BracketKind),
    AroundBracket(BracketKind),
    InnerQuote(QuoteKind),
    AroundQuote(QuoteKind),
}
```

The `Operator` and `Action::Operation` shapes do not change.

### Normal Mode Keymap

Register direct operator-pending sequences for the new BigWord text objects:

```text
diW -> Operation(Delete, TextObject::InnerBigWord)
daW -> Operation(Delete, TextObject::AroundBigWord)
ciW -> Operation(Change, TextObject::InnerBigWord)
caW -> Operation(Change, TextObject::AroundBigWord)
```

The capital `W` binding should be distinct from the existing `w`-family word object bindings and should not alter their behavior.

### Buffer Range Resolution

Add BigWord-aware range helpers in the buffer layer:

```rust
impl Buffer {
    pub fn get_inner_big_word_range(
        &self,
        cursor: Cursor,
    ) -> Option<TextObjectRange>;

    pub fn get_around_big_word_range(
        &self,
        cursor: Cursor,
    ) -> Option<TextObjectRange>;
}
```

The helpers should:

- treat a BigWord as a contiguous run of non-whitespace characters
- select the BigWord containing the cursor when the cursor is inside a run
- select the whitespace region when the cursor is on whitespace between runs
- for the around variant, include trailing whitespace immediately after the resolved BigWord or whitespace region
- preserve the existing `TextObjectRange` contract of start-inclusive, end-exclusive ranges

The BigWord resolver should follow the same cursor-adjacent behavior as the existing `iw`/`aw` implementation, but use `Boundary::BigWord` semantics instead of alphanumeric word semantics.

## Data Models

### `TextObject`

- Type: enum
- Purpose: represent operator-pending text objects
- Constraints:
  - existing word, bracket, and quote variants remain unchanged
  - BigWord variants must be copyable and explicit
  - `InnerBigWord` and `AroundBigWord` should map cleanly to the `W` key family

### `TextObjectRange`

No schema change is required.

The range model remains:

- `start: Cursor`
- `end: Cursor`

## Key Components

### `src/editor/normal.rs`

Responsibilities:

- register `diW` and `daW` in the normal-mode keymap
- keep `d` and `c` waiting while a BigWord text-object prefix is partial
- preserve existing count parsing and operator-pending behavior

### `src/editor/action.rs`

Responsibilities:

- define the BigWord text-object variants on `TextObject`
- keep `Action::Operation` unchanged so new bindings flow through the same operator path

### `src/buffer/text_object.rs`

Responsibilities:

- resolve inner and around BigWord ranges
- preserve the existing whitespace-handling model used by the current word text objects
- keep BigWord logic separate from bracket and quote resolvers

If the implementation grows, the BigWord helpers can share private cursor-scanning utilities with the existing word logic, but the public buffer API should stay focused and explicit.

### `src/window/commands.rs`

Responsibilities:

- apply the resolved BigWord range through the existing delete/change execution path
- preserve cursor placement and undo snapshot behavior

## User Interaction

### Key Sequence Behavior

The new commands should behave the same way as other operator-pending text objects:

```text
d -> wait
  i -> wait for inner object completion
  a -> wait for around object completion
```

BigWord commands resolve on the third keystroke, for example `diW`, `daW`, `ciW`, or `caW`.

### Range Semantics

BigWord text objects should behave as follows:

1. Inner BigWord
   - select only the contiguous non-whitespace run under the cursor when the cursor is inside a token
   - select the contiguous whitespace region when the cursor is on whitespace

2. Around BigWord
   - include the same region as `iW`
   - extend the selection through any whitespace immediately following that region

3. Counts
   - counts should multiply just like the existing text objects
   - `3diW`, `d3iW`, and `3d3iW` should continue to resolve through the normal `Action::Count` path

### Example Behavior

| Cursor Position | `diW` deletes | `daW` deletes |
| --- | --- | --- |
| inside `foo-bar` in `foo-bar baz` | `foo-bar` | `foo-bar ` |
| inside whitespace between `foo-bar` and `baz` | the whitespace region | the whitespace region plus `baz` |
| inside `baz` at end of line | `baz` | `baz` plus trailing whitespace if present |

## External Dependencies

No new external dependencies are required.

The feature should reuse:

- existing trie keymap support
- existing buffer cursor and grapheme traversal logic
- existing undo/redo infrastructure

## Error Handling

| Scenario | Behavior |
| --- | --- |
| Cursor is on an empty buffer | `diW`/`daW` do nothing |
| Cursor is on a line without any non-whitespace characters | `diW`/`daW` do nothing |
| Escape during operator-pending input | Cancel the operation and return to normal mode |
| Invalid operator-pending sequence | Return no action and leave the buffer unchanged |

The implementation should not fabricate a range when no contiguous BigWord or whitespace region can be resolved.

## Security

No security-sensitive behavior is introduced.

The feature only interprets local editor text and key input already available to the process.

## Configuration

No new configuration options are required.

## Component Interactions

```mermaid
flowchart LR
    K[Normal mode key input] --> M[Trie keymap]
    M --> A[Action::Operation]
    A --> W[Window command handler]
    W --> B[Buffer text-object resolver]
    B --> D[BigWord range helper]
    D --> R[TextObjectRange]
    R --> W
```

The BigWord resolver should stay inside the buffer layer so operator execution does not need to know about delimiter families or whitespace scanning details.

## Platform Considerations

- The implementation must remain Unicode-safe for cursor positions and range boundaries.
- BigWord selections should behave consistently across supported terminals.
- Capital `W` key bindings should not depend on platform-specific keyboard behavior beyond normal character input.
