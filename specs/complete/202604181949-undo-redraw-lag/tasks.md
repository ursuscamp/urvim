# Undo Redraw Lag - Implementation Tasks

## Overview
Implement a redraw invalidation path for undo and redo so buffer state changes are reflected immediately in every affected editor window. Add regression coverage for the stale-screen repro and verify the fix with the project build checks.

## Backend
- [x] **1.** Trace the undo/redo command path to find where state changes stop reaching the render invalidation logic.
  - [x] **1.1** Inspect the normal-mode handlers and any buffer/editor methods they call for undo and redo.
  - [x] **1.2** Identify the smallest layer that can request a redraw after a successful undo or redo.
- [x] **2.** Wire undo and redo to mark the UI dirty after a successful state change.
  - [x] **2.1** Update the relevant action or editor flow so redraw invalidation happens on both undo and redo.
  - [x] **2.2** Keep no-op undo/redo cases from forcing unnecessary repaint work.

## Testing
- [x] **3.** Add a regression test for the stale-screen repro.
  - [x] **3.1** Cover the `o -> hello -> Esc -> u` flow and assert the buffer state is reflected immediately after undo.
  - [x] **3.2** Add a redo case that confirms the refreshed frame appears without a follow-up cursor move.
- [x] **4.** Verify the change with project checks.
  - [x] **4.1** Run `cargo fmt` if any formatting changes are needed.
  - [x] **4.2** Run `cargo check` and address any warnings or regressions.

## Completion Summary

| Task | Status | Notes |
| --- | --- | --- |
| 1. Trace undo/redo path | Complete | Undo/redo bypassed redraw invalidation in `src/main.rs` |
| 2. Wire redraw invalidation | Complete | Successful undo/redo now marks the frame dirty |
| 3. Add regression tests | Complete | Covered the undo/redo helper path directly |
| 4. Verify with checks | Complete | `cargo fmt`, `cargo check`, and targeted test passed |
