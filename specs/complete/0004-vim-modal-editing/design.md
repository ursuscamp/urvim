# Vim-Style Modal Editing - Technical Design

## Architecture Overview

This design implements vim-style modal editing with two modes (Normal and Insert). The architecture consists of:

1. **Mode trait** - Defines how each mode processes keys and returns actions
2. **KeyAction enum** - Represents actions triggered by keypresses that the main event loop processes
3. **Box<dyn Mode>** - Stored in main loop to handle current mode dynamically
4. **Cursor style integration** - Uses existing Terminal::CursorStyle for block/bar cursor rendering

### Component Flow

```
KeyEvent → Box<dyn Mode> → KeyAction → Main Loop Executes Action
                                  ↓
                        Mode Change → Replace Box → Update Cursor Style
```

## Interface Design

### KeyAction Enum

```rust
/// Actions that the main event loop processes.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
    /// Move cursor left (h key in Normal mode)
    MoveLeft,
    /// Move cursor down (j key in Normal mode)
    MoveDown,
    /// Move cursor up (k key in Normal mode)
    MoveUp,
    /// Move cursor right (l key in Normal mode)
    MoveRight,
    /// Insert a character at cursor position
    InsertChar(char),
    /// Switch to Normal mode
    SwitchToNormal,
    /// Switch to Insert mode
    SwitchToInsert,
    /// Quit the editor
    Quit,
    /// No action (ignored key)
    None,
}
```

### Mode Trait

```rust
/// Trait for mode-specific key handling.
pub trait Mode {
    /// Process a key event and return the corresponding action.
    fn handle_key(&self, key: &Key) -> KeyAction;
    
    /// Get the cursor style for this mode.
    fn cursor_style(&self) -> CursorStyle;
}
```

### Normal Mode Implementation

```rust
/// Normal mode for vim-style navigation and commands.
pub struct NormalMode;

impl NormalMode {
    pub fn new() -> Self;
}

impl Mode for NormalMode {
    fn handle_key(&self, key: &Key) -> KeyAction {
        match (key.code, key.modifiers) {
            // Movement keys (h, j, k, l)
            (KeyCode::Char('h'), Modifiers::NONE) => KeyAction::MoveLeft,
            (KeyCode::Char('j'), Modifiers::NONE) => KeyAction::MoveDown,
            (KeyCode::Char('k'), Modifiers::NONE) => KeyAction::MoveUp,
            (KeyCode::Char('l'), Modifiers::NONE) => KeyAction::MoveRight,
            
            // Mode switching
            (KeyCode::Char('i'), Modifiers::NONE) => KeyAction::SwitchToInsert,
            
            // Quit (Ctrl-q)
            (KeyCode::Char('q'), Modifiers::CTRL) => KeyAction::Quit,
            
            // Pass through arrow keys for convenience
            (KeyCode::Left, _) => KeyAction::MoveLeft,
            (KeyCode::Down, _) => KeyAction::MoveDown,
            (KeyCode::Up, _) => KeyAction::MoveUp,
            (KeyCode::Right, _) => KeyAction::MoveRight,
            
            // Ignore other keys in normal mode
            _ => KeyAction::None,
        }
    }
    
    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBlock
    }
}
```

### Insert Mode Implementation

```rust
/// Insert mode for text input.
pub struct InsertMode;

impl InsertMode {
    pub fn new() -> Self;
}

impl Mode for InsertMode {
    fn handle_key(&self, key: &Key) -> KeyAction {
        match (key.code, key.modifiers) {
            // Mode switching
            (KeyCode::Esc, _) => KeyAction::SwitchToNormal,
            
            // Quit (Ctrl-q)
            (KeyCode::Char('q'), Modifiers::CTRL) => KeyAction::Quit,
            
            // Character insertion
            (KeyCode::Char(c), Modifiers::NONE) => KeyAction::InsertChar(c),
            
            // Enter key inserts newline
            (KeyCode::Enter, _) => KeyAction::InsertChar('\n'),
            
            // Pass through arrow keys for cursor movement while in insert mode
            (KeyCode::Left, _) => KeyAction::MoveLeft,
            (KeyCode::Down, _) => KeyAction::MoveDown,
            (KeyCode::Up, _) => KeyAction::MoveUp,
            (KeyCode::Right, _) => KeyAction::MoveRight,
            
            // Ignore other keys
            _ => KeyAction::None,
        }
    }
    
    fn cursor_style(&self) -> CursorStyle {
        CursorStyle::SteadyBar
    }
}
```

## Main Event Loop Integration

The main.rs will be modified to use the mode system with Box<dyn Mode>:

