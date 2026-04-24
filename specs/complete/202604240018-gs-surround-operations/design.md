# GS Surround Operations - Technical Design

## Architecture Overview

This feature introduces two new normal-mode command families under the existing keymap/action pipeline:

- `gsr` + two selector keys for replace-surround
- `gsd` + one selector key for delete-surround

The architecture reuses existing responsibilities:

- Normal-mode key buffering and multi-key matching
- Action dispatch in the main editor loop
- Buffer-side delimiter pair resolution used by text objects
- Existing buffer mutation and undo/redo tracking

The design adds a small surround-operation interface layer so key decoding and surround editing remain separate concerns.

## Interface Design

### New action intents

Add explicit action intents for surround operations in `ActionKind`:

- `SurroundReplace { target: DelimiterFamily, replacement: DelimiterFamily }`
- `SurroundDelete { target: DelimiterFamily }`

Constraints:

- `target` and `replacement` are required at action-construction time.
- `SurroundReplace` with equal `target` and `replacement` is treated as a no-op at execution time.

### Delimiter-family parsing

Expose a public parser from single-key canonical string to delimiter family:

- `DelimiterFamily::from_selector_key(key: &str) -> Option<DelimiterFamily>`

Behavior contract:

- Accept both opener and closer for bracket families.
- Accept exact quote character for quote families.
- Return `None` for unsupported keys.

### Buffer surround APIs

Expose buffer-level operations that execute a full surround edit atomically:

- `Buffer::replace_surround(cursor: Cursor, target: DelimiterFamily, replacement: DelimiterFamily) -> Option<Cursor>`
- `Buffer::delete_surround(cursor: Cursor, target: DelimiterFamily) -> Option<Cursor>`

Response semantics:

- `Some(cursor)` on successful mutation.
- `None` when no resolvable surrounding pair exists or operation is an intentional no-op.

## Data Models

### `DelimiterFamily`

A new public enum representing all supported surround families:

- `Paren`
- `Square`
- `Curly`
- `Angle`
- `DoubleQuote`
- `SingleQuote`
- `Backtick`

Fields: none (closed enum).

Constraints:

- Must map bidirectionally to delimiter characters used for matching and replacement.
- Bracket families expose opening and closing delimiters.
- Quote families expose same delimiter for open/close.

### `SurroundPairRange`

Internal buffer-only data structure describing the located surrounding pair:

- `open: Cursor`
- `close: Cursor`

Constraints:

- `open` is strictly before `close` in document order.
- Positions are guaranteed to point at delimiters of the requested family.

## Key Components

### Normal mode keymap binding

Responsibility:

- Convert buffered keys matching `gsd?` / `gsr??` into the corresponding new action payload.

Public API impact:

- Extends existing normal-mode keymap registration.

Dependencies:

- Existing trie/chained keymap interfaces.
- `DelimiterFamily::from_selector_key`.

### Action dispatch integration

Responsibility:

- Route `SurroundReplace` and `SurroundDelete` through existing mutation flow.

Public API impact:

- Adds execution match arms for new `ActionKind` variants.

Dependencies:

- Window/buffer access helpers.
- New `Buffer` surround APIs.
- Existing undo snapshot lifecycle.

### Buffer surround resolver/editor

Responsibility:

- Resolve nearest enclosing pair for target family (cross-line) and apply delimiter-only edits.

Public API impact:

- Adds two public buffer methods listed above.

Dependencies:

- Existing bracket and quote pair-resolution internals (`bracket_text_object`, `quote_text_object`).
- Existing single-character and range edit primitives.

## User Interaction

### Replace surround (`gsr`)

Flow:

1. User presses `g`, `s`, `r`.
2. Editor waits for target selector key.
3. Editor waits for replacement selector key.
4. On valid selectors, editor resolves nearest surrounding pair of target family around cursor, across lines.
5. If found, delimiters are replaced with replacement family delimiters.
6. If not found or selectors invalid, nothing changes.

Examples:

- `gsr{[` replaces `{ ... }` with `[ ... ]`.
- `gsr)"` replaces `( ... )` with `" ... "`.

### Delete surround (`gsd`)

Flow:

1. User presses `g`, `s`, `d`.
2. Editor waits for target selector key.
3. On valid selector, editor resolves nearest surrounding pair of that family around cursor, across lines.
4. If found, open and close delimiters are removed.
5. If not found or selector invalid, nothing changes.

## External Dependencies

No new external crates or services.

The design relies exclusively on existing urvim modules and utilities.

## Error Handling

- Unsupported selector keys are handled as safe no-ops.
- Missing surrounding pair is handled as safe no-op.
- Same-family replace request is handled as safe no-op.
- Execution failures do not panic; they return `None` and leave buffer unchanged.

Recovery strategy:

- User remains in normal mode and can retry command immediately.

## Security

No new I/O, privilege, or trust boundaries are introduced.

Input validation scope is limited to key-sequence parsing and selector mapping. Invalid keys are discarded safely via no-op behavior.

## Configuration

No new configuration options in v1.

`gsr` and `gsd` are fixed normal-mode bindings.

## Component Interactions

```text
Key input
  -> Normal mode key buffer
  -> Keymap resolves gsr/gsd command
  -> ActionKind::SurroundReplace/SurroundDelete
  -> Main action dispatcher
  -> Focused window cursor + buffer lookup
  -> Buffer::replace_surround / Buffer::delete_surround
      -> Resolve surrounding pair (cross-line, nearest enclosing)
      -> Apply delimiter mutation(s)
      -> Record as one undoable edit
  -> Cursor update + render
```

## Platform Considerations

- Works uniformly on macOS/Linux terminal environments because it uses canonical key strings already normalized by existing input handling.
- No platform-specific terminal escape handling is required beyond current key event normalization.
