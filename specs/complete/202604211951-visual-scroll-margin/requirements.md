# Visual Scroll Margin

## Summary
Add a configurable visual scroll margin so the viewport starts scrolling before the cursor reaches the visible border. The setting is exposed as `scroll_margin = { vertical = 5, horizontal = 5 }` by default.

## Problem Statement
Current scrolling starts only when the cursor crosses the viewport edge. This creates abrupt viewport movement and keeps the cursor pinned to borders during sustained navigation, which is harder to read and track. Users need configurable buffer space between the cursor and viewport edges while editing.

## User Stories
- As a user, I want vertical pre-scroll near the top and bottom edges, so that cursor movement feels less abrupt.
- As a user, I want horizontal pre-scroll near the left and right edges, so that long-line navigation keeps context visible.
- As a user, I want to configure vertical and horizontal margins independently in one setting, so that behavior matches my editing style.
- As a user, I want sane behavior in tiny viewports, so that margins do not break scrolling when the window is small.

## Functional Requirements
- [ ] **REQ-001**: Add a user-facing `scroll_margin` configuration setting as a TOML table with keys `vertical` and `horizontal`.
- [ ] **REQ-002**: The default `scroll_margin` value must be `{ vertical = 5, horizontal = 5 }`.
- [ ] **REQ-003**: Vertical scrolling must begin when the cursor enters the configured top or bottom margin band, rather than waiting for edge crossing.
- [ ] **REQ-004**: Horizontal scrolling must begin when the cursor enters the configured left or right margin band, rather than waiting for edge crossing.
- [ ] **REQ-005**: Effective vertical and horizontal margins must clamp automatically for small viewports so scrolling remains valid and stable.
- [ ] **REQ-006**: Scroll-margin behavior must apply consistently to all cursor movement paths that trigger viewport resolution (including normal motions, insert-mode cursor movement caused by edits/newlines, and jump/search motions).
- [ ] **REQ-007**: Existing `scroll_offset` internal state and semantics must remain intact; `scroll_margin` is an additional trigger policy and not a rename of viewport origin state.
- [ ] **REQ-008**: Unknown fields in `scroll_margin` must continue to be rejected by existing strict configuration parsing behavior.

## Non-Functional Requirements
- [ ] **NFR-001**: Cursor movement and redraw performance must remain responsive during continuous movement in large files.
- [ ] **NFR-002**: Scrolling behavior must be deterministic for identical cursor, buffer, viewport, and configuration state.
- [ ] **NFR-003**: The feature must preserve existing behavior when margins are effectively zero.
- [ ] **NFR-004**: Configuration and behavior documentation must be updated to keep user-facing terminology consistent.

## Acceptance Criteria
- [ ] **AC-001**: With default config, moving downward causes vertical scrolling once the cursor reaches the bottom 5-line margin band.
- [ ] **AC-002**: With default config, moving upward causes vertical scrolling once the cursor reaches the top 5-line margin band.
- [ ] **AC-003**: With default config, moving right causes horizontal scrolling once the cursor reaches the right 5-column margin band.
- [ ] **AC-004**: With default config, moving left causes horizontal scrolling once the cursor reaches the left 5-column margin band.
- [ ] **AC-005**: With custom `scroll_margin` values, trigger points reflect configured `vertical` and `horizontal` values.
- [ ] **AC-006**: In small viewports where configured margins are too large, effective margins clamp and scrolling still behaves correctly without oscillation or invalid offsets.
- [ ] **AC-007**: Cursor movement via representative motion categories (line motions, page motions, insert edits/newlines, and search/jump motions) all honor the same scroll-margin policy.
- [ ] **AC-008**: Invalid `scroll_margin` shapes or unknown nested keys produce startup config errors consistent with current validation behavior.

## Out of Scope
- Renaming or removing internal `scroll_offset` data structures.
- Center-cursor policies or separate recenter commands.
- Per-window or per-buffer margin overrides.
- Mode-specific scroll margins.

## Assumptions
- Viewport scrolling is centrally resolved by existing cursor-to-viewport logic.
- Configuration loading continues to use strict TOML schema (`deny_unknown_fields`).
- Margin values are non-negative integers represented as `usize`.

## Dependencies
- Existing config schema and validation pipeline in `src/config.rs`.
- Existing cursor-to-viewport scroll resolution in `src/window/view.rs`.
- User documentation in `docs/config.md`.
- Existing render and motion tests in `src/window/tests.rs` and related modules.
