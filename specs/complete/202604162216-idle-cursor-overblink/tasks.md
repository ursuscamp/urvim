# Idle Cursor Overblink - Implementation Tasks
## Overview
Stop idle poll wakeups from forcing full redraws so the terminal cursor keeps its normal blink cadence even on very large screens with empty or very small files. Preserve background wakeups, resize handling, and visible updates when real editor state changes.

## Backend
- [x] **1.** Add explicit redraw invalidation to the main editor loop.
  - [x] **1.1** Update `src/main.rs` so the editor renders on startup, after input that changes visible state, after resize, and after accepted background work that requests repaint.
  - [x] **1.2** Ensure idle `Event::Tick` wakeups do not imply a redraw by themselves.
  - [x] **1.3** Preserve the existing background result processing so completed syntax catch-up can still trigger a repaint when needed.

- [x] **2.** Tighten cursor visibility handling around actual paint work.
  - [x] **2.1** Update the main render call site so cursor hide/show only happens for real frame renders, not for idle no-op wakeups.
  - [x] **2.2** Keep the current diff-based render behavior intact for changed cells and cursor positioning.

## Testing
- [x] **3.** Add regression coverage for idle redraw behavior.
  - [x] **3.1** Add a test that repeated idle tick wakeups do not force redraw-required state once the screen is already current.
  - [x] **3.2** Add a test that accepted background work can still request and consume a redraw.
  - [x] **3.3** Add a test that resize handling still forces a redraw after the idle-redraw change.

## Validation
- [x] **4.** Run project validation for the redraw fix.
  - [x] **4.1** Run `cargo fmt`.
  - [x] **4.2** Run `cargo check` and fix any build or warning issues introduced by the change.
  - [x] **4.3** Run the focused tests that cover idle ticks, redraw invalidation, and affected render behavior.

## Completion Summary
| Area | Tasks | Status |
|---|---|---|
| Backend | 2 | Complete |
| Testing | 1 | Complete |
| Validation | 1 | Complete |
| Total | 4 | Complete |
