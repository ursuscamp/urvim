# Todo Comment Highlighting - Implementation Tasks

## Overview
Implement comment-scoped todo marker highlighting as a render-time overlay, add configurable marker list support, expose theme style hooks for marker-specific tags, and cover the feature with regression tests for matching, styling, and configuration.

## Backend
- [x] **1.** Add configurable todo-marker support to startup configuration and validation. (depends on: none)
  - [x] **1.1** Extend the config schema with an optional `todo_markers` field and wire it into the resolved `Config`.
  - [x] **1.2** Validate configured markers as non-empty standalone word tokens and keep matching case-sensitive.
  - [x] **1.3** Ensure the resolved config falls back to the built-in default marker list when no override is provided.
  - [x] **1.4** Add config parsing tests that cover defaults, custom marker lists, and invalid marker values.

- [x] **2.** Extend the theme vocabulary for marker-specific comment styles. (depends on: 1)
  - [x] **2.1** Add marker-specific syntax tags to the documented tag vocabulary and keep them compatible with hierarchical lookup.
  - [x] **2.2** Add or update theme loading tests to verify marker tags resolve through the existing syntax style map.
  - [x] **2.3** Confirm marker-specific styles fall back safely to broader comment styling or the theme default style when missing.

## Rendering
- [x] **3.** Implement render-time marker scanning and styling inside comment spans. (depends on: 1, 2)
  - [x] **3.1** Add a comment-only scanner that runs on visible lines during rendering and finds configured markers using standalone-word checks.
  - [x] **3.2** Split rendered comment chunks at marker boundaries so marker text can receive a distinct style without changing buffer contents.
  - [x] **3.3** Resolve marker styles through the active theme using marker-specific tags with safe fallbacks.
  - [x] **3.4** Keep non-comment text, cursor behavior, gutter rendering, and empty-row filling unchanged.

## Testing
- [x] **4.** Add regression coverage for todo marker highlighting. (depends on: 1, 2, 3)
  - [x] **4.1** Add scanner tests for case-sensitive matching, standalone-word boundaries, and multiple marker types.
  - [x] **4.2** Add renderer tests that verify markers inside comments receive marker-specific styles.
  - [x] **4.3** Add renderer tests that verify text outside comments is not highlighted.
  - [x] **4.4** Add fallback tests for themes that omit one or more marker-specific styles.
  - [x] **4.5** Run `cargo check` and the relevant test targets to confirm the change is clean.

## Documentation
- [x] **5.** Update user-facing documentation for todo marker configuration and theme tags. (depends on: 1, 2)
  - [x] **5.1** Document `todo_markers` in `docs/config.md`.
  - [x] **5.2** Document the recommended marker-specific syntax tags in `docs/syntax/tags.md`.
  - [x] **5.3** Update the syntax-highlighting tutorial if needed to mention comment-scoped marker overlays.

## Theme Assets
- [x] **6.** Add marker-specific styling to the built-in themes. (depends on: 2)
  - [x] **6.1** Add `comment.todo`, `comment.fixme`, `comment.bug`, and `comment.note` entries to each built-in theme syntax map where they are currently missing.
  - [x] **6.2** Preserve each built-in theme's existing comment styling while layering marker-specific entries on top.
  - [x] **6.3** Add regression coverage that confirms the built-in themes expose marker-specific syntax styles.

## Completion Summary
| Area | Tasks | Status |
| --- | --- | --- |
| Backend | 2 | Complete |
| Rendering | 1 | Complete |
| Testing | 1 | Complete |
| Documentation | 1 | Complete |
| Theme Assets | 1 | Complete |
| Total | 6 | Complete |
