# Undo/Redo - Technical Design

## 2. Architecture Overview

The undo/redo system uses a **snapshot-based approach** where each buffer maintains its own undo history. Since `imbl::Vector` clones in O(1), storing full text snapshots is cheap.

**Key Principles:**
1. **One snapshot per edit session**: A snapshot is taken when entering or exiting Insert mode, not per character
2. **Text deduplication**: If a snapshot's text matches the current active snapshot's text, only the cursor is updated
3. **Cursor tracking**: Cursor movements call `track_cursor()` to record position in the active snapshot so undo/redo can restore cursor

**Data Flow:**
```
User Input → NormalMode::handle_key() → Action
    ↓ (if Action::is_snapshottable())
push_snapshot(lines, cursor) → process action
    ↓
Window::process_action() → UndoState API
```

**Key Components:**
- `Snapshot`: Stores text lines AND cursor position for one point in time
- `UndoState`: Manages snapshot history via encapsulated API
- `Buffer::undo_state`: New field storing optional `UndoState>`
- `Action::is_snapshottable()`: Determines if action should trigger snapshot

## 3. Interface Design

### UndoState API (Package-private)

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| `new()` | none | `Self` | Creates empty undo state |
| `push_snapshot()` | `lines: Vector<Arc<str>>, cursor: Cursor` | `()` | Saves state; deduplicates if text matches active snapshot |
| `update_cursor()` | `cursor: Cursor` | `()` | Updates cursor in active snapshot for undo/redo |
| `undo()` | none | `Option<(Vector<Arc<str>>, Cursor)>` | Restores previous state |
| `redo()` | none | `Option<(Vector<Arc<str>>, Cursor)>` | Restores next state |
| `can_undo()` | none | `bool` | True if undo available |
| `can_redo()` | none | `bool` | True if redo available |
| `clear()` | none | `()` | Clears all history |

### Action Enum

| Variant | Parameters | Description |
|---------|------------|-------------|
| `Undo` | none | Restore previous buffer state |
| `Redo` | none | Restore next buffer state |

### Action Methods

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| `is_snapshottable()` | none | `bool` | True if action should trigger snapshot |
| `updates_snapshot_cursor()` | none | `bool` | True if action should update cursor in active snapshot |

## 4. Data Models

### Snapshot Struct

```rust
/// A single snapshot of buffer state (text + cursor).
#[derive(Debug, Clone)]
struct Snapshot {
    /// The text content at this point in time.
    lines: Vector<Arc<str>>,
    /// The cursor position at this point in time.
    cursor: Cursor,
}
```

### UndoState Struct

```rust
/// Stores undo/redo history for a buffer.
#[derive(Debug, Clone)]
struct UndoState {
    /// History of snapshots, oldest first.
    history: Vector<Snapshot>,
    /// Current position in history.
    /// - position == 0: no snapshots yet (or at oldest)
    /// - position > 0: "active snapshot" is at position - 1
    /// - position == history.len(): at "current" state (no redo available)
    /// 
    /// The "active snapshot" (history.get(position - 1)) represents the state
    /// we are currently at. After undo, position decreases; after redo, increases.
    position: usize,
}
```

**Invariants:**
- `0 <= position <= history.len()`
- A snapshot represents state BEFORE a change was made
- Cursor-only changes (movement) update the active snapshot's cursor

### Buffer Struct Changes

```rust
pub struct Buffer {
    lines: Vector<Arc<str>>,
    path: Option<AbsolutePath>,
    undo_state: Option<UndoState>,  // NEW: undo/redo support
}
```

## 5. Key Components

### UndoState::push_snapshot()

**Responsibilities:**
- If text equals the current active snapshot's text, only update cursor (deduplication)
- Otherwise, truncate redo history and push new snapshot
- Cursor position is stored with the snapshot (state BEFORE the change)

**Important:** The "current active snapshot" is `history.get(position - 1)` if `position > 0`, NOT `history.last()`. After undo operations, `history.last()` may be in the redo portion.

**Algorithm:**
```
fn push_snapshot(&mut self, lines: Vector<Arc<str>>, cursor: Cursor) {
    // Check if text matches current active snapshot (deduplication)
    // position - 1 is the snapshot representing our current state
    if self.position > 0 {
        let active_idx = self.position - 1;
        if let Some(active) = self.history.get(active_idx) {
            if active.lines == lines {
                // Text unchanged, just update cursor of active snapshot
                self.history.set(active_idx, Snapshot { lines, cursor });
                return;
            }
        }
    }
    
    // Truncate redo history (anything after current position)
    while self.history.len() > self.position {
        self.history.pop_back();
    }
    
    // Push new snapshot
    self.history.push_back(Snapshot { lines, cursor });
    self.position = self.history.len();
}
```

