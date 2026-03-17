# Motion Count Parsing - Technical Design

## Architecture Overview

The count parsing feature will be implemented by modifying the `NormalMode` to track pending count prefixes and adding a `Count` variant to the `Action` enum to carry count information for repeatable motions.

### Data Flow

```
Key Press → NormalMode::handle_key() 
  → Check for count prefix (digits matching [1-9][0-9]*)
  → Accumulate digits OR complete motion with count
  → Return HandleKeyResult::Complete(Action::Count(count, Box::new(action)))
  → Window::process_action() executes motion N times
```

## Interface Design

### Modified Action Enum

The `Action` enum will be extended with a Count variant:

```rust
// In src/editor.rs

enum Action {
    // Basic movements (with optional count)
    MoveLeft,        // Can be preceded by count
    MoveDown,        // Can be preceded by count  
    MoveUp,          // Can be preceded by count
    MoveRight,       // Can be preceded by count
    
    // Boundary motions (with optional count)
    ForwardTo(Boundary),   // Can be preceded by count
    BackTo(Boundary),      // Can be preceded by count
    
    // Line position commands (with optional count)
    MoveToLineEnd,         // Can be preceded by count
    MoveToLineStart,       // Can be preceded by count
    MoveToLineContentStart,// Can be preceded by count
    
    // NEW: Count wrapper - repeats the inner action count times
    Count(usize, Box<Action>),
    
    // Other actions remain unchanged...
}
```

### Helper Methods on Action

```rust
impl Action {
    /// Returns true if this action can be wrapped in a Count
    pub fn is_countable(&self) -> bool {
        matches!(
            self,
            Action::MoveLeft 
                | Action::MoveRight 
                | Action::MoveUp 
                | Action::MoveDown 
                | Action::ForwardTo(_) 
                | Action::BackTo(_)
        )
    }
    
    /// Returns true if this action is a line action that takes a line count.
    /// Line actions take an absolute line number and perform the action at that line.
    /// Examples: 5$ = go to line 5 and move to end, 5^ = go to line 5 and move to content start
    pub fn is_line_action(&self) -> bool {
        matches!(
            self,
            Action::MoveToLineEnd
                | Action::MoveToLineStart
                | Action::MoveToLineContentStart
        )
    }
    
    /// Wraps this action in a Count variant if it's countable or a line action
    pub fn with_count(self, count: usize) -> Option<Action> {
        if (self.is_countable() || self.is_line_action()) && count > 0 && count < 10000 {
            Some(Action::Count(count, Box::new(self)))
        } else {
            None
        }
    }
}
```

### Modified NormalMode

```rust
pub struct NormalMode {
    keymap: SimpleKeymap,
    buffer: Vec<String>,
    waiting: bool,
    // NEW: pending count being accumulated
    pending_count: Option<usize>,
}

impl NormalMode {
    /// Check if a key string represents a digit that can start a count
    fn is_count_digit(s: &str) -> bool {
        s.len() == 1 && s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
    }
    
    /// Check if current buffer forms a valid count prefix [1-9][0-9]*
    fn is_count_prefix(&self) -> bool {
        // Join buffer and check if it matches [1-9][0-9]*
        let combined: String = self.buffer.iter().cloned().collect();
        Self::is_valid_count(&combined)
    }
    
    /// Check if a string is a valid count (at least one non-zero digit)
    fn is_valid_count(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        // Must start with 1-9 (non-zero)
        let first_char = s.chars().next().unwrap();
        if first_char < '1' || first_char > '9' {
            return false;
        }
        // All characters must be digits
        s.chars().all(|c| c.is_ascii_digit())
    }
}
```

## Key Components

### NormalMode::handle_key() - Modified Flow

1. **Escape pressed**: Clear `pending_count` and buffer
2. **Digit key pressed**:
   - Add to buffer
   - Check if buffer now forms valid count prefix
   - If valid count, set `waiting = true` and return `WaitForMore`
   - If becomes invalid (e.g., starts with 0), clear buffer and return `InvalidSequence`
