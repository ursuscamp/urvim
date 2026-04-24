# Surround Add Keybind - Technical Design

## Architecture Overview

The feature extends the existing `gs` surround command family with add-surround actions:

```text
Normal keymap: gsa + text object + delimiter
        -> ActionKind::SurroundAdd { target, delimiter }
        -> Window resolves target range
        -> Buffer inserts opening and closing delimiters

Visual keymap: gsa + delimiter
        -> ActionKind::SurroundAddSelection { delimiter }
        -> Window resolves active visual range
        -> Buffer inserts opening and closing delimiters
        -> Mode transitions to Normal
```

The design keeps delimiter selection shared with existing `gsr` and `gsd` commands through `DelimiterFamily`. Normal-mode target resolution reuses existing text object range resolution. Visual-mode target resolution reuses the active visual selection range helpers already used by delete, change, yank, paste, and case operations.

## Interface Design

### Normal-Mode Action

Add an action payload for text-object surround-add:

```rust
ActionKind::SurroundAdd {
    target: TextObject,
    delimiter: DelimiterFamily,
}
```

- `target` is an existing `TextObject` value.
- `delimiter` is the selected surround delimiter family.
- The action succeeds only when the text object resolves to a non-empty range.
- The action does not accept boundary motions in the first version.

### Visual-Mode Action

Add an action payload for selection surround-add:

```rust
ActionKind::SurroundAddSelection {
    delimiter: DelimiterFamily,
}
```

- In character-wise Visual mode, the active character selection is surrounded in place.
- In Visual Line mode, the selected line range is surrounded as a linewise block.
- The action carries `to_mode: Some(ModeKind::Normal)` from the visual keymap.

### Buffer API

Add focused buffer methods for two-sided insertion:

```rust
pub fn add_surround(
    &mut self,
    range: TextObjectRange,
    delimiter: DelimiterFamily,
) -> Option<Cursor>

pub fn add_linewise_surround(
    &mut self,
    start_line: usize,
    count: usize,
    delimiter: DelimiterFamily,
) -> Option<Cursor>
```

`add_surround` inserts the closing delimiter at `range.end` first, then inserts the opening delimiter at `range.start`. This preserves the original end cursor while mutating the buffer.

`add_linewise_surround` inserts the opening delimiter before `start_line` and the closing delimiter after the selected line range. The selected lines remain unchanged between the inserted delimiter lines unless the caller requests auto-indentation.

Both methods return the cursor position to keep after success. They return `None` for invalid or empty targets.

## Data Models

### `DelimiterFamily`

No new delimiter model is required. The existing enum remains the canonical delimiter family type:

```rust
pub enum DelimiterFamily {
    Paren,
    Square,
    Curly,
    Angle,
    DoubleQuote,
    SingleQuote,
    Backtick,
}
```

The existing `opening_delimiter()`, `closing_delimiter()`, and `from_selector_key()` behavior should be reused.

### Text Ranges

Character-wise surround uses existing `TextObjectRange`:

```rust
pub struct TextObjectRange {
    pub start: Cursor,
    pub end: Cursor,
}
```

Visual Line surround uses the existing `(start_line, count)` shape returned by `BufferView::visual_line_selection_range()`.

## Key Components

### `src/editor/action.rs`

Responsibilities:

- Add public documentation comments for the new `ActionKind` variants.
- Ensure the new actions participate in existing action metadata checks:
  - remembered-column reset
  - movement classification
  - mutation classification
  - snapshot eligibility
  - repeatability policy, if applicable

`SurroundAdd` and `SurroundAddSelection` should be mutating, snapshottable actions. Normal-mode `SurroundAdd` can follow existing surround edit repeatability policy. Visual-mode `SurroundAddSelection` should follow current visual-edit repeat policy unless the existing architecture has no visual repeat support for comparable edits.

### `src/editor/normal/bindings.rs`

Responsibilities:

- Extend surround binding registration with `gsa{text object}{delimiter}` sequences.
- Reuse the existing surround selector list for the delimiter suffix.
- Register the existing text object families:
  - `iw`, `aw`, `iW`, `aW`
  - bracket objects for `i(`/`i)`, `a(`/`a)`, `i[`/`i]`, `a[`/`a]`, `i{`/`i}`, `a{`/`a}`, `i<`/`i>`, `a<`/`a>`
  - quote objects for `i'`, `a'`, `i"`, `a"`, `i```, and `a```

The normal keymap should not register `gsa` with boundary motions such as `w`, `e`, `$`, `gg`, or `G` in this feature slice.

### `src/editor/visual_common.rs`

Responsibilities:

- Register `gsa{delimiter}` for both visual modes through the shared visual keymap builder.
- Reuse the same surround selector list or an equivalent shared helper to avoid drift from normal-mode surround bindings.
- Mark the resulting action as transitioning to Normal mode.
- Treat the new action like delete, change, yank, and visual case commands for count handling, so numeric prefixes do not create unexpected repeated surround insertions.

