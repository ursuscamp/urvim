# Repeat Character Search Motions - Technical Design

## 2. Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         main.rs                                  │
│                    (event loop: mode.handle_key)                │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                    NormalMode (editor.rs)                        │
│  - keymap: TrieKeymap + CharScanKeymap                          │
│  - buffer: Vec<String> (key sequence)                           │
│  - handle_key(): returns HandleKeyResult::Complete(Action)     │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Action enum (editor.rs)                       │
│  - FindForward(char), FindBackward(char)                        │
│  - TillForward(char), TillBackward(char)                       │
│  - RepeatLastFind, RepeatLastFindReverse (NEW)                  │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Window (window.rs)                             │
│  - process_action(&Action) -> ActionResult                     │
│  - move_cursor_to_char_forward/backward                         │
│  - move_cursor_till_forward/backward                            │
└─────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   globals.rs (NEW)                               │
│  - LAST_FIND: Mutex<Option<FindState>>                         │
│  - get_last_find(), set_last_find()                            │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow Summary

1. User presses `f x` → `NormalMode` → `CharScanKeymap` → returns `Action::FindForward('x')`
2. `Window::process_action(FindForward('x'))`:
   - Calls `move_cursor_to_char_forward('x', 1)` to move cursor
   - Calls `set_last_find(FindState { target: 'x', kind: Find, direction: Forward })`
3. User presses `;` → `NormalMode` → `TrieKeymap` → returns `Action::RepeatLastFind`
4. `Window::process_action(RepeatLastFind)`:
   - Calls `get_last_find()` → returns `FindState { target: 'x', kind: Find, direction: Forward }`
   - Since direction is Forward, calls `move_cursor_to_char_forward('x', 1)`
5. User presses `,` → `NormalMode` → `TrieKeymap` → returns `Action::RepeatLastFindReverse`
6. `Window::process_action(RepeatLastFindReverse)`:
   - Calls `get_last_find()` → returns `FindState { target: 'x', kind: Find, direction: Forward }`
   - Since we need opposite direction, calls `move_cursor_to_char_backward('x', 1)`

### Key Architectural Decisions

- **Global static storage**: `FindState` stored in `src/globals.rs` as a `static` with `Mutex` to allow mutation. This ensures persistence across mode switches and future multi-window support.
- **Window handles state updates**: `Window::process_action` updates `LAST_FIND` after executing any `Find*` or `Till*` action. This avoids coupling `NormalMode` with global state.
- **`;` and `,` do not update state**: These actions only read from `LAST_FIND`, never write to it. This matches Vim behavior where `;` and `,` are pure repeat operations.

## 3. Interface Design

### New Module: `src/globals.rs`

```rust
use std::sync::Mutex;

/// Direction of character search
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// Kind of character search motion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindKind {
    /// f or F - lands ON the character
    Find,
    /// t or T - lands BEFORE/AFTER the character
    Till,
}

/// State of the last character search motion
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindState {
    pub target_char: char,
    pub kind: FindKind,
    pub direction: Direction,
}

/// Global storage for the last character search state
static LAST_FIND: Mutex<Option<FindState>> = Mutex::new(None);

/// Set the last character search state
pub fn set_last_find(state: FindState) {
    let mut last = LAST_FIND.lock().unwrap();
    *last = Some(state);
}

/// Get the last character search state
pub fn get_last_find() -> Option<FindState> {
    let last = LAST_FIND.lock().unwrap();
    last.clone()
}
```

**Important**: The repeat actions (`RepeatLastFind`, `RepeatLastFindReverse`) call the motion methods directly WITHOUT going through `process_action()`, so `LAST_FIND` is NOT updated during repeat operations. This ensures `;` and `,` are pure read-only repeat operations.

## 4. Data Models

### New Types

| Type | Variants/Fields | Description |
|------|-----------------|-------------|
| `Direction` | `Forward`, `Backward` | Search direction |
| `FindKind` | `Find`, `Till` | Motion type (land on vs land before/after) |
| `FindState` | `target_char: char`, `kind: FindKind`, `direction: Direction` | Captures a character search motion |

### Modified Types

