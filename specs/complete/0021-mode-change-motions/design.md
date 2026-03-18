# Mode-Change Motions: a, A, I - Technical Design

## Architecture Overview

This feature adds three new mode-change motions in normal mode. The implementation follows the existing action-based architecture.

## Interface Design

### New Action Variants

| Action | Description |
|--------|-------------|
| `AppendAfterCursor` | Move cursor right one position, switch to insert mode |
| `AppendToLineEnd` | Move to end of current line, switch to insert mode |
| `InsertAtLineStart` | Move to first non-whitespace of current line, switch to insert mode |

### Key Bindings

| Key | Action | Count Support |
|-----|--------|---------------|
| `a` | `AppendAfterCursor` | No |
| `A` | `AppendToLineEnd` | Yes - line action |
| `I` | `InsertAtLineStart` | Yes - line action |

## Action Handling

### Decision: Handle mode switch in main.rs after Window processes the motion

Window::process_action() executes the cursor motion and returns Handled. Then main.rs checks if the action was a mode-change motion and switches to insert mode.

Implementation in main.rs:
```rust
if window.process_action(&action) == ActionResult::Handled {
    match action {
        Action::AppendAfterCursor | Action::AppendToLineEnd | Action::InsertAtLineStart => {
            mode = Box::new(InsertMode::new());
            terminal.set_cursor_style(mode.cursor_style())?;
        }
        _ => {}
    }
}
```

## Implementation Steps

1. Add `AppendAfterCursor`, `AppendToLineEnd`, `InsertAtLineStart` to `Action` enum
2. Update `Action::resets_remembered_column()` to include new actions
3. Update `Action::is_line_action()` to include `AppendToLineEnd` and `InsertAtLineStart`
4. Add key bindings in `NormalMode::new()`
5. Handle new actions in `Window::process_action()` - execute cursor motion, return Handled
6. Update `main.rs` to switch to insert mode after these actions
7. Add unit tests for action properties
8. Update documentation (docs/motions.md)
