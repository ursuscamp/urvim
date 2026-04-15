# Active Line Highlighting
## Summary
Add optional active-line highlighting for the focused editor window. The feature is disabled by default, applies only in normal mode, and uses a theme-provided UI style so built-in themes can render the line with a subtly lighter background than the theme's default background.

## Problem Statement
When editing text in multiple windows, it can be harder to quickly locate the cursor in the focused window. A dedicated active-line highlight improves visual orientation without changing the editor's behavior or affecting non-focused windows.

## User Stories
- As a user, I want the focused window's current line highlighted in normal mode, so that I can locate my cursor more quickly.
- As a user, I want to disable active-line highlighting from my config, so that I can keep the editor visually quieter when I prefer.
- As a theme author, I want a dedicated UI style for the active line, so that I can tune its appearance independently from other UI elements.

## Functional Requirements
- [ ] **REQ-001**: The editor must provide a user-facing config option that enables or disables active-line highlighting.
- [ ] **REQ-002**: Active-line highlighting must be disabled by default.
- [ ] **REQ-003**: When enabled, active-line highlighting must apply only to the focused window.
- [ ] **REQ-004**: When enabled, active-line highlighting must appear only in normal mode.
- [ ] **REQ-005**: When enabled, active-line highlighting must target exactly the line containing the cursor.
- [ ] **REQ-006**: The theme system must expose a dedicated UI style for the active line.
- [ ] **REQ-007**: Built-in themes must define an active-line UI style with a background that is slightly lighter than the theme's default background.
- [ ] **REQ-008**: The feature must preserve the current appearance when the config option is disabled.

## Non-Functional Requirements
- Compatibility: Existing editor behavior must remain unchanged when active-line highlighting is disabled.
- Usability: The highlight must improve cursor visibility without obscuring text content.
- Maintainability: The active-line style must be represented as a first-class theme UI style rather than being inferred indirectly from unrelated styles.

## Acceptance Criteria
- [ ] **AC-001**: With the config option disabled, the focused window renders exactly as it does today with respect to active-line highlighting.
- [ ] **AC-002**: With the config option enabled, the focused window's cursor line is highlighted in normal mode.
- [ ] **AC-003**: With the config option enabled, inactive windows do not show active-line highlighting.
- [ ] **AC-004**: With the config option enabled, insert mode and visual mode do not show active-line highlighting.
- [ ] **AC-005**: The theme schema and built-in themes can represent an active-line style independently from other UI styles.
- [ ] **AC-006**: Built-in themes render the active line with a visibly lighter background than the default background while remaining subtle.

## Out of Scope
- Highlighting the active line in insert mode or visual mode.
- Highlighting the active line in inactive windows.
- Adding per-window or per-buffer overrides for the feature.
- Changing cursor shape, cursor color, or selection styling.

## Assumptions
- The editor already knows which window is focused and which mode is active at render time.
- The new config option will be a boolean-style toggle that can be read from the existing startup config system.
- Existing theme files will be updated together with the schema so built-in themes remain valid.

## Dependencies
- The window rendering pipeline must be able to distinguish the focused window from other windows.
- The theme schema must support a dedicated active-line UI style.
- The configuration system must support a new toggle for active-line highlighting.
