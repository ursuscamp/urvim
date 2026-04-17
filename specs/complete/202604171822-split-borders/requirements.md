# Split Borders

## Summary
urvim should render visible borders between panes in split layouts. The borders should follow the final on-screen pane arrangement, not the underlying split tree shape, and they should disappear entirely when only one pane is visible. The separator space for those borders should be reserved outside the pane content regions so cursor movement and buffer rendering never treat border cells as part of a window. The editor should also support an additional advanced glyph capability for Unicode border characters, alongside `nerdfont`, so users can choose between ASCII and Unicode border rendering. Border styling should use a normal highlight in regular use and a distinct highlight while the editor is in resizing mode.

## Problem Statement
Split layouts currently rely on their geometry alone to separate panes, which makes adjacent panes harder to distinguish at a glance. This becomes more noticeable in nested layouts, where the split tree is internal data rather than a visual hierarchy. The editor also needs a clear and accessible way to render borders with plain ASCII characters when Unicode line-drawing characters are not desired, while still allowing a polished Unicode presentation when supported. Finally, resizing mode should be easier to recognize by giving the active borders a distinct visual emphasis.

## User Stories
- As a user working with multiple panes, I want visible borders between panes, so that I can tell where one pane ends and another begins.
- As a user working in a single pane, I want no split borders, so that the editing area stays clean and uncluttered.
- As a user who prefers Unicode line-drawing characters, I want split borders to use Unicode glyphs, so that the layout looks polished.
- As a user who prefers plain text borders, I want an ASCII fallback, so that borders remain readable without Unicode line-drawing support.
- As a user resizing panes, I want the borders to stand out differently, so that I can see the resize target more easily.

## Functional Requirements
- [ ] **REQ-001**: The editor must render split borders whenever the layout contains more than one visible pane.
- [ ] **REQ-002**: The editor must not render any split borders when the layout contains exactly one visible pane.
- [ ] **REQ-003**: Split borders must follow the flattened on-screen pane arrangement, even when the underlying split structure is nested.
- [ ] **REQ-004**: The advanced glyph capability set must support `nerdfont` and `unicode_borders` as independent enabled values.
- [ ] **REQ-005**: When `unicode_borders` is enabled, split borders must use Unicode line-drawing glyphs.
- [ ] **REQ-006**: When `unicode_borders` is not enabled, split borders must use ASCII-compatible border glyphs.
- [ ] **REQ-007**: Split borders must use a normal theme highlight that is distinct from regular text styling.
- [ ] **REQ-008**: When resizing mode is active, split borders must use a distinct resize highlight that is different from the normal border highlight.
- [ ] **REQ-009**: Split border rendering must not change buffer contents, cursor position, or tab selection.
- [ ] **REQ-010**: Split borders must remain correct after terminal resizes and when panes are arranged in nested split trees.
- [ ] **REQ-011**: Split borders must be allocated in reserved separator space rather than inside either pane's content region.

## Non-Functional Requirements
- [ ] **NFR-001**: Split border rendering must remain responsive during normal redraws and terminal resizes.
- [ ] **NFR-002**: The feature must remain compatible with existing split navigation, split creation, and resizing behavior.
- [ ] **NFR-003**: The feature must be covered by unit tests for single-pane suppression, nested split flattening, glyph selection, and resize-mode border styling.

## Acceptance Criteria
- [ ] **AC-001**: A layout with one pane shows no split borders.
- [ ] **AC-002**: A layout with two or more panes shows visible borders between panes.
- [ ] **AC-003**: Nested split layouts render borders as a single flattened screen layout rather than exposing the tree structure.
- [ ] **AC-004**: Enabling `unicode_borders` renders Unicode line-drawing borders, and disabling it renders ASCII borders.
- [ ] **AC-005**: Normal editing uses the regular border highlight, while resizing mode uses a distinct resize highlight.
- [ ] **AC-006**: Rendering split borders does not alter the active buffer, cursor, or tab selection.
- [ ] **AC-007**: Split borders still render correctly after the terminal is resized.

## Out of Scope
- Changing how splits are created, closed, focused, or resized.
- Adding user-configurable per-pane border geometry.
- Persisting border appearance separately from the existing advanced glyph configuration.
- Mouse-driven border interactions.

## Assumptions
- The editor already has enough layout information to determine how many panes are visible and where pane boundaries fall.
- The theme system can expose separate normal and resize border highlights.
- The advanced glyph configuration is already a runtime concept that can be extended with `unicode_borders`.
- The border rendering path can distinguish between regular editing and resizing mode.
- The layout engine can reserve a one-cell separator band between adjacent panes without changing the pane content regions.

## Dependencies
- Existing nested split layout and pane geometry.
- Existing theme and highlight resolution.
- Existing advanced glyph configuration plumbing.
- Existing resizing mode state.
- Existing terminal redraw and resize handling.