### `src/window/commands.rs`

Responsibilities:

- Add a normal-mode command handler that resolves `TextObject` against the current cursor and calls `Buffer::add_surround`.
- Add a selection command handler that dispatches by `from_mode`:
  - `ModeKind::Visual`: resolve `visual_selection_range()` and call `Buffer::add_surround`
  - `ModeKind::VisualLine`: resolve `visual_line_selection_range()` and call `Buffer::add_linewise_surround`
- For Visual Line surround-add, check the resolved config value for `auto_indent`. When it is not `AutoIndentMode::Off`, indent only the originally selected lines by one existing indentation step after inserting the delimiter lines.
- Leave buffer text, cursor state, selection state, and undo history unchanged when target resolution fails.
- Place the cursor at the inserted opening delimiter after successful edits.

### `src/window/widget.rs`

Responsibilities:

- Route the new action variants to the command handlers.
- Preserve the existing `ActionResult::NotHandled` convention for failed surround edits.

### `src/buffer/surround.rs`

Responsibilities:

- Keep surround mutation logic near existing replace/delete surround operations.
- Add character-wise and linewise add-surround helpers.
- Insert the later delimiter before the earlier delimiter for character-wise ranges.
- Carefully document linewise insertion ordering because it spans logical lines, can touch end-of-file, and shifts the originally selected lines down after the opening delimiter is inserted.
- Reuse the existing indentation step behavior from `increase_line_indentation` for any Visual Line auto-indent pass requested by the window command layer.

## User Interaction

### Normal Mode

`gsa{text object}{delimiter}` surrounds the resolved text object:

```text
hello world
^ cursor in hello

gsaiw"

"hello" world
^ cursor moves to opening quote
```

Unsupported delimiter selectors, unresolvable text objects, and canceled pending sequences are safe no-ops.

### Character-Wise Visual Mode

`gsa{delimiter}` surrounds the active selection and exits to Normal mode:

```text
foo bar baz
    --- selected

gsa]

foo [bar] baz
    ^ cursor moves to opening bracket
```

### Visual Line Mode

`gsa{delimiter}` surrounds the selected line range as a block and exits to Normal mode:

```text
alpha
beta
```

With both lines selected, `gsa{` becomes:

```text
{
alpha
beta
}
```

If `auto_indent` is disabled, the original selected lines are preserved exactly between the delimiter lines.

If `auto_indent` is enabled, only the originally selected lines are indented by one existing indentation step. The inserted delimiter lines remain at the surrounding level:

```text
{
    alpha
    beta
}
```

## External Dependencies

No new external crates or runtime dependencies are required.

## Error Handling

- If `DelimiterFamily::from_selector_key()` cannot resolve a selector, no action is produced.
- If a normal-mode text object cannot resolve, the window command returns `ActionResult::NotHandled`.
- If there is no active visual selection for `SurroundAddSelection`, the command returns `ActionResult::NotHandled`.
- If a linewise selection has `count == 0`, the command returns `ActionResult::NotHandled`.
- Escape during a pending key sequence clears the mode buffer through existing key handling and returns to Normal mode without mutating the buffer.
- Visual Line auto-indentation is applied only after the linewise surround insertion succeeds. If insertion fails, no indentation is attempted.
- Failed actions must not push undo snapshots because snapshotting is driven only after handled mutating actions.

## Security

The feature only mutates the in-memory buffer based on local key input. It does not read external files, execute commands, access secrets, or introduce authentication or authorization concerns.

## Configuration

No new configuration options are introduced. The keybindings are built-in editor commands under the existing `gs` surround prefix.

## Component Interactions

```text
Key input
  -> Mode keymap parses complete sequence
  -> ActionKind::SurroundAdd or ActionKind::SurroundAddSelection
  -> Window::process_action routes action
  -> Window command resolves range from cursor or visual selection
  -> Buffer surround helper inserts delimiters
  -> Visual Line command optionally indents original selected lines
  -> Window sets cursor to opening delimiter
  -> Main loop snapshots handled mutating edit
  -> Visual actions transition to Normal mode
```

Tests should cover:

- normal-mode keymap parsing for representative text objects and delimiter families
- visual-mode keymap parsing for delimiter families
- successful normal-mode add-surround edits
- successful character-wise visual surround-add edits
- successful visual-line surround-add edits
- visual-line surround-add with auto-indent enabled and disabled
- unsupported delimiter no-ops
- unresolvable text object no-ops
- single-undo restoration after successful edits

## Platform Considerations

The implementation is platform-independent. Terminal key canonicalization for `<` and `>` already uses `<LessThan>` and `<GreaterThan>` tokens, so the new keybindings should reuse the existing selector representation.
