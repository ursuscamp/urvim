# Dollar Key - End of Line Navigation - Technical Design

## Architecture Overview

This feature adds a new `$` key binding in Normal mode that moves the cursor to the end of the current line. The implementation follows the existing pattern for movement keys in urvim:

1. **Input**: User presses `$` key in Normal mode
2. **Mode Handler**: `NormalMode::handle_key` returns `Action::MoveToLineEnd`
3. **Widget Handler**: `Window::process_action` handles the action and calls `move_cursor_to_line_end`
4. **Buffer Logic**: The movement is computed in `Buffer::cursor_end_of_line`

## Interface Design

### Action Enum (editor.rs)

Add a new variant to the `Action` enum:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // ... existing variants ...
    /// Move cursor to end of current line
    MoveToLineEnd,
}
```

### Normal Mode Key Handler (editor.rs)

Add key binding in `NormalMode::handle_key`:

```rust
// Line end navigation
(KeyCode::Char('$'), _) if !key.modifiers.has_ctrl() => Action::MoveToLineEnd,
```

### Boundary Enum (buffer.rs)

Add a new variant to the `Boundary` enum:

```rust
pub enum Boundary {
    // ... existing variants ...
    /// End of line
    LineEnd,
}
```

### Buffer Method (buffer.rs)

Add a new method to calculate cursor at end of line:

```rust
/// Move cursor to the end of the current line (last non-whitespace character).
/// If already at end of current line, moves to end of next line.
/// Returns None if already at end of last line.
pub fn cursor_end_of_line(&self, cursor: Cursor) -> Option<Cursor>
```

### Window Method (window.rs)

Add a new method to handle the movement:

```rust
/// Move cursor to end of current line.
/// If already at end of line, moves to end of next line.
pub fn move_cursor_to_line_end(&mut self) {
    let cursor = self.buffer_view.cursor();
    if let Some(new_cursor) = self.buffer_view.buffer().cursor_end_of_line(cursor) {
        self.buffer_view.set_cursor(new_cursor);
    }
}
```

Add handling in `Window::process_action`:

```rust
Action::MoveToLineEnd => {
    self.move_cursor_to_line_end();
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

### Boundary Enum (modified)

```rust
pub enum Boundary {
    Word,           // Word boundary (alphanumeric + underscore)
    WordEnd,        // End of word boundary
    BigWord,        // BigWord boundary (non-whitespace)
    BigWordEnd,     // End of BigWord boundary
    LineEnd,        // NEW: End of line
}
```

## Key Components

### 1. NormalMode (editor.rs)

**Responsibilities:**
- Handle key events in Normal mode
- Map `$` key to `Action::MoveToLineEnd`

**Public API:**
- `handle_key(&Key) -> Action`

**Changes:**
- Add case for `KeyCode::Char('$')` in match statement

### 2. Buffer (buffer.rs)

**Responsibilities:**
- Store text content
- Provide cursor movement calculations

**Public API:**
- `cursor_end_of_line(cursor: Cursor) -> Option<Cursor>`

**Implementation Logic:**
1. Get current line length
2. If cursor is before end of line, move to last non-whitespace character
3. If cursor is already at or past end of line:
   - If not on last line, move to end of next line
   - If on last line, return None (no movement)

### 3. Window (window.rs)

**Responsibilities:**
- Manage buffer view and cursor
- Handle user actions

**Public API:**
- `move_cursor_to_line_end(&mut self)`

**Changes:**
- Add new method `move_cursor_to_line_end`
- Add `Action::MoveToLineEnd` case in `process_action`

## User Interaction

### Primary Flow

1. User is in Normal mode with cursor at position (line, col)
2. User presses `$`
3. Cursor moves to (line, line_len - 1) if there are non-whitespace chars, or last non-whitespace char
4. If cursor was already at end of line, moves to next line's end

### Edge Cases

| Scenario | Current Position | Expected Result |
|----------|-------------------|-----------------|
| Middle of line | (0, 2) on "hello" | (0, 4) |
| Already at end | (0, 4) on "hello" | (1, 4) on "hello\nworld" |
| Last line, at end | (1, 4) on "hello\nworld" | No movement |
| Empty line | (0, 0) on "" | No movement |
| Empty buffer | (0, 0) | No movement |

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
| Empty line | Return None - no movement |
| At end of last line | Return None - no movement |
| At end of line (not last) | Wrap to next line's end |

## Security

No security concerns - this is a read navigation operation that only affects cursor position.

## Configuration

No configuration needed - this is a built-in vim-compatible key binding.

## Component Interactions

```
User presses '$'
       │
       ▼
Terminal captures key event
       │
       ▼
NormalMode::handle_key($) → Action::MoveToLineEnd
       │
       ▼
Window::process_action(Action::MoveToLineEnd)
       │
       ▼
Window::move_cursor_to_line_end()
       │
       ▼
Buffer::cursor_end_of_line(cursor) → new_cursor
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

**Decision**: Add `LineEnd` as a separate method rather than using the Boundary system

**Reasoning**:
- The Boundary system is designed for word-based navigation with complex wrapping logic
- Line end is simpler - just find last non-whitespace char on current or next line
- Keeps the Boundary enum focused on word-based boundaries
- Simpler to implement and maintain

**Impact**:
- Slightly different code path than word movements, but consistent behavior from user perspective

## Implementation Plan

1. Add `Action::MoveToLineEnd` to editor.rs
2. Add `$` key handling in NormalMode::handle_key
3. Add `cursor_end_of_line` method in buffer.rs
4. Add `move_cursor_to_line_end` method in window.rs
5. Handle `Action::MoveToLineEnd` in Window::process_action
6. Add unit tests for the new functionality

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Edge case not handled | Low | Medium | Comprehensive unit tests |
| Performance issue | Low | Low | Simple O(line_length) operation |
| Regression in existing code | Low | High | Run full test suite |
