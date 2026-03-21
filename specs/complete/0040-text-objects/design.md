# Text Objects: Inner Word and Around Word - Technical Design

## 1. Architecture Overview

Text objects extend urvim's operator system to support compositional editing. Instead of fixed sequences like `dd`, operators (like `d`) now wait for a motion or text object to define the target region.

### Data Flow

```
Keypress → NormalMode.handle_key()
  → TrieKeymap matches "diw" sequence
  → Returns Action::Operation(Operator::Delete, TextObject::InnerWord)
  → Window.process_action() executes the operation
  → Buffer modified, cursor positioned
```

### Key Architectural Decisions

1. **Compositional Action**: `Action::Operation(Operator, TextObject)` avoids many action variants
2. **Keymap handles operator-pending naturally**: The TrieKeymap already waits for more keys when a sequence has children. Registering `"diw"` and `"daw"` as sequences means `d` alone will wait, and `di` will wait for `w`.
3. **No special state needed**: `pending_operator` is not needed - the keymap itself handles the waiting behavior.

## 2. Interface Design

### New Enums

```rust
/// Operators that wait for a motion or text object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    // Future: Change, Yank, etc.
}

/// Text objects that define a selection region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObject {
    InnerWord,
    AroundWord,
}
```

### Modified Action Enum

```rust
pub enum Action {
    // ... existing variants ...
    
    /// Compositional operation: apply Operator to TextObject
    /// Examples: Operation(Delete, InnerWord) = "diw"
    Operation(Operator, TextObject),
}
```

### NormalMode State

No changes needed to NormalMode structure. The existing `waiting` state and keymap behavior handle operator-pending naturally.

When `d` is pressed:
1. Keymap recognizes `d` has children (dd, diw, daw)
2. Returns `WaitForMore`, buffer = `["d"]`
3. When `i` is pressed, buffer = `["d", "i"]`, keymap sees `di` has children (`diw`)
4. When `w` is pressed, buffer = `["d", "i", "w"]`, keymap matches `Action::Operation(Delete, InnerWord)`

### Buffer Methods for Text Objects

**Responsibilities:**
- Compute text object boundaries from cursor position
- Delete text within a range

**Public API:**
```rust
impl Buffer {
    /// Get the inner word range from cursor position.
    /// - If cursor is inside a word: returns that word
    /// - If cursor is inside whitespace: returns the whitespace region
    pub fn get_inner_word_range(&self, cursor: Cursor) -> Option<TextObjectRange>;
    
    /// Get the around word range from cursor position.
    /// - If cursor is inside a word: returns word + all trailing whitespace
    /// - If cursor is inside whitespace: returns whitespace + trailing word
    pub fn get_around_word_range(&self, cursor: Cursor) -> Option<TextObjectRange>;
    
    /// Delete text in the given range.
    /// Returns new cursor position after deletion.
    pub fn delete_range(&mut self, range: TextObjectRange) -> Option<Cursor>;
}
```

### Window Operation Handler

**Responsibilities:**
- Execute `Action::Operation(Operator, TextObject)`
- Apply operator to text object's selected range

**Algorithm:**
```
handle_operation(op, text_obj):
    cursor = buffer_view.cursor()
    range = buffer.get_text_object_range(cursor, text_obj)
    if range is None:
        return  // No word at cursor position
    
    match op:
        Operator::Delete:
            // Save snapshot for undo
            buffer.save_snapshot()
            new_cursor = buffer.delete_range(range)
            buffer_view.set_cursor(new_cursor)
        // Future: Operator::Change, Operator::Yank, etc.
```

## 5. User Interaction

### Key Sequence Flow

The keymap handles operator-pending naturally:

```
NORMAL MODE:
  Press 'd'
    → TrieKeymap looks up "d": has children (dd, diw, daw)
    → Returns WaitForMore, buffer = ["d"]
  
  Press 'i'
    → TrieKeymap looks up "di": has children (diw)
    → Returns WaitForMore, buffer = ["d", "i"]
  
  Press 'w'
    → TrieKeymap looks up "diw": exact match
    → Returns Complete(Action::Operation(Delete, InnerWord))
```

### Escape During Key Sequence

```
NORMAL MODE:
  Press 'd' → WaitForMore
  Press <Esc>
    → Buffer cleared, waiting = false
    → Returns InvalidSequence
    → No operation performed
```

### Count Handling

The existing `CountParser` already handles counts correctly:

