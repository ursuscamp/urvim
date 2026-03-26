# Buffer Read Lock Refactor

## Summary
urvim will stop returning cloned `Buffer` snapshots from the global buffer accessor and will instead expose lock-scoped read access to the live buffer pool. The global `BufferPool` synchronization primitive will also change from `Mutex` to `RwLock` so multiple read-only callers can proceed concurrently while writes remain exclusive.

## Problem Statement
The current `globals::get_buffer` path clones an entire `Buffer` for every read. That is unnecessary overhead because `Buffer` is not cheaply clonable, and the clone hides the real ownership model from callers.

The current global pool lock is also a `Mutex`, which serializes all reads even when callers only need immutable access. That is correct but unnecessarily restrictive for a buffer pool that will likely be read far more often than it is written.

## User Stories
- As a renderer, I want to inspect a live buffer through a short-lived read closure, so that I do not need to clone the entire buffer to draw a frame.
- As an editor action, I want buffer mutations to continue to happen through exclusive pool access, so that edits remain serialized and consistent.
- As a future maintainer, I want the global buffer pool to support concurrent readers, so that read-heavy code paths scale better without changing buffer ownership.

## Functional Requirements
- [ ] **REQ-001**: The global buffer read API must no longer return an owned `Buffer` clone.
- [ ] **REQ-002**: The buffer read API must provide short-lived access to the live buffer while the pool lock is held.
- [ ] **REQ-003**: The global buffer pool must be protected by a read-write lock rather than a mutex.
- [ ] **REQ-004**: Read-only callers must be able to access the pool concurrently when no writer is active.
- [ ] **REQ-005**: Mutable buffer access must remain exclusive and must continue to run while the pool is locked.
- [ ] **REQ-006**: Existing buffer identity and deduplication behavior must remain unchanged.
- [ ] **REQ-007**: Missing buffer IDs must still be handled safely and predictably.
- [ ] **REQ-008**: Callers that only need buffer metadata or line text must be able to obtain it without cloning the full buffer.
- [ ] **REQ-009**: Existing editor behavior for rendering, motion calculations, and editing commands must remain functionally unchanged.

## Non-Functional Requirements
- **Performance**: Read-heavy paths should avoid full-buffer cloning and should allow concurrent read access where possible.
- **Reliability**: The refactor must not introduce stale buffer views or detached mutable snapshots.
- **Compatibility**: Buffer identity, deduplication, and editor behavior must remain stable for existing workflows.
- **Usability**: The new API should make the live ownership model obvious to callers.
- **Security**: The change must not introduce `unsafe` code or lifetime tricks that obscure synchronization.

## Acceptance Criteria
- [ ] **AC-001**: No public helper returns an owned `Buffer` solely for read access through the global pool.
- [ ] **AC-002**: Read-only buffer access can be expressed as a closure over the live buffer state.
- [ ] **AC-003**: The global pool uses a read-write lock and supports multiple concurrent readers.
- [ ] **AC-004**: Existing rendering and editing flows continue to pass their tests after the refactor.
- [ ] **AC-005**: Buffer-related tests confirm that reads do not require cloning and that writes still mutate the live buffer.
- [ ] **AC-006**: `cargo check` completes successfully with no new warnings introduced by the refactor.

## Out of Scope
- Changing buffer internals or making `Buffer` cheaply clonable.
- Introducing per-buffer locks or lock sharding.
- Changing undo/redo semantics.
- Changing file loading, saving, or path deduplication rules.
- Reworking the editor into a multi-threaded rendering architecture.

## Assumptions
- The repository is willing to accept a repo-local API break if it removes unnecessary cloning.
- Closure-scoped read access is preferable to exposing long-lived read guards throughout the codebase.
- The current single-process editor model is sufficient for a coarse `RwLock` around the global pool.
- Existing call sites can be updated without changing user-visible behavior.

## Dependencies
- The existing `Buffer` type and its read-only query methods.
- The existing `BufferPool` ownership model.
- The existing global state module that exposes buffer access helpers.
- The current window, layout, and rendering code paths that read buffer data.
