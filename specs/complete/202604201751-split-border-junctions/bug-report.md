# 202604201751-split-border-junctions: Split border junctions render without connector glyphs
## Summary
Split layouts currently leave border-crossing cells blank instead of drawing the correct junction glyph for the borders that meet there. This affects both ASCII and Unicode border modes.

## Severity: Medium

## Environment
urvim terminal editor, split-pane layouts, ASCII border mode, and Unicode border mode (`advanced_glyphs = ["unicode_borders"]`).

## Reproduction Steps
1. Start urvim with at least one buffer open.
2. Create a vertical split.
3. Split one side horizontally so a vertical border and a horizontal border cross.
4. Render the layout in ASCII border mode.
5. Repeat the same layout in Unicode border mode.
6. Inspect the border intersection cell where the two split lines cross.

## Expected Behavior
Every split-border junction should render the most appropriate connector glyph for the active border style.

## Actual Behavior
The border-crossing cell has no visible connector glyph, so the split lines do not visually join at the intersection.

## Impact
Users see broken split borders in nested and mixed-axis layouts, which makes pane structure harder to read in both available border styles.

## Root Cause
`src/layout/render.rs` treated split borders as coarse per-cell state and stopped each border segment one cell too early at the separator boundary. That meant junction cells could be skipped entirely, and even when a cell was marked, the glyph lookup only considered the simplified horizontal/vertical state instead of the neighboring border topology needed to distinguish tees and crosses.

## Solution Approach
Include the separator cell when marking border segments, then choose the glyph from the surrounding border topology so every junction cell renders the correct ASCII or Unicode connector. Keep the existing split-border styling behavior unchanged.

Rejected alternative: hard-coding one universal connector glyph for all crossings, which would still be wrong for T-junctions and corner cases.

## Code Changes
- `src/layout/render.rs`: expand split-border cell tracking and glyph selection so junctions render the correct connector character in both modes.
- `src/layout/tests.rs`: add regression coverage for nested mixed-axis splits and verify the intersection glyphs in ASCII and Unicode mode.

## Edge Cases
- Nested vertical and horizontal splits that create T-junctions.
- Four-way intersections in deeper split trees.
- Resize mode styling should continue to apply to the same border cells.
- Single-pane layouts should still render no split borders.
- Terminal resizes should not change which glyph appears at a given junction.
