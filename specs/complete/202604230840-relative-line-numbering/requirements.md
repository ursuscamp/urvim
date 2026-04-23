# Relative Line Numbering and Active Gutter Highlight

## Summary
Add an optional relative line numbering mode for the editor gutter. When enabled, the line on the cursor remains absolute while surrounding visible lines show their distance from the cursor line. Also add a themeable active-line gutter highlight so the current line can be emphasized across the full gutter row, with built-in themes updated to style it appropriately.

## Problem Statement
urvim currently shows absolute gutter line numbers only. That makes nearby-line navigation less scannable than it could be, especially in larger files or split views. The current active-line emphasis also stops at the buffer area, so the gutter does not visually reinforce the cursor line in the same way as the main editor surface.

## User Stories
- As a user, I want to turn on relative line numbering, so that I can more quickly judge how far nearby lines are from the cursor.
- As a user, I want the cursor line to keep its absolute number, so that I can still identify the exact buffer line at a glance.
- As a user, I want relative line numbers to work in every editor mode, so that the gutter stays useful while I edit and navigate.
- As a user, I want the active line to be highlighted across the full gutter row, so that the cursor location is easier to track visually.
- As a theme author, I want the active gutter row to have its own style, so that I can tune it independently from the normal gutter and the main active-line style.

## Functional Requirements
- [ ] **REQ-001**: The editor must provide a user-facing config option that enables or disables relative line numbering.
- [ ] **REQ-002**: Relative line numbering must be disabled by default.
- [ ] **REQ-003**: When relative line numbering is enabled, the cursor line in the gutter must show its absolute buffer line number.
- [ ] **REQ-004**: When relative line numbering is enabled, other visible gutter rows must show their absolute distance from the cursor line as a positive integer.
- [ ] **REQ-005**: Relative line numbering must be computed from the current window cursor position, not from a global buffer position.
- [ ] **REQ-006**: Relative line numbering must apply in normal mode, insert mode, and visual mode.
- [ ] **REQ-007**: Relative line numbering must preserve the existing gutter layout and wrapping behavior when line numbers are rendered.
- [ ] **REQ-008**: The active line gutter highlight must fill the entire gutter row for the cursor line.
- [ ] **REQ-009**: The active line gutter highlight must use a dedicated theme style that is distinct from the base gutter style.
- [ ] **REQ-010**: The active line gutter highlight must follow the same focus and mode gating as the existing active-line emphasis behavior.
- [ ] **REQ-011**: Built-in themes must define an appropriate active line gutter style for each theme.
- [ ] **REQ-012**: The active line gutter style in built-in themes must remain readable against the base gutter style and preserve the theme's overall visual tone.

## Non-Functional Requirements
- **Compatibility**: Disabling relative line numbering must preserve the current absolute-number gutter behavior.
- **Usability**: Relative numbers and the active gutter highlight must improve scanability without making the gutter harder to read.
- **Maintainability**: The active gutter row should be represented as a first-class theme style so future theme changes remain localized.

## Acceptance Criteria
- [ ] **AC-001**: With relative line numbering disabled, the gutter renders the same absolute line numbers it renders today.
- [ ] **AC-002**: With relative line numbering enabled, the cursor line keeps its absolute number and the surrounding visible lines show relative distances from that line.
- [ ] **AC-003**: With relative line numbering enabled, the gutter uses the current window cursor line as the reference point.
- [ ] **AC-004**: With relative line numbering enabled, the gutter behavior is the same in normal, insert, and visual modes.
- [ ] **AC-005**: The active line gutter highlight fills the full gutter width on the current line when active-line emphasis is active.
- [ ] **AC-006**: The active line gutter highlight does not appear in windows or modes that do not receive active-line emphasis today.
- [ ] **AC-007**: Each built-in theme provides a usable active line gutter style.
- [ ] **AC-008**: The built-in themes keep the active line gutter visually distinct from the base gutter while staying consistent with each theme's palette.
- [ ] **AC-009**: Existing wrapping and gutter width behavior remain unchanged apart from the new numbering mode and highlight styling.

## Out of Scope
- Changing cursor shape, cursor color, or the main active-line highlight behavior in the buffer area.
- Adding per-window overrides for relative numbering.
- Making the active gutter row clickable or interactive.
- Altering motion behavior or buffer contents based on line-number display.

## Assumptions
- The existing active-line emphasis rules remain the source of truth for when the active gutter highlight should appear.
- The new relative line numbering option will be a boolean-style configuration value in the existing config system.
- The gutter already has access to the current window's cursor line when it renders.
- Built-in themes will be updated together with any theme schema changes needed for the active gutter style.

## Dependencies
- The window rendering path must be able to distinguish the cursor line from the other visible gutter rows.
- The configuration system must support a new toggle for relative line numbering.
- The theme system must support a dedicated active gutter row style.
- Built-in theme files under `src/theme/builtin/` must be updated together with the theme schema.
