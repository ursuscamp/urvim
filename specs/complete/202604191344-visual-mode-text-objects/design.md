# Visual Mode Text Objects - Technical Design

## Architecture Overview
The feature extends urvim's existing text-object resolver so character-wise visual mode can consume the same object families already available to normal-mode operator-pending commands.

The implementation is split into three responsibilities:

1. Key handling in visual mode recognizes text-object sequences such as `iw`, `aw`, `iW`, `aW`, bracket objects, and quote objects.
2. Buffer-level text-object resolution computes the object range using the existing resolver APIs.
3. Buffer-view selection state updates the current visual selection to the resolved range and keeps visual mode active.

The key design choice is idempotent retargeting. When a text object is invoked while visual mode is active, urvim resolves the object and updates the selection to that range. If the active selection already matches the resolved range, the second invocation is a no-op.

## Interface Design
### Action model
Add a visual-selection action variant that carries a `TextObject` and, where applicable, a count.

Conceptually:

```rust
ActionKind::VisualTextObject(TextObject)
```

or an equivalent variant with the same meaning. This action is distinct from `ActionKind::Operation(Operator, OperatorTarget)` because it does not wait for an operator and does not mutate the buffer.

### Visual mode keymap
Extend the character-wise visual keymap with direct bindings for:

- `iw`, `aw`
- `iW`, `aW`
- bracket objects for `()`, `[]`, `{}`, `<>`
- quote objects for `'`, `"`, and `` ` ``

These bindings should exist only in character-wise visual mode. Linewise visual mode does not gain text-object expansion in this first pass.

### Selection update helpers
Add a small buffer-view helper that accepts a resolved `TextObjectRange` and updates the visual selection state to that resolved range.

Expected responsibilities:

- normalize the resolved selection if needed
- compare it with the current visual selection and skip the update when they already match
- store the new anchor/cursor pair in a form compatible with existing visual rendering
- leave the mode unchanged

### Resolver reuse
Reuse `Buffer::get_operator_target_range_with_count` for the actual object resolution. The new visual-mode action should not duplicate scanning logic for words, BigWords, brackets, or quotes.

## Data Models
### TextObjectRange
The existing `TextObjectRange { start, end }` model remains the unit of resolution.

Constraints:

- `start` and `end` must describe a non-empty or safely expandable byte-range boundary pair.
- The range must already be synced to valid cursor positions before it is applied to visual selection state.

### VisualSelection
The existing `VisualSelection { anchor, kind }` record remains the source of truth for active visual state.

For this feature, the selection update path should treat the current anchor/cursor pair as a resolved span rather than as a fixed origin. The helper that applies the text object can translate the resolved range back into anchor/cursor coordinates.

## Key Components
### `src/editor/visual_common.rs`
Owns visual-mode key parsing and dispatch.

Responsibilities:

- recognize visual text-object sequences
- preserve existing motion and mode-switch bindings
- reject invalid text-object prefixes cleanly
- keep the shared count parsing behavior consistent with the rest of visual mode

### `src/buffer/operator_target.rs`
Continues to resolve text-object ranges for all supported families.

Responsibilities:

- provide a single range-resolution API for normal and visual mode
- keep counts and delimiter-family behavior centralized

### `src/window/view.rs`
Owns visual selection state and rendering-derived range normalization.

Responsibilities:

- normalize selection coordinates before applying a new text-object range
- replace the active visual selection with the resolved range when it differs from the current one
- keep the selection in character-wise visual mode after updates

### `src/window/commands.rs`
Keeps buffer mutations and selection mutations separated.

Responsibilities:

- continue handling delete/change/yank operations for existing visual selections
- dispatch the new visual text-object action into selection expansion rather than buffer mutation

### Tests
Add focused regression tests for:

- `viw` entering visual mode and selecting the inner word
- invoking a second text object in visual mode expanding the current selection
- invalid text-object input leaving the selection unchanged
- normal-mode operator-pending text objects remaining unchanged

## User Interaction
The first-pass workflow should feel simple and predictable:

1. User presses `v` to enter character-wise visual mode.
2. User presses a text-object sequence such as `iw`.
3. urvim resolves the object under the cursor and makes it the active selection.
4. If the user presses the same text-object sequence again and the active selection already matches it, urvim leaves the selection unchanged.
5. If the user presses a different text-object sequence, urvim updates the selection to the new resolved object.

The selection should remain stable and stay in visual mode after each successful update.

Example behavior:

- `viw` selects the inner word under the cursor.
- A subsequent `iw` in visual mode leaves the active selection unchanged if it already matches that inner-word range.

## External Dependencies
No new external dependencies are required. The feature reuses:

- existing buffer text-object scanning
- existing Unicode-aware cursor normalization
- existing visual rendering and selection styling

## Error Handling
Expected failures should be handled conservatively:

- If a visual text-object sequence is incomplete, keep waiting for the next key.
- If the sequence is invalid, clear the pending buffer and leave the visual selection unchanged.
- If the text object cannot be resolved at the current cursor location, preserve the existing selection.
- If the current buffer has changed underneath the selection, sync the relevant cursors before comparing the resolved range and applying the update.

The feature should fail closed. It should never partially mutate the buffer or leave visual mode unexpectedly because a visual text-object lookup failed.

## Security
No additional security concerns are introduced.

The feature only changes in-memory editor state and does not add new file-system, network, or privilege-sensitive behavior.

## Configuration
No new configuration options are required.

The feature should inherit the editor's existing behavior for:

- cursor syncing
- Unicode grapheme handling
- selection rendering

## Component Interactions
The core interaction path is:

1. `VisualModeState` parses a text-object key sequence and emits a visual-selection action.
2. The action is dispatched by the editor loop to a selection-update handler.
3. The handler asks the buffer to resolve the object range for the current cursor.
4. The handler reads the current visual selection, if any, and compares it with the resolved range.
5. The buffer view stores the resolved selection and the renderer highlights the new region.

Pseudocode for the update step:

```text
resolved = buffer.resolve_text_object(cursor, object, count)
current = buffer_view.current_visual_range_or_cursor()
if current != resolved:
    buffer_view.store_visual_range(resolved)
```

The important invariant is that the same resolved text object is idempotent when invoked repeatedly.

## Platform Considerations
The feature must stay compatible with the editor's existing grapheme-aware cursor model and byte-backed buffer storage.

That means:

- selection bounds must be synced to valid grapheme boundaries before use
- multi-byte Unicode text must expand correctly
- multi-line selections must continue to render using the existing selection overlay logic

The design intentionally avoids any platform-specific behavior. The feature should behave the same on all supported terminal environments.
