# Buffer Cache Refactor - Implementation Tasks

## Overview
Refactor buffer-derived cache ownership so syntax-specific cache data is separated from indent-scope cache data, then route refresh, undo/redo, and call sites through a new `BufferCache` container.

## Backend
- [x] **1.** Introduce the `BufferCache` container and move indent-scope cache ownership out of `SyntaxCache`.
  - [x] **1.1** Create `BufferCache` with syntax cache plus indent-scope cache fields and stale tracking.
  - [x] **1.2** Remove indent-scope cache fields and accessors from `SyntaxCache`.
  - [x] **1.3** Update `SyntaxCache` methods to focus only on syntax tokenization state.
  - [x] **1.4** Add or update module documentation comments for the new public cache types and methods.

- [x] **2.** Update buffer state, snapshotting, and undo/redo to store `BufferCache`.
  - [x] **2.1** Replace the buffer's direct `SyntaxCache` field with a `BufferCache` field.
  - [x] **2.2** Update `Snapshot` and `UndoState` to store and restore `BufferCache`.
  - [x] **2.3** Update buffer clone, push-snapshot, undo, redo, and cache-update paths to use `BufferCache`.
  - [x] **2.4** Preserve generation invalidation behavior when text or cache state changes.

- [x] **3.** Rename and reshape the background catch-up flow into a buffer cache worker.
  - [x] **3.1** Rename the catch-up result and job types to buffer-cache terminology where appropriate.
  - [x] **3.2** Update the worker input/output to carry `BufferCache` instead of separate syntax and indent cache pieces.
  - [x] **3.3** Refresh both syntax and indent caches inside the worker using the existing line text and tab-width inputs.
  - [x] **3.4** Keep latest-only submission and generation-token validation behavior intact.

- [x] **4.** Update buffer-facing cache accessors and invalidation paths.
  - [x] **4.1** Route buffer cache reads through `BufferCache` accessors.
  - [x] **4.2** Update invalidation so the buffer cache marks the affected data stale together.
  - [x] **4.3** Keep synchronous `syntax_spans_for_line` and related reads working for visible lines.
  - [x] **4.4** Update any call sites that should talk to `BufferCache` directly instead of `SyntaxCache`.

## Testing
- [x] **5.** Add regression coverage for cache ownership, refresh, and restoration behavior.
  - [x] **5.1** Update unit tests that construct or inspect syntax cache snapshots.
  - [x] **5.2** Add tests that verify undo/redo restores the full buffer cache state.
  - [x] **5.3** Add tests that verify buffer-cache invalidation and refresh keep indent cache and syntax cache aligned.
  - [x] **5.4** Update worker-related tests to validate the new buffer-cache result shape and stale-result rejection.

- [x] **6.** Run formatting and build validation.
  - [x] **6.1** Format the touched Rust code.
  - [x] **6.2** Run `cargo check` and fix any resulting warnings or errors.
  - [x] **6.3** Run the relevant buffer/syntax test subset.

## Completion Summary
| Item | Status |
| --- | --- |
| Backend tasks | Complete |
| Testing tasks | Complete |
| Total tasks | 6 / 6 complete |