3. **Motion key pressed**:
   - If buffer has valid count prefix: wrap action in `Action::Count(count, Box::new(action))`
   - If buffer is empty: execute motion without count
   - Clear buffer and `pending_count`
4. **Other key pressed**: Clear buffer and return `InvalidSequence`

### Window::process_action() - Modified Flow

```rust
fn process_action(&mut self, action: &Action) -> ActionResult {
    match action {
        Action::Count(count, inner) => {
            if inner.is_line_action() {
                // Line action: go to target line (absolute), then perform action
                let target_line = count.saturating_sub(1); // Lines are 0-indexed internally
                // Move to target line (stay at current column if possible)
                self.buffer_view.set_cursor(Cursor::new(target_line, self.buffer_view.cursor().col));
                // Then execute the line action
                self.process_action(inner)
            } else {
                // Repeatable action: execute motion count times
                for _ in 0..*count {
                    self.process_action(inner)?;
                }
                ActionResult::Handled
            }
        }
        // ... other action handlers
    }
}
```

## User Interaction

### Input Examples

| Input | Result |
|-------|--------|
| `j` | Move down 1 line |
| `5j` | Move down 5 lines |
| `10w` | Move forward 10 words |
| `3b` | Move backward 3 words |
| `2k` | Move up 2 lines |
| `100W` | Move forward 100 BIGWORDs |
| `2$` | Move to end of line 2 (absolute) |
| `3^` | Move to content start of line 3 (absolute) |
| `Esc` during count entry | Clear count, return to idle |

### Key Sequences

1. **Single motion**: Key pressed → Action executed
2. **Counted motion**: Digit(s) pressed → Motion key pressed → Motion executed N times

### Error States

| State | Behavior |
|-------|----------|
| Count starts with 0 | Invalid, clear buffer |
| Count entered but no motion | Invalid, clear buffer |
| Count followed by non-motion | Invalid, clear buffer |

## Key Components

### MotionExecutor

A helper trait or methods to apply count to motions:

```rust
impl Window {
    /// Execute a motion action, repeating count times if applicable
    fn execute_motion_counted(&mut self, action: &Action, count: usize) {
        for _ in 0..count.saturating_sub(1) {
            self.execute_motion(action);
        }
        // Execute final time (which may fail at end of buffer)
        self.execute_motion(action);
    }
}
```

## External Dependencies

No external dependencies required. This feature uses only existing internal modules.

## Error Handling

| Error Condition | Handling |
|-----------------|-----------|
| Count prefix becomes invalid (e.g., "0j") | Clear buffer, return InvalidSequence |
| Count with non-motion key | Clear buffer, return InvalidSequence |
| Motion at buffer boundary with count | Execute as many as possible (vim behavior: stop at boundary) |

## Security

No security concerns - this is purely input handling within the editor.

## Configuration

No new configuration required.

## Trade-offs

**Decision**: Use `Action::Count(usize, Box<Action>)` variant in the Action enum

**Reasoning**:
- Keeps everything within the Action enum - no separate type needed
- Consistent with Rust enum patterns
- Easy to match and handle in process_action
- Natural recursion for nested counts if needed

**Alternative considered**: Separate CountedAction struct (what we decided against)
- Would require modifying HandleKeyResult to have CompleteCounted variant
- More types to manage
- Slightly more complex dispatch

## Implementation Plan

1. Add `Action::Count(usize, Box<Action>)` variant to the Action enum in `editor.rs`
2. Add helper methods `is_countable()` and `with_count()` on Action
3. Modify `NormalMode` to track pending count
4. Implement count prefix detection in `NormalMode::handle_key`
5. Modify `Window::process_action` to unwrap and repeat Action::Count
6. Add tests for count parsing

## Testing Strategy

- Unit tests for count prefix validation
- Unit tests for NormalMode::handle_key with counts
- Integration tests for motion execution with counts
- Edge cases: count at buffer boundaries, invalid sequences
