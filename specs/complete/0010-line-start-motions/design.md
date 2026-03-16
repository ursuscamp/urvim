# Line Start Motions - Technical Design

## Architecture Overview

This feature adds two new key bindings in Normal mode:
- `0` - moves to absolute start of line (column 0)
- `^` - moves to first non-whitespace character of line, wrapping to previous line

The implementation follows the existing pattern for the `$` key (end of line) implemented in spec 0008:

1. **Input**: User presses `0` or `^` key in Normal mode
2. **Mode Handler**: `NormalMode::handle_key` returns `Action::MoveToLineStart` or `Action::MoveToLineContentStart`
3. **Widget Handler**: `Window::process_action` handles the actions and calls movement methods
4. **Buffer Logic**: The movements are computed in `Buffer::cursor_start_of_line` and `Buffer::cursor_content_start_of_line`

## Interface Design

### Action Enum (editor.rs)

Add two new variants to the `Action` enum:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // ... existing variants ...
    /// Move cursor to absolute start of line (column 0)
    MoveToLineStart,
    /// Move cursor to first non-whitespace of line
    MoveToLineContentStart,
}
```

### Normal Mode Key Handler (editor.rs)

Add key bindings in `NormalMode::handle_key`:

```rust
// Line start navigation
(KeyCode::Char('0'), _) if !key.modifiers.has_ctrl() => Action::MoveToLineStart,
(KeyCode::Char('^'), _) if !key.modifiers.has_ctrl() => Action::MoveToLineContentStart,
```

### Buffer Methods (buffer.rs)

Add two new methods to calculate cursor positions. Note: cursor column is stored as a **byte offset**, so we use `grapheme_indices(true)` which returns `(byte_index, grapheme)` pairs.

```rust
/// Move cursor to absolute start of line (column 0).
/// If already at column 0, returns None (no movement).
/// Returns byte offset position.
pub fn cursor_start_of_line(&self, cursor: Cursor) -> Option<Cursor>

/// Move cursor to first non-whitespace character of current line.
/// If already at first non-whitespace, wraps to previous line.
/// Returns None if already at first non-whitespace of first line.
/// Returns byte offset position.
pub fn cursor_content_start_of_line(&self, cursor: Cursor) -> Option<Cursor>
```

### Window Methods (window.rs)

Add new methods to handle the movements:

```rust
/// Move cursor to absolute start of line (column 0).
/// If already at column 0, does nothing.
pub fn move_cursor_to_line_start(&mut self) {
    let cursor = self.buffer_view.cursor();
    if let Some(new_cursor) = self.buffer_view.buffer().cursor_start_of_line(cursor) {
        self.buffer_view.set_cursor(new_cursor);
    }
}

