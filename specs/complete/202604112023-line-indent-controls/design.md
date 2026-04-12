# Line Indent Controls - Technical Design

## Architecture Overview
Add line-wise indentation controls on top of the editor's existing indentation inference and line-editing primitives. The feature has three pieces:

1. Normal mode binds `<<` and `>>` to line indentation decrease and increase actions.
2. Insert mode binds canonical `<S-Tab>` to a dedent action that applies to the current line without leaving insert mode.
3. Insert mode treats backspace specially while the cursor is inside leading indentation, stepping the line back by one indent increment at a time instead of always deleting a typed character.

The implementation should keep the text transformation logic close to the buffer layer and keep key handling in the editor modes. The buffer should own indentation measurement and prefix rewriting, while window/editor code decides when a keypress should invoke the behavior.

## Interface Design
### Normal mode key bindings
Add two normal-mode actions for line indentation control:

```rust
pub enum ActionKind {
    IndentDecrease,
    IndentIncrease,
}
```

Semantics:

- `<<` decreases indentation on the current line and, when a count is present, on the next `count - 1` lines as well.
- `>>` increases indentation on the current line and, when a count is present, on the next `count - 1` lines as well.
- The commands act on whole lines, not on arbitrary text selections.
- If the requested decrease would remove more whitespace than the line has, the line is left with no leading whitespace rather than failing.

### Insert mode reverse-tab binding
Insert mode should bind canonical `<S-Tab>` to a dedent action that keeps the editor in insert mode. The action should apply to the line containing the cursor, regardless of the cursor's horizontal position.

If the terminal layer surfaces a dedicated reverse-tab event, it should be normalized to the canonical `<S-Tab>` representation before reaching editor key handling. The editor-facing binding should remain modifier-based rather than introducing a user-visible keycode name.

### Insert-mode backspace behavior
Backspace should continue to behave as ordinary text deletion unless the cursor is within the line's leading indentation. When the cursor is in leading indentation, backspace should remove one indentation increment from the line instead of deleting a single character.

The editor should decide this by comparing the cursor column to the current line's leading-whitespace extent. When the cursor is at or before the indentation boundary, the dedent behavior wins; otherwise, normal backspace behavior applies.

### Indentation step resolution
Introduce a small helper that resolves the indentation increment for the active buffer. The helper should answer two related questions:

1. How many visual columns one indentation step represents.
2. Whether the current line's leading whitespace should be extended with tabs, spaces, or a mixture that matches the buffer's existing style.

The helper should reuse the editor's current indentation detection rather than introducing language-specific rules. The first version only needs local, buffer-derived behavior.

## Data Models
### Indentation direction
Represent line shifts with a direction enum:

```rust
pub enum IndentDirection {
    Decrease,
    Increase,
}
```

This keeps the line-shift implementation symmetric and makes the buffer helper reusable from both normal mode and insert mode.

### Indentation increment
The indentation increment is the resolved unit used for one shift step.

Constraints:

- It must be deterministic for the same buffer state.
- It must preserve the buffer's existing indentation style as closely as possible.
- It must never remove non-whitespace text.
- If the line cannot provide a full decrement step, the implementation should remove only the available leading whitespace.

## Key Components
### `src/editor/normal.rs`
Extend the normal-mode key trie to recognize `<<` and `>>`.

Responsibilities:

- map the key sequences to indentation actions
- preserve existing count parsing behavior
- keep the new actions line-oriented rather than text-object oriented

### `src/editor/insert.rs`
Teach insert mode to recognize reverse-tab and indentation-aware backspace.

Responsibilities:

- bind `Shift-Tab` to a dedent action
- keep insert mode active after dedenting
- route backspace to line dedent only while the cursor is inside leading indentation
- fall back to existing backspace behavior elsewhere

### `src/window/commands.rs`
Add the actual line transformation helpers that increase or decrease leading indentation.

Responsibilities:

- apply a shift to one or more consecutive lines
- preserve the non-whitespace contents of each line
- use the resolved indentation increment
- keep cursor movement predictable after the edit

### `src/buffer/indent.rs` or equivalent
Extend the indentation helper module so it can both infer and rewrite indentation.

Responsibilities:

- measure the leading-whitespace width of a line
- resolve the indentation increment for the active buffer
- remove or insert exactly one indent increment at the start of a line

## User Interaction
### Normal mode
`<<` and `>>` should feel like Vim-style line shifts:

- `<<` moves the current line left by one indentation step.
- `>>` moves the current line right by one indentation step.
- With a count, the command should apply to a run of consecutive lines starting at the cursor line.

### Insert mode reverse-tab
`<S-Tab>` should dedent the current line without leaving insert mode. The command should be useful anywhere on the line, not only at the first character.

### Insert-mode backspace
While the cursor is inside the line's leading indentation, backspace should remove indentation in steps. Once the cursor reaches the non-whitespace text, backspace should revert to ordinary character deletion.

## External Dependencies
No new external crates are required. The feature should reuse the existing terminal key handling, buffer mutation, and indentation inference code paths.

## Error Handling
- If a buffer has no usable indentation history, indentation increase should still work by using the current configured indentation step.
- If a requested decrease exceeds the available leading whitespace, the implementation should stop at column 0.
- If reverse-tab is unavailable from the terminal layer, the editor should continue to behave safely with the existing key set rather than misclassifying it as `Tab`.
- Insert-mode backspace should never delete non-indentation text when the cursor is still within the indentation region.

## Security
This feature only changes local text editing behavior. It does not introduce new trust boundaries, credentials, or external I/O.

## Configuration
No new configuration field is required for the first version. The feature should consume the editor's existing indentation detection and tab-width configuration.

## Component Interactions
1. Normal mode maps `<<` and `>>` to line indentation actions.
2. The window layer resolves the active buffer and applies a line-wise indent increase or decrease.
3. Insert mode binds `<S-Tab>` to the same line-dedent logic used by normal mode.
4. Insert mode intercepts backspace while the cursor is still within leading indentation and applies a dedent step instead of plain deletion.
5. The buffer layer measures leading whitespace and rewrites only the indentation prefix, leaving the rest of each line untouched.

## Platform Considerations
The feature is terminal- and platform-agnostic except for `<S-Tab>` support in the terminal event layer. The text-editing behavior itself is entirely buffer-local and should behave consistently across platforms once the reverse-tab key is surfaced correctly.
