# Count Action Handler Refactoring

## Summary

Refactor the `Action::Count` handler in the Window module from a large conditional chain into a more maintainable, extensible architecture using a trait-based approach or strategy pattern.

## Problem Statement

The current `Action::Count` match arm in `Window::process_action()` has grown into a complex conditional chain (~90 lines) that handles different types of counted actions:

1. Line motions (gg, G) with count → go to absolute line
2. Screen-relative motions (H, L) with count → go to N lines from top/bottom
3. Line actions (0, $, ^, A, I) with count → go to target line then execute
4. Join motions (J, gJ) with count → join N+1 lines
5. Repeatable motions (j, k, h, l, w, b, e, etc.) → execute N times

This structure is problematic because:
- **Hard to extend**: Adding new counted action types requires modifying the existing match arm
- **Duplicated code**: Similar patterns (viewport calculations, cursor positioning) are repeated
- **Low cohesion**: The handler knows about too many different action types
- **Testing difficulty**: Hard to test individual cases in isolation
- **Hard to understand**: The intent is obscured by the implementation details

## User Stories

- **As a** developer, **I want** the Count handler to be extensible **so that** I can add new counted action behaviors without modifying existing code.

- **As a** developer, **I want** each counted action type to have its own handler **so that** I can test and debug individual behaviors independently.

- **As a** developer, **I want** the Count handler code to be self-documenting **so that** it's easy to understand how counted actions work.

## Functional Requirements

- [ ] **REQ-001**: Refactor Count handler to use a trait-based approach where each action type can define its own count behavior
- [ ] **REQ-002**: Extract line motion (gg, G) count handling into its own method
- [ ] **REQ-003**: Extract screen-relative motion (H, L) count handling into its own method  
- [ ] **REQ-004**: Extract line action count handling into its own method
- [ ] **REQ-005**: Extract join motion count handling into its own method
- [ ] **REQ-006**: Keep default repeatable action count handling (execute N times)
- [ ] **REQ-007**: All existing behaviors must be preserved (backward compatible)
- [ ] **REQ-008**: New action types should be able to implement their own count behavior without modifying existing code

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
- [ ] **AC-007**: New counted action types can be added without modifying the Count handler match arm

## Out of Scope

- Adding new action types
- Changing the Action enum structure
- Refactoring other parts of process_action()
- Performance optimization (unless needed)

## Assumptions

- The Window struct has access to buffer_view and size information needed for calculations
- Action enum variants are stable and won't change during this refactoring
- The trait-based approach will be implemented using a trait on Action or a separate handler struct

## Dependencies

- None - this is an internal refactoring

## Example Design (for reference)

```rust
// Approach 1: Trait on Action
trait CountHandler {
    fn handle_count(&self, count: usize, window: &mut Window) -> ActionResult;
}

// Approach 2: Separate handler struct
struct CountActionHandler;
impl CountActionHandler {
    fn handle(count: usize, inner: &Action, window: &mut Window) -> ActionResult { ... }
}

// Either approach would allow:
Action::Count(count, inner) => {
    CountActionHandler::handle(*count, inner, self)
}
```
