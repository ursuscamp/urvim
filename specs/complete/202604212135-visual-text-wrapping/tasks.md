# Visual Text Wrapping - Implementation Tasks

## Overview
Implement per-window visual wrapping with grapheme-width-aware hard/soft line segmentation, preserve logical motion semantics, and update docs/tests accordingly.

## Core Data and Config
- [x] **1.** Add wrap mode configuration and defaults.
  - [x] **1.1** Add a public `WrapMode` enum with `Hard` and `Soft` variants.
  - [x] **1.2** Add config field parsing/validation for `wrap_mode` with default `hard`.
  - [x] **1.3** Thread resolved wrap mode into runtime config access used by window rendering.
  - [x] **1.4** Add unit tests for valid/invalid `wrap_mode` values and default behavior.

## Window State and Actions
- [x] **2.** Add per-window wrap enablement and toggle action.
  - [x] **2.1** Add window-local `wrap_enabled` state, defaulting to `false` for new windows.
  - [x] **2.2** Introduce an action variant for wrap toggle and map `<C-w>w` to it.
  - [x] **2.3** Handle wrap toggle action in the correct window/pane scope without affecting other windows.
  - [x] **2.4** Add tests proving same buffer can be wrapped in one window and unwrapped in another.

## Wrap Planning and Rendering
- [x] **3.** Implement grapheme-aware wrap planner and integrate into render data building.
  - [x] **3.1** Add wrapped segment helper types for visible render planning (start/end byte offsets, continuation metadata, visual width).
  - [x] **3.2** Implement hard-wrap planner that breaks at exact visual width without splitting grapheme clusters.
  - [x] **3.3** Implement soft-wrap planner that breaks at nearest word boundary at/before width with hard-break fallback when needed.
  - [x] **3.4** Integrate planner into `build_render_data_with_style` so one logical line may produce multiple render rows when wrapping is enabled.
  - [x] **3.5** Keep existing rendering path unchanged when wrapping is disabled.
  - [x] **3.6** Add unit tests for hard-wrap boundaries, soft-wrap boundaries, fallback hard breaks, and Unicode grapheme safety.

## Gutter and Cursor Projection
- [x] **4.** Make gutter and cursor mapping wrap-aware.
  - [x] **4.1** Update gutter rendering to show line number only for first visual row of a wrapped logical line.
  - [x] **4.2** Ensure continuation visual rows render blank gutter number cells with gutter styling intact.
  - [x] **4.3** Update cursor visual projection to map logical cursor byte position into wrapped row/column correctly.
  - [x] **4.4** Add tests covering cursor placement on wrapped continuations and gutter behavior on wrapped lines.

## Motion and Viewport Compatibility
- [x] **5.** Preserve logical motion semantics while wrapped rendering is active.
  - [x] **5.1** Verify `h/j/k/l/w/e` behavior remains logical-buffer based with wrapping on.
  - [x] **5.2** Ensure remembered visual column behavior for vertical motions remains consistent with current semantics.
  - [x] **5.3** Verify viewport scroll-to-cursor behavior remains stable when wrapped rows are rendered.
  - [x] **5.4** Add regression tests for `j/k` moving by logical lines (not visual wrapped rows).

## Documentation and Verification
- [x] **6.** Update docs and run project checks.
  - [x] **6.1** Update `docs/config.md` with `wrap_mode` options and defaults.
  - [x] **6.2** Update `docs/motions.md` with `<C-w>w` behavior and wrapped-motion semantics.
  - [x] **6.3** Add/adjust inline and module/type docs for any new public APIs.
  - [x] **6.4** Run formatting and test suite for touched modules.
  - [x] **6.5** Run `cargo check` and resolve warnings/regressions introduced by this feature.

## Completion Summary
| Metric | Value |
|---|---|
| Total Tasks | 6 |
| Completed | 6 |
| In Progress | 0 |
| Remaining | 0 |
