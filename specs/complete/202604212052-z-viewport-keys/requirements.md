# Z Viewport Keys

## Summary
Add Vim-style normal-mode `z` viewport positioning commands `zt`, `zz`, and `zb` so users can reposition the visible window around the current cursor line without changing cursor location.

## Problem Statement
Users currently cannot quickly align the current cursor line to the top, center, or bottom of the viewport using familiar Vim navigation commands. This makes it harder to inspect surrounding context while keeping editing focus on the same cursor position.

## User Stories
- As a Vim user, I want `zt` to place the cursor line at the top of the viewport, so that I can inspect lines below my current location.
- As a Vim user, I want `zz` to place the cursor line at the center of the viewport, so that I can view balanced context above and below.
- As a Vim user, I want `zb` to place the cursor line at the bottom of the viewport, so that I can inspect lines above my current location.

## Functional Requirements
- [ ] **REQ-001**: The editor shall recognize `zt` in normal mode and reposition the viewport so the cursor line is rendered at the top visible row when possible.
- [ ] **REQ-002**: The editor shall recognize `zz` in normal mode and reposition the viewport so the cursor line is rendered at the center visible row when possible.
- [ ] **REQ-003**: The editor shall recognize `zb` in normal mode and reposition the viewport so the cursor line is rendered at the bottom visible row when possible.
- [ ] **REQ-004**: Executing `zt`, `zz`, or `zb` shall not modify the cursor buffer position (line and column).
- [ ] **REQ-005**: The commands `zt`, `zz`, and `zb` shall execute without count semantics in this iteration; numeric prefixes must not alter their behavior.
- [ ] **REQ-006**: When exact top/center/bottom placement is impossible due to buffer length or cursor proximity to file boundaries, viewport placement shall clamp to the nearest valid scroll position.
- [ ] **REQ-007**: The commands shall apply to the focused window only and shall not change non-focused windows.

## Non-Functional Requirements
- **Usability**: Command behavior should align with common Vim expectations for `zt`, `zz`, and `zb` in normal mode.
- **Reliability**: Viewport repositioning must be deterministic and stable across repeated command execution.
- **Compatibility**: Existing normal-mode key behavior not involving these `z` sequences must remain unchanged.

## Acceptance Criteria
- [ ] **AC-001**: Given a buffer taller than the viewport and cursor away from boundaries, pressing `zt` places the cursor line at the top row while cursor line/column values remain unchanged.
- [ ] **AC-002**: Given a buffer taller than the viewport and cursor away from boundaries, pressing `zz` places the cursor line at the viewport center row while cursor line/column values remain unchanged.
- [ ] **AC-003**: Given a buffer taller than the viewport and cursor away from boundaries, pressing `zb` places the cursor line at the bottom row while cursor line/column values remain unchanged.
- [ ] **AC-004**: Given cursor near the start or end of buffer where exact placement is impossible, `zt`, `zz`, and `zb` clamp to nearest valid viewport offset without cursor movement.
- [ ] **AC-005**: Given a numeric prefix before these commands, behavior is identical to invoking the command without a count.
- [ ] **AC-006**: Existing non-`z` normal-mode motions and commands continue to function as before.

## Out of Scope
- Vim `z` commands other than `zt`, `zz`, and `zb`.
- Supporting count-based variants for `zt`, `zz`, and `zb`.
- Changes to cursor motion semantics, jumplist behavior, or mark behavior.
- Documentation updates beyond this spec stage.

## Assumptions
- The editor has a single focused window context for normal-mode command execution.
- Viewport position is represented by a scroll origin or equivalent window offset that can be updated independently of cursor position.

## Dependencies
- Existing normal-mode multi-key command sequencing for `z` prefixed commands.
- Existing window viewport/scroll offset infrastructure used by render and motion flows.
- Existing test harness support for asserting cursor position and viewport positioning.
