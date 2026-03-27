# Filetype Detection - Implementation Tasks

## Overview

Total: 6 tasks
Estimated completion: 1-2 days
Prerequisites: Approved requirements and design

## Implementation

- [x] **1.** Add the filetype model and detection helpers in the buffer layer
  - [x] **1.1** Introduce a public `Filetype` enum with a curated set of common and less common code-editor filetypes plus a fallback label.
  - [x] **1.2** Add detection helpers for filename-based matches, extensionless filenames, and shebang-based matches.
  - [x] **1.3** Document the new public buffer/filetype APIs with rustdoc comments.

- [x] **2.** Store and expose filetype on buffers
  - [x] **2.1** Add filetype storage to `Buffer` and initialize it for new, loaded, and path-backed buffers.
  - [x] **2.2** Add a read-only buffer method that returns the current filetype.
  - [x] **2.3** Refresh the filetype when relevant buffer metadata or first-line content changes.

- [x] **3.** Thread filetype into the status bar rendering path
  - [x] **3.1** Extend `StatusBarContext` with a filetype label field.
  - [x] **3.2** Update `Layout` to read the active buffer filetype and pass a display label to the status bar.
  - [x] **3.3** Update the footer text format so the filetype appears alongside the existing metadata.

- [x] **4.** Add tests for filename and shebang detection
  - [x] **4.1** Test common filename and extension mappings for representative code-editor filetypes.
  - [x] **4.2** Test shebang parsing for `/usr/bin/env` wrappers and direct interpreter paths.
  - [x] **4.3** Test fallback behavior for unnamed buffers and unknown file patterns.

- [x] **5.** Add tests for status bar integration
  - [x] **5.1** Test that the status bar text includes the filetype label.
  - [x] **5.2** Test that existing footer fields still render after the filetype field is added.

- [x] **6.** Verify and fix regressions
  - [x] **6.1** Run `cargo check` and fix compile errors or warnings.
  - [x] **6.2** Run the targeted buffer, layout, and status bar tests.
  - [x] **6.3** Run the full test suite before marking the work complete.

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
| --- | --- | --- | --- |
| Implementation | 4 | 4 | 100% |
| Testing | 2 | 2 | 100% |
| **Total** | **6** | **6** | **100%** |