/// Move cursor to first non-whitespace of current line.
/// If already there, wraps to previous line's first non-whitespace.
pub fn move_cursor_to_line_content_start(&mut self) {
    let cursor = self.buffer_view.cursor();
    if let Some(new_cursor) = self.buffer_view.buffer().cursor_content_start_of_line(cursor) {
        self.buffer_view.set_cursor(new_cursor);
    }
}
```

Add handling in `Window::process_action`:

```rust
Action::MoveToLineStart => {
    self.move_cursor_to_line_start();
    ActionResult::Handled
}
Action::MoveToLineContentStart => {
    self.move_cursor_to_line_content_start();
    ActionResult::Handled
}
```

## Data Models

### Cursor

The `Cursor` struct represents position in the buffer:

```rust
pub struct Cursor {
    pub line: usize,  // Line index (0-based)
    pub col: usize,   // Column index (0-based)
}
```

No changes to this model are needed.

## Key Components

### 1. NormalMode (editor.rs)

**Responsibilities:**
- Handle key events in Normal mode
- Map `0` and `^` keys to appropriate actions

**Public API:**
- `handle_key(&Key) -> Action`

**Changes:**
- Add case for `KeyCode::Char('0')` → `Action::MoveToLineStart`
- Add case for `KeyCode::Char('^')` → `Action::MoveToLineContentStart`

### 2. Buffer (buffer.rs)

**Responsibilities:**
- Store text content
- Provide cursor movement calculations

**Public API:**
- `cursor_start_of_line(cursor: Cursor) -> Option<Cursor>`
- `cursor_content_start_of_line(cursor: Cursor) -> Option<Cursor>`

**Implementation Logic for `cursor_start_of_line`:**
1. If cursor is already at column 0, return None (no movement)
2. Otherwise, return cursor at column 0 of current line

**Implementation Logic for `cursor_content_start_of_line`:**
1. Find first non-whitespace character position on current line
2. If cursor is before first non-whitespace, move to it
3. If cursor is at or past first non-whitespace:
   - If not on first line, move to first non-whitespace of previous line
   - If on first line, return None (no movement)

### 3. Window (window.rs)

**Responsibilities:**
- Manage buffer view and cursor
- Handle user actions

**Public API:**
- `move_cursor_to_line_start(&mut self)`
- `move_cursor_to_line_content_start(&mut self)`

**Changes:**
- Add new methods for line start movements
- Add `Action::MoveToLineStart` case in `process_action`
- Add `Action::MoveToLineContentStart` case in `process_action`

## User Interaction

### Primary Flow

**For `0` key:**
1. User is in Normal mode with cursor at position (line, col)
2. User presses `0`
3. If col > 0, cursor moves to (line, 0)
4. If col == 0, cursor stays (no movement)

**For `^` key:**
1. User is in Normal mode with cursor at position (line, col)
2. User presses `^`
3. If col > first non-whitespace, cursor moves to first non-whitespace
4. If col == first non-whitespace:
   - If line > 0, cursor moves to previous line's first non-whitespace
   - If line == 0, cursor stays (no movement)

### Edge Cases

| Scenario | Key | Current Position | Expected Result |
|----------|-----|------------------|-----------------|
| Middle of line | 0 | (0, 5) on "  hello" | (0, 0) |
| At column 0 | 0 | (0, 0) on "  hello" | No movement |
| Middle of line | ^ | (0, 5) on "  hello" | (0, 2) |
| At first non-whitespace | ^ | (0, 2) on "  hello" | (0, 2) on "  hello\n  world" → moves to prev line |
| At first non-whitespace, first line | ^ | (0, 2) on "  hello" | No movement |
| No leading whitespace | ^ | (0, 0) on "hello" | No movement (already at first non-ws) |

## External Dependencies

| Dependency | Purpose |
|------------|---------|
| buffer::Cursor | Cursor position representation |
| buffer::Buffer | Text buffer with line access |
| editor::Action | Action enum for key handling |
| window::Window | View management and cursor control |

No external dependencies required.

## Error Handling

The implementation handles edge cases gracefully:

| Condition | Handling |
|-----------|----------|
| Empty buffer | Return None - no movement |
| Cursor at column 0 with `0` | Return None - no movement |
| Cursor at first non-whitespace with `^` | Wrap to previous line if available |
| At first non-whitespace of first line with `^` | Return None - no movement |
| Line with no non-whitespace | Return column 0 |

## Security

No security concerns - this is a read navigation operation that only affects cursor position.

## Configuration

No configuration needed - this is a built-in vim-compatible key binding.

## Component Interactions

```
User presses '0'
        │
        ▼
Terminal captures key event
        │
        ▼
NormalMode::handle_key(0) → Action::MoveToLineStart
        │
        ▼
Window::process_action(Action::MoveToLineStart)
        │
        ▼
Window::move_cursor_to_line_start()
        │
        ▼
Buffer::cursor_start_of_line(cursor) → new_cursor
        │
        ▼
Window::set_cursor(new_cursor)
        │
        ▼
Cursor position updated on screen
```

```
User presses '^'
        │
        ▼
Terminal captures key event
        │
        ▼
NormalMode::handle_key(^) → Action::MoveToLineContentStart
        │
        ▼
Window::process_action(Action::MoveToLineContentStart)
        │
        ▼
Window::move_cursor_to_line_content_start()
        │
        ▼
Buffer::cursor_content_start_of_line(cursor) → new_cursor
        │
        ▼
Window::set_cursor(new_cursor)
        │
        ▼
Cursor position updated on screen
```

## Platform Considerations

This feature is platform-independent - it operates at the buffer abstraction level.

## Trade-offs

**Decision**: Implement `0` and `^` as separate actions rather than using the Boundary system

**Reasoning**:
- The Boundary system is designed for word-based navigation
- Line start is simpler and different from word boundaries
- Keeps the Boundary enum focused on word-based boundaries
- Simpler to implement and maintain

**Impact**:
- Slightly different code path than word movements, but consistent behavior from user perspective

## Implementation Plan

1. Add `Action::MoveToLineStart` and `Action::MoveToLineContentStart` to editor.rs
2. Add `0` and `^` key handling in NormalMode::handle_key
3. Add `cursor_start_of_line` method in buffer.rs
4. Add `cursor_content_start_of_line` method in buffer.rs
5. Add `move_cursor_to_line_start` method in window.rs
6. Add `move_cursor_to_line_content_start` method in window.rs
7. Handle `Action::MoveToLineStart` and `Action::MoveToLineContentStart` in Window::process_action
8. Add unit tests for the new functionality
9. Run full test suite

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Edge case not handled | Low | Medium | Comprehensive unit tests |
| Performance issue | Low | Low | Simple O(line_length) operation |
| Regression in existing code | Low | High | Run full test suite |
