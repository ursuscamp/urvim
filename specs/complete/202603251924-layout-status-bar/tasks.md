# Layout Status Bar - Implementation Tasks

## Overview
Add a persistent status bar to the root layout, track the current mode kind in `Layout`, and keep the existing editing workflow unchanged. The footer should show the current mode, active buffer name, cursor position in bytes, and file progress while remaining safe on small terminals.

## Backend

- [x] **1.** Extend the root layout to track mode kind and own a footer renderer
  - [x] **1.1** Add a focused `status_bar` component that renders one footer row from derived layout state
  - [x] **1.2** Add `ModeKind` state to `Layout` so status rendering no longer depends on the live mode object for shared state
  - [x] **1.3** Expose layout accessors needed for mode display and active buffer metadata

- [x] **2.** Split layout rendering into content and footer regions
  - [x] **2.1** Reserve a bottom row for the status bar when the layout has at least one usable row
  - [x] **2.2** Keep the existing tab-group tab bar at the top of the content region
  - [x] **2.3** Clamp geometry safely on tiny terminals so status rendering never panics
  - [x] **2.4** Keep action routing and active-buffer ownership behavior unchanged

- [x] **3.** Update the main loop to synchronize mode kind into `Layout`
  - [x] **3.1** Add a `kind()` accessor to the Mode trait and implement it for normal and insert modes
  - [x] **3.2** Keep direct mode ownership in `main` while pushing the current `ModeKind` into `Layout`
  - [x] **3.3** Keep cursor-style updates working from the live mode stored in `main`
  - [x] **3.4** Preserve undo, redo, snapshot, and cursor-update handling through the active buffer view

## Testing

- [x] **4.** Add unit tests for footer formatting and derived values
  - [x] **4.1** Verify the status bar renders mode, buffer name, cursor position, and file percentage
  - [x] **4.2** Verify unnamed buffers use the existing fallback label instead of blank output
  - [x] **4.3** Verify the cursor column is reported as bytes and the last line renders `100%`
  - [x] **4.4** Verify narrow-width rendering truncates safely

- [x] **5.** Add unit tests for layout geometry and mode routing
  - [x] **5.1** Verify the layout reserves a footer row without breaking tab-group rendering
  - [x] **5.2** Verify layout resize handling keeps content and footer within bounds
  - [x] **5.3** Verify mode changes pushed into `Layout` update the footer-relevant state

## Verification

- [x] **6.** Run project verification for the status-bar layout change
  - [x] **6.1** Run `cargo check` and fix any build or warning regressions
  - [x] **6.2** Run the focused test set for status-bar formatting, layout sizing, and mode routing
  - [x] **6.3** Run the relevant broader UI tests if the layout root change affects shared rendering behavior

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Backend | 3 | 3 | 100% |
| Testing | 2 | 2 | 100% |
| Verification | 1 | 1 | 100% |
| **Total** | **6** | **6** | **100%** |