| Type | Change | Description |
|------|--------|-------------|
| `Action` | Add `RepeatLastFind`, `RepeatLastFindReverse` | New action variants |
| `Action::resets_remembered_column()` | Add `RepeatLastFind`, `RepeatLastFindReverse` | These are horizontal movements |
| `Action::is_countable()` | Add `RepeatLastFind`, `RepeatLastFindReverse` | Count prefixes work with these |

## 5. Key Components

### `src/globals.rs` (NEW)

**Responsibilities:**
- Store and retrieve the last character search state
- Provide thread-safe access via `Mutex`

**Public API:**
- `set_last_find(state: FindState) -> ()`
- `get_last_find() -> Option<FindState>`

**Dependencies:** None (uses only `std::sync::Mutex`)

### `src/editor.rs`

**Changes:**
- Add `RepeatLastFind` and `RepeatLastFindReverse` to `Action` enum
- Add these actions to `resets_remembered_column()` and `is_countable()`
- Add keybindings in `NormalMode::new()` for `;` and `,`
- Add `pub use globals::{FindState, FindKind, Direction}` for re-export

**Dependencies:** `globals` module

### `src/window.rs`

**Changes:**
- In `process_action()`, update global state when executing `FindForward`, `FindBackward`, `TillForward`, `TillBackward`
- Handle `RepeatLastFind` and `RepeatLastFindReverse` actions

**Dependencies:** `globals` module

### `src/terminal/keys.rs`

**Changes:**
- Remove `';' => Some(':')` and `',' => Some('<')` from the shift mapping

## 6. User Interaction

### Key Sequences

| Input | Action | Description |
|-------|--------|-------------|
| `f x` | `FindForward('x')` | Find next 'x' forward |
| `F x` | `FindBackward('x')` | Find previous 'x' backward |
| `t x` | `TillForward('x')` | Till (before) next 'x' forward |
| `T x` | `TillBackward('x')` | Till (after) previous 'x' backward |
| `;` | `RepeatLastFind` | Repeat last search same direction |
| `,` | `RepeatLastFindReverse` | Repeat last search opposite direction |

### Interaction Flows

**Flow 1: Basic repeat forward**
```
User: f x
  → Cursor moves to next 'x'
  → LAST_FIND = { target: 'x', kind: Find, direction: Forward }

User: ;
  → Cursor moves to next 'x' after current position
```

**Flow 2: Repeat reverse**
```
User: F x
  → Cursor moves to previous 'x'
  → LAST_FIND = { target: 'x', kind: Find, direction: Backward }

User: ,
  → Cursor moves to next 'x' forward (opposite of stored direction)
```

**Flow 3: Count prefix with repeat**
```
User: 3 f x
  → Cursor moves to 3rd 'x' forward
  → LAST_FIND = { target: 'x', kind: Find, direction: Forward }

User: 2 ;
  → Cursor moves to 2nd 'x' forward from current position
```

**Flow 4: Mode switch (state persists)**
```
User: f x
  → Cursor moves, LAST_FIND updated

User: i
  → Switch to InsertMode (NormalMode dropped, state preserved in globals)

User: [type text]

User: <Esc>
  → Switch back to NormalMode (fresh instance, but globals still has state)

User: ;
  → Still works - repeats 'x' search from globals
```

## 7. External Dependencies

| Dependency | Purpose | Version/Notes |
|------------|---------|---------------|
| `std::sync::Mutex` | Thread-safe interior mutability for global state | Standard library |
| `std::sync::OnceLock` | Not needed - simple Mutex is sufficient | N/A |

No new external dependencies required.

## 8. Error Handling

| Scenario | Behavior |
|----------|----------|
| `;` or `,` with no previous search | Silent fail - cursor stays, no error message |
| `get_last_find()` returns `None` | Same as above - no movement |
| Mutex poisoned (panic) | Program terminates (acceptable for single-threaded app) |

No new error types introduced. The existing `ActionResult` system handles action outcomes.

## 9. Security

Not applicable - terminal-based text editor with no network access, user input validation, or security boundaries.

## 10. Configuration

No new configuration options. All behavior is implicit from user input.

