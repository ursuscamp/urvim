# Active line background overlay - Implementation Tasks

## Overview
Fix active-line rendering so the line base style is the theme default overlaid with the active-line style, while preserving token-specific background colors.

## Backend

- [x] **1.** Trace the active-line style composition path in `src/window/mod.rs` and `src/window/render.rs` to identify where the theme default background is being preserved for tokenized spans.
  - [x] **1.1** Confirm how the active-line style is injected into `RenderData` and how `RenderChunk` styles are composed during rendering.
  - [x] **1.2** Decide the smallest code change that makes the active-line style part of the base style for tokenized cells without overriding explicit token backgrounds.

- [x] **2.** Update the rendering style merge so the active-line overlay participates in the base style for the active row.
  - [x] **2.1** Adjust line-base style composition in the render pipeline so the active-line style is layered onto the theme default before chunk styles are applied.
  - [x] **2.2** Preserve token-specific background colors when a chunk already defines one.
  - [x] **2.3** Keep blank trailing cells on the active line filled with the active-line background.

## Testing

- [x] **3.** Add or update regression coverage in `src/window/tests.rs` for active-line rendering with syntax-highlighted text.
  - [x] **3.1** Assert that tokenized text on an active line uses the active-line overlay instead of the theme default background.
  - [x] **3.2** Assert that a token with its own background still wins over the active-line background.
  - [x] **3.3** Keep the existing insert-mode and disabled-config active-line tests passing.

- [x] **4.** Run `cargo check` and the relevant window/render test subset after the fix.
  - [x] **4.1** Fix any clippy warnings or test failures uncovered by the change.

## Completion Summary

| Section | Total | Done | Status |
|---|---:|---:|---|
| Backend | 2 | 2 | Done |
| Testing | 2 | 2 | Done |
| Overall | 4 | 4 | Done |
