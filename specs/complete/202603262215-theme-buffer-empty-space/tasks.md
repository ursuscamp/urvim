# Buffer Viewport Theming - Implementation Tasks
## Overview
Fix the buffer viewport so blank cells inherit the active theme's default style instead of falling back to the screen's neutral default. The implementation should add an explicit style-aware clear/fill helper to `Screen`, use it from `Window::render`, and cover the behavior with regression tests.

## Backend
- [x] **1.** Add a style-aware screen clearing primitive
  - [x] **1.1** Add `clear_with_style(style: Style)` or an equivalent fill helper to `src/screen.rs` (depends on: 1.2)
  - [x] **1.2** Keep the existing `clear()` behavior as the neutral/default-style clear path so non-themed callers do not change behavior
  - [x] **1.3** Add doc comments for the new public screen API

- [x] **2.** Paint the full buffer viewport with the theme default style
  - [x] **2.1** Update `Window::render` in `src/window/mod.rs` to clear or fill the content viewport with the theme default style before text rendering
  - [x] **2.2** Keep gutter rendering unchanged so the buffer-area fill does not leak into the gutter
  - [x] **2.3** Preserve existing horizontal scroll and cursor-visibility behavior

- [x] **3.** Keep render chunk layering correct
  - [x] **3.1** Verify `RenderData::render_with_base_style` still overlays line text on top of the base style in `src/window/render.rs`
  - [x] **3.2** Avoid introducing theme-specific logic into `Screen` beyond style-aware clearing/filling

## Testing
- [x] **4.** Add regression tests for themed blank buffer space
  - [x] **4.1** Add a test showing short lines leave trailing buffer cells with the theme default style
  - [x] **4.2** Add a test showing rows below the last buffer line use the theme default style
  - [x] **4.3** Add a test confirming the gutter still uses its own style
  - [x] **4.4** Run `cargo check` and the affected window/screen test modules

## Completion Summary
| Area | Tasks | Done |
| --- | --- | --- |
| Backend | 3 | 3 |
| Testing | 1 | 1 |
| Total | 4 | 4 |
