# Count Action Handler Refactoring - Technical Design

## 1. Architecture Overview

This design refactors the `Action::Count` handler in `Window::process_action()` from a large conditional chain (~140 lines) into a modular architecture using method extraction.

### Current State
- Single match arm in `Window::process_action()` with 9+ conditional branches
- Hard to test individual cases
- Adding new counted action types requires modifying the match arm

### Target State
- Centralized dispatcher that routes to specialized handlers
- Each counted action type has its own dedicated method
- Easy to extend without modifying existing code

### Key Architectural Decision

**Approach: Methods on Window**

We extract handler methods directly onto `Window` rather than creating a separate handler struct or trait. This:
- Keeps related code together
- No new types to manage
- Easy access to `buffer_view` and `size` via `self`
- Each handler is a private method with clear responsibility

## 2. Interface Design

### Main Handler

```rust
// New methods on Window
impl Window {
    /// Main entry point - dispatches to appropriate handler based on inner action
    pub(crate) fn handle_count(&mut self, count: usize, inner: &Action) -> ActionResult {
        // Dispatch logic
    }

    /// Handles line motions (gg, G) - go to absolute line
    fn handle_count_line_motion(&mut self, count: usize, action: &Action) -> ActionResult { ... }

    /// Handles screen-relative motions (H, L) - N lines from top/bottom
    fn handle_count_screen_motion(&mut self, count: usize, action: &Action) -> ActionResult { ... }

    /// Handles line actions (0, $, ^, A, I) - go to target line then execute
    fn handle_count_line_action(&mut self, count: usize, action: &Action) -> ActionResult { ... }

    /// Handles join motions (J, gJ) - join N+1 lines
    fn handle_count_join(&mut self, count: usize, action: &Action) -> ActionResult { ... }

    /// Handles DeleteLine (dd) - delete N lines
    fn handle_count_delete_line(&mut self, count: usize) -> ActionResult { ... }

    /// Handles ChangeLine (cc) - change N lines
    fn handle_count_change_line(&mut self, count: usize) -> ActionResult { ... }

    /// Handles OpenLineBelow (o) - create N lines below
    fn handle_count_open_line_below(&mut self, count: usize) -> ActionResult { ... }

    /// Handles OpenLineAbove (O) - create N lines above
    fn handle_count_open_line_above(&mut self, count: usize) -> ActionResult { ... }

    /// Default: execute repeatable action N times
    fn handle_count_repeatable(&mut self, count: usize, action: &Action) -> ActionResult { ... }
}
```

## 3. Data Models

No new data models required. This is a refactoring that reorganizes existing code.

## 4. Key Components

### 4.1 Count Handler Methods (on Window)

**Responsibilities:**
- Central dispatcher for all counted actions
- Route to appropriate handler method based on inner action type

**Public API:**
- `handle_count(count, inner) -> ActionResult`

**Algorithm:**
```
handle_count(count, inner):
    if inner is MoveToFirstLine or MoveToLastLine:
        return handle_count_line_motion(count, inner)
    if inner is MoveToScreenTop or MoveToScreenBottom:
        return handle_count_screen_motion(count, inner)
    if inner is line action (via is_line_action()):
        return handle_count_line_action(count, inner)
    if inner is JoinWithSpace or JoinWithoutSpace:
        return handle_count_join(count, inner)
    if inner is DeleteLine:
        return handle_count_delete_line(count)
    if inner is ChangeLine:
        return handle_count_change_line(count)
    if inner is OpenLineBelow:
        return handle_count_open_line_below(count)
    if inner is OpenLineAbove:
        return handle_count_open_line_above(count)
    return handle_count_repeatable(count, inner)
```

**Dependencies:**
- `self.buffer_view` (mutable)
- `self.size`

### 4.2 Individual Handler Methods

Each handler method encapsulates one type of counted action:

