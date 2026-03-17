# Column Preservation for Vertical Movement

## Summary

Add column preservation behavior when moving vertically in the editor. When the user moves up or down, the cursor attempts to maintain its visual horizontal position (column) from the previous line, clamping to the end of shorter lines. The remembered column resets when any horizontal movement occurs.

## Problem Statement

Currently, when moving vertically (up/down) in urvim, the cursor moves to the same absolute column position. This differs from vim's behavior where the cursor "remembers" its desired column and attempts to preserve it across line changes. Users familiar with vim expect column preservation and find the current behavior jarring when working with lines of varying lengths.

## User Stories

- **As a** vim user, **I want** vertical movement to preserve my visual column position, **so that** I can quickly navigate through lines of varying lengths without manually repositioning the cursor.

- **As a** user editing code with varying indentation, **I want** my cursor to stay at the same visual column when moving between lines, **so that** I can quickly scan through structured code.

- **As a** user who accidentally moved horizontally, **I want** the remembered column to reset, **so that** I can naturally adjust my position without fighting the preservation logic.

## Functional Requirements

- [ ] **REQ-001**: When moving vertically (up/down), the cursor should attempt to maintain the same visual column from the previous line
- [ ] **REQ-002**: If the target line is shorter than the remembered column, clamp the cursor to the end of that line (last valid column)
- [ ] **REQ-003**: The remembered column should persist across multiple consecutive vertical movements
- [ ] **REQ-004**: Any horizontal movement (left/right, word movements, etc.) should reset the remembered column to the current position
- [ ] **REQ-005**: After resetting, the next vertical move should remember the new horizontal position
- [ ] **REQ-006**: Column preservation should work with all vertical movement commands (line up, line down, etc.)

## Non-Functional Requirements

- **Performance**: Column calculation should be O(1) - simply check line length and clamp if needed
- **Compatibility**: Behavior should match vim's 'virtualedit' mode disabled (the default)

## Acceptance Criteria

- [ ] **AC-001**: Moving down from a long line to a shorter line places cursor at end of shorter line
- [ ] **AC-002**: Moving up from a long line to a shorter line places cursor at end of shorter line
- [ ] **AC-003**: Moving down from a short line to a longer line places cursor at the remembered column
- [ ] **AC-004**: Moving vertically multiple times preserves the original column on lines long enough
- [ ] **AC-005**: Pressing left/right arrow resets the remembered column
- [ ] **AC-006**: Word motions (w, b, e) reset the remembered column
- [ ] **AC-007**: After reset, next vertical move remembers the new column position

## Out of Scope

- Tab handling/visual column vs byte column differences (treat as single column per character for now)
- Blockwise visual mode column preservation
- Integration with 'virtualedit' feature

## Assumptions

- Cursor position is stored as a simple column index (0-based)
- Line lengths can be queried efficiently
- The editor has a notion of "current position" that can track the remembered column separately

## Dependencies

- None - this is a self-contained feature in the cursor/movement handling
