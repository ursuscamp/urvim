# Line Comment Toggle - Technical Design
## Architecture Overview
The feature adds a new normal-mode editing action that toggles line comments on one or more consecutive lines. The action will be exposed through the editor action system, bound from normal mode, and executed against the active buffer in the window layer.

The implementation should reuse existing count handling and line-oriented command patterns already used by `dd`, `cc`, and other line actions. The only new syntax-specific input is a per-filetype line comment prefix read from syntax metadata.

## Interface Design
### Action model
- Add a new `ActionKind` variant for line comment toggling.
- Expose a constructor on `Action` that creates the new action.
- Mark the action as countable and repeatable in the same way as other line-editing commands.

### Normal mode binding
- Bind `gcc` in `NormalMode::new()` to the new action.
- Keep the binding inside the trie-based normal-mode keymap so it participates in count parsing and multi-key dispatch like the existing `cc` and `dd` bindings.

### Syntax metadata
- Extend `RawSyntaxMetadata` and `SyntaxMetadata` with a `comment_prefix` field.
- Store the canonical prefix as an optional string value.
- Preserve existing metadata fields and resolution behavior for aliases, filenames, and shebangs.

### Buffer editing surface
- Add a helper that can toggle the comment state of a line given:
  - a line index
  - a line comment prefix
  - the current line text
- The helper should return the updated cursor position when needed by the caller.

## Data Models
### SyntaxMetadata
- `comment_prefix: Option<SmolStr>`
- Constraint: empty or whitespace-only prefixes should be rejected during syntax loading.

### RawSyntaxMetadata
- `comment_prefix: Option<String>`
- Constraint: the field is optional so syntax definitions without line comments remain valid.

### Comment toggle behavior
- For a non-commented line, insert the prefix immediately after leading indentation.
- For a commented line, remove the prefix and one following space if present.
- Preserve the rest of the line exactly, including trailing text and inline spacing.

## Key Components
### `src/editor/action.rs`
- Add the new action kind and helper constructors.
- Update action classification helpers so the command participates in counts, dot-repeat, and line-action handling as required.

### `src/editor/normal.rs`
- Register `gcc` in the normal-mode trie keymap.

### `src/window/commands.rs`
- Route the new action to a dedicated handler.
- Handle count-prefixed toggling by applying the action across consecutive lines.

### `src/buffer` editing helpers
- Implement the line transformation logic that adds or removes the prefix while preserving indentation.

### `src/syntax/definition.rs`, `src/syntax/loader.rs`, `src/syntax/error.rs`
- Add the metadata field, compile it, and report invalid values with a load error.

### `src/syntax/builtins/*.toml`
- Populate the line comment prefix for built-in filetypes that support line comments.

## User Interaction
The user will press `gcc` in normal mode to toggle the current line. A count prefix such as `3gcc` will toggle several lines starting from the cursor line. The command should feel immediate and predictable, matching the surrounding Vim-like line commands.

## External Dependencies
No new external dependencies are required. The feature should use the existing buffer, syntax, and keymap infrastructure.

## Error Handling
- If the active syntax omits a line comment prefix, the action should not mutate the buffer.
- If metadata contains an invalid empty prefix, syntax loading should fail with a clear error.
- If the requested line range exceeds the end of the buffer, the action should clamp to the available lines rather than fail.

## Security
The feature does not introduce new security-sensitive behavior. It only rewrites text in the active buffer using trusted syntax metadata.

## Configuration
No new user-facing configuration options are required.

## Component Interactions
1. Normal mode receives `gcc`.
2. The keymap resolves it to the new action.
3. Window command handling determines the target line range from any count prefix.
4. The buffer helper reads the active syntax's line comment prefix.
5. The line is toggled in place and the cursor remains on the edited line.

## Platform Considerations
The command should work the same in every terminal environment because it only manipulates buffer text and does not depend on platform-specific UI behavior.
