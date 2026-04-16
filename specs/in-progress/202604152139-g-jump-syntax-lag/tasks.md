# `G` Jump Syntax Lag - Implementation Tasks

## Overview
Restore instant `G`-to-bottom navigation in long syntax-highlighted files by removing the blocking synchronous syntax warmup from the deep-jump render path, while keeping background syntax catch-up intact.

## Backend
- [x] **1.** Remove the blocking syntax warmup from the bottom-jump render path.
  - [x] **1.1** Identify the syntax work that must remain synchronous for the initial visible viewport on open.
  - [x] **1.2** Update `src/window/view.rs` so rendering still highlights the initial viewport immediately, but no longer forces `ensure_syntax_through(visible_end_line)` for an incomplete long-range jump.
  - [x] **1.3** Keep the background catch-up request in place so offscreen syntax still completes after the first frame. `(depends on: 1.2)`
  - [x] **1.4** Keep syntax-disabled buffers on the existing no-op path. `(depends on: 1.2)`

- [x] **2.** Make sure nearby navigation still behaves correctly after the render-path change.
  - [x] **2.1** Verify ordinary scrolling and short moves still produce correct visible syntax without extra blocking.
  - [x] **2.2** Confirm `G` and other long-distance motions share the same non-blocking viewport update path.
  - [x] **2.3** Ensure stale background syntax results still get rejected after a new jump or edit. `(depends on: 1.2)`
  - [x] **2.4** Keep the initial open-path highlight behavior so the first viewport does not flash plain text. `(depends on: 1.1)`

## Testing
- [x] **3.** Add a regression test for the bottom-jump responsiveness issue.
  - [x] **3.1** Add a window/render test that jumps a large syntax-highlighted buffer to the end and asserts the first render does not fully complete the syntax cache.
  - [x] **3.2** Add a follow-up assertion that background catch-up still fills in the remaining syntax data after the initial render. `(depends on: 3.1)`
  - [x] **3.3** Cover the syntax-disabled path to ensure the new behavior stays a no-op there. `(depends on: 3.1)`

- [x] **4.** Run project validation.
  - [x] **4.1** Run `cargo check` and address any build or warning issues.
  - [x] **4.2** Run the relevant window and syntax regression tests for the changed paths.

## Completion Summary
| Area | Tasks | Status |
| --- | --- | --- |
| Backend | 2 | Complete |
| Testing | 2 | Complete |
| Total | 4 | Complete |
