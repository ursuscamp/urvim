# Open Line Below and Above - Technical Design

## Architecture Overview

This feature implements Vim-style `o` and `O` commands in normal mode:
- `o` creates a new empty line below the current line and enters insert mode
- `O` creates a new empty line above the current line and enters insert mode

The implementation follows the existing action pattern:
1. NormalMode maps keys to Actions
2. Window processes Actions via `process_action()`
3. Main loop switches to InsertMode if the action requires it

## Interface Design

### New Actions

| Action | Description |
|--------|-------------|
| `OpenLineBelow` | Insert empty line below current line, cursor at column 0 |
| `OpenLineAbove` | Insert empty line above current line, cursor at column 0 |

### Key Bindings

| Key | Action | Count Support |
|-----|--------|---------------|
| `o` | OpenLineBelow | Yes (e.g., `3o` creates 3 lines) |
| `O` | OpenLineAbove | Yes (e.g., `3O` creates 3 lines) |

## Data Models

### Buffer Method (New)

```rust
/// Insert `count` empty lines AFTER the given line index.
/// Returns the cursor position at the start of the first inserted line.
pub fn insert_lines_after(&mut self, line: usize, count: usize) -> Option<Cursor>
```

Note: `OpenLineAbove` will use `insert_lines_after(line - 1, 1)` to insert before the current line.

## Key Components

### 1. Action Enum (editor.rs)

Add two new variants:
```rust
/// Open a new line below current line and enter insert mode
OpenLineBelow,
/// Open a new line above current line and enter insert mode  
OpenLineAbove,
```

Update `resets_remembered_column()` to include both new actions.

Update `switches_to_insert_mode()` to return true for both new actions.

### 2. NormalMode (editor.rs)

Add keybindings in `NormalMode::new()`:
```rust
keymap.insert("o".to_string(), Action::OpenLineBelow);
keymap.insert("O".to_string(), Action::OpenLineAbove);
```

### 3. Buffer (buffer.rs)

Add method `insert_lines_after()`:
- Insert `count` empty lines after the given line index
- Returns cursor at start of first inserted line
- Handles edge cases: empty buffer, first line, last line

### 4. Window (window.rs)

Handle new actions in `process_action()`:
```rust
Action::OpenLineBelow => {
    let cursor = self.buffer_view.cursor();
    if let Some(new_cursor) = self.buffer_view.buffer.insert_lines_after(cursor.line, 1) {
        self.buffer_view.set_cursor(new_cursor);
    }
    ActionResult::Handled
}
Action::OpenLineAbove => {
    let cursor = self.buffer_view.cursor();
    // Insert after the line before current (or at 0 if on first line)
    let insert_after = cursor.line.saturating_sub(1);
    if let Some(new_cursor) = self.buffer_view.buffer.insert_lines_after(insert_after, 1) {
        self.buffer_view.set_cursor(new_cursor);
    }
    ActionResult::Handled
}
```

Handle count prefix for OpenLineBelow:
```rust
} else if matches!(inner.as_ref(), Action::OpenLineBelow) {
    let cursor = self.buffer_view.cursor();
    if let Some(new_cursor) = self.buffer_view.buffer.insert_lines_after(cursor.line, *count) {
        self.buffer_view.set_cursor(new_cursor);
    }
    ActionResult::Handled
}
```

## User Interaction

### Flow for `o` (Open Line Below)
1. User presses `o` in normal mode
2. NormalMode returns `HandleKeyResult::Complete(Action::OpenLineBelow)`
3. Window processes action: inserts empty line after current line
4. Window sets cursor to column 0 of new line
5. Main loop detects `switches_to_insert_mode()` returns true
6. Mode switches to InsertMode

### Flow for `O` (Open Line Above)
1. User presses `O` in normal mode
2. NormalMode returns `HandleKeyResult::Complete(Action::OpenLineAbove)`
3. Window processes action: inserts empty line before current line (at line-1)
4. Window sets cursor to column 0 of new line
5. Main loop switches to InsertMode

### Edge Cases
- **Empty buffer**: `o` or `O` creates first line and enters insert mode
- **First line + `O`**: Inserts at line 0, cursor on new line 0
- **Last line + `o`**: Appends after last line, cursor on new line
- **Count prefix with `o`**: Creates N lines below current

## External Dependencies

None - all functionality uses existing buffer and window infrastructure.

## Error Handling

- Invalid line index: Return None, action handled gracefully (no-op)
- Empty buffer: Create first line automatically

## Security

No security concerns - this is a local text editing feature.

## Configuration

None required.

## Component Interactions

```
NormalMode.handle_key("o")
    → HandleKeyResult::Complete(Action::OpenLineBelow)
    → Window.process_action(Action::OpenLineBelow)
    → Buffer.insert_lines_after(cursor.line, 1)
    → Main loop: action.switches_to_insert_mode() == true
    → Mode switches to InsertMode
```

## Trade-offs

**Decision**: Support count prefix for both `o` and `O`

**Reasoning**: Matches Vim's behavior. Both o and O support counts in Vim.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Buffer edge cases at boundaries | Low | Medium | Thorough unit tests for first/last line |
