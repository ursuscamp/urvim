# Visual Text Wrapping

## Summary
Add grapheme-width-aware visual text wrapping for editor windows. Wrapping is render-only, configurable by wrap kind (`hard` or `soft`), and toggleable per window with `<C-w>w` (default off).

## Problem Statement
Long logical lines currently extend past the visible window width, reducing readability and making on-screen cursor tracking harder. Users need optional visual wrapping that preserves logical editing semantics while improving readability in constrained window widths.

## User Stories
- As a terminal editing user, I want long lines to wrap visually in a window, so that I can read and navigate content without horizontal overflow.
- As a user working in split layouts, I want wrapping to be toggleable per window, so that I can compare wrapped and unwrapped views of the same buffer.
- As a user editing Unicode-heavy text, I want wrap and cursor rendering to respect grapheme display width, so that cursor placement and line breaks stay visually correct.
- As a Vim-style navigation user, I want movement commands to keep logical-buffer behavior, so that wrapping does not change motion semantics.

## Functional Requirements
- [ ] **REQ-001**: The editor SHALL support a per-window visual wrap toggle mapped to `<C-w>w`.
- [ ] **REQ-002**: Visual wrapping SHALL be disabled by default for newly created windows.
- [ ] **REQ-003**: When visual wrapping is enabled, rendering SHALL split a single logical buffer line into one or more visual rows based on the active window text width.
- [ ] **REQ-004**: Wrapping SHALL be render-only and SHALL NOT mutate buffer contents.
- [ ] **REQ-005**: The editor SHALL provide a configuration option for wrap kind with supported values `hard` and `soft`.
- [ ] **REQ-006**: In `hard` wrap mode, the renderer SHALL break wrapped rows at the exact maximum visual width.
- [ ] **REQ-007**: In `soft` wrap mode, the renderer SHALL break at the nearest word boundary at or before the maximum visual width.
- [ ] **REQ-008**: In `soft` wrap mode, when no eligible word boundary exists at or before the maximum visual width, wrapping SHALL fall back to a hard break.
- [ ] **REQ-009**: Wrap point decisions SHALL use grapheme cluster boundaries and display width, not byte offsets.
- [ ] **REQ-010**: Cursor placement in wrapped rendering SHALL map logical cursor position to the correct wrapped visual row and column.
- [ ] **REQ-011**: Core movement commands (`h`, `j`, `k`, `l`, `w`, `e`, and equivalent logical motions) SHALL preserve logical-buffer semantics when wrapping is enabled.
- [ ] **REQ-012**: When a logical line renders across multiple wrapped visual rows, the gutter SHALL display the logical line number only on the first visual row and SHALL render no duplicate line number on continuation rows.
- [ ] **REQ-013**: Wrapped rendering SHALL work consistently for all visible windows, including splits with different widths and independent wrap toggles.

## Non-Functional Requirements
- Performance: Wrapping calculations SHALL keep interactive rendering responsive during scrolling and cursor movement in typical editing workloads.
- Reliability: Wrapping and cursor placement SHALL remain stable across mixed-width Unicode text (including combining characters and emoji).
- Compatibility: Existing behavior when wrapping is off SHALL remain unchanged.
- Usability: Visual output SHALL make continuation rows clearly readable without duplicating gutter line numbers.

## Acceptance Criteria
- [ ] **AC-001**: Given wrapping is off in a window, long logical lines render as a single row with existing overflow behavior unchanged.
- [ ] **AC-002**: Given wrapping is off in one window and on in another, toggling `<C-w>w` in one window changes only that window.
- [ ] **AC-003**: Given wrapping is on and wrap kind is `hard`, a long logical line is rendered into fixed-width wrapped rows.
- [ ] **AC-004**: Given wrapping is on and wrap kind is `soft`, a long line with spaces wraps at the nearest word boundary at or before the width limit.
- [ ] **AC-005**: Given wrapping is on and wrap kind is `soft`, a long unbroken token wraps using fallback hard breaks.
- [ ] **AC-006**: Given a wrapped logical line spans multiple visual rows, only the first visual row shows the gutter line number.
- [ ] **AC-007**: Given cursor motions (`h`, `j`, `k`, `l`, `w`, `e`) on wrapped content, resulting logical cursor position matches behavior with wrapping off.
- [ ] **AC-008**: Given Unicode text with multi-width graphemes and combining marks, wrap boundaries and cursor rendering align to grapheme display width and do not split grapheme clusters.

## Out of Scope
- Changing underlying buffer storage or introducing hard newline insertion (reflowing file contents).
- Changing logical motion semantics to visual-row-aware movement.
- Adding hyphenation or language-specific advanced line-breaking rules beyond word-boundary soft wrap.

## Assumptions
- Existing window-width calculations already account for gutter and other UI columns to determine available text width.
- Existing motion engine remains authoritative for logical cursor updates; wrapping only affects projection of cursor onto rendered rows.
- Word boundaries for soft wrap can be defined by the editor's current token/whitespace boundary rules without requiring locale-specific dictionaries.

## Dependencies
- Window rendering pipeline support for wrapped row segmentation.
- Cursor rendering/projection logic for logical-to-visual mapping in wrapped windows.
- Configuration parsing/validation for wrap kind.
- Keymap/action plumbing for per-window wrap toggle.
- Documentation updates for wrapping config and toggle motion/keybinding.