| Input | action_keys | count | Result |
|-------|-------------|-------|--------|
| `3diw` | `["d", "i", "w"]` | 3 | `Count(3, Operation(Delete, InnerWord))` |
| `d3iw` | `["d", "i", "w"]` | 3 | `Count(3, Operation(Delete, InnerWord))` |
| `3d3iw` | `["d", "i", "w"]` | 9 | `Count(9, Operation(Delete, InnerWord))` |

The `Count` wrapper around `Operation` is handled in `Window::handle_count()`:
```rust
Action::Count(count, Box::new(Action::Operation(op, obj))) => {
    for _ in 0..count {
        self.execute_operation(op, obj)?;
    }
}
```

## 6. External Dependencies

No new external dependencies. Uses existing:
- `unicode-segmentation` for grapheme-aware text operations
- Existing `Boundary` enum and word detection logic in `buffer.rs`

## 7. Error Handling

| Scenario | Behavior |
|----------|----------|
| Cursor on empty buffer | `diw`/`daw` does nothing (no word to select) |
| Cursor on line with no word | `diw`/`daw` does nothing |
| Escape during operator-pending | Cancel operation, return to normal mode |
| Invalid sequence (e.g., `dxyz`) | Return `InvalidSequence`, no operation |

## 8. Security

Not applicable - text editor local operation only.

## 9. Configuration

No new configuration options.

## 10. Component Interactions

```
┌─────────────────────────────────────────────────────────────┐
│ NormalMode                                                 │
│  ┌─────────────────────────────────────────────────────┐  │
│  │ TrieKeymap (ChainedKeymap)                           │  │
│  │   - "dd" → DeleteLine                                │  │
│  │   - "diw" → Operation(Delete, InnerWord)              │  │
│  │   - "daw" → Operation(Delete, AroundWord)             │  │
│  │   - "d" alone → has children, so WaitForMore         │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼ Action::Operation(Op, Obj)
┌─────────────────────────────────────────────────────────────┐
│ Window.process_action()                                     │
│  ┌─────────────────────────────────────────────────────┐  │
│  │ Action::Operation(op, obj) → handle_operation()       │  │
│  │   → Buffer.get_*_word_range(cursor, obj)             │  │
│  │   → Buffer.delete_range(range)                       │  │
│  │   → set_cursor(new_pos)                              │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## 11. Platform Considerations

No platform-specific considerations. Uses existing:
- Unicode-aware text operations (already handles cross-platform)
- No filesystem operations
- No platform-specific APIs

## 12. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking existing `di` binding (delete inner...?) | Low | Medium | Currently `di` is invalid; we're not breaking existing behavior |
| Count with text object not working correctly | Low | Medium | Add unit tests for `Count(3, Operation(Delete, InnerWord))` execution |
| Text object range calculation edge cases | Medium | Medium | Extensive testing with whitespace-only lines, empty lines, etc. |

## 13. File Changes Summary

| File | Change |
|------|--------|
| `src/editor.rs` | Add `Operator`, `TextObject` enums; add `Action::Operation`; register "diw" and "daw" sequences in TrieKeymap |
| `src/buffer.rs` | Add `TextObjectRange`, `get_inner_word_range()`, `get_around_word_range()`, `delete_range()` |
| `src/window.rs` | Handle `Action::Operation` in `process_action()` and `handle_count()` |
| `src/editor/tests.rs` | Add tests for text object key sequences |

## 14. Test Plan

### Unit Tests

1. **TrieKeymap text object sequence tests**:
   - `"d", "i", "w"` → `Complete(Action::Operation(Delete, InnerWord))`
   - `"d", "a", "w"` → `Complete(Action::Operation(Delete, AroundWord))`
   - `"d"` alone → `WaitForMore` (d has children)
   - `"d", "i"` alone → `WaitForMore` (di has children)

2. **Buffer text object range tests**:
   - `"hello world"` at 'h' → inner word: (0, 0) to (0, 5)
   - `"hello world"` at 'h' → around word: (0, 0) to (0, 6) (includes trailing space)
   - `"  hello world"` at ' ' → inner word: (0, 0) to (0, 2)
   - `"hello world  "` at 'd' → around word: (0, 8) to (0, 11) (all trailing spaces)

3. **Count with Operation tests**:
   - `3diw` → `Count(3, Operation(Delete, InnerWord))`
   - `d3iw` → `Count(3, Operation(Delete, InnerWord))`
   - `3d3iw` → `Count(9, Operation(Delete, InnerWord))`
