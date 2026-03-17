# Column Preservation for Vertical Movement - Technical Design

## Architecture Overview

Column preservation logic is centralized in `Window::process_action()`. Both horizontal reset and vertical preservation are handled uniformly after the action is processed:

- **Horizontal movement**: Reset remembered column to current position
- **Vertical movement**: Use remembered column (with clamping), then remember the target

The movement methods (`move_cursor_up`, `move_cursor_down`, etc.) perform basic cursor movement only, without any column preservation logic.

## Data Model

### BufferView Struct Changes

| Field | Type | Description |
|-------|------|-------------|
| `remembered_visual_col` | `Option<usize>` | Persisted column for vertical movement. `None` means "use current position". |

The `Option<usize>` allows us to distinguish between:
- `None`: First vertical move (use current visual column, then remember it)
- `Some(col)`: Use the remembered column

## Interface Design

### Action Enum Extension

Add methods to `Action` to categorize movements:

```rust
impl Action {
    /// Returns true if this action resets the remembered visual column
    /// to the current position. This includes horizontal movements and
    /// text modifications (insert/delete) that change cursor position.
    pub fn resets_remembered_column(&self) -> bool {
        matches!(self, 
            Action::MoveLeft 
            | Action::MoveRight 
            | Action::ForwardTo(_) 
            | Action::BackTo(_) 
            | Action::MoveToLineEnd 
            | Action::MoveToLineStart 
            | Action::MoveToLineContentStart
            | Action::InsertChar(_)
            | Action::DeleteBackward
            | Action::DeleteForward
        )
    }

    /// Returns true if this action is a vertical movement that should use
    /// and update the remembered visual column.
    pub fn uses_remembered_column(&self) -> bool {
        matches!(self, Action::MoveUp | Action::MoveDown)
    }
}
```

### New Methods on BufferView

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| `remembered_visual_col(&self)` | - | `Option<usize>` | Get the remembered column |
| `set_remembered_visual_col(col: usize)` | `usize` | `()` | Explicitly set remembered column |
| `update_remembered_to_current(&mut self)` | - | `()` | Update remembered col to current cursor position |
| `get_or_compute_target_col(&self)` | - | `usize` | Get remembered col, or current if None |

### Window.process_action Changes

All column preservation logic handled centrally after action:

```rust
impl Widget for Window {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        let result = match action {
            Action::MoveLeft => {
                self.move_cursor_left();
                ActionResult::Handled
            }
            Action::MoveDown => {
                // Get target column BEFORE move
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_down();
                // Remember the column we used
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveUp => {
                // Get target column BEFORE move
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_up();
                // Remember the column we used
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveRight => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            // ... other actions ...
            _ => NotHandled,
        };
        
        // Centralized column preservation logic
        if action.resets_remembered_column() {
            self.buffer_view.update_remembered_to_current();
        }
        
        result
    }
}
```

## Implementation Details

### Step 1: Add Field to BufferView

```rust
pub struct BufferView {
    buffer: Buffer,
    scroll_offset: Position,
    cursor: Cursor,
    remembered_visual_col: Option<usize>,  // NEW FIELD
}
```

Initialize to `None` in constructor.

### Step 2: Helper Methods on BufferView

```rust
impl BufferView {
    /// Get the target column for vertical movement.
    /// Returns remembered column if set, otherwise calculates from current position.
    fn get_or_compute_target_col(&self) -> usize {
        if let Some(col) = self.remembered_visual_col {
            return col;
        }
        // First vertical move: use current position
        self.buffer.visual_col_at(self.cursor)
    }

    /// Update remembered column from current cursor position.
    fn update_remembered_to_current(&mut self) {
        let cursor = self.cursor;
        self.remembered_visual_col = Some(self.buffer.visual_col_at(cursor));
    }

    /// Set remembered column to a specific value.
    fn set_remembered_visual_col(&mut self, col: usize) {
        self.remembered_visual_col = Some(col);
    }
}
```

### Step 3: Simplify move_cursor_up/down (Basic Movement Only)

These methods perform simple movement without column logic:

```rust
pub fn move_cursor_up(&mut self) {
    let cursor = self.cursor;
    // Use provided target_col parameter from caller
    if let Some(new_cursor) = self.buffer.cursor_up(cursor, target_col) {
        self.cursor = new_cursor;
    }
}

pub fn move_cursor_down(&mut self) {
    let cursor = self.cursor;
    // Use provided target_col parameter from caller
    if let Some(new_cursor) = self.buffer.cursor_down(cursor, target_col) {
        self.cursor = new_cursor;
    }
}
```

Wait - the buffer's `cursor_up`/`cursor_down` already take a target column. So we need to pass it through. Let me revise:

```rust
pub fn move_cursor_up(&mut self, target_col: usize) {
    let cursor = self.cursor;
    if let Some(new_cursor) = self.buffer.cursor_up(cursor, target_col) {
        self.cursor = new_cursor;
    }
}

pub fn move_cursor_down(&mut self, target_col: usize) {
    let cursor = self.cursor;
    if let Some(new_cursor) = self.buffer.cursor_down(cursor, target_col) {
        self.cursor = new_cursor;
    }
}
```

### Step 4: Update Window.process_action

Pass target column to vertical movement methods:

```rust
Action::MoveDown => {
    let target_col = self.buffer_view.get_or_compute_target_col();
    self.move_cursor_down(target_col);
    self.buffer_view.set_remembered_visual_col(target_col);
    ActionResult::Handled
}
Action::MoveUp => {
    let target_col = self.buffer_view.get_or_compute_target_col();
    self.move_cursor_up(target_col);
    self.buffer_view.set_remembered_visual_col(target_col);
    ActionResult::Handled
}
```

## Component Interactions

```
Key Press → NormalMode.handle_key() → Action
    ↓
Window.process_action(&Action)
    ↓
  [If vertical]
    → get_or_compute_target_col()
    → move_cursor_up/down(target_col)
    → set_remembered_visual_col(target_col)
  [If horizontal]
    → move_cursor_*()
    → update_remembered_to_current()
```

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| First vertical move ever | Use current visual column, remember it |
| Consecutive vertical moves | Use remembered column across all moves |
| Move to shorter line | Clamp to end of line (via `byte_pos_at_visual_col`) |
| Move to longer line | Place at remembered column |
| Horizontal movement | Reset remembered to current position |
| Insert/delete operations | Reset remembered to current position (cursor moved) |
| Vertical move at end of buffer | Stay at last line, clamp column |
| Mode switch (Normal ↔ Insert) | Keep remembered column (persists) |
| Actions that don't move cursor (e.g., SwitchToNormal) | No change to remembered column |

## Testing Strategy

1. **Unit tests for BufferView**:
   - Test remembered column persistence across moves
   - Test column clamping on short lines
   - Test reset on horizontal movement

2. **Integration tests**:
   - Full key sequences: jjj (down 3 lines) - should maintain column
   - ljjj (right then down) - should reset and maintain new column

## Out of Scope (Same as Requirements)

- Tab handling differences
- Blockwise visual mode
- Virtualedit integration