## 11. Component Interactions

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  NormalMode  │────▶│    Action    │────▶│    Window    │
│  (editor.rs) │     │  (editor.rs) │     │ (window.rs)  │
└──────────────┘     └──────────────┘     └──────────────┘
                                                │
                                                ▼
                                         ┌──────────────┐
                                         │   globals    │
                                         │ (globals.rs) │
                                         └──────────────┘
```

**Synchronous call flow for `f x`:**
1. `NormalMode::handle_key()` returns `Action::FindForward('x')`
2. `Window::process_action(FindForward('x'))`:
   - Calls `globals::set_last_find(FindState { target: 'x', Find, Forward })`
   - Calls `move_cursor_to_char_forward('x', 1)` to move cursor

**Synchronous call flow for `;`:**
1. `NormalMode::handle_key()` returns `Action::RepeatLastFind`
2. `Window::process_action(RepeatLastFind)`:
   - Calls `globals::get_last_find()` → `Some(FindState { target: 'x', Find, Forward })`
   - Since direction is Forward, calls `move_cursor_to_char_forward('x', 1)`
   - **Does NOT update LAST_FIND** (repeat is read-only)

**Synchronous call flow for `,`:**
1. `NormalMode::handle_key()` returns `Action::RepeatLastFindReverse`
2. `Window::process_action(RepeatLastFindReverse)`:
   - Calls `globals::get_last_find()` → `Some(FindState { target: 'x', Find, Forward })`
   - Since direction is Forward, calls `move_cursor_to_char_backward('x', 1)` (opposite)
   - **Does NOT update LAST_FIND** (repeat is read-only)

## 12. Platform Considerations

Not applicable - pure Rust implementation with no platform-specific code.

## 13. Trade-offs

**Decision**: Use `Mutex<Option<FindState>>` instead of `OnceLock<FindState>`

**Reasoning**:
- `Option` allows representing "no previous search" (None)
- `Mutex` is safe for single-threaded event loop with possible future multi-threading
- Simple and familiar pattern

**Impact**:
- Slight overhead from lock acquisition (negligible for single keypress)
- Could use `parking_lot::Mutex` for better performance, but adds dependency

## 14. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Global state causes subtle bugs with multiple windows | Low | Medium | Design ensures `;`/, read from globals but don't write; each window's searches update the same global |
| `Mutex` contention if adding threading later | Low | Low | `std::sync::Mutex` is standard, easily replaceable |
| State persists incorrectly after file close | Low | Low | Global state is editor-level, not file-level - intentional |

## 15. Testing Strategy

### Unit Tests

**`globals.rs`:**
- `set_last_find()` then `get_last_find()` returns `Some(state)`
- `get_last_find()` on empty state returns `None`

**`window.rs` (existing test file):**
- Add tests for `RepeatLastFind` and `RepeatLastFindReverse` actions
- Test that `FindForward` updates global state
- Test that `;` with no previous search doesn't move cursor
- Test count prefix with `3 RepeatLastFind`
- Test that `RepeatLastFind` does NOT update LAST_FIND
- Test that `RepeatLastFindReverse` does NOT update LAST_FIND

**`editor.rs` (existing test file):**
- Test that `Action::RepeatLastFind` is countable
- Test that keymap returns correct action for `;` and `,`

### Integration Tests

**`CharScanKeymap` + `Repeat`:**
1. Press `f x` → verify cursor moved and state set
2. Press `;` → verify cursor moved again in same direction
3. Press `,` → verify cursor moved opposite direction
4. Press `;` again → should still move in original direction (state was NOT updated by repeat)
5. Switch to insert mode, switch back, press `;` → verify still works (global state persisted)

## 16. File Changes Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `src/globals.rs` | **NEW** | Global `LAST_FIND` static and accessors |
| `src/editor.rs` | MODIFY | Add `RepeatLastFind`, `RepeatLastFindReverse` actions; add keybindings; update `resets_remembered_column()` and `is_countable()` |
| `src/window.rs` | MODIFY | Handle new actions; update global state after `Find*`/`Till*` |
| `src/terminal/keys.rs` | MODIFY | Remove `';' => Some(':')` and `',' => Some('<')` |
| `src/lib.rs` | MODIFY | Add `pub mod globals;` |
| `docs/motions.md` | MODIFY | Document `;` and `,` motions |
