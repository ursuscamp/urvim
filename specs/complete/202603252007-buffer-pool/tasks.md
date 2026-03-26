# Global Buffer Pool - Implementation Tasks

## Overview
Refactor buffer ownership so a global buffer pool owns all `Buffer` values, while views and windows keep only `BufferId` handles. Update rendering, file open/create flows, and editing access paths to resolve buffers through the pool. Add tests that cover ID generation, path deduplication, and shared-buffer rendering behavior.

## Backend
- [x] **1.** Add `BufferId` and the global `BufferPool` data model. (depends on: none)
  - [x] **1.1** Introduce the `BufferId` newtype with copy/clone/debug/equality behavior and sequential ID generation starting at zero.
  - [x] **1.2** Implement the pool storage for buffer entries plus the absolute-path lookup index.
  - [x] **1.3** Add the global accessor used by the rest of the editor to reach the pool.
- [x] **2.** Move buffer creation and file open logic into the pool. (depends on: 1)
  - [x] **2.1** Implement creation of new unsaved buffers that return a `BufferId`.
  - [x] **2.2** Implement opening file-backed buffers through absolute-path resolution and reuse of already-open entries.
  - [x] **2.3** Preserve existing failure behavior so failed opens do not register partial entries.
- [x] **3.** Update buffer metadata and I/O access paths to work through `BufferId`. (depends on: 1, 2)
  - [x] **3.1** Provide pool methods for read-only buffer access, mutable access, and file-name lookup.
  - [x] **3.2** Route save/write operations through the pool instead of direct buffer ownership.
  - [x] **3.3** Update any buffer helpers that assume direct ownership so they work with pool-managed buffers.
- [x] **4.** Convert `BufferView`, `Window`, `TabGroup`, and `Layout` to use buffer IDs. (depends on: 1, 2, 3)
  - [x] **4.1** Change `BufferView` to store `BufferId` and view-local state only.
  - [x] **4.2** Update window rendering and motion/edit handlers to resolve buffers from the pool at the point of use.
  - [x] **4.3** Update tab creation and CLI path loading so they store IDs instead of `Buffer` values.
  - [x] **4.4** Update status bar buffer-name rendering to use pool-backed metadata.
- [x] **5.** Clean up buffer ownership APIs and direct buffer constructors. (depends on: 1, 2, 4)
  - [x] **5.1** Remove or redirect direct buffer ownership entry points that are no longer used by the editor flow.
  - [x] **5.2** Update module documentation and public type docs to describe the pool-based ownership model.

## Testing
- [x] **6.** Add unit tests for buffer pool behavior. (depends on: 1, 2)
  - [x] **6.1** Verify that generated IDs start at zero and increase monotonically.
  - [x] **6.2** Verify that opening the same absolute path returns the same `BufferId`.
  - [x] **6.3** Verify that failed opens do not create pool entries.
- [x] **7.** Update existing tests to use `BufferId`-backed views and windows. (depends on: 4)
  - [x] **7.1** Adjust window and layout tests to construct views through the pool.
  - [x] **7.2** Add a rendering regression test proving shared buffers update all views that reference them.
  - [x] **7.3** Update any buffer-related tests that depended on direct ownership assumptions.
- [x] **8.** Run the repository checks after the refactor. (depends on: 1, 2, 3, 4, 5, 6, 7)
  - [x] **8.1** Run `cargo check`.
  - [x] **8.2** Run the relevant test suite for buffers, windows, tab groups, and layout.

## Completion Summary

| Area | Status | Notes |
| --- | --- | --- |
| Backend | Complete | Buffer pool and ID-based ownership are in place |
| Testing | Complete | Buffer pool tests plus full `cargo test` passed |
| Overall | Complete | Refactor implemented and verified |
