# Count Action Handler Refactoring

## Summary

Refactor the `Action::Count` handler in the Window module from a large conditional chain into a more maintainable, extensible architecture using method extraction.

## Problem Statement

The current `Action::Count` match arm in `Window::process_action()` has grown into a complex conditional chain (~140 lines) that handles different types of counted actions:

1. Line motions (gg, G) with count → go to absolute line
2. Screen-relative motions (H, L) with count → go to N lines from top/bottom
3. Line actions (0, $, ^, A, I) with count → go to target line then execute
4. Join motions (J, gJ) with count → join N+1 lines
5. DeleteLine (dd) with count → delete N lines
6. ChangeLine (cc) with count → change N lines
7. OpenLineBelow (o) with count → create N lines below
8. OpenLineAbove (O) with count → create N lines above
9. Repeatable motions (j, k, h, l, w, b, e, etc.) → execute N times

This structure is problematic because:
- **Hard to extend**: Adding new counted action types requires modifying the existing match arm
- **Duplicated code**: Similar patterns (viewport calculations, cursor positioning) are repeated
- **Low cohesion**: The handler knows about too many different action types
- **Testing difficulty**: Hard to test individual cases in isolation
- **Hard to understand**: The intent is obscured by the implementation details
- **Already includes 9 action types**: The handler has grown to cover more than originally anticipated

## User Stories

- **As a** developer, **I want** the Count handler to be extensible **so that** I can add new counted action behaviors without modifying existing code.

- **As a** developer, **I want** each counted action type to have its own handler **so that** I can test and debug individual behaviors independently.

- **As a** developer, **I want** the Count handler code to be self-documenting **so that** it's easy to understand how counted actions work.

## Functional Requirements

- [ ] **REQ-001**: Refactor Count handler by extracting each action type's handling into its own private method on Window
- [ ] **REQ-002**: Extract line motion (gg, G) count handling into its own method
- [ ] **REQ-003**: Extract screen-relative motion (H, L) count handling into its own method  
- [ ] **REQ-004**: Extract line action count handling into its own method
- [ ] **REQ-005**: Extract join motion count handling into its own method
- [ ] **REQ-006**: Extract DeleteLine (dd) count handling into its own method
- [ ] **REQ-007**: Extract ChangeLine (cc) count handling into its own method
- [ ] **REQ-008**: Extract OpenLineBelow (o) count handling into its own method
- [ ] **REQ-009**: Extract OpenLineAbove (O) count handling into its own method
- [ ] **REQ-010**: Keep default repeatable action count handling (execute N times)
- [ ] **REQ-011**: All existing behaviors must be preserved (backward compatible)
- [ ] **REQ-012**: New action types should be able to implement their own count behavior without modifying existing code

## Non-Functional Requirements

- **Maintainability**: Code should be self-documenting with clear names
- **Extensibility**: Adding new counted action types should not require modifying existing handler code
- **Testability**: Each count behavior should be testable in isolation
- **Performance**: No significant performance regression (same or better performance)

## Acceptance Criteria

- [ ] **AC-001**: All existing tests pass without modification
- [ ] **AC-002**: `5j`, `10k` work as before (repeatable motions)
- [ ] **AC-003**: `5G`, `5gg` work as before (line motions)
- [ ] **AC-004**: `3H`, `3L` work as before (screen-relative)
- [ ] **AC-005**: `3$`, `3^` work as before (line actions)
- [ ] **AC-006**: `3J`, `3gJ` work as before (join motions)
- [ ] **AC-007**: `3dd` works as before (delete lines)
- [ ] **AC-008**: `3cc` works as before (change lines)
- [ ] **AC-009**: `3o` works as before (open lines below)
- [ ] **AC-010**: `3O` works as before (open lines above)
- [ ] **AC-011**: New counted action types can be added without modifying the Count handler match arm

## Out of Scope

- Adding new action types
- Changing the Action enum structure
- Refactoring other parts of process_action()
- Performance optimization (unless needed)

## Assumptions

- The Window struct has access to buffer_view and size information needed for calculations
- Action enum variants are stable and won't change during this refactoring
- Handler methods will be private methods on Window

## Dependencies

- None - this is an internal refactoring

## Example Design (for reference)

```rust
// Methods on Window - simple extraction
impl Window {
    pub(crate) fn handle_count(&mut self, count: usize, inner: &Action) -> ActionResult {
        // Dispatch to appropriate handler
    }

    fn handle_count_line_motion(&mut self, count: usize, action: &Action) -> ActionResult { ... }
    fn handle_count_screen_motion(&mut self, count: usize, action: &Action) -> ActionResult { ... }
    fn handle_count_line_action(&mut self, count: usize, action: &Action) -> ActionResult { ... }
    fn handle_count_join(&mut self, count: usize, action: &Action) -> ActionResult { ... }
    fn handle_count_delete_line(&mut self, count: usize) -> ActionResult { ... }
    fn handle_count_change_line(&mut self, count: usize) -> ActionResult { ... }
    fn handle_count_open_line_below(&mut self, count: usize) -> ActionResult { ... }
    fn handle_count_open_line_above(&mut self, count: usize) -> ActionResult { ... }
    fn handle_count_repeatable(&mut self, count: usize, action: &Action) -> ActionResult { ... }
}

// In process_action():
Action::Count(count, inner) => {
    self.handle_count(*count, inner)
}
```
