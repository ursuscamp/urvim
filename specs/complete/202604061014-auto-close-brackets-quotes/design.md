# Auto Close Brackets and Quotes - Technical Design

## Architecture Overview
Add automatic pairing as a small insert-mode feature that is driven by startup configuration and implemented with existing buffer edit primitives. The design keeps the behavior split across three layers:

- `Config` carries the user preference and defaults it to enabled.
- `InsertMode` translates typed keys into the right editor action for openers, closers, and plain text.
- `Window` performs the actual buffer mutation or cursor movement, including the pair-aware backspace path.

The feature does not introduce a new editing mode or a new persistence layer. It reuses the existing action dispatch, buffer mutation, cursor movement, and snapshot-after-action undo model.

## Interface Design
### Configuration
Extend the resolved startup config and TOML schema with a boolean flag:

```rust
pub struct Config {
    pub theme: String,
    pub insert_escape: Option<String>,
    pub syntax: bool,
    pub auto_close_pairs: bool,
}

pub struct PartialConfig {
    pub theme: Option<String>,
    pub insert_escape: Option<String>,
    pub syntax: Option<bool>,
    pub auto_close_pairs: Option<bool>,
}
```

`Config::resolve` should default `auto_close_pairs` to `true` when the config file omits it.

### Insert Mode
Keep the public insert-mode constructor stable:

```rust
impl InsertMode {
    pub fn new() -> Self;
}
```

`InsertMode::new()` should read the resolved config once, cache the pairing flag, and use it while resolving character keys.

### Pair Lookup Helper
Add a focused helper module under `src/editor/` for supported delimiter pairs. It should expose static lookups for opener-to-closer and closer-to-opener mapping.

Representative shape:

```rust
pub fn opener_for(ch: char) -> Option<char>;
pub fn closer_for(ch: char) -> Option<char>;
pub fn is_supported_opener(ch: char) -> bool;
pub fn is_supported_closer(ch: char) -> bool;
```

### Buffer and Window Editing
Keep the buffer layer on primitive text operations. Add a pair-aware backspace path in the window layer that can remove a matched opener and closer in one call by using the buffer’s range-removal primitive.

## Data Models
### Supported Delimiter Pairs
The initial supported set is fixed:

- `(` with `)`
- `[` with `]`
- `{` with `}`
- `"` with `"`
- `'` with `'`
- `` ` `` with `` ` ``

These pairs are treated independently. There is no angle-bracket pairing in this feature, even though angle brackets exist elsewhere in the editor for text objects.

### Insert-Mode Decision
The insert-mode key handler should resolve typed characters into one of four outcomes:

- Insert an opener and its matching closer as a single text edit.
- Skip over an existing matching closer by moving the cursor right.
- Insert the typed character plainly.
- Fall back to the existing special-key map for non-character bindings.

## Key Components
### `src/config.rs`
Add `auto_close_pairs` to both the resolved config and partial TOML schema. Validation stays simple because the option is boolean.

Responsibilities:

- Parse the new field from config.
- Default it to `true` when absent.
- Preserve current behavior for existing fields.

### `src/editor/insert.rs`
Teach insert mode how to inspect typed characters before falling back to plain insertion.

Responsibilities:

- Detect supported openers and emit `Action::InsertText` with both delimiter characters.
- Detect supported closers and, when the cursor is immediately before the matching closer, emit `Action::MoveRight` instead of inserting another closer.
- Preserve the existing special-key keymap for `<Esc>`, `<Backspace>`, `<Delete>`, and configured insert escape bindings.

Why `InsertText` for openers:

- It keeps the opener and closer insertion as one logical edit.
- It matches the current snapshot-after-action undo model.
- It keeps repeat capture aligned with the full inserted text.

### `src/window/commands.rs` and `src/window/widget_impl.rs`
Extend backspace handling so that `DeleteBackward` can remove a matched pair when the cursor sits between a supported opener and closer.

Responsibilities:

- If pairing is enabled and the characters around the cursor form one of the supported pairs, remove both characters with a single buffer range delete.
- Otherwise, preserve the existing single-character backspace behavior.

### `src/editor/pairs.rs` or equivalent
Keep the pair table in one place so insert-mode matching and backspace matching stay consistent.

Responsibilities:

- Define the six supported pairs.
- Provide opener/closer lookups.
- Avoid duplicating delimiter lists across insert-mode and backspace code.

## User Interaction
### Opening Delimiter
When the user types a supported opener in insert mode and pairing is enabled, the editor inserts both delimiters and leaves the cursor between them.

Example flow:

1. User types `(`.
2. Insert mode resolves it to `Action::InsertText("()")`.
3. The window inserts the pair.
4. The cursor ends between the two characters.

### Closing Delimiter Skip
When the user types a supported closer and the next buffer character is the same closer, the editor moves the cursor right instead of inserting a duplicate.

This is a cursor movement only. It does not mutate the buffer and does not create a separate undo snapshot.

### Backspace Pair Deletion
When the cursor is between a supported opener and its matching closer, pressing backspace deletes both characters together.

This behaves as one edit from the user’s perspective, so one undo restores both characters and one redo removes both again.

### Disabled State
When pairing is disabled, insert mode behaves exactly like plain insertion:

- Openers insert only themselves.
- Closers insert only themselves.
- Backspace deletes only the character immediately before the cursor.

## External Dependencies
No new external crates are required. The implementation reuses:

- `toml` and `serde` for config parsing.
- The existing editor action system.
- The existing buffer insert/remove primitives.
- The current undo/redo snapshot system.

## Error Handling
- Invalid config is not expected for this option because it is a boolean field.
- If config loading omits the field, the editor should silently use the default-on behavior.
- If cursor context does not actually form a supported pair, backspace must fall back to normal deletion rather than fail.
- If a typed closer does not match the character at the cursor, it must insert normally.

## Security
- No new trust boundary is introduced.
- The feature consumes local keyboard input only.
- No secrets, file paths, or networked data are involved.
- The implementation should remain free of `unsafe`.

## Configuration
Add the option to `docs/config.md` with the rest of the startup schema.

Proposed config name:

```toml
auto_close_pairs = true
```

Documentation should note:

- The option defaults to `true`.
- It applies in insert mode only.
- It covers the six explicit supported pairs.
- Turning it off restores plain text insertion and single-character backspace behavior.

## Component Interactions
1. Startup config loads `auto_close_pairs`.
2. `globals::set_config` stores the resolved config.
3. `InsertMode::new()` reads the flag from globals and caches it.
4. When a key arrives, insert mode decides whether to emit `InsertText`, `MoveRight`, `InsertChar`, or an existing special-key action.
5. `main.rs` dispatches the action to the active window or handles mode switching.
6. The window mutates the buffer and updates the cursor.
7. After a handled edit, the existing snapshot flow records undo state.

The key requirement is that opener insertion and pair deletion each remain a single handled edit so undo and redo treat them as one step.

## Platform Considerations
The design is terminal-agnostic and depends only on canonical key input and buffer cursor coordinates. It should behave consistently across platforms and keyboard layouts because the supported delimiters are plain ASCII characters.

One caution: the closer-skip path is based on the character immediately at the cursor, so it intentionally stays local to the current line and does not attempt language-aware heuristics or cross-line pair matching.
