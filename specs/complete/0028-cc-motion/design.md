# cc Motion - Technical Design

## 1. Architecture Overview

The `cc` command will be implemented as a new `Action` variant in the editor, similar to `DeleteLine`. It combines line deletion with automatic mode switch to insert mode.

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        Keymap                               │
│  "c" → pending, "c" (after "c") → Action::ChangeLine       │
└─────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                    Action::ChangeLine                        │
│  (with optional Action::Count wrapper for prefixes)        │
└─────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────┐
│                    Window::process_action                   │
│  → buffer.delete_lines(start_cursor, count)                  │
│  → buffer.insert_line(start_cursor) (to leave blank line)   │
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
    
    /// Change current line (or N lines with count prefix) - delete and enter insert mode
    ChangeLine,
}
```

### Keymap Configuration

| Key Sequence | Action | Description |
|--------------|--------|-------------|
| `c` then `c` | `Action::ChangeLine` | Change current line |
| `2` then `c` then `c` | `Action::Count(2, ChangeLine)` | Change 2 lines |
| `3` then `c` then `c` | `Action::Count(3, ChangeLine)` | Change 3 lines (leaves 1 blank) |

The keymap will use the existing sequence handling mechanism (similar to "dd").

### Behavior Difference from dd

| Command | Behavior | Example (lines [1,2,3,4,5], cursor at 2) |
|---------|----------|------------------------------------------|
| `dd` | Delete line entirely | Result: [1,3,4,5] |
| `cc` | Delete line, leave blank | Result: [1,"",3,4,5] |
| `3dd` | Delete 3 lines entirely | Result: [1,5] |
| `3cc` | Delete 3 lines, leave 1 blank | Result: [1,"",5] |

## 3. Data Models

No new data structures required. Uses existing:
- `Cursor` - for cursor position
- `Buffer` - for text storage
- `Action::Count` - for count prefix handling

## 4. Key Components

### 4.1 Action Definition (src/editor.rs)

**Responsibilities:**
- Define the ChangeLine action variant
- Update trait implementations

**Public API:**
- Add `Action::ChangeLine` variant to enum
- Add `Action::ChangeLine` to `is_countable()` → returns true (repeatable)
- Add `Action::ChangeLine` to `resets_remembered_column()` → returns true
- Add `Action::ChangeLine` to `switches_to_insert_mode()` → returns true

### 4.2 Keymap Setup (src/editor.rs)

**Responsibilities:**
- Register "c" as a prefix key
- Register "cc" sequence to trigger ChangeLine

**Public API:**
- Add keymap entry: `keymap.insert("c".to_string(), Action::Pending)` (if not already)
- Add sequence: `"cc"` → `Action::ChangeLine`

### 4.3 Window Action Handler (src/window.rs)

**Responsibilities:**
- Execute ChangeLine action
- Handle count prefix for multiple line replacement
- Update cursor position after replacement

**Public API:**
```rust
Action::ChangeLine => {
    self.change_line(1);
    ActionResult::Handled
}
Action::Count(count, inner) if matches!(inner.as_ref(), Action::ChangeLine) => {
    self.change_line(*count);
    ActionResult::Handled
}
```

### 4.4 Buffer Helper Method (src/buffer.rs)

**Responsibilities:**
- Delete N lines and insert one blank line in their place

**Public API:**
```rust
/// Changes `count` lines starting from `start_line`.
/// Deletes the lines and replaces them with a single empty line.
/// Returns the new cursor position.
pub fn change_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor>
```

### 4.5 Window Helper Method (src/window.rs)

**Responsibilities:**
- Call buffer.change_lines and update cursor

**Public API:**
```rust
fn change_line(&mut self, count: usize) {
    let cursor = self.buffer_view.cursor();
    if let Some(new_cursor) = self.buffer_view.buffer.change_lines(cursor.line, count) {
        self.buffer_view.set_cursor(new_cursor);
    }
}
```

## 5. User Interaction

### Invocation Patterns

1. **Single line change**: Press `c` then `c` (requires sequence handling)
2. **Multiple line change**: Press `[count]` then `c` then `c` (e.g., `3cc`)

### Execution Flow

```
User presses 'c'
    │
    ▼
Keymap returns Pending (waiting for second key)
    │
    ▼
User presses second 'c'
    │
    ▼
Keymap resolves "cc" → Action::ChangeLine
    │
    ▼
