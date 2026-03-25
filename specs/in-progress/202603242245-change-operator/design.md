# Change Operator - Technical Design

## Architecture Overview

The change operator extends urvim's existing operator-pending editing flow by adding `Change` as a first-class operator. The keymaps for `c`-prefixed commands will resolve to `Action::Operation(Operator::Change, OperatorTarget::...)`, and the existing operator-target resolution machinery will delete the target text.

The only semantic difference from delete is the post-operation mode transition:

- successful change operations enter insert mode
- empty or impossible targets do not modify the buffer and do not switch modes

The current implementation already has generalized operator-target support in `src/editor/action.rs`, `src/editor/normal.rs`, `src/buffer/operator_target.rs`, and `src/window/commands.rs`, so this feature is primarily a behavioral extension rather than a new architecture.

### Flow

```text
Keypresses
  -> NormalMode keymap resolves a c-prefixed sequence
  -> Action::Operation(Operator::Change, target)
  -> Window resolves target range
  -> Buffer deletes the resolved range
  -> Window returns handled only when text actually changed
  -> main.rs switches to InsertMode for successful change actions
```

## Interface Design

### Action Model

No new `Action` variant is required. The existing operator-pending shape remains:

```rust
pub enum Operator {
    Delete,
    Change,
}

pub enum Action {
    // ...
    Operation(Operator, OperatorTarget),
}
```

### Action Semantics

- `Action::Operation(Operator::Change, ...)` is countable.
- `Action::Operation(Operator::Change, ...)` is snapshottable.
- `Action::Operation(Operator::Change, ...)` switches to insert mode when the operation succeeds.
- `Action::Operation(Operator::Delete, ...)` keeps its existing normal-mode behavior.
- `Count(_, Operation(Change, ...))` must recurse the same way as other counted actions.

### Keymap Configuration

Register the change-operator sequences alongside the existing delete sequences:

| Sequence | Target |
| --- | --- |
| `cw` | `BoundaryMotion::WordForward` |
| `ce` | `BoundaryMotion::WordEnd` |
| `cb` | `BoundaryMotion::WordBackward` |
| `cW` | `BoundaryMotion::BigWordForward` |
| `cE` | `BoundaryMotion::BigWordEnd` |
| `cB` | `BoundaryMotion::BigWordBackward` |
| `ciw` | `TextObject::InnerWord` |
| `caw` | `TextObject::AroundWord` |
| `c$` | `BoundaryMotion::LineEnd` |
| `c0` | `BoundaryMotion::LineStart` |
| `c^` | `BoundaryMotion::LineContentStart` |
| `cgg` | `LinewiseMotion::FirstLine` |
| `cG` | `LinewiseMotion::LastLine` |

The initial `c` remains a prefix key, so the normal mode waiting behavior does not need new state.

## Data Models

No new persistent data structures are required.

The design reuses:

- `Operator`
- `OperatorTarget`
- `BoundaryMotion`
- `LinewiseMotion`
- `TextObjectRange`
- `LinewiseDeleteRange`

The only model change is that `Operator` gains a `Change` variant.

## Key Components

### `src/editor/action.rs`

Responsibilities:

- define `Operator::Change`
- classify change operations as countable and snapshottable
- make `switches_to_insert_mode()` return `true` for successful change actions

### `src/editor/normal.rs`

Responsibilities:

- register `c`-prefixed operator sequences
- preserve partial-sequence waiting for `c`, `ci`, `cg`, and `c` plus count prefixes
- keep existing delete bindings unchanged

### `src/window/commands.rs`

Responsibilities:

- route `Operator::Change` through the same target-resolution helpers used by delete
- apply the deletion, then report success only if text was actually removed
- leave the cursor at the start of the changed region
- preserve the existing linewise target handling path

### `src/main.rs`

Responsibilities:

- continue switching to insert mode when `action.switches_to_insert_mode()` is true
- rely on the window layer to suppress the mode switch when the operation was a no-op

## User Interaction

### Command Behavior

| Command | Behavior |
| --- | --- |
| `cw` | Change through the next word boundary and enter insert mode |
| `ciw` | Change the inner word under the cursor and enter insert mode |
| `caw` | Change around the current word and enter insert mode |
| `c$` | Change from the cursor to the end of the line and enter insert mode |
| `cgg` | Change from the current line to the first line and enter insert mode |
| `cG` | Change from the current line to the last line and enter insert mode |

### Count Behavior

Counts should behave the same way they do for the matching delete commands:

- `2cw` changes through the second word-forward boundary
- `d3iw` is already supported for delete, and `c3iw` should behave analogously
- combined counts continue to multiply where existing parsing already does so

### No-Op Behavior

If the resolved target cannot remove any text, the editor should:

- leave the buffer unchanged
- keep the cursor where it was
- remain in normal mode
- avoid creating a change-mode snapshot

This keeps edge cases like `cw` at end-of-line or `cG` on an empty selection predictable.

## External Dependencies

No new external dependencies are required.

The feature reuses:

- existing trie-based key handling
- existing buffer range-resolution helpers
- existing snapshot infrastructure
- existing mode switching in the main event loop

## Error Handling

| Scenario | Behavior |
| --- | --- |
| Invalid `c` sequence | Existing normal-mode invalid-sequence handling applies |
| Empty or impossible range | No-op, remain in normal mode |
| Count of zero | Rejected by existing count validation |
| Range resolution failure at buffer edge | No-op, remain in normal mode |

## Security

Not applicable. This is a local text-editing feature with no new security surface.

## Configuration

No new configuration is required.

## Component Interactions

```text
NormalMode
  -> Action::Operation(Operator::Change, target)
Window::process_action
  -> resolve range
  -> delete range if possible
  -> return handled only on success
main.rs
  -> if handled and action switches modes, enter InsertMode
```

## Platform Considerations

No platform-specific concerns. The implementation uses the existing Unicode-aware buffer and terminal handling already present in urvim.
