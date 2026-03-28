# Save Indicators and Save-on-Command - Technical Design
## Architecture Overview
This feature adds a save-centered state model to the buffer layer and threads that state into the tab bar and status bar renderers. The buffer remains the source of truth for both its modified state and its resolved filetype, while the editor input layer translates `<C-s>` into a save action for the active buffer.

The key behavioral changes are:

- edited text marks the buffer as modified
- successful save clears the modified state
- filetype classification refreshes only on load, path assignment, and successful save
- unnamed buffers are not saved yet

### Flow

```text
User presses <C-s>
  -> mode produces save action
  -> active buffer save is attempted
  -> if buffer has no filename, skip save
  -> if save succeeds, rewrite file on disk
  -> refresh filetype from saved filename + first line
  -> clear modified state
  -> tab bar and status bar render the updated indicators
```

## Interface Design

### Save action

Add a new editor action for saving a specific buffer or the active buffer.

Representative shape:

```rust
pub enum Action {
    SaveBuffer(Option<BufferId>),
}
```

`<C-s>` should map to `SaveBuffer(None)` in both normal mode and insert mode. The editor should resolve `None` to the active buffer at dispatch time.

### Modified-state access

Expose whether a buffer is modified from the buffer and buffer-view layers.

Representative shape:

```rust
impl Buffer {
    pub fn is_modified(&self) -> bool;
}

impl BufferView {
    pub fn is_modified(&self) -> bool;
}
```

### Save operations

The buffer pool should continue to own persistence operations. A save operation should:

- require a known path
- write the current buffer contents to that path
- refresh the buffer's filetype from the saved path and current first line
- clear the modified state only after a successful write

Unnamed buffers should return a no-op outcome rather than inventing a filename.

### Status bar context

Extend the footer context with a compact modified indicator.

Representative shape:

```rust
pub struct StatusBarContext<'a> {
    pub mode_label: &'a str,
    pub modified_marker: &'a str,
    pub modified_style: Style,
    pub filetype_label: &'a str,
    pub buffer_name: &'a str,
    pub cursor_line: usize,
    pub cursor_byte_col: usize,
    pub line_count: usize,
}
```

### Tab bar labels

Tab labels should include the same modified marker used in the status bar. A clean buffer renders its existing name label; a modified buffer appends or prepends a short marker such as `*`.

The modified marker should use its own theme-provided style so it reads as a state indicator rather than part of the buffer name. The exact palette should reuse existing UI styling conventions, but the indicator should be visually separable from the label text.

## Data Models

### Modified buffer state

The buffer needs a stable way to answer whether its current contents differ from the last successful save or loaded file state.

Recommended shape:

```rust
pub struct Buffer {
    lines: Vector<Arc<str>>,
    path: Option<AbsolutePath>,
    filetype: Filetype,
    saved_lines: Vector<Arc<str>>,
    undo_state: UndoState,
}
```

The saved baseline should update on:

- buffer load
- path assignment for newly opened content
- successful save

This lets undo and redo return to a clean state when the text matches the saved baseline again.

### Filetype classification

Reuse the existing `Filetype` enum and detector. The only change is when detection runs:

- load and path assignment still initialize filetype
- edit-time mutations no longer recalculate it
- successful saves recalculate it from the current filename and first line

## Key Components

### Buffer

Responsibilities:

- track modified state against the last saved baseline
- expose whether the buffer is modified
- refresh filetype only when the save/load lifecycle requires it

Dependencies:

- `Filetype` detection helpers
- undo/redo state
- buffer text mutations

### Buffer pool

Responsibilities:

- keep save operations path-aware
- return a no-op result for unnamed buffers
- preserve the existing path deduplication behavior after a successful save

Dependencies:

- `Buffer`
- `AbsolutePath`
- filesystem write APIs

### Input handling

Responsibilities:

- map `<C-s>` to the save action in both normal and insert mode
- preserve the current mode after saving

Dependencies:

- `NormalMode`
- `InsertMode`
- `Action`

### Layout, tab bar, and status bar

Responsibilities:

- read modified state from the active buffer
- render a compact modified marker in the tab bar and status bar
- keep existing metadata and footer layout intact
- apply a distinct theme-provided style to the modified marker in both regions

### Theme updates

Add a dedicated UI style entry for modified markers to the theme model and schema, then define it in each built-in theme. The style should be readable on top of the existing tab and status bar backgrounds and should be chosen to stand apart from the active/inactive label colors without clashing with them.

Dependencies:

- `TabGroup`
- `BufferView`
- `StatusBar`
- `Layout`

## User Interaction

- `<C-s>` saves the active buffer when it has a filename.
- The user stays in the current mode after saving.
- A modified marker appears as soon as a buffer's contents diverge from the saved baseline.
- The marker disappears after a successful save or after undo/redo returns the buffer to the saved baseline.
- Unnamed buffers ignore save requests for now.

## External Dependencies

No new external crates are required. The feature stays within the current buffer, input, rendering, and filesystem layers.

## Error Handling

- If the buffer has no filename, save should be skipped without creating a file.
- If the filesystem write fails, the buffer should remain modified and keep its previous filetype.
- If filetype detection fails to find a more specific match on save, the fallback filetype should remain in use.
- If the active buffer disappears from the pool, the save request should behave like a no-op rather than crash the editor.

## Security

Saving should only write to a known buffer path and should not execute any file contents. Filetype refresh remains read-only and only inspects the filename and current first line.

## Configuration

No new configuration options are required.

The theme model should gain a new style slot for modified markers, but it does not require a new user-facing config surface beyond the existing theme files.

## Component Interactions

1. A user presses `<C-s>`.
2. The active mode emits a save action.
3. The editor routes the action to the active buffer.
4. The buffer pool writes the active buffer when it has a path.
5. On success, the buffer refreshes filetype and clears the modified baseline.
6. Layout pulls modified state and filetype labels from the active buffer view.
7. Tab bar and status bar render the updated markers and metadata.

## Platform Considerations

The save shortcut must remain usable in terminal raw mode on Unix-like systems. The modified marker should use plain ASCII so it remains readable across terminals and fonts.
