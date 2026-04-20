# Split border junctions render without connector glyphs - Implementation Tasks
## Overview
Fix split-border rendering so every junction cell uses the correct connector glyph for the active border style in both ASCII and Unicode modes. Add regression coverage for mixed-axis split layouts and verify the renderer still behaves correctly for single-pane and resize-mode cases.

## Rendering
- [x] **1.** Replace the split-border cell model in `src/layout/render.rs` so each rendered border cell can represent the exact junction shape instead of only horizontal/vertical presence.
  - [x] **1.1** Update the border-marking pass to track all touching border directions for each cell, including T-junctions and four-way intersections.
  - [x] **1.2** Extend border glyph selection so ASCII and Unicode modes each map every supported junction shape to the appropriate character.
  - [x] **1.3** Keep resize-mode styling and single-pane behavior unchanged.

## Testing
- [x] **2.** Add regression tests in `src/layout/tests.rs` for nested split layouts that create border crossings.
  - [x] **2.1** Verify the rendered intersection glyphs in ASCII mode.
  - [x] **2.2** Verify the rendered intersection glyphs in Unicode mode.
  - [x] **2.3** Cover both T-junction and four-way crossing layouts if the test setup can distinguish them cleanly.

## Verification
- [x] **3.** Run formatter and targeted checks after the code change.
  - [x] **3.1** Run `cargo fmt`.
  - [x] **3.2** Run `cargo check`.
  - [x] **3.3** Run the focused layout tests for split-border rendering.

## Completion Summary
| Item | Status |
| --- | --- |
| Rendering | Complete |
| Testing | Complete |
| Verification | Complete |
