# dd Command - Technical Design

## 1. Architecture Overview

The `dd` command will be implemented as a new `Action` variant in the editor, integrated into the existing action handling system. It follows the same pattern as existing line operations like `JoinWithSpace`.

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        Keymap                               │
│  "d" → pending, "d" (after "d") → Action::DeleteLine      │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Action::DeleteLine                      │
│  (with optional Action::Count wrapper for prefixes)        │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Window::process_action                   │
│  → buffer.remove(start_cursor, end_cursor)                  │
│  → Update cursor position                                    │
└─────────────────────────────────────────────────────────────┘
```

## 2. Interface Design

### Action Enum Addition

```rust
// In src/editor.rs
pub enum Action {
    // ... existing variants ...
    
    /// Delete current line (or N lines with count prefix)
    DeleteLine,
}
```

### Keymap Configuration

| Key Sequence | Action | Description |
|--------------|--------|-------------|
| `d` then `d` | `Action::DeleteLine` | Delete current line |
| `2` then `d` then `d` | `Action::Count(2, DeleteLine)` | Delete 2 lines |

The keymap will use the existing sequence handling mechanism (similar to how "gJ" works).

## 3. Data Models

No new data structures required. Uses existing:
- `Cursor` - for cursor position
- `Buffer` - for text storage
- `Action::Count` - for count prefix handling

## 4. Key Components

### 4.1 Action Definition (src/editor.rs)

**Responsibilities:**
- Define the DeleteLine action variant
- Update trait implementations (is_countable, resets_remembered_column, with_count)

**Public API:**
- Add `Action::DeleteLine` variant to enum
- Add `Action::DeleteLine` to `is_countable()` → returns true (repeatable)
- Add `Action::DeleteLine` to `resets_remembered_column()` → returns true

### 4.2 Keymap Setup (src/editor.rs)

**Responsibilities:**
- Register "d" as a prefix key
- Register "dd" sequence to trigger DeleteLine

**Public API:**
- Update normal mode keymap: `keymap.insert("d".to_string(), Action::Pending)` (if not already)
- Add sequence: `"dd"` → `Action::DeleteLine`

### 4.3 Window Action Handler (src/window.rs)

**Responsibilities:**
- Execute DeleteLine action
- Handle count prefix for multiple line deletion
- Update cursor position after deletion

**Public API:**
```rust
Action::DeleteLine => {
    self.delete_lines(1);
    ActionResult::Handled
}
Action::Count(count, inner) if matches!(inner.as_ref(), Action::DeleteLine) => {
    self.delete_lines(*count);
    ActionResult::Handled
}
```

### 4.4 Buffer Helper Method (src/buffer.rs)

**Responsibilities:**
- Remove entire lines from the buffer

**Public API:**
```rust
/// Deletes `count` lines starting from `start_line`.
/// Returns the new cursor position after deletion.
pub fn delete_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor>
```

## 5. User Interaction

### Invocation Patterns

1. **Single line deletion**: Press `d` then `d` (requires sequence handling)
2. **Multiple line deletion**: Press `[count]` then `d` then `d` (e.g., `3dd`)

### Execution Flow

```
User presses 'd'
    │
    ▼
Keymap returns Pending (waiting for second key)
    │
    ▼
User presses second 'd'
    │
    ▼
Keymap resolves "dd" → Action::DeleteLine
    │
    ▼
Window::process_action receives Action::DeleteLine
    │
    ▼
buffer.delete_lines(cursor.line, 1)
    │
    ▼
Update cursor to next line (or previous if was last line)
```

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| `dd` on last line | Delete line, move to previous line |
| `dd` on only line | Delete line, buffer now has 1 empty line |
| `5dd` when only 3 lines remain | Delete 3 lines (no error) |
| Count = 0 | No-op (count 0 is rejected by with_count) |

## 6. External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| buffer.remove() | Remove text between cursors | Already exists, can be reused |
| Buffer.line_count() | Check available lines | Already exists |
| Action::Count | Handle count prefix | Already exists |

## 7. Error Handling

| Error Condition | Handling |
|-----------------|----------|
| Empty buffer | No-op, return early |
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
buffer.rs (delete_lines method)
```

## 11. Platform Considerations

No platform-specific considerations - this is a pure logic implementation.

## 12. Trade-offs

**Decision**: Reuse existing `buffer.remove()` method vs. creating dedicated line deletion

**Reasoning**: 
- The existing `remove(start, end)` method already handles multi-line removal
- Less code duplication
- Consistent with existing patterns

**Impact**:
- Slightly more complex delete_lines implementation but better maintainability

## 13. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Sequence handling "d" prefix conflicts with future "dw", "d$" | Medium | Medium | Design "d" as prefix key, add new sequences as needed |
| Cursor position after deletion is incorrect | Low | High | Add comprehensive tests |

## 14. Implementation Notes

### 14.1 DeleteLine Behavior Summary

- **Base action**: `Action::DeleteLine`
- **Countable**: Yes (can repeat with count prefix)
- **Resets column**: Yes
- **Count behavior**: Delete N lines starting from cursor position

### 14.2 Cursor Positioning After Deletion

| Current Position | After `dd` |
|------------------|-------------|
| Not last line | Same line number (now contains next line) |
| Last line | Previous line (line_count - 1) |
| Only line | Line 0 (empty buffer state) |

### 14.3 Buffer.delete_lines Implementation Strategy

```rust
pub fn delete_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor> {
    let total_lines = self.line_count();
    if total_lines == 0 {
        return None;
    }
    
    // Clamp count to available lines
    let actual_count = (total_lines - start_line).min(count);
    if actual_count == 0 {
        return None;
    }
    
    // Calculate end position
    let end_line = start_line + actual_count;
    
    // Remove from start of first line to start of line after last deleted
    // This removes the entire lines including newlines
    let start = Cursor::new(start_line, 0);
    let end = if end_line < total_lines {
        Cursor::new(end_line, 0)  // Start of next line (the newline is skipped)
    } else {
        // Deleting to end of file
        Cursor::new(total_lines - 1, self.line_len(total_lines - 1))
    };
    
    self.remove(start, end);
    
    // Return new cursor position
    let new_line_count = self.line_count();
    if new_line_count == 0 {
        Some(Cursor::new(0, 0))
    } else if start_line >= new_line_count {
        Some(Cursor::new(new_line_count - 1, 0))
    } else {
        Some(Cursor::new(start_line, 0))
    }
}
```

### 14.4 Keymap Sequence Handling

The keymap currently handles "dd" through the existing prefix/sequence mechanism:
- "d" is registered as a prefix key (similar to how it might work for future "dw", "d$")
- "dd" explicitly maps to `Action::DeleteLine`

This ensures that pressing "d" then another key gives expected behavior.
