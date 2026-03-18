# H/M/L Motions - Technical Design

## Architecture Overview

H/M/L motions are line-relative cursor movements that navigate to specific screen positions without scrolling the viewport. They are implemented as:

1. **New Action variants** in `editor.rs` - Define `MoveToScreenTop`, `MoveToScreenMiddle`, `MoveToScreenBottom`
2. **Key bindings** in `NormalMode` - Map `H`, `M`, `L` keys to these actions
3. **Action processing** in `Window::process_action` - Calculate target buffer line based on current viewport

The motion logic uses:
- `buffer_view.scroll_offset().row` - First visible buffer line (viewport top)
- `viewport_size.rows` - Number of visible rows in viewport

## Interface Design

### Action Variants

| Action | Count Support | Description |
|--------|---------------|-------------|
| `MoveToScreenTop` | Yes (N = lines from top) | Move cursor to Nth line from top of viewport |
| `MoveToScreenMiddle` | No | Move cursor to middle line of viewport |
| `MoveToScreenBottom` | Yes (N = lines from bottom) | Move cursor to Nth line from bottom of viewport |

### Key Bindings

| Key | Action | Count Behavior |
|-----|--------|----------------|
| `H` | `MoveToScreenTop` | `3H` = 3rd line from top |
| `M` | `MoveToScreenMiddle` | Ignores count |
| `L` | `MoveToScreenBottom` | `3L` = 3rd line from bottom |

### Edge Cases

- **Fewer lines than viewport**: Clamp to available lines
- **Count 0 or 1**: Both treated as first line from top/bottom
- **Count exceeds viewport**: Clamp to last visible line

## Data Flow

```
User presses 'H' key (capital H)
        ↓
NormalMode::handle_key() - returns Action::MoveToScreenTop
        ↓
Window::process_action() - receives Action::MoveToScreenTop
        ↓
Calculate target line:
  - Get scroll_offset.row (first visible buffer line)
  - Get viewport_size.rows (visible rows)
  - target = scroll_offset.row + (count - 1).min(visible_rows - 1)
        ↓
Set cursor to target line with preserved column
        ↓
Update remembered visual column (like other vertical motions)
```

## Key Components

### 1. Action Enum (`editor.rs`)

Add three new action variants:

```rust
/// Move cursor to top of screen (or N lines from top with count)
MoveToScreenTop,
/// Move cursor to middle of screen
MoveToScreenMiddle,
/// Move cursor to bottom of screen (or N lines from bottom with count)
MoveToScreenBottom,
```

Update trait methods:
- `resets_remembered_column()`: Return `false` (these are vertical movements)
- `uses_remembered_column()`: Return `true` (preserve column like j/k/gg/G)
- `is_countable()`: Return `true` for H and L, `false` for M
- `is_line_action()`: Return `false` (not absolute line numbers)
- `with_count()`: Only allow for H and L motions

### 2. NormalMode Keymap (`editor.rs`)

Add key bindings in `NormalMode::new()`:

```rust
// H/M/L screen-relative motions
keymap.insert("H".to_string(), Action::MoveToScreenTop);
keymap.insert("M".to_string(), Action::MoveToScreenMiddle);
keymap.insert("L".to_string(), Action::MoveToScreenBottom);
```

### 3. Window Action Processing (`window.rs`)

Add handler methods:

```rust
/// Move cursor to top of viewport (or N lines from top)
pub fn move_cursor_to_screen_top(&mut self, count: Option<usize>, viewport_rows: usize) {
    let start_line = self.buffer_view.scroll_offset().row as usize;
    let offset = count.unwrap_or(1).saturating_sub(1);
    let target_line = (start_line + offset).min(
        start_line + viewport_rows - 1
    ).min(self.buffer_view.buffer().line_count() - 1);
    
    let target_col = self.buffer_view.get_or_compute_target_col();
    self.buffer_view.set_cursor(Cursor::new(target_line, target_col));
    self.buffer_view.set_remembered_visual_col(target_col);
}

/// Move cursor to middle of viewport
pub fn move_cursor_to_screen_middle(&mut self, viewport_rows: usize) {
    let start_line = self.buffer_view.scroll_offset().row as usize;
    let target_line = start_line + (viewport_rows / 2);
    let target_col = self.buffer_view.get_or_compute_target_col();
    
    self.buffer_view.set_cursor(Cursor::new(target_line, target_col));
    self.buffer_view.set_remembered_visual_col(target_col);
}

/// Move cursor to bottom of viewport (or N lines from bottom)
pub fn move_cursor_to_screen_bottom(&mut self, count: Option<usize>, viewport_rows: usize) {
    let start_line = self.buffer_view.scroll_offset().row as usize;
    let end_line = (start_line + viewport_rows - 1).min(
        self.buffer_view.buffer().line_count() - 1
    );
    let offset = count.unwrap_or(1).saturating_sub(1);
    let target_line = (end_line - offset).max(start_line);
    
    let target_col = self.buffer_view.get_or_compute_target_col();
    self.buffer_view.set_cursor(Cursor::new(target_line, target_col));
    self.buffer_view.set_remembered_visual_col(target_col);
}
```

Update `process_action()` to handle new actions:

```rust
Action::MoveToScreenTop => {
    // Extract count if present, calculate target, move cursor
    ActionResult::Handled
}
Action::MoveToScreenMiddle => {
    // Move to middle, ignore count
    ActionResult::Handled
}
Action::MoveToScreenBottom => {
    // Extract count if present (from bottom), calculate target, move cursor
    ActionResult::Handled
}
Action::Count(count, inner) => {
    // Handle count wrapping for H/L motions
    if matches!(inner.as_ref(), Action::MoveToScreenTop | Action::MoveToScreenBottom) {
        // Execute with count
    }
}
```

## Count Handling

The count prefix is handled by the existing `Action::Count(count, inner)` pattern:

- **H with count**: `3H` → `Count(3, MoveToScreenTop)` → offset = 3-1 = 2 lines from top
- **L with count**: `3L` → `Count(3, MoveToScreenBottom)` → offset = 3-1 = 2 lines from bottom  
- **M**: Ignores count - pressing `3M` should behave like `M` (or be invalid)

The count handling in `Action::with_count()` needs modification to:
- Return `Some(Action::Count(...))` for H and L motions when count > 0
- Return `None` for M motion (not countable)

## Column Preservation

H/M/L are vertical motions, so they:
1. Use `get_or_compute_target_col()` to get target column (preserve or compute)
2. Update `remembered_visual_col` after moving (like MoveUp/MoveDown/gg/G)
3. Do NOT call `resets_remembered_column()` - they use vertical motion behavior

## Viewport Calculation

The viewport information comes from:
- `scroll_offset.row`: First visible buffer line (0-indexed)
- `window.size.rows`: Visible rows in the window

Screen line mapping:
- Screen row 0 = Buffer line `scroll_offset.row`
- Screen row `r` = Buffer line `scroll_offset.row + r`

H target: `scroll_offset.row + (count - 1)`
M target: `scroll_offset.row + (visible_rows / 2)`
L target: `scroll_offset.row + visible_rows - 1 - (count - 1)`

## Error Handling

| Condition | Handling |
|-----------|----------|
| Empty buffer | Stay at line 0 |
| Count = 0 | Treat as 1 |
| Count exceeds viewport | Clamp to last visible line |
| Document shorter than viewport | Clamp to last buffer line |

## Test Strategy

1. **Unit tests** for action classification (`is_countable`, `is_line_action`, etc.)
2. **Integration tests** for key → action mapping in NormalMode
3. **Window tests** for cursor positioning with various viewport sizes
4. **Edge case tests** for count handling and boundary conditions

## Trade-offs

**Decision**: Implement H/M/L as separate actions rather than a single parameterized action

**Reasoning**:
- Clearer separation of concerns
- Easier to handle M's count-ignoring behavior differently
- Matches existing codebase pattern (MoveToFirstLine vs MoveToLastLine are separate)
- Uses capital letters to avoid conflict with lowercase h (MoveLeft) motion

**Impact**:
- Slightly more code (3 actions instead of 1)
- More explicit handling in each branch