### UndoState::update_cursor()

**Responsibilities:**
- Update the cursor in the current active snapshot (without changing text)
- Only works if there is an active snapshot (position > 0)
- Used by cursor movement actions so undo/redo can restore cursor position

**Algorithm:**
```
fn update_cursor(&mut self, cursor: Cursor) {
    // Can only update if we have an active snapshot
    if self.position == 0 {
        return;  // No active snapshot
    }
    
    let active_idx = self.position - 1;
    if let Some(active) = self.history.get_mut(active_idx) {
        active.cursor = cursor;
    }
}
```

### UndoState::undo()

**Responsibilities:**
- Return None if no undo available
- Decrement position
- Return the snapshot at new position

**Algorithm:**
```
fn undo(&mut self) -> Option<(Vector<Arc<str>>, Cursor)> {
    if self.position == 0 {
        return None;  // Nothing to undo
    }
    
    self.position -= 1;
    let snapshot = self.history.get(self.position).clone();
    Some((snapshot.lines, snapshot.cursor))
}
```

### UndoState::redo()

**Responsibilities:**
- Return None if no redo available
- Return snapshot at current position
- Increment position

**Algorithm:**
```
fn redo(&mut self) -> Option<(Vector<Arc<str>>, Cursor)> {
    if self.position >= self.history.len() {
        return None;  // Nothing to redo
    }
    
    let snapshot = self.history.get(self.position).clone();
    self.position += 1;
    Some((snapshot.lines, snapshot.cursor))
}
```

### Action::is_snapshottable()

Determines which actions should trigger a new snapshot (i.e., start a new edit session).

```rust
impl Action {
    pub fn is_snapshottable(&self) -> bool {
        match self {
            // Mode switches - snapshot to capture all insert mode changes
            Action::SwitchToInsert => true,
            Action::SwitchToNormal => true,
            
            // Text-modifying actions in normal mode - snapshot
            Action::DeleteBackward
            | Action::DeleteForward
            | Action::DeleteLine
            | Action::ChangeLine
            | Action::ChangeToLineEnd
            | Action::JoinWithSpace
            | Action::JoinWithoutSpace
            | Action::AppendAfterCursor
            | Action::AppendToLineEnd
            | Action::InsertAtLineStart
            | Action::OpenLineBelow
            | Action::OpenLineAbove => true,
            
            // InsertChar - NO snapshot (handled by SwitchToNormal)
            Action::InsertChar(_) => false,
            
            // Undo/Redo - no snapshot (they ARE the undo/redo)
            Action::Undo | Action::Redo => false,
            
            // Count wraps the inner action
            Action::Count(_, inner) => inner.is_snapshottable(),
            
            // Everything else (movement, Quit, etc.) - no snapshot
            _ => false,
        }
    }
}
```

### Action::updates_snapshot_cursor()

Determines which actions should update the cursor in the active snapshot (without creating a new snapshot).

```rust
impl Action {
    pub fn updates_snapshot_cursor(&self) -> bool {
        match self {
            // All movement actions update cursor in active snapshot
            Action::MoveLeft
            | Action::MoveDown
            | Action::MoveUp
            | Action::MoveRight
            | Action::ForwardTo(_)
            | Action::BackTo(_)
            | Action::MoveToLineEnd
            | Action::MoveToLineStart
            | Action::MoveToLineContentStart
            | Action::MoveToFirstLine
            | Action::MoveToLastLine
            | Action::MoveToScreenTop
            | Action::MoveToScreenMiddle
            | Action::MoveToScreenBottom
            | Action::MoveToMatchingBracket
            | Action::MoveToPreviousParagraph
            | Action::MoveToNextParagraph
            | Action::FindForward(_)
            | Action::FindBackward(_)
            | Action::TillForward(_)
            | Action::TillBackward(_)
            | Action::RepeatLastFind
            | Action::RepeatLastFindReverse => true,
            
            Action::Count(_, inner) => inner.updates_snapshot_cursor(),
            
            _ => false,
        }
    }
}
```

## 6. User Interaction

### Key Bindings (Normal Mode)

