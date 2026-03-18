# H/M/L Motions

## Summary

Implement H, M, and L key motions that move the cursor to the top, middle, and bottom lines of the currently visible screen respectively, without scrolling. These are line motions that preserve the column position when possible.

## Problem Statement

Users need efficient navigation to specific positions within the visible viewport. Currently, urvim lacks quick motions to jump to the top, middle, or bottom of the screen. In Vim, these are fundamental navigation keys that provide quick access to screen-relative positions without scrolling the viewport.

## User Stories

- **As a** user, **I want** to press `H` to jump to the first visible line of the screen, **so that** I can quickly access the top of the current viewport.
- **As a** user, **I want** to press `M` to jump to the middle line of the screen, **so that** I can quickly center my view on the document.
- **As a** user, **I want** to press `L` to jump to the last visible line of the screen, **so that** I can quickly access the bottom of the current viewport.
- **As a** user, **I want** these motions to preserve my column position when possible, **so that** I don't lose my horizontal position when navigating vertically.

## Functional Requirements

- [ ] **REQ-001**: Implement `H` motion that moves cursor to the first visible line (top line) of the screen
- [ ] **REQ-002**: Implement `M` motion that moves cursor to the middle visible line of the screen
- [ ] **REQ-003**: Implement `L` motion that moves cursor to the last visible line (bottom line) of the screen
- [ ] **REQ-004**: These motions do NOT scroll the viewport - cursor moves but view stays fixed
- [ ] **REQ-005**: These are line motions - they move to entire lines, not characters
- [ ] **REQ-006**: Column position is preserved if the target line is long enough; otherwise cursor moves to end of line
- [ ] **REQ-007**: H motion with count N moves cursor to N lines from top of screen (e.g., 3H = 3rd line from top)
- [ ] **REQ-008**: L motion with count N moves cursor to N lines from bottom of screen (e.g., 3L = 3rd line from bottom)
- [ ] **REQ-009**: M motion ignores count prefixes (pressing a number before M has no effect)
- [ ] **REQ-010**: Handle edge cases when document has fewer lines than screen height

## Non-Functional Requirements

- **Performance**: Motions should be instant (O(1) operation)
- **Compatibility**: Behavior should match Vim's H/M/L as closely as reasonable for a terminal editor

## Acceptance Criteria

- [ ] **AC-001**: Pressing `H` in normal mode moves cursor to the first visible line
- [ ] **AC-002**: Pressing `M` in normal mode moves cursor to the middle visible line
- [ ] **AC-003**: Pressing `L` in normal mode moves cursor to the last visible line
- [ ] **AC-004**: The viewport does not scroll when these motions are used
- [ ] **AC-005**: Column position is preserved when moving with H/M/L
- [ ] **AC-006**: H with count N moves to Nth line from top (e.g., 3H goes to line 3 from top)
- [ ] **AC-007**: L with count N moves to Nth line from bottom (e.g., 3L goes to line 3 from bottom)
- [ ] **AC-008**: M ignores count prefixes (count is treated as 0 or ignored)
- [ ] **AC-009**: When document has fewer lines than screen, motions clamp to available lines

## Out of Scope

- Visual mode H/M/L (not implementing visual-specific behavior)
- Count prefix with M key (M ignores counts, only H and L support counts)
- Scroll binding (these are cursor motions only, not scroll motions)

## Assumptions

- Screen line calculations account for wrapped lines (each physical screen row counts)
- The editor has access to current viewport information (window dimensions)
- Normal mode is the primary mode for these motions

## Dependencies

- None - this is a self-contained motion implementation

## Related Features

- Line motions (gg, G) - spec 0020
- Column preservation - spec 0018
- Mode change motions - spec 0021
