# Screen Resize Jumble - Implementation Tasks

## Overview

Fix the terminal corruption that appears after window resize by ensuring the visible terminal surface is repainted correctly when `Event::Resize` is received. The fix should preserve the existing diff-based renderer for normal frames while eliminating stale screen contents after a resize.

## Backend

- [x] **1.** Repaint the terminal surface on resize
  - [x] **1.1** Update the main event loop to clear the physical terminal immediately after applying a resize event
  - [x] **1.2** Keep the in-memory `Screen` dimensions in sync with the new terminal size
  - [x] **1.3** Preserve the existing render order so the next frame is drawn against the resized geometry

- [x] **2.** Add regression coverage for resize-driven repaint behavior
  - [x] **2.1** Add a test for the resize path that verifies the terminal clear escape sequence is emitted when a resize event is handled
  - [x] **2.2** Add or update a screen-level test that confirms resize resets internal dimensions cleanly without panicking
  - [x] **2.3** Cover a resize sequence that changes both rows and columns so the fix is exercised on the common corruption case

## Testing

- [x] **3.** Verify the fix across the project
  - [x] **3.1** Run `cargo test` for the affected terminal and screen paths
  - [x] **3.2** Run `cargo check` and fix any build or warning regressions
  - [x] **3.3** Sanity-check that the existing rendering and status bar tests still pass with the resize repaint behavior

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Backend | 2 | 2 | 100% |
| Testing | 1 | 1 | 100% |
| **Total** | **3** | **3** | **100%** |
