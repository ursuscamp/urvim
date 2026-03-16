# Text Delete Keys - Technical Design

## Architecture Overview

This feature adds text deletion capabilities using backspace and delete keys in both insert and normal modes. The implementation follows the existing action-based architecture where:

1. **Keybindings** in `editor.rs` map keys to `Action` variants
2. **Buffer methods** in `buffer.rs` handle the actual text manipulation
3. **Window** in `window.rs` processes actions and coordinates between buffer and cursor

The design leverages existing infrastructure:
- `Buffer::remove()` for removing text ranges
- `Buffer::cursor_left()`/`cursor_right()` for grapheme-aware navigation
- Unicode grapheme handling already exists via `unicode_segmentation`

## Interface Design

### New Action Variants

| Action | Input | Output | Description |
|--------|-------|--------|-------------|
| DeleteBackward | - | - | Delete grapheme before cursor (backspace) |
| DeleteForward | - | - | Delete grapheme at cursor (delete key) |

### New Buffer Methods

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| delete_char_before_cursor(cursor) | Cursor | Option<Cursor> | Removes grapheme before cursor, returns new cursor position (or None if at boundary) |
| delete_char_at_cursor(cursor) | Cursor | Option<Cursor> | Removes grapheme at cursor, returns cursor (stays in place or joins lines) |

### New Window Methods

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| delete_char_before_cursor() | - | - | Handles DeleteBackward action |
| delete_char_at_cursor() | - | - | Handles DeleteForward action |

## Data Models

### Action Enum (editor.rs)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    // ... existing variants ...
    
    /// Delete character before cursor (backspace)
    DeleteBackward,
    /// Delete character at cursor (delete key)
    DeleteForward,
}
```

### Cursor (buffer.rs)

Existing `Cursor` struct is used:
```rust
pub struct Cursor {
    pub line: usize,
    pub col: usize,  // byte position
}
```

## Key Components

### 1. Buffer Deletion Methods (buffer.rs)

#### `delete_char_before_cursor(cursor: Cursor) -> Option<Cursor>`

**Responsibilities:**
- Remove grapheme cluster before the cursor
- Join with previous line if at start of line
- Return new cursor position (after deletion)

**Algorithm:**
1. If cursor is at start of line (col == 0):
   - If not at first line, join current line with previous line
   - Remove newline between lines, cursor moves to end of previous line
   - Return new cursor position
2. Otherwise, find grapheme cluster before cursor
3. Remove that grapheme using `Buffer::remove()`
4. Return cursor position at start of deleted grapheme

**Edge Cases:**
- At document start (line 0, col 0): return None (no-op)
- Empty line: treat as start of line, join with previous

#### `delete_char_at_cursor(cursor: Cursor) -> Option<Cursor>`

**Responsibilities:**
- Remove grapheme cluster at the cursor position
- Join with next line if at end of line
- Return cursor position (stays or adjusts for line join)

**Algorithm:**
1. If cursor is at end of line (col == line_len):
   - If not at last line, join current line with next line
   - Remove newline, cursor stays at end of merged content
   - Return cursor position
2. Otherwise, find grapheme cluster at cursor
3. Remove that grapheme using `Buffer::remove()`
4. Return cursor position (same as input, grapheme was removed)

**Edge Cases:**
- At document end (last line, col == line_len): return None (no-op)
- Empty line: treat as end of line, join with next

### 2. Keybindings (editor.rs)

#### Normal Mode Keybindings

```rust
// In NormalMode::new()
keymap.insert("x".to_string(), Action::DeleteForward);
keymap.insert("X".to_string(), Action::DeleteBackward);
```

#### Insert Mode Keybindings

```rust
// In InsertMode::new()
keymap.insert("<Backspace>".to_string(), Action::DeleteBackward);
keymap.insert("<Delete>".to_string(), Action::DeleteForward);
```

### 3. Window Action Processing (window.rs)

```rust
impl Widget for Window {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        match action {
            // ... existing cases ...
            Action::DeleteBackward => {
                self.delete_char_before_cursor();
                ActionResult::Handled
            }
            Action::DeleteForward => {
                self.delete_char_at_cursor();
                ActionResult::Handled
            }
        }
    }
}
```

## User Interaction

### Insert Mode

**Backspace:**
- Press `<Backspace>` → removes character before cursor
- At line start: joins with previous line
- At document start: does nothing

**Delete:**
- Press `<Delete>` → removes character at cursor
- At line end: joins with next line
- At document end: does nothing

### Normal Mode

**x:**
- Press `x` → removes character at cursor (same as insert mode Delete)
- Cursor stays in place

**X:**
- Press `X` → removes character before cursor (same as insert mode Backspace)
- Cursor moves back

## External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| unicode_segmentation | Grapheme iteration | Already used in buffer.rs |
| Buffer::remove() | Text removal | Already exists |
| Buffer::cursor_left() | Grapheme navigation | Already exists |
| Buffer::cursor_right() | Grapheme navigation | Already exists |

## Error Handling

| Condition | Behavior |
|-----------|----------|
| Backspace at doc start | No operation, return None |
| Delete at doc end | No operation, return None |
| Empty buffer | No operation |
| Invalid cursor position | Should not occur (valid cursor passed) |

## Implementation Tasks

### buffer.rs additions:

1. Add `delete_char_before_cursor(cursor: Cursor) -> Option<Cursor>` method
2. Add `delete_char_at_cursor(cursor: Cursor) -> Option<Cursor>` method

### editor.rs additions:

1. Add `DeleteBackward` and `DeleteForward` variants to `Action` enum
2. Add keybindings in `NormalMode::new()`: x → DeleteForward, X → DeleteBackward
3. Add keybindings in `InsertMode::new()`: \<Backspace> → DeleteBackward, \<Delete> → DeleteForward

### window.rs additions:

1. Add `delete_char_before_cursor()` method
2. Add `delete_char_at_cursor()` method
3. Add action handling in `process_action()` for new action variants

## Trade-offs

**Decision:** x/X in normal mode behave identically to insert mode Delete/Backspace

**Reasoning:**
- Consistent user experience between modes
- Simpler mental model: "delete does the same thing regardless of mode"
- Slightly different from vim (where x is linewise in some cases), but more intuitive

**Impact:**
- May confuse vim purists expecting exact vim behavior
- Easier to learn for new users

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Grapheme boundary bugs | Low | Medium | Test with complex Unicode (emoji, combining chars) |
| Line join edge cases | Low | High | Test at document boundaries, empty lines |
| Cursor position invalid after delete | Low | High | Use is_valid_cursor to verify, clamp if needed |