Window::process_action receives Action::ChangeLine
    │
    ▼
buffer.change_lines(cursor.line, 1)
    │ (deletes line, inserts blank line)
    │
    ▼
Cursor positioned at start of blank line
    │
    ▼
main.rs checks switches_to_insert_mode() → true
    │
    ▼
Mode switches to InsertMode
```

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| `cc` on last line | Replace last line with blank, cursor on blank line |
| `cc` on only line | Buffer has 1 empty line, cursor in insert mode |
| `5cc` when only 3 lines remain | Replace 3 lines with 1 blank line (no error) |
| Count = 0 | No-op (count 0 is rejected by with_count) |

## 6. External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| buffer.delete_lines() | Remove lines | Already exists |
| Buffer.insert_line() | Insert blank line | Need to verify exists or create |
| Action::Count | Handle count prefix | Already exists |

## 7. Error Handling

| Error Condition | Handling |
|-----------------|----------|
| Empty buffer | Create one empty line, enter insert mode |
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
buffer.rs (change_lines method)
```

## 11. Platform Considerations

No platform-specific considerations - this is a pure logic implementation.

## 12. Trade-offs

**Decision**: Create new `change_lines` method vs. reusing delete_lines + insert_line separately

**Reasoning**:
- Single method is cleaner and ensures atomic operation
- Easier to maintain cursor positioning logic
- Consistent with existing buffer API patterns

**Impact**:
- Slightly more code but better encapsulation

## 13. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Sequence handling "c" prefix conflicts with future "cw", "c$" | Medium | Medium | Design "c" as prefix key, add new sequences as needed |
| Cursor position after change is incorrect | Low | High | Add comprehensive tests |
| Insert line method doesn't exist | Low | Medium | Create if needed |

## 14. Implementation Notes

### 14.1 ChangeLine Behavior Summary

- **Base action**: `Action::ChangeLine`
- **Countable**: Yes (can repeat with count prefix)
- **Resets column**: Yes
- **Switches to insert mode**: Yes
- **Count behavior**: Delete N lines, replace with 1 blank line

### 14.2 Cursor Positioning After Change

| Current Position | After `cc` |
|------------------|-------------|
| Not last line | Same line number (now contains empty line) |
| Last line | Previous line (now contains empty line) |
| Only line | Line 0 (empty line) |

### 14.3 Buffer.change_lines Implementation Strategy

```rust
pub fn change_lines(&mut self, start_line: usize, count: usize) -> Option<Cursor> {
    let total_lines = self.lines.len();
    
    // Handle empty buffer - create one empty line
    if total_lines == 0 {
        self.lines.push_back(Arc::from(""));
        return Some(Cursor::new(0, 0));
    }
    
    // Validate start_line
    if start_line >= total_lines {
        return None;
    }
    
    // Clamp count to available lines
    let actual_count = (total_lines - start_line).min(count);
    if actual_count == 0 {
        return Some(Cursor::new(start_line, 0));
    }
    
    // Delete the lines (similar to delete_lines)
    let end_line = start_line + actual_count;
    
    if end_line >= total_lines {
        // Deleting to end of file
        let mut left = self.lines.take(start_line);
        // Always keep at least one line (the blank line)
        if left.is_empty() {
            left.push_back(Arc::from(""));
        }
        self.lines = left;
    } else {
        // Deleting in middle of file
        // Keep lines before start_line, add blank line, then lines after end_line
        let mut left = self.lines.take(start_line);
        left.push_back(Arc::from(""));  // Insert blank line
        let right = self.lines.skip(end_line);
        left.append(right);
        self.lines = left;
    }
    
    // Return new cursor position (at start of blank line)
    Some(Cursor::new(start_line, 0))
}
```

### 14.4 Keymap Sequence Handling

The keymap will handle "cc" through the existing prefix/sequence mechanism:
- "c" is registered as a prefix key (similar to "d")
- "cc" explicitly maps to `Action::ChangeLine`

This ensures that pressing "c" then another key gives expected behavior.

### 14.5 Mode Switch Flow

The mode switch to insert mode is handled automatically by main.rs:
1. Action::ChangeLine returns `switches_to_insert_mode() = true`
2. After window processes the action and returns Handled
3. main.rs checks `action.switches_to_insert_mode()`
4. If true, switches to `InsertMode::new()`

No explicit mode switch needed in window.rs handler.
