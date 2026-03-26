# Detached Buffer Mutation Guard - Implementation Tasks

## Overview
Replace the detached clone-and-commit mutable buffer guard with a pool-mediated mutation path that keeps writes synchronized to the live `BufferPool` entry. Update call sites to use the new API, preserve existing editor behavior, and add concurrency-focused tests that verify concurrent writes do not silently overwrite each other.

## Backend

- [x] **1.** Redesign mutable buffer access in `src/buffer/pool.rs`. (depends on: none)
  - [x] **1.1** Remove the detached `BufferMutGuard` snapshot/`Drop` commit pattern.
  - [x] **1.2** Add a closure-based mutation API on `BufferPool` that mutates the live buffer while the pool lock is held.
  - [x] **1.3** Keep path indexing and save behavior correct when mutations change a buffer's path or contents.
  - [x] **1.4** Update public documentation comments for the revised pool and mutation APIs.

- [x] **2.** Update buffer access helpers and editor call sites to use synchronized mutation. (depends on: 1)
  - [x] **2.1** Update `src/globals.rs` to expose the new buffer mutation helper and remove any snapshot-based write path.
  - [x] **2.2** Update `BufferView::buffer_mut` and related window editing code to route through the pool-managed closure API.
  - [x] **2.3** Adjust any save, undo, redo, or command flows that assumed a detached mutable guard could outlive the pool lock.

## Testing

- [x] **3.** Add unit tests that prove the new mutation model is race-safe at the API boundary. (depends on: 1, 2)
  - [x] **3.1** Cover successful in-place mutation through the pool helper.
  - [x] **3.2** Cover missing-buffer behavior for stale `BufferId` values.
  - [x] **3.3** Add a concurrency-oriented regression test showing two mutations to the same buffer are serialized rather than last-writer-wins by detached snapshots.
  - [x] **3.4** Verify existing buffer pool path deduplication and save tests still pass under the new API.

- [x] **4.** Run repository validation for the refactor. (depends on: 1, 2, 3)
  - [x] **4.1** Run `cargo check`.
  - [x] **4.2** Run the relevant test subset or full test suite if practical.
  - [x] **4.3** Fix any clippy issues introduced by the refactor.

## Completion Summary

| Item | Status |
| --- | --- |
| Backend | Complete |
| Testing | Complete |
| Total | 4 / 4 complete |