```rust
// In main.rs
use urvim::editor::{KeyAction, Mode, NormalMode, InsertMode};
use std::boxed::Box;

// Initialize with Normal mode (Box containing the mode)
let mut mode: Box<dyn Mode> = Box::new(NormalMode::new());

// Set initial cursor style
terminal.set_cursor_style(mode.cursor_style())?;

loop {
    // ... render code ...
    
    let event = terminal.read_event()?;
    
    if let Event::Key(key) = event {
        let action = mode.handle_key(&key);
        
        match action {
            KeyAction::MoveLeft => {
                if let Some(cursor) = buffer.cursor_left(cursor) {
                    window.set_cursor(cursor);
                }
            }
            KeyAction::MoveDown => {
                let visual_col = buffer.visual_col_at(cursor);
                if let Some(new_cursor) = buffer.cursor_down(cursor, visual_col) {
                    window.set_cursor(new_cursor);
                }
            }
            KeyAction::MoveUp => {
                let visual_col = buffer.visual_col_at(cursor);
                if let Some(new_cursor) = buffer.cursor_up(cursor, visual_col) {
                    window.set_cursor(new_cursor);
                }
            }
            KeyAction::MoveRight => {
                if let Some(cursor) = buffer.cursor_right(cursor) {
                    window.set_cursor(cursor);
                }
            }
            KeyAction::InsertChar(c) => {
                buffer.insert_char(cursor, c);
                // Update cursor position after insert
                cursor = match c {
                    '\n' => Cursor::new(cursor.line + 1, 0),
                    _ => Cursor::new(cursor.line, cursor.col + c.len_utf8()),
                };
                window.set_cursor(cursor);
            }
            KeyAction::SwitchToNormal => {
                mode = Box::new(NormalMode::new());
                terminal.set_cursor_style(CursorStyle::SteadyBlock)?;
            }
            KeyAction::SwitchToInsert => {
                mode = Box::new(InsertMode::new());
                terminal.set_cursor_style(CursorStyle::SteadyBar)?;
            }
            KeyAction::Quit => break,
            KeyAction::None => { /* Ignore */ }
        }
    }
}
```

## File Structure

New file: `src/editor.rs`

```
src/
├── editor.rs      (NEW: Mode trait, KeyAction, NormalMode, InsertMode)
├── lib.rs         (MODIFIED: add `pub mod editor;`)
└── main.rs        (MODIFIED: integrate mode system with Box<dyn Mode>)
```

## Key Components

### NormalMode

**Responsibilities:**
- Handle key events in Normal mode
- Map h/j/k/l to cursor movement
- Map i to SwitchToInsert
- Map Esc to no-op (already in Normal mode)
- Map Ctrl-q to Quit
- Return block cursor style
- Cloneable via clone_box()

**Public API:**
- `NormalMode::new() -> Self`
- `impl Mode for NormalMode`

### InsertMode

**Responsibilities:**
- Handle key events in Insert mode
- Map printable characters to InsertChar
- Map Enter to InsertChar('\n')
- Map Esc to SwitchToNormal
- Map Ctrl-q to Quit
- Support arrow keys for cursor movement
- Return bar cursor style
- Cloneable via clone_box()

**Public API:**
- `InsertMode::new() -> Self`
- `impl Mode for InsertMode`

## User Interaction

### Mode Switching Flow

1. **Start in Normal mode**: Editor launches with block cursor
2. **Press `i`**: Switches to Insert mode, cursor changes to bar
3. **Type text**: Characters are inserted at cursor position
4. **Press `Esc`**: Returns to Normal mode, cursor changes to block

### Cursor Movement in Normal Mode

- `h` - Move left (character/grapheme)
- `j` - Move down (preserving visual column)
- `k` - Move up (preserving visual column)
- `l` - Move right (character/grapheme)

### Cursor Movement in Insert Mode

- Arrow keys move cursor without inserting
- Cursor stays in Insert mode after movement

### Quit

- Ctrl-q in any mode exits the editor

## External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| terminal::CursorStyle | Cursor rendering | Already exists |
| terminal::Terminal | Terminal I/O | Already exists |
| buffer::Buffer | Text storage | Already exists |

## Error Handling

- **Invalid cursor movement**: Return None from buffer methods, do nothing
- **Invalid key in mode**: Return KeyAction::None (ignore)
- **Mode transition failure**: Replace mode Box regardless of cursor style success

## Trade-offs

**Decision**: Use enum-based action system instead of direct buffer modifications in mode handlers

**Reasoning**:
- Clear separation between key interpretation (mode handlers) and action execution (main loop)
- Easier to test mode handlers in isolation
- More extensible for future commands

**Impact**:
- Slightly more boilerplate in main loop
- Easier to add new modes or commands later

**Decision**: Use Box<dyn Mode> for dynamic dispatch

**Reasoning**:
- Simple way to switch between mode implementations
- Each mode is self-contained
- Trait object allows runtime polymorphism

**Impact**:
- Slight heap allocation on mode switch
- Clean, simple API

**Decision**: Steady cursor styles instead of blinking

**Reasoning**:
- Blinking cursors can be distracting
- Steady cursors are easier to read
- User can still customize via terminal settings

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Terminal doesn't support cursor styles | Low | Medium | Graceful fallback to default cursor |
| Performance overhead of Box dyn dispatch | Low | Low | Only on keypress, negligible overhead |

## Testing Strategy

1. **Unit tests** for NormalMode::handle_key - verify all key mappings
2. **Unit tests** for InsertMode::handle_key - verify all key mappings
3. **Unit tests** for KeyAction variants
4. **Integration test** - manual testing of mode switching and cursor changes
