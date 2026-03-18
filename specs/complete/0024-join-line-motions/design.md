# Join Line Motions (J and gJ) - Technical Design

## Architecture Overview

The J and gJ motions are implemented as new Action variants in the editor's action system. They follow the same pattern as other motions like j, k, h, l, but instead of moving the cursor, they modify the buffer by joining lines together.

### Data Flow

```
User Input: J or gJ
    ↓
NormalMode.handle_key() (with optional count prefix)
    ↓
Action::JoinWithSpace or Action::JoinWithoutSpace (wrapped in Action::Count if count provided)
    ↓
Window.process_action()
    ↓
Buffer.join_lines() - performs the actual text manipulation
    ↓
Cursor positioned at final join point
```

## Interface Design

### Action Variants (editor.rs)

| Action | Description |
|--------|-------------|
| `JoinWithSpace` | Joins current line with next, inserting a space |
| `JoinWithoutSpace` | Joins current line with next, without inserting space |

### Key Bindings (editor.rs)

| Key Sequence | Action | Notes |
|--------------|--------|-------|
| `J` | `Action::JoinWithSpace` | Single keystroke |
| `gJ` | `Action::JoinWithoutSpace` | Two-keystroke sequence (g then J) |

### Buffer API (buffer.rs)

```rust
/// Join lines starting at `start_line` for `line_count` lines.
/// If `with_space` is true, inserts a single space between joined lines.
/// Returns the cursor position at the end of the joined content.
pub fn join_lines(&mut self, start_line: usize, line_count: usize, with_space: bool) -> Option<Cursor>
```

Parameters:
- `start_line`: The line index to start joining from (0-indexed)
- `line_count`: Number of lines to join (at least 2 for a meaningful join)
- `with_space`: Whether to insert a space between joined lines

Returns:
- `Some(Cursor)` with the position at the end of the joined content
- `None` if start_line is out of bounds or line_count < 2

## Data Models

### Cursor Position

```rust
pub struct Cursor {
    pub line: usize,  // 0-indexed line number
    pub col: usize,   // byte position within the line
}
```

### Buffer Lines Storage

```rust
// In Buffer struct
lines: LinkedList<Arc<String>>
```

The buffer uses a linked list of strings where each element represents one line.

## Key Components

### 1. Action Enum Extension (editor.rs)

**Add to Action enum:**
```rust
pub enum Action {
    // ... existing variants
    JoinWithSpace,
    JoinWithoutSpace,
}
```

**Implement traits:**
```rust
impl Action {
    pub fn is_countable(&self) -> bool {
        matches!(
            self,
            // ... existing
            Action::JoinWithSpace | Action::JoinWithoutSpace
        )
    }
    
    pub fn resets_remembered_column(&self) -> bool {
        matches!(
            self,
            // ... existing  
            Action::JoinWithSpace | Action::JoinWithoutSpace
        )
    }
}
```

### 2. Key Binding Setup (editor.rs)

In `NormalMode::new()`:
```rust
// Add to keymap
keymap.insert("J".to_string(), Action::JoinWithSpace);

// For gJ, need prefix handling for 'g'
// The keymap already supports prefixes - 'g' will need to be registered
keymap.insert("gJ".to_string(), Action::JoinWithoutSpace);
```

Or alternatively, handle the 'g' prefix in the keymap setup similar to how it may be done elsewhere.

### 3. Window Action Handler (window.rs)

```rust
impl Widget for Window {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        match action {
            // ... existing matches
            Action::JoinWithSpace => {
                self.join_lines_with_space();
                ActionResult::Handled
            }
            Action::JoinWithoutSpace => {
                self.join_lines_without_space();
                ActionResult::Handled
            }
        }
    }
}
```

**Helper methods in Window:**
```rust
impl Window {
    fn join_lines_with_space(&mut self) {
        let count = self.get_current_count().unwrap_or(1);
        let cursor = self.buffer_view.get_cursor();
        
        if let Some(new_cursor) = self.buffer.join_lines(cursor.line, count + 1, true) {
            self.buffer_view.set_cursor(new_cursor);
            self.buffer_view.set_remembered_visual_col(new_cursor.col);
        }
    }
    
    fn join_lines_without_space(&mut self) {
        let count = self.get_current_count().unwrap_or(1);
        let cursor = self.buffer_view.get_cursor();
        
        if let Some(new_cursor) = self.buffer.join_lines(cursor.line, count + 1, false) {
            self.buffer_view.set_cursor(new_cursor);
            self.buffer_view.set_remembered_visual_col(new_cursor.col);
        }
    }
    
    /// Gets the current count from pending count or returns default
    fn get_current_count(&self) -> Option<usize> {
        // Implementation depends on how count is stored in Window
        // May need to check if Action::Count wrapper was used
    }
}
```

### 4. Buffer join_lines Implementation (buffer.rs)

