# Auto-Indent - Technical Design

## Architecture Overview
Add auto-indent as a small editing feature that is resolved from startup config and applied when the editor creates a new editable line. The first supported strategy is a neighbor mode that copies nearby leading whitespace from the active buffer. The design keeps indentation inference in the buffer layer and keeps mode-specific behavior in the editor and window layers.

The implementation should follow the existing editor flow:

1. Configuration resolves the active auto-indent mode.
2. Insert mode and normal-mode open-line actions request an indent prefix when they create a newline.
3. Buffer helpers inspect surrounding lines and return the leading whitespace to reuse.
4. The editor inserts the newline plus the computed indent as a single edit so cursor movement and repeat capture stay consistent.

## Interface Design
### Configuration schema
Add an extensible auto-indent setting to the resolved config and TOML schema.

Recommended shape:

```rust
pub struct Config {
    pub auto_indent: AutoIndentMode,
}

pub struct PartialConfig {
    pub auto_indent: Option<AutoIndentMode>,
}
```

Define the mode as an enum, not a boolean:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoIndentMode {
    Off,
    Neighbor,
}
```

Semantics:

- `off` preserves plain newline behavior.
- `neighbor` enables the first auto-indent strategy.
- The enum should be easy to extend with future styles without changing the field type.

### Indent resolution helper
Add a focused helper on `Buffer` that can resolve the indentation prefix for a new line at a given cursor position.

Recommended shape:

```rust
/// Returns the leading-whitespace prefix that should be inserted for a new line
/// created at `cursor`, or `None` when no useful indentation can be inferred.
pub fn inferred_auto_indent_prefix(&self, cursor: Cursor) -> Option<String>;
```

The helper should operate only on nearby buffer lines and should not mutate the buffer. It should be reusable from both insert-mode newline handling and normal-mode open-line actions.

### Insert mode
Insert mode keeps ownership of key translation. When `<Enter>` is pressed:

- if auto-indent is `off`, behave like the current plain newline action
- if auto-indent is `neighbor`, ask the buffer for an indent prefix and insert the newline plus prefix together

The public constructor remains unchanged:

```rust
impl InsertMode {
    pub fn new() -> Self;
}
```

### Normal mode open-line actions
`o` and `O` should continue to create new editable lines, but the inserted line should begin with the resolved auto-indent prefix when the mode is enabled. The window layer remains responsible for the actual cursor placement after the line is inserted.

## Data Models
### `AutoIndentMode`
The enum is the user-facing configuration model for auto-indent behavior.

Fields and constraints:

- `Off`: no auto-indent
- `Neighbor`: infer indentation from neighboring buffer lines
- future variants may be added without changing the config field shape

### Indent prefix
The inferred prefix is the exact leading whitespace sequence taken from the chosen source line.

Rules:

- preserve tabs and spaces as written
- do not normalize indentation width in the first version
- return no prefix when inference is ambiguous or unavailable

### Source-line selection
For `neighbor`, the helper should examine nearby non-blank lines around the insertion point and choose the best candidate using a simple local rule:

- prefer the more-indented relevant neighbor when both sides are present
- ignore blank lines
- if no usable indentation is found, return `None`

## Key Components
### `src/config.rs`
Extend config resolution to load and validate the new auto-indent enum.

Responsibilities:

- deserialize the new field from TOML
- apply `off` as the default when absent
- reject unknown or invalid enum values at startup
- keep the existing startup config loading behavior unchanged

### `src/buffer/indent.rs` or equivalent
Add a focused buffer helper module for indentation inference.

Responsibilities:

- inspect surrounding lines without mutating the buffer
- determine the best local indentation source
- expose a reusable prefix-returning API for editor paths

### `src/editor/insert.rs`
Teach insert mode to use the auto-indent setting when `<Enter>` is pressed.

Responsibilities:

- resolve `<Enter>` into the correct insert action based on config
- preserve the existing special-key keymap behavior
- keep repeat capture aligned with the exact inserted text

### `src/window/commands.rs`
Update open-line handling so `o` and `O` create lines that begin with the resolved indentation prefix when auto-indent is enabled.

Responsibilities:

- insert the new editable line
- place the cursor at the start of the indented line
- leave the existing count behavior intact

## User Interaction
### Insert-mode newline
When auto-indent is enabled, pressing `<Enter>` in insert mode should insert a newline and then the inferred leading whitespace for the new line. The cursor should end after the inserted indentation, ready for typing.

### Open line below
When the user presses `o`, the editor should create a new line below the current line, insert the inferred indentation prefix if available, and enter insert mode on that new line.

### Open line above
When the user presses `O`, the editor should create a new line above the current line, insert the inferred indentation prefix if available, and enter insert mode on that new line.

### Disabled state
When auto-indent is `off`, these commands should continue to behave like plain newline creation with no extra indentation added by the feature.

## External Dependencies
No new external crates are required. The design reuses:

- existing config deserialization
- existing buffer text inspection
- existing insert-mode action dispatch
- existing window/buffer mutation primitives

## Error Handling
- Invalid auto-indent config values should fail configuration loading with a clear error.
- If inference cannot find a usable indentation source, the editor should insert a plain newline with no extra prefix.
- If the helper is asked to inspect an out-of-range cursor position, it should fail safely by returning `None`.
- The feature should not alter unrelated actions if the active mode is `off`.

## Security
This feature only changes local text editing behavior. It does not add new trust boundaries, network access, or filesystem access.

## Configuration
Update the user-facing config docs to describe the new enum-style option.

Proposed config form:

```toml
auto_indent = "off"
```

Document that:

- the field is extensible and not boolean
- `off` is the default
- `neighbor` is the first supported enabled style
- future styles may be added later without changing the config shape

## Component Interactions
1. Startup config loads the resolved `AutoIndentMode`.
2. The active config is stored globally as part of normal startup.
3. Insert mode reads the config when it receives `<Enter>`.
4. Window open-line actions ask the buffer for an inferred indentation prefix.
5. The buffer returns the local leading-whitespace sequence, or `None` if no inference is possible.
6. The editor inserts the newline plus prefix as a single edit and updates the cursor accordingly.

## Platform Considerations
The feature is terminal-agnostic because it only manipulates text already in the buffer. It should behave consistently across platforms because indentation inference is based on local buffer content rather than keyboard layout or platform-specific key codes.
