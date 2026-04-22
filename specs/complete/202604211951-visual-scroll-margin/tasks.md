# Visual Scroll Margin - Implementation Tasks

## Overview
Implement configurable margin-triggered scrolling using `scroll_margin = { vertical, horizontal }` while preserving existing `scroll_offset` semantics and ensuring consistent behavior across all cursor movement paths.

## Backend
- [x] **1.** Add `scroll_margin` to configuration schema and resolved config.
  - [x] **1.1** Introduce `ScrollMargin` and `PartialScrollMargin` data models in `src/config.rs`.
  - [x] **1.2** Add `scroll_margin` fields to `Config` and `PartialConfig`.
  - [x] **1.3** Resolve defaults (`vertical = 5`, `horizontal = 5`) and partial-key fallback behavior.
  - [x] **1.4** Add validation coverage for valid/invalid TOML shapes and unknown nested keys. (depends on: 1.1)

- [x] **2.** Implement margin-aware cursor scrolling logic.
  - [x] **2.1** Update `BufferView::scroll_to_cursor` in `src/window/view.rs` to use margin-triggered keep-zone checks.
  - [x] **2.2** Clamp effective margins by visible rows/cols for tiny viewports.
  - [x] **2.3** Preserve existing final offset clamping to buffer max row/col bounds.
  - [x] **2.4** Ensure zero-sized viewport and gutter edge cases remain safe with saturating behavior.

- [x] **3.** Update user documentation.
  - [x] **3.1** Add `scroll_margin` to `docs/config.md` schema list.
  - [x] **3.2** Add a dedicated `scroll_margin` section with defaults and behavior examples.
  - [x] **3.3** Document small-viewport effective-margin clamping behavior.

## Testing
- [x] **4.** Add configuration tests in `src/config.rs`.
  - [x] **4.1** Test default resolution when `scroll_margin` is omitted.
  - [x] **4.2** Test partial table resolution when one key is omitted.
  - [x] **4.3** Test custom value loading for both keys.
  - [x] **4.4** Test parse rejection for unknown nested keys and invalid types.

- [x] **5.** Add scrolling behavior regression tests in `src/window/tests.rs` (or appropriate view tests).
  - [x] **5.1** Test vertical scrolling starts at configured bottom margin.
  - [x] **5.2** Test vertical scrolling starts at configured top margin.
  - [x] **5.3** Test horizontal scrolling starts at configured right margin.
  - [x] **5.4** Test horizontal scrolling starts at configured left margin.
  - [x] **5.5** Test small viewport clamping prevents invalid/oscillating behavior.
  - [x] **5.6** Test zero-margin compatibility path preserves edge-trigger behavior.

- [x] **6.** Run project quality checks.
  - [x] **6.1** Run `cargo fmt`.
  - [x] **6.2** Run targeted tests for config and window scrolling.
  - [x] **6.3** Run `cargo check` and resolve warnings.

## Completion Summary
| Section | Total | Done | Status |
| --- | ---: | ---: | --- |
| Backend | 3 | 3 | Done |
| Testing | 3 | 3 | Done |
| Total | 6 | 6 | Done |
