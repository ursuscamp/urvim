# Fuzzy File Finder - Implementation Tasks

## Overview
Implement a reusable picker overlay with async streamed file search, then wire the first concrete file picker into the editor. Total: 8 tasks. Key milestones are picker event/state modeling, generic widget implementation, file-search worker streaming, editor integration, and regression testing.

## Core Architecture
- [x] **1.** Define the picker event and state model (test: unit tests for event/state invariants)
  - [x] **1.1** Add picker search event types for `PickerSearchStarted`, `PickerChunk`, `PickerSearchStale`, and `PickerSearchComplete`.
  - [x] **1.2** Add generic picker state for query text, results, highlighted index, generation, and open/closed lifecycle.
  - [x] **1.3** Add generation/staleness helpers so the picker can ignore late results from old searches.

- [x] **2.** Implement the generic reusable picker widget (test: widget unit tests for navigation, cancel, selection, and empty-state rendering)
  - [x] **2.1** Add a generic `PickerWidget<T>` that implements `Widget` and renders a search bar plus results window.
  - [x] **2.2** Handle query editing and restart search on text changes.
  - [x] **2.3** Handle `Esc` and `Ctrl-C` to close without selection.
  - [x] **2.4** Handle `Ctrl-N` / `Ctrl-P` and `Up` / `Down` to move the highlighted result.
  - [x] **2.5** Handle `Enter` and `Ctrl-Y` to emit the source-specific selection intent.

## File Picker Search
- [x] **3.** Add the file picker source and selection action (test: unit tests for matching and selection intent mapping)
  - [x] **3.1** Add a file-picker source that is rooted at the current working directory.
  - [x] **3.2** Filter results to files only and match case-insensitively.
  - [x] **3.3** Map selected files to open-or-focus tab behavior.

- [x] **4.** Implement the async search worker and streamed event delivery (test: worker tests for chunking, stale results, and completion events)
  - [x] **4.1** Walk the filesystem with `walkdir` and collect file matches asynchronously.
  - [x] **4.2** Emit `PickerSearchStarted` before traversal begins.
  - [x] **4.3** Emit `PickerChunk` events in chunks as matches are found.
  - [x] **4.4** Emit `PickerSearchStale` when a generation becomes obsolete.
  - [x] **4.5** Emit `PickerSearchComplete` when traversal completes.

## Editor Integration
- [x] **5.** Wire picker overlay plumbing into layout and command dispatch (test: integration tests for open, close, and overlay precedence)
  - [x] **5.1** Add a command to open the file picker from the root dispatch path.
  - [x] **5.2** Install the picker as an overlay that captures input before editor text handling.
  - [x] **5.3** Ensure the picker closes cleanly without disturbing the active buffer when cancelled.

- [x] **6.** Bind picker controls in the editor keymap layer (test: keymap regression tests for open, cancel, and navigation bindings)
  - [x] **6.1** Bind `F1` to open the file picker.
  - [x] **6.2** Preserve `Esc` / `Ctrl-C` cancel behavior while the picker is active.
  - [x] **6.3** Preserve `Enter` / `Ctrl-Y` selection behavior while the picker is active.

## Testing and Verification
- [x] **7.** Add regression coverage for picker interaction and file-opening behavior (test: integration tests for file selection and stale-result handling)
  - [x] **7.1** Verify query changes clear prior results and replace them with the latest generation.
  - [x] **7.2** Verify stale background results do not overwrite current results.
  - [x] **7.3** Verify selecting a file opens a new tab or focuses the already-open tab.

- [x] **8.** Run project quality gates after implementation (test: `cargo fmt`, `cargo check`, and targeted `cargo test`)
  - [x] **8.1** Run `cargo fmt` and fix formatting issues.
  - [x] **8.2** Run `cargo check` and resolve build or warning issues.
  - [x] **8.3** Run targeted tests for picker, layout, and file-opening paths.

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Core Architecture | 2 | 2 | 100% |
| File Picker Search | 2 | 2 | 100% |
| Editor Integration | 2 | 2 | 100% |
| Testing and Verification | 2 | 2 | 100% |
| **Total** | **8** | **8** | **100%** |
