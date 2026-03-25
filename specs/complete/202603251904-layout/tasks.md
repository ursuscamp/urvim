# Layout - Implementation Tasks

## Overview
Introduce a root `Layout` container above the existing `TabGroup`, preserve the current tab-group user experience, and make the root geometry explicit so future UI regions can be added later. The first implementation should behave like the current editor from the user’s point of view while routing rendering and actions through the new layout layer.

## Backend

- [x] **1.** Add the root `Layout` container and its public API
  - [x] **1.1** Create a new `layout` module that owns the root geometry and a single `TabGroup` child
  - [x] **1.2** Provide constructors for building the layout from an existing tab group and from startup file paths
  - [x] **1.3** Expose accessors for the active buffer view and visual cursor so the app can keep its current undo and cursor flow
  - [x] **1.4** Add public doc comments for the new module, type, and methods

- [x] **2.** Wire the application root through `Layout`
  - [x] **2.1** Export `Layout` from the crate root so `main` can construct it directly
  - [x] **2.2** Replace the `TabGroup` root in `main` with `Layout` while preserving current mode handling and action processing
  - [x] **2.3** Keep undo/redo, snapshot, and cursor updates attached to the active tab group child through the layout accessors
  - [x] **2.4** Ensure startup file loading still behaves the same from the user’s point of view, including the empty-tab fallback

- [x] **3.** Keep layout geometry and child routing stable across redraws
  - [x] **3.1** Store the latest root origin and size in `Layout` during render
  - [x] **3.2** Forward the full available region to the child tab group for the first implementation
  - [x] **3.3** Preserve existing tab-bar and window rendering behavior when the layout is resized
  - [x] **3.4** Keep layout action forwarding limited to the child tab group for this stage

## Testing

- [x] **4.** Add unit tests for layout ownership and forwarding behavior
  - [x] **4.1** Verify `Layout` can be constructed around a tab group and exposes the active buffer view
  - [x] **4.2** Verify layout action forwarding reaches the child tab group unchanged
  - [x] **4.3** Verify layout render preserves root geometry and forwards the assigned size to the child
  - [x] **4.4** Verify resizing through the layout keeps the existing tab-group UI visible and bounded

- [x] **5.** Add integration-focused checks for startup and cursor placement
  - [x] **5.1** Verify startup with and without files still produces a usable editor state through `Layout`
  - [x] **5.2** Verify the terminal cursor position remains aligned with the active tab group after layout rendering

## Verification

- [x] **6.** Run project verification for the layout layer
  - [x] **6.1** Run `cargo check` and fix any build or warning regressions
  - [x] **6.2** Run the focused test set for layout ownership, action forwarding, and resize behavior
  - [x] **6.3** Run the relevant broader test suite if the root-container change exposes shared-state regressions

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Backend | 3 | 3 | 100% |
| Testing | 2 | 2 | 100% |
| Verification | 1 | 1 | 100% |
| **Total** | **6** | **6** | **100%** |
