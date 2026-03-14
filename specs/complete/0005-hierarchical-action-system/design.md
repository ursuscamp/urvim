# Hierarchical Action System - Technical Design

## Architecture Overview

The hierarchical action system introduces a widget-based architecture for processing user input. Instead of handling all actions directly in the main event loop, actions flow through a hierarchy:

```
┌─────────────────────────────────────────────────────────┐
│                     Main Event Loop                      │
│  1. Get KeyEvent from terminal                          │
│  2. Mode handler converts to Action                  │
│  3. Window.process_action(action) → ActionResult        │
│     - Handled: done, continue to render                │
│     - NotHandled: App-level handling                   │
└─────────────────────────────────────────────────────────┘
```

### Key Changes from Current Design

| Aspect | Current | New |
|--------|---------|-----|
| Action handling location | main.rs match statement | Widget trait + app fallback |
| Action result tracking | Implicit (methods return ()) | Explicit (ActionResult enum) |
| Extensibility | None (hardcoded in main) | Widget trait for new widgets |
| Flow | Linear: action → direct handling | Hierarchical: widget first, then app |

## Interface Design

### ActionResult Enum

```rust
/// Result of a widget processing an action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActionResult {
    /// The widget handled the action
    Handled,
    /// The widget did not handle the action
    NotHandled,
}
```

### Widget Trait

```rust
use crate::editor::Action;

/// Trait for widgets that can process actions.
///
/// Widgets are UI components (window, status bar, etc.) that can
/// handle user actions. The main event loop passes actions to widgets
/// first, and if no widget handles them, processes them at the app level.
pub trait Widget {
    /// Process an action and return whether it was handled.
    ///
    /// # Arguments
    /// * `action` - The action to process
    ///
    /// # Returns
    /// * `ActionResult::Handled` - Widget handled the action
    /// * `ActionResult::NotHandled` - Widget did not handle the action
    fn process_action(&mut self, action: &Action) -> ActionResult;
}
```

## Data Models

### Action (Existing)

No changes required - continues to represent all possible actions:

```rust
pub enum Action {
    MoveLeft,
    MoveDown,
    MoveUp,
    MoveRight,
    InsertChar(char),
    SwitchToNormal,
    SwitchToInsert,
    Quit,
    None,
}
```

### New Components

| Component | Location | Description |
|-----------|----------|-------------|
| `ActionResult` | `src/action.rs` | Enum indicating if action was handled |
| `Widget` trait | `src/widget.rs` | Trait for widget action processing |
| `Window` impl | Update `src/window.rs` | Implement Widget for Window |

## Key Components

### ActionResult (new file: src/action.rs)

**Responsibilities:**
- Represent the result of action processing
- Provide clear Handled/NotHandled semantics

**Public API:**
- `ActionResult::Handled` - Constant variant
- `ActionResult::NotHandled` - Constant variant

**Dependencies:** None

### Widget Trait (new file: src/widget.rs)

**Responsibilities:**
- Define interface for widgets that process actions
- Enable hierarchical action processing

**Public API:**
- `process_action(&mut self, action: &Action) -> ActionResult`

**Dependencies:**
- `Action` from editor module

### Window Widget (update: src/window.rs)

**Responsibilities:**
- Handle buffer-related actions (movement, insertion)
- Return Handled for actions it processes
- Return NotHandled for actions it doesn't process

**Implementation:**

```rust
impl Widget for Window {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        match action {
            Action::MoveLeft => {
                self.move_cursor_left();
                ActionResult::Handled
            }
            Action::MoveDown => {
                self.move_cursor_down();
                ActionResult::Handled
            }
            Action::MoveUp => {
                self.move_cursor_up();
                ActionResult::Handled
            }
            Action::MoveRight => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            Action::InsertChar(c) => {
                self.insert_char(*c);
                ActionResult::Handled
            }
            // All other actions are not handled by window
            _ => ActionResult::NotHandled,
        }
    }
}
```

### Main Event Loop (update: src/main.rs)

**Responsibilities:**
- Coordinate action flow between mode handler, window, and app

**Updated flow:**

```rust
if let Event::Key(key) = event {
    let action = mode.handle_key(&key);

    // First, try to process action through the widget (window)
    if window.process_action(&action) == ActionResult::NotHandled {
        // Fall back to app-level handling
        match action {
            Action::SwitchToNormal => {
                mode = Box::new(NormalMode::new());
                terminal.set_cursor_style(mode.cursor_style())?;
            }
            Action::SwitchToInsert => {
                mode = Box::new(InsertMode::new());
                terminal.set_cursor_style(mode.cursor_style())?;
            }
            Action::Quit => break,
            Action::None => { /* Ignore */ }
            _ => { /* Should have been handled by window */ }
        }
    }
}
```

## Component Interactions

```
Terminal.read_event()
         ↓
      KeyEvent
         ↓
   Mode.handle_key()
         ↓
      Action
         ↓
Window.process_action()
    ├── Handled ──────────→ Render loop
    │
    └── NotHandled
         ↓
   App-level match
    ├── SwitchToNormal → Update mode
    ├── SwitchToInsert → Update mode  
    ├── Quit → Break loop
    └── None → Ignore
```

## External Dependencies

| Dependency | Purpose | Version/Notes |
|------------|---------|---------------|
| None | This is an internal architecture change | - |

## Error Handling

| Scenario | Handling |
|----------|----------|
| Unknown action reaches app | Log debug warning, ignore (shouldn't happen) |
| Window returns Handled for mode switch | Mode switch handled at both levels (window returns NotHandled) |
| No widget handles action | App-level handling as fallback |

## Security

Not applicable - this is an internal input handling architecture change with no security implications.

## Configuration

No new configuration options required.

## Trade-offs

**Decision**: Use enum for action result instead of Option or bool

**Reasoning**:
- More explicit and self-documenting (`Handled`/`NotHandled` vs `Some`/`None` or `true`/`false`)
- Easier to extend later (add `PartialHandled` for bubbling, etc.)
- Matches Rust idioms for result-like semantics

**Decision**: Process all actions through window first, even mode switches

**Reasoning**:
- Simpler architecture - single flow
- Window can always return NotHandled for actions it doesn't process
- Future widgets might want to intercept mode switches

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Accidental double-handling | Low | Medium | Window explicitly returns NotHandled for mode/quit actions |
| Performance regression | Low | Low | Action processing is O(1) match, negligible overhead |
| Missing action handling | Low | Medium | Compiler warnings for non-exhaustive matches |

## Testing Strategy

1. **Unit tests for Window::process_action()**
   - Test each action returns correct ActionResult
   - Test cursor movement after MoveLeft/Right/Up/Down
   - Test character insertion after InsertChar

2. **Integration test for main loop**
   - Verify action flows correctly through hierarchy
   - Verify mode switching still works
   - Verify quit still works

3. **Manual testing**
   - All existing keybindings work as before
   - Cursor movement in both modes
   - Character insertion in insert mode
   - Mode switching (Escape, 'i')
   - Quit (Ctrl-q)
