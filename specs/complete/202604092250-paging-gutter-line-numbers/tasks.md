# Paging Gutter Line Numbers - Implementation Tasks

## Overview

Fix the paging render-order bug so the `Gutter` always reflects the same first visible line as the content area during the render pass that follows page-up, page-down, and related viewport motions. Cover the fix with regression tests that fail if the gutter lags one render behind the `Buffer View` scroll position.

## Implementation

- [x] **1.** Update window rendering to derive gutter state from the finalized scroll position
  - [x] **1.1** Rework `Window::render` in `src/window/mod.rs` so `scroll_to_cursor` runs before the gutter start line is captured
  - [x] **1.2** Preserve consistent gutter width and content width calculations within the same render pass
  - [x] **1.3** Keep the render flow clear with any minimal comments needed to explain the ordering dependency

## Testing

- [x] **2.** Add regression coverage for paging-related gutter rendering
  - [x] **2.1** Add a `src/window/tests.rs` test that pages the cursor, renders immediately, and asserts the gutter line numbers match the new page
  - [x] **2.2** Cover both upward and downward paging behavior, or document why one direction is sufficient if the shared path is identical
  - [x] **2.3** Verify nearby paging variants still behave correctly, especially clamped viewport edges when applicable

- [x] **3.** Verify the fix with project checks
  - [x] **3.1** Run the targeted window tests related to paging and gutter rendering
  - [x] **3.2** Run `cargo check` and resolve any warnings or compile issues caused by the change

## Completion Summary

| Section | Total | Completed | Remaining | Status |
| --- | ---: | ---: | ---: | --- |
| Implementation | 3 | 3 | 0 | Complete |
| Testing | 5 | 5 | 0 | Complete |
| Overall | 8 | 8 | 0 | Complete |