| Method | Lines of Code (est.) | Responsibility |
|--------|----------------------|----------------|
| `handle_count_line_motion` | ~15 | Go to absolute line (gg, G) |
| `handle_count_screen_motion` | ~20 | N lines from viewport top/bottom (H, L) |
| `handle_count_line_action` | ~10 | Go to line then execute (0, $, ^, A, I) |
| `handle_count_join` | ~15 | Join N+1 lines (J, gJ) |
| `handle_count_delete_line` | ~10 | Delete N lines (dd) |
| `handle_count_change_line` | ~10 | Change N lines (cc) |
| `handle_count_open_line_below` | ~10 | Create N lines below (o) |
| `handle_count_open_line_above` | ~10 | Create N lines above (O) |
| `handle_count_repeatable` | ~5 | Execute N times |

**Total estimated: ~105 lines** (down from ~140 in single match arm, with better organization)

## 5. User Interaction

Not applicable - this is an internal refactoring with no user-facing changes.

## 6. External Dependencies

None - this is an internal refactoring with no new dependencies.

## 7. Error Handling

All handler methods maintain existing error handling behavior:
- Empty buffer: return `ActionResult::Handled` early
- Cursor out of bounds: clamp to valid range
- All methods return `ActionResult::Handled` on success

## 8. Security

Not applicable - this is an internal refactoring with no security implications.

## 9. Configuration

Not applicable - no new configuration options.

## 10. Component Interactions

```
Window::process_action()
    │
    └──► Window::handle_count()
              │
              ├──► handle_count_line_motion() ──► self.buffer_view
              ├──► handle_count_screen_motion() ──► self.buffer_view, self.size
              ├──► handle_count_line_action() ──► self.process_action() (recursive)
              ├──► handle_count_join() ──► self.buffer_view.buffer
              ├──► handle_count_delete_line() ──► self.buffer_view.buffer
              ├──► handle_count_change_line() ──► self.buffer_view.buffer
              ├──► handle_count_open_line_below() ──► self.buffer_view.buffer
              ├──► handle_count_open_line_above() ──► self.buffer_view.buffer
              └──► handle_count_repeatable() ──► self.process_action() (loop)
```

## 11. Platform Considerations

Not applicable - this is a pure Rust refactoring with no platform-specific code.

## 12. Trade-offs

### Decision: Methods on Window over Separate Handler Struct

**Reasoning:**
- Keeps related code together in one place
- Direct access to `self.buffer_view` and `self.size` without passing as parameters
- Simpler - no new types to manage
- Minimal abstraction overhead

**Impact:**
- `Window` gains more methods (but all are private, internal)
- Could consider extracting to a helper struct later if Window grows too large

### Decision: Central Dispatcher Pattern

**Reasoning:**
- Single point of modification for adding new counted action types
- Follows Open/Closed Principle: open for extension, closed for modification
- Makes it easy to see all counted action types at a glance

**Impact:**
- Adding new counted action types requires adding a new condition in the dispatcher
- However, this is a one-time cost per new action type

## 13. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Regression - existing behaviors broken | Low | High | Comprehensive test suite, manual testing of all acceptance criteria |
| Performance regression | Low | Medium | Current implementation is simple dispatch; no added overhead |
| Missing edge cases | Low | Medium | Code review, test each handler method independently |

## 14. Testing Strategy

### Unit Tests for Each Handler Method

Each `handle_*` method should have unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_motion_count_to_first_line() {
        // Test: 5gg goes to line 4 (0-indexed)
    }

    #[test]
    fn test_line_motion_count_to_last_line_beyond_bounds() {
        // Test: 100G in 10-line file goes to line 9
    }

    #[test]
    fn test_screen_motion_count_h() {
        // Test: 3H goes to 3rd line from viewport top
    }

    #[test]
    fn test_join_with_count() {
        // Test: 2J joins 3 lines
    }

    // ... etc for each handler
}
```

### Integration Tests

Existing tests in `editor.rs` should pass without modification:
- `test_count_motion_gg`
- `test_count_motion_G`
- `test_count_motion_H`
- `test_count_motion_L`
- `test_count_motion_dollar`
- `test_count_motion_caret`
- And any new tests for dd, cc, o, O with counts
