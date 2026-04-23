# Buffer Cache Refactor

## Summary
Refactor buffer-derived cache state so `SyntaxCache` only owns syntax highlighting data, while a new `BufferCache` owns syntax cache plus indent-scope cache and any future buffer-level caches. Update cache refresh, invalidation, and undo/redo restoration paths to use the new container without changing editor-visible behavior.

## Problem Statement
Buffer-derived cache state is currently mixed together inside `SyntaxCache`, which couples syntax highlighting concerns to indent-scope caching and makes it harder to introduce additional buffer-level caches later. The background worker is also named and structured as a syntax-specific cache refresher even though it already refreshes more than syntax data. Undo/redo currently restores syntax state, but the cache ownership model is not explicit at the buffer level.

## User Stories
- As a maintainer, I want buffer cache concerns grouped under a single container, so that related cache state has a clear owner.
- As a maintainer, I want the background cache worker to refresh all stale buffer caches, so that future buffer-level caches can be added without redesigning the worker surface.
- As a user, I want undo and redo to restore the same buffer cache state that existed with the text snapshot, so that highlighting and indent-aware features remain consistent after history traversal.
- As a maintainer, I want call sites to use the most appropriate cache API, so that syntax-related code does not have to know about unrelated cache internals.

## Functional Requirements
- [ ] **REQ-001**: The editor shall introduce a `BufferCache` type that owns buffer-derived cache state for a buffer.
- [ ] **REQ-002**: `SyntaxCache` shall no longer own indent-scope cache state directly.
- [ ] **REQ-003**: `BufferCache` shall expose syntax highlighting cache state and indent-scope cache state together as one logical buffer snapshot.
- [ ] **REQ-004**: Cache invalidation caused by buffer edits shall mark the affected buffer cache state stale so it can be refreshed again.
- [ ] **REQ-005**: The background cache worker shall refresh whatever buffer cache entries are stale for a buffer, not only syntax highlighting state.
- [ ] **REQ-006**: Undo and redo shall restore the full `BufferCache` associated with the text snapshot.
- [ ] **REQ-007**: Existing callers shall be updated to use the new buffer-cache API where that better matches their responsibility, while preserving current behavior.
- [ ] **REQ-008**: Buffer cache state shall continue to support immediate synchronous reads for visible buffer lines while background refresh is still pending.
- [ ] **REQ-009**: Buffer cache state shall remain isolated per buffer and shall not leak between buffers.

## Non-Functional Requirements
- [ ] **NFR-001**: The refactor shall preserve current editor behavior for syntax highlighting and indent-aware features.
- [ ] **NFR-002**: The refactor shall not require a change to the single-worker background job model.
- [ ] **NFR-003**: Buffer cache cloning and snapshot restoration shall remain inexpensive enough for undo/redo and worker handoff.
- [ ] **NFR-004**: Public cache-facing types and methods shall have documentation comments where they are introduced or changed.
- [ ] **NFR-005**: The implementation shall include regression coverage for cache restoration and cache refresh behavior.

## Acceptance Criteria
- [ ] **AC-001**: A buffer can hold syntax cache and indent-scope cache through a `BufferCache` container instead of through `SyntaxCache` internals.
- [ ] **AC-002**: After a text edit, the relevant buffer cache entries become stale and can be refreshed again by the worker.
- [ ] **AC-003**: Undo and redo restore both syntax-related cache data and indent-scope cache data from the snapshot associated with the restored text.
- [ ] **AC-004**: The background worker continues to produce correct cache results for the active buffer after the refactor.
- [ ] **AC-005**: No user-visible syntax highlighting or indent-guide regressions are introduced by the ownership change.

## Out of Scope
- Introducing new cache types beyond the buffer-level container needed for this refactor.
- Changing the worker scheduling model, concurrency model, or job priority policy.
- Redesigning syntax tokenization rules or indent-scope computation algorithms.
- Changing the user-facing configuration surface unless a cache-related call site requires it incidentally.

## Assumptions
- The current single background worker model remains in place.
- `SyntaxCache` continues to represent syntax-specific state and can be reused inside `BufferCache`.
- The buffer cache container will be the long-term home for additional buffer-derived caches as needed.
- Existing behavior for syntax highlighting and indent-aware rendering should stay functionally equivalent after the refactor.

## Dependencies
- The buffer history and undo/redo code paths that currently snapshot syntax cache state.
- The background job framework that schedules cache catch-up work.
- Existing syntax highlighting tests and indent-scope tests for regression coverage.
- The glossary entries for `Buffer Cache`, `Buffer Cache Worker`, and `Syntax Cache`.
