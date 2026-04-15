# Visible Syntax Catch-Up - Implementation Tasks

## Overview
Make syntax highlighting refresh synchronously for the visible lines in every visible window after any buffer edit, then let the remaining offscreen lines catch up in the background.

## Backend
- [x] **1.** Add a synchronous visible-range syntax refresh path for edited buffers.
  - [x] **1.1** Identify the visible line ranges for every window currently showing the edited buffer.
  - [x] **1.2** Recompute syntax state for those visible ranges immediately on the edit path. 
  - [x] **1.3** Preserve the existing deferred syntax catch-up path for offscreen lines. `(depends on: 1.2)`

- [x] **2.** Make buffer mutation paths trigger the visible-range refresh consistently.
  - [x] **2.1** Wire the refresh into insert-mode text changes and other mutation helpers that already invalidate syntax. `(depends on: 1.2)`
  - [x] **2.2** Ensure delete, change, join, newline split/merge, and line-removal paths all reach the same refresh behavior. `(depends on: 2.1)`
  - [x] **2.3** Keep syntax-disabled buffers on the no-op path. `(depends on: 2.1)`

- [x] **3.** Keep background catch-up work safe after a synchronous visible refresh.
  - [x] **3.1** Ensure stale background work cannot overwrite a newer visible refresh after additional edits.
  - [x] **3.2** Verify the background job resumes from the correct invalidation point after the immediate visible pass.
  - [x] **3.3** Confirm multi-window edits do not create conflicting syntax states between windows sharing the same buffer.

## Testing
- [x] **4.** Add regression coverage for immediate visible highlighting after edits.
  - [x] **4.1** Add a test that edits a syntax-highlighted buffer and asserts the visible spans are refreshed before redraw completes.
  - [x] **4.2** Add a test that the same buffer shown in multiple windows refreshes all visible windows immediately. `(depends on: 4.1)`
  - [x] **4.3** Add a test that background catch-up still completes for offscreen lines after the visible region is corrected. `(depends on: 4.1)`

- [x] **5.** Run project validation.
  - [x] **5.1** Run `cargo check` and fix any build or warning issues.
  - [x] **5.2** Run the relevant syntax and window regression tests for the changed paths.

## Completion Summary
| Area | Tasks | Status |
| --- | --- | --- |
| Backend | 3 | Complete |
| Testing | 2 | Complete |
| Total | 5 | Complete |
