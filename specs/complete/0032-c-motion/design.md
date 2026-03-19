# C Motion - Change to End of Line - Technical Design

## 1. Architecture Overview

The `C` command will be implemented as a new `Action` variant that deletes text from cursor position to the end of the line (and subsequent lines with count) and enters insert mode at the truncation point.

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        Keymap                                │
│  "C" → Action::ChangeToLineEnd                              │
│  "2" then "C" → Action::Count(2, ChangeToLineEnd)           │
└─────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│               Action::ChangeToLineEnd                        │
│  (with optional Action::Count wrapper for prefixes)         │
└─────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                    Window::process_action                    │
│  → buffer.change_to_line_end(cursor, count)                 │
│  → Update cursor position                                    │
│  (Mode switch handled by main.rs via switches_to_insert_mode)│
└─────────────────────────────────────────────────────────────┘
```

## 2. Interface Design

### Action Enum Addition

```rust
// In src/editor.rs
pub enum Action {
    // ... existing variants ...
    
    /// Change from cursor to end of line (or N lines with count prefix) - delete and enter insert mode
    ChangeToLineEnd,
}
```

### Keymap Configuration

| Key Sequence | Action | Description |
|--------------|--------|-------------|
| `C` | `Action::ChangeToLineEnd` | Change from cursor to end of line |
| `2` then `C` | `Action::Count(2, ChangeToLineEnd)` | Change current line remainder + 1 more line |
| `3` then `C` | `Action::Count(3, ChangeToLineEnd)` | Change current line remainder + 2 more lines |

### Behavior Comparison

| Command | Behavior | Cursor After |
|---------|----------|-------------|
| `cc` | Delete entire current line, leave blank line | At start of blank line |
| `C` | Delete from cursor to end of line | At end of truncated line (in insert mode) |
| `c$` | Same as `C` | At end of truncated line |

## 3. Data Models

No new data structures required. Uses existing:
- `Cursor` - for cursor position (line, col)
- `Buffer` - for text storage
- `Action::Count` - for count prefix handling

## 4. Key Components

### 4.1 Action Definition (src/editor.rs)

**Responsibilities:**
- Define the `ChangeToLineEnd` action variant
- Update trait implementations

**Public API:**
- Add `Action::ChangeToLineEnd` variant to enum
- Add `Action::ChangeToLineEnd` to `is_countable()` → returns true
- Add `Action::ChangeToLineEnd` to `resets_remembered_column()` → returns true
- Add `Action::ChangeToLineEnd` to `switches_to_insert_mode()` → returns true

### 4.2 Keymap Setup (src/editor.rs)

**Responsibilities:**
- Register "C" key to trigger `ChangeToLineEnd`

**Public API:**
```rust
keymap.insert("C".to_string(), Action::ChangeToLineEnd);
```

### 4.3 Window Action Handler (src/window.rs)

**Responsibilities:**
- Execute `ChangeToLineEnd` action
- Handle count prefix for multiple line change
- Update cursor position after change

**Public API:**
```rust
Action::ChangeToLineEnd => {
    self.handle_count_change_to_line_end(1);
    ActionResult::Handled
}
Action::Count(count, inner) if matches!(inner.as_ref(), Action::ChangeToLineEnd) => {
    self.handle_count_change_to_line_end(*count);
    ActionResult::Handled
}
```

### 4.4 Buffer Helper Method (src/buffer.rs)

**Responsibilities:**
- Delete text from cursor to end of line(s)
- Return new cursor position at the end of remaining text

**Public API:**
```rust
/// Changes text from cursor to end of `count` lines.
/// Deletes from `start` cursor to end of `count` lines.
/// Returns the new cursor position at the end of the remaining text on the first line.
///
/// # Arguments
///
/// * `start` - Cursor position to start deletion from (on first line)
/// * `count` - Number of lines to affect (starting from start.line)
///
/// # Example
///
/// ```
/// use urvim::buffer::{Buffer, Cursor};
///
/// let mut buf = Buffer::from_str("hello world");
/// let cursor = Cursor::new(0, 5);  // after "hello"
/// let new_cursor = buf.change_to_line_end(cursor, 1);
/// assert_eq!(new_cursor, Some(Cursor::new(0, 5)));  // at "hello"
/// assert_eq!(buf.as_str(), "hello");
/// ```
pub fn change_to_line_end(&mut self, start: Cursor, count: usize) -> Option<Cursor>
```

### 4.5 Window Helper Method (src/window.rs)

**Responsibilities:**
- Call `buffer.change_to_line_end` and update cursor

**Public API:**
```rust
fn handle_count_change_to_line_end(&mut self, count: usize) -> ActionResult {
    let cursor = self.buffer_view.cursor();
    if let Some(new_cursor) = self.buffer_view.buffer.change_to_line_end(cursor, count) {
        self.buffer_view.set_cursor(new_cursor);
    }
    ActionResult::Handled
}
```

## 5. User Interaction

### Invocation Patterns

1. **Single line change**: Press `C` (delete cursor to EOL, enter insert mode)
2. **Multiple line change**: Press `[count]` then `C` (e.g., `2C`)

### Execution Flow

```
User presses 'C'
    │
    ▼
Keymap resolves "C" → Action::ChangeToLineEnd
    │
    ▼
Window::process_action receives Action::ChangeToLineEnd
    │
    ▼
window.handle_count_change_to_line_end(1)
    │
    ▼