```rust
impl Buffer {
    /// Join `line_count` lines starting from `start_line`.
    /// If `with_space` is true, insert a space between joined lines.
    /// Returns cursor position at end of joined content.
    pub fn join_lines(&mut self, start_line: usize, line_count: usize, with_space: bool) -> Option<Cursor> {
        // Validate inputs
        if line_count < 2 {
            return None;
        }
        
        let line_count = self.lines.len().saturating_sub(start_line).min(line_count);
        if line_count < 2 {
            return None; // Not enough lines to join
        }
        
        // Collect content from all lines to join
        let mut joined_content = String::new();
        
        for i in 0..line_count {
            let line_idx = start_line + i;
            if let Some(line) = self.lines.get(line_idx) {
                if i > 0 {
                    // Add space before content of subsequent lines (if with_space is true)
                    if with_space {
                        joined_content.push(' ');
                    }
                }
                joined_content.push_str(line);
            }
        }
        
        // Get remaining lines after the joined section
        let end_line = start_line + line_count;
        let right = self.lines.skip(end_line);
        
        // Replace the lines
        let mut left = self.lines.take(start_line);
        left.push_back(Arc::from(joined_content));
        left.append(right);
        self.lines = left;
        
        // Return cursor at end of joined content
        Some(Cursor::new(start_line, joined_content.len()))
    }
}
```

### 5. Count Handling

The existing `Action::Count(count, Box::new(Action))` pattern will wrap our join actions. In `Window::process_action()`, the count handling loop will execute the join action `count` times:

```rust
Action::Count(count, inner) => {
    // ... existing handling for line actions
    // For countable actions (including join), it loops:
    for _ in 0..*count {
        self.process_action(inner);
    }
    ActionResult::Handled
}
```

However, this would execute join N times, each time joining current with next. We need to join N+1 lines in a single operation instead. So we need special handling:

```rust
Action::Count(count, inner) => {
    match inner.as_ref() {
        Action::JoinWithSpace | Action::JoinWithoutSpace => {
            // Special handling: join count+1 lines at once
            let with_space = matches!(inner.as_ref(), Action::JoinWithSpace);
            let cursor = self.buffer_view.get_cursor();
            let actual_count = *count + 1;
            
            if let Some(new_cursor) = self.buffer.join_lines(cursor.line, actual_count, with_space) {
                self.buffer_view.set_cursor(new_cursor);
                self.buffer_view.set_remembered_visual_col(new_cursor.col);
            }
            ActionResult::Handled
        }
        // ... existing handling
    }
}
```

## User Interaction

### Invocation Patterns

| Input | Behavior |
|-------|----------|
| `J` | Join current line with next, with space |
| `gJ` | Join current line with next, without space |
| `2J` | Join 3 lines (current + 2), with space |
| `5gJ` | Join 6 lines (current + 5), without space |

### Cursor Positioning

After join:
- Cursor is positioned at the end of the joined content
- Column position is set to the byte length of the merged line
- Visual column is remembered for subsequent movements

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| `J` on last line | No operation (no next line to join) |
| `2J` when only 2 lines exist | Join both lines (equivalent to `J`) |
| `10J` when only 3 lines exist | Join all 3 lines |
| Empty lines | Handled correctly - spaces may be added before content |
| Single line buffer | No operation |

## External Dependencies

No external dependencies required. The implementation uses existing:
- Buffer API for line manipulation
- Action system for key handling
- Cursor/BufferView for cursor positioning

## Error Handling

| Error Condition | Handling |
|-----------------|----------|
| Start line beyond buffer | No operation, return None |
| Line count < 2 | No operation, return None |
| Join at last line | No operation, return None |

All errors are handled gracefully with no panic - the action simply does nothing.

## Security

Not applicable - this is a local text editing operation with no security implications.

## Configuration

No configuration required - these are standard Vim motions with standard behavior.

## Component Interactions

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│ NormalMode  │────▶│ Window       │────▶│ Buffer      │
│ (key input) │     │ (action exec)│     │ (line manip)│
└─────────────┘     └──────────────┘     └─────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │ BufferView   │
                    │ (cursor pos) │
                    └──────────────┘
```

## Trade-offs

**Decision**: Implement join as a single operation that joins N+1 lines rather than executing N separate joins.

**Reasoning**:
- More efficient (O(total_length) vs O(N * total_length))
- Simpler cursor positioning (single position vs multiple)
- Matches Vim behavior more closely

**Impact**:
- Requires special handling in the Count action branch
- Slightly more complex Window code

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Count handling incorrect | Low | Medium | Thorough test coverage |
| Edge cases not handled | Low | Medium | Test with boundary conditions |
| Performance with large joins | Low | Low | O(n) is acceptable for text editing |

## Testing Strategy

### Unit Tests (buffer.rs)

- Test join with space between two lines
- Test join without space between two lines
- Test join with count (3J = join 3 lines)
- Test join on last line (no operation)
- Test join when fewer lines than count
- Test join with empty lines

### Integration Tests (window.rs)

- Test J key binding produces expected result
- Test gJ key binding produces expected result  
- Test count prefix (2J, 5gJ)
- Test cursor positioning after join
- Test with visual mode (if applicable)

### Manual Testing

- `echo "hello\nworld" | urvim` → press J → should show "hello world"
- `echo "hello\nworld" | urvim` → press gJ → should show "helloworld"
- `echo "a\nb\nc\nd" | urvim` → press 2J → should show "a b c"
