# Tab Group - Implementation Tasks

## Overview
Implement a root tab-group container that owns multiple windows, renders a horizontally scrolling tab bar with edge indicators, and supports countable `[b` / `]b` tab navigation. Startup should open each CLI file in its own tab, with fallback to a single empty tab when needed.

## Backend

- [x] **1.** Extend the editor action and keymap surface for tab navigation
  - [x] **1.1** Add `PreviousTab` and `NextTab` action variants and classify them as countable navigation actions
  - [x] **1.2** Ensure the new actions do not switch insert mode, snapshot state, or remembered column state
  - [x] **1.3** Bind `[b` to previous-tab and `]b` to next-tab in normal mode
  - [x] **1.4** Add unit tests for key parsing and count handling of tab-switch actions

- [x] **2.** Introduce the `TabGroup` container and tab state accessors
  - [x] **2.1** Add a new `TabGroup` type that owns the tab list, active index, and tab-bar viewport start
  - [x] **2.2** Provide accessors for the active window and active buffer view so the app can keep its current undo/snapshot flow
  - [x] **2.3** Add wrap-around tab switching for both single-step and counted navigation
  - [x] **2.4** Add public doc comments for the new module, type, and methods

- [x] **3.** Rewire startup and the main event loop around `TabGroup`
  - [x] **3.1** Load every CLI file into a separate tab, logging load failures without aborting startup
  - [x] **3.2** Fall back to a single empty tab when no files load successfully
  - [x] **3.3** Replace the single-window root in the main loop with a `TabGroup`
  - [x] **3.4** Keep undo/redo and snapshot bookkeeping attached to the active tab's buffer

## UI Rendering

- [x] **4.** Implement tab bar rendering, labels, and scrolling behavior
  - [x] **4.1** Render the tab bar in the top row and the active window below it
  - [x] **4.2** Derive tab labels from buffer file names, falling back to `Untitled`
  - [x] **4.3** Measure tab label widths with Unicode display width rules
  - [x] **4.4** Render left/right edge indicators when tabs exist offscreen
  - [x] **4.5** Scroll the tab-bar viewport only when the selected tab would otherwise be offscreen
  - [x] **4.6** Keep the tab-bar viewport unchanged when the selected tab is already visible
  - [x] **4.7** Clip labels safely when the terminal is too narrow to fit the full entry

## Testing

- [x] **5.** Add editor-level tests for the tab-navigation surface
  - [x] **5.1** Verify `[b` and `]b` parse to the correct actions
  - [x] **5.2** Verify `3]b` is treated as a counted next-tab action
  - [x] **5.3** Verify the new tab actions are countable but do not switch modes

- [x] **6.** Add tab-group behavior tests
  - [x] **6.1** Verify startup with multiple files creates one tab per file and activates the first successful load
  - [x] **6.2** Verify startup with no files creates one empty tab
  - [x] **6.3** Verify tab navigation wraps at the ends of the tab list
  - [x] **6.4** Verify counted tab navigation moves multiple tabs at once
  - [x] **6.5** Verify tab switching preserves each tab's cursor and buffer state

- [x] **7.** Add rendering tests for the tab bar viewport
  - [x] **7.1** Verify the active tab is visibly distinguished from inactive tabs
  - [x] **7.2** Verify the tab bar scrolls only when the active tab would otherwise be offscreen
  - [x] **7.3** Verify moving to an already-visible tab does not shift the viewport
  - [x] **7.4** Verify left and right indicators appear only when tabs are hidden on that side
  - [x] **7.5** Verify Unicode tab labels use display width correctly
  - [x] **7.6** Verify cursor positioning is offset by the tab bar row

## Verification

- [x] **8.** Run project verification for the tab-group feature
  - [x] **8.1** Run `cargo check` and fix any build or warning regressions
  - [x] **8.2** Run the focused test set for editor keymaps, tab group behavior, and rendering
  - [x] **8.3** Run the relevant broader test suite if focused tests uncover shared-state regressions

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Backend | 3 | 3 | 100% |
| UI Rendering | 1 | 1 | 100% |
| Testing | 3 | 3 | 100% |
| Verification | 1 | 1 | 100% |
| **Total** | **8** | **8** | **100%** |
