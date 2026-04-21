# Indent Guides

## Summary
Add a cursor-aware indent guide that displays one active vertical guide for the current indent scope. The guide is derived from the deepest indent level at or before the cursor's visual column on the cursor line, and it renders only between the opening and closing scope lines.

## Problem Statement
Users currently have to infer scope depth from surrounding whitespace alone. This makes it harder to quickly identify which block the cursor is currently inside, especially in deeply nested code. The editor should provide a lightweight visual indicator for the active indentation scope without obscuring source text.

## User Stories
- As a user, I want to see a vertical indent guide for the scope around my cursor, so that I can orient myself in nested code quickly.
- As a user, I want the guide to align with visual indentation width (including tabs), so that the indicator matches what I see on screen.
- As a user, I want ASCII fallback behavior by default and Unicode styling when capability support is present, so that rendering remains compatible across terminals.

## Functional Requirements
- [ ] **REQ-001**: Add a user-facing `indent_guides` configuration option that controls whether indent guides are rendered.
- [ ] **REQ-002**: The default value for `indent_guides` must be `true`.
- [ ] **REQ-003**: When `indent_guides` is `false`, no indent guide must be rendered.
- [ ] **REQ-004**: When `indent_guides` is `true`, at most one active indent guide must be rendered for the cursor line.
- [ ] **REQ-005**: The active scope must be selected as the deepest indent scope whose visual indent column is less than or equal to the cursor's visual column on the cursor line.
- [ ] **REQ-006**: Scope selection and guide column placement must use visual indentation width (tab-expanded columns), including mixed tab/space leading whitespace.
- [ ] **REQ-007**: The rendered guide must run vertically between the scope boundaries and must not overwrite characters on the opening or closing boundary lines.
- [ ] **REQ-008**: The scope end boundary for guide rendering must be the line immediately before the first following line with shallower indentation than the selected scope.
- [ ] **REQ-009**: Blank lines within the selected scope must not break continuity of the rendered guide.
- [ ] **REQ-010**: If no scope exists at or before the cursor visual column, no guide must be rendered.
- [ ] **REQ-011**: If the selected scope has no interior lines between opening and closing boundaries, no guide must be rendered.
- [ ] **REQ-012**: Without Unicode indent capability support, the guide glyph must be the ASCII character `|`.
- [ ] **REQ-013**: With Unicode indent capability support (`unicode_indent`), the guide glyph must use the editor's existing Unicode line-drawing style.

## Non-Functional Requirements
- [ ] **NFR-001**: Guide rendering must not introduce perceptible cursor-move lag during normal editing in typical file sizes.
- [ ] **NFR-002**: Guide rendering must remain deterministic and stable across redraws for unchanged buffer and cursor state.
- [ ] **NFR-003**: The feature must preserve existing text rendering and syntax highlighting behavior outside the single guide column.
- [ ] **NFR-004**: The implementation must integrate with the existing indent scope cache lifecycle rather than introducing conflicting scope computations.

## Acceptance Criteria
- [ ] **AC-001**: With `indent_guides = true`, moving the cursor within nested indentation renders exactly one guide for the deepest eligible scope.
- [ ] **AC-002**: With `indent_guides = false`, no indent guide is rendered in any mode or cursor position.
- [ ] **AC-003**: On a line indented with tabs or mixed tabs/spaces, guide selection and placement matches visual columns.
- [ ] **AC-004**: The guide appears only on lines strictly between scope opening and closing lines and never overwrites opening/closing line characters.
- [ ] **AC-005**: A shallower-indent line terminates guide rendering at the immediately previous line.
- [ ] **AC-006**: Blank lines inside a scope still display the guide column continuously.
- [ ] **AC-007**: If there is no eligible scope or no interior lines for the chosen scope, no guide is rendered.
- [ ] **AC-008**: In environments without `unicode_indent`, guide glyphs are rendered as `|`.
- [ ] **AC-009**: In environments with `unicode_indent`, guide glyphs follow the editor's existing Unicode line-drawing style.

## Out of Scope
- Rendering multiple simultaneous indent guides for non-active scopes.
- Theme-customizable indent guide colors or per-filetype guide styling.
- New user configuration for choosing custom guide glyphs.
- Changes to fold behavior or fold UI based on indent guides.

## Assumptions
- The indent scope cache already exposes enough scope membership and boundaries for cursor-line scope selection.
- The renderer can overlay a guide glyph in a computed visual column without mutating buffer text.
- Terminal capability negotiation already determines whether Unicode indent line-drawing is available.

## Dependencies
- Existing `Indent Scope` and `Indent Scope Cache` behavior.
- Existing terminal capability model including `unicode_indent`.
- Existing window/render pipeline for drawing per-cell overlays.
- Configuration parsing, validation, and documentation pipeline (`docs/config.md`).