buffer.change_to_line_end(cursor, 1)
    │ (truncate line at cursor position)
    │
    ▼
Cursor positioned at end of truncated line
    │
    ▼
main.rs checks switches_to_insert_mode() → true
    │
    ▼
Mode switches to InsertMode
```

### Count Behavior

For `nC`:
1. Take `n` lines starting from cursor line
2. Delete from cursor position in line 0 to end of line `n-1`
3. Keep text before cursor in line 0
4. Result is one line (the truncated first line) with cursor at end

**Example**: `"hell|o world"` with `2C`:
- Line 0: `"hello world"`
- Line 1: `"second line"`
- Cursor at `line=0, col=5` (after "hello")
- `2C` deletes from `" world"` to end of line 1
- Result: `["hello"]` with cursor at `(0, 5)` (after "hello", in insert mode)

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| `C` with cursor at end of line | No-op, stay in normal mode |
| `C` on last line | Delete cursor to end of last line |
| `2C` on last 2 lines | Delete remainder of line N-1 and line N |
| `C` on empty line | No-op (nothing to delete) |
| Count = 0 | No-op (count 0 is rejected by with_count) |

## 6. External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| Buffer.remove() | Delete cursor to EOL | Already exists |
| Action::Count | Handle count prefix | Already exists |
| Action trait methods | Mode switching | Already exists |

## 7. Error Handling

| Error Condition | Handling |
|----------------|----------|
| Cursor at end of line | No-op (don't delete nothing) |
| Count exceeds available lines | Clamp to available lines |
| Invalid cursor position | debug_assert in debug, clamp in release |

## 8. Security

Not applicable - this is a local text editing operation with no security implications.

## 9. Configuration

No new configuration needed.

## 10. Component Interactions

```
editor.rs (Action enum + keymap)
         │
         ▼
window.rs (process_action handler)
         │
         ▼
buffer.rs (change_to_line_end method)
```

## 11. Platform Considerations

No platform-specific considerations - this is a pure logic implementation.

## 12. Trade-offs

**Decision**: Create new `change_to_line_end` method vs. reusing `remove` directly

**Reasoning**:
- Single method encapsulates the complete behavior
- Clearer API for the action semantics
- Consistent with existing buffer API patterns (e.g., `change_lines`)

**Impact**:
- Slightly more code but better encapsulation

## 13. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Cursor at end of line causes issues | Low | Medium | Test explicitly, handle as no-op |
| Count handling incorrect for edge cases | Low | High | Add comprehensive tests |
| Mode switch timing | Low | Medium | Follow established pattern from cc |

## 14. Implementation Notes

### 14.1 ChangeToLineEnd Behavior Summary

- **Base action**: `Action::ChangeToLineEnd`
- **Countable**: Yes (can repeat with count prefix)
- **Resets column**: Yes
- **Switches to insert mode**: Yes
- **Count behavior**: Delete from cursor to EOL of `count` lines

### 14.2 Cursor Positioning After Change

| Current Position | After `C` |
|------------------|-----------|
| Middle of line | Same line, same column (now at end of truncated text) |
| End of line | No-op (nothing to delete) |
| Start of line | Column 0 (entire line deleted, empty line remains) |

### 14.3 Buffer.change_to_line_end Implementation Strategy

```rust
pub fn change_to_line_end(&mut self, start: Cursor, count: usize) -> Option<Cursor> {
    let total_lines = self.lines.len();
    
    // Handle empty buffer
    if total_lines == 0 {
        return Some(Cursor::new(0, 0));
    }
    
    // Validate start position
    if start.line >= total_lines {
        return None;
    }
    
    // If cursor is at end of line and count is 1, nothing to do
    let line_len = self.lines.get(start.line).map(|l| l.len()).unwrap_or(0);
    if start.col >= line_len && count == 1 {
        return Some(start);
    }
    
    // Clamp count to available lines
    let actual_count = (total_lines - start.line).min(count);
    if actual_count == 0 {
        return Some(start);
    }
    
    // Calculate end position: end of line (start.line + actual_count - 1)
    let end_line = start.line + actual_count - 1;
    let end_col = self.lines.get(end_line).map(|l| l.len()).unwrap_or(0);
    
    // Create end cursor at end of last line
    let end = Cursor::new(end_line, end_col);
    
    // Use remove to delete from start to end
    self.remove(start, end);
    
    // Return cursor at the original start position (which is now at end of truncated text)
    Some(start)
}
```

### 14.4 No-op Handling (Cursor at End of Line)

If `C` is pressed when cursor is already at the end of the line (and count is 1):
- Delete nothing (nothing to delete)
- **DO** switch to insert mode at current cursor position
- This is equivalent to `a` (append) behavior

This is consistent with Vim's `c$` behavior - when at EOL, it deletes 0 characters and enters insert mode at that position.

### 14.5 Mode Switch Flow

The mode switch to insert mode is handled automatically by main.rs:
1. `Action::ChangeToLineEnd` returns `switches_to_insert_mode() = true`
2. After window processes the action and returns `Handled`
3. main.rs checks `action.switches_to_insert_mode()`
4. If true, switches to `InsertMode::new()`

**Implementation**: Always return `Handled` and call `buffer.change_to_line_end()`:
- If cursor at EOL: buffer unchanged, cursor unchanged, mode switches to insert
- If cursor mid-line: buffer truncated at cursor, cursor unchanged, mode switches to insert

This matches Vim's `c$` behavior where at EOL it becomes equivalent to `a` (append).
