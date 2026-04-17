# Split Borders - Implementation Tasks

## Overview

Add visible split borders to pane layouts, including a Unicode border capability, themeable normal and resize border styles, and ASCII fallbacks. The implementation should keep borders out of single-pane layouts, render them against the flattened screen arrangement, and update every builtin theme to provide the new styles.

## Backend

- [x] **1.** Extend advanced glyph configuration with `unicode_borders` and add a runtime helper for checking it. (depends on: 1.1)
  - [x] **1.1** Add the `UnicodeBorders` variant to `AdvancedGlyphCapability` and update config parsing/validation.
  - [x] **1.2** Add a `unicode_borders_enabled()` convenience method on `Config`.
  - [x] **1.3** Update config tests to cover loading, deduplication, and rejection of unknown capabilities.

- [x] **2.** Add split border theme styles to the closed UI theme schema and resolved theme model. (depends on: 2.1)
  - [x] **2.1** Add raw UI style fields for normal and resize split borders in `src/theme/schema.rs`.
  - [x] **2.2** Add matching `UiStyles` fields and theme resolution plumbing in `src/theme/model.rs` and `src/theme/loader.rs`.
  - [x] **2.3** Add closed UI style key variants in `src/theme/keys.rs`.
  - [x] **2.4** Update builtin theme sources so every bundled theme defines both border styles.

- [x] **3.** Update layout rendering to draw flattened split borders with ASCII and Unicode glyph sets. (depends on: 1.2, 2.2)
  - [x] **3.1** Add a layout-level border rendering pass in `src/layout/render.rs`.
  - [x] **3.2** Skip border rendering entirely when only one pane is visible.
  - [x] **3.3** Reuse flattened pane regions so nested split trees render as a single on-screen border network.
  - [x] **3.4** Select ASCII or Unicode glyphs based on the `unicode_borders` capability.
  - [x] **3.5** Clip border drawing safely to the visible screen area.

- [x] **4.** Make border rendering use a distinct resize highlight when resizing mode is active. (depends on: 2.2)
  - [x] **4.1** Read the active mode kind during layout rendering.
  - [x] **4.2** Switch from the normal split border style to the resize border style while in resizing mode.

## Testing

- [x] **5.** Add regression tests for configuration, theming, and border selection behavior. (depends on: 1.1, 2.1, 3.1)
  - [x] **5.1** Verify `unicode_borders` is accepted, deduplicated, and rejected when unknown.
  - [x] **5.2** Verify every builtin theme exposes split border styles.
  - [x] **5.3** Verify the border renderer chooses ASCII fallback when `unicode_borders` is disabled.
  - [x] **5.4** Verify the border renderer chooses Unicode glyphs when `unicode_borders` is enabled.

- [x] **6.** Add layout rendering tests for single-pane suppression, nested split flattening, and resize highlighting. (depends on: 3.1, 4.1)
  - [x] **6.1** Verify a single-pane layout renders no split borders.
  - [x] **6.2** Verify multi-pane layouts render visible borders.
  - [x] **6.3** Verify nested split layouts render borders according to the flattened screen layout.
  - [x] **6.4** Verify resizing mode uses the resize border style instead of the normal border style.

## Completion Summary

| Area | Status | Notes |
| --- | --- | --- |
| 1. Advanced glyph config | Done | Add `unicode_borders` capability and config helper |
| 2. Theme schema and builtin themes | Done | Add split border styles everywhere they are required |
| 3. Layout border rendering | Done | Draw flattened borders with ASCII/Unicode glyph sets |
| 4. Resize-mode highlighting | Done | Use a distinct resize border style |
| 5. Config/theme tests | Done | Cover capability and theme wiring |
| 6. Layout rendering tests | Done | Cover border visibility and flattening |