| Key | Action | is_snapshottable | updates_snapshot_cursor |
|-----|--------|------------------|------------------------|
| `u` | Undo | No | No |
| `U` | Redo | No | No |
| `i/a/A/I` | Mode change | Yes | No |
| `x/d/c/J/etc` | Delete/Change | Yes | No |
| `h/j/k/l` | Movement | No | Yes |
| `w/b/e/etc` | Word motion | No | Yes |
| Arrow keys | Movement | No | Yes |

### Interaction Flows

**Insert Session Undo:**
```
1. User presses 'i' to enter Insert mode
   → push_snapshot(empty buffer, cursor at 0)
   
2. User types "hello world" (InsertChar actions)
   → No snapshots taken (InsertChar is_snapshottable() == false)
   → Active snapshot cursor updated by cursor movements
   
3. User presses Esc to return to Normal mode
   → push_snapshot("hello world", cursor at end)
   
4. User presses 'u' (Undo)
   → Buffer restored to empty, cursor at 0
```

**Cursor Preservation with Redo:**
```
1. User presses 'i' to enter Insert mode
2. User types "hello", moves cursor to position 2
3. User presses Esc
   → Active snapshot: text="hello", cursor=2
   
4. User presses 'U' (Redo)
   → Text="hello" restored, cursor=2
```

**Text Deduplication:**
```
1. User types 'i', types "hello", presses Esc
   → Active snapshot: "hello", cursor at end
   
2. User moves cursor around with h/l
   → Active snapshot's cursor updated
   
3. User presses 'i' again (enters insert mode)
   → Snapshot text "hello" matches active snapshot
   → Only cursor updated, no new snapshot pushed
   
4. User types "x", presses Esc
   → Now texts differ, new snapshot pushed: "hellox"
```

## 7. External Dependencies

| Dependency | Purpose | Version/Notes |
|------------|---------|---------------|
| `imbl::Vector` | Text storage, cheap cloning | Already in use |
| `std::sync::Arc` | Line storage | Already in use |

No new external dependencies required.

## 8. Error Handling

| Scenario | Behavior |
|----------|----------|
| `undo()` with nothing to undo | Returns `None` |
| `redo()` with nothing to redo | Returns `None` |
| `push_snapshot()` with text matching active snapshot | Updates cursor only |
| `update_cursor()` with no active snapshot (position == 0) | No-op |

## 9. Security

Not applicable - this is a local text editor with no network access or authentication.

## 10. Configuration

No new configuration options required. Undo/redo is always enabled.

## 11. Component Interactions

**Enter Insert Mode Flow:**
```
Terminal → NormalMode::handle_key('i')
    ↓
Action::SwitchToInsert.is_snapshottable() == true
    ↓
Buffer::push_snapshot(current lines, cursor)
    ↓
Window::process_action(Action::SwitchToInsert)
```

**Character Typing Flow:**
```
Terminal → InsertMode::handle_key('a')
    ↓
Action::InsertChar('a').is_snapshottable() == false
    ↓
Action::InsertChar('a').updates_snapshot_cursor() == false
    ↓
Window::process_action(Action::InsertChar('a'))
    (buffer modified, no snapshot)
```

**Exit Insert Mode Flow:**
```
Terminal → InsertMode::handle_key(Esc)
    ↓
Action::SwitchToNormal.is_snapshottable() == true
    ↓
Buffer::push_snapshot(current lines, cursor)
    ↓
Window::process_action(Action::SwitchToNormal)
```

**Cursor Movement Flow:**
```
Terminal → NormalMode::handle_key('l')
    ↓
Action::MoveRight.updates_snapshot_cursor() == true
    ↓
Buffer::update_cursor(new_cursor)
    (active snapshot's cursor updated)
```

## 12. Platform Considerations

Not applicable - pure Rust with no platform-specific code.

## 13. Trade-offs

**Decision**: Snapshot on mode switch, not per character

**Reasoning:**
- User thinks in "edit sessions" (time in insert mode), not individual keystrokes
- More natural undo behavior - undo reverts the whole typed phrase
- Reduces snapshot count significantly

**Decision**: Cursor deduplication in snapshots

**Reasoning:**
- Text is the primary thing to restore; cursor is secondary tracking
- If user types then moves cursor, undo should restore both text AND final cursor position
- Simpler than tracking cursor changes separately

## 14. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Cursor gets "stuck" on old snapshot | Low | Low | track_cursor only works when position > 0 |
| Large insert session = large undo | Low | Low | O(1) clone makes this acceptable |
