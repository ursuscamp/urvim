# Syntax Cache Undo Snapshots

## Summary
Improve editor responsiveness and correctness by making syntax-related cache snapshots restorable across undo and redo, while reducing cache snapshot copy costs for main-thread and worker-thread handoff.

## Problem Statement
Undo and redo currently focus on text state restoration, but syntax-related cache restoration is not guaranteed to be immediate and cheap. This can cause temporary syntax mismatch after history navigation and adds avoidable copy overhead when cache snapshots move between threads.

## User Stories
- As an editor user, I want undo to restore syntax state immediately so that highlighted text always matches restored content.
- As an editor user, I want redo to restore syntax state immediately so that replayed history remains visually correct.
- As a maintainer, I want syntax cache snapshots to clone cheaply so that background processing and state handoff do not add unnecessary latency.

## Functional Requirements
- [ ] **REQ-001**: Undo history entries must capture and retain syntax-related cache state associated with the corresponding text snapshot.
- [ ] **REQ-002**: Redo history traversal must restore syntax-related cache state from history entries in the same way as undo traversal.
- [ ] **REQ-003**: Restoring an undo or redo entry must re-establish syntax-related cache state so visible highlighting is immediately consistent with restored text.
- [ ] **REQ-004**: Syntax cache storage types must use persistent vector-backed structures (`imbl::Vector`) instead of `Vec`.
- [ ] **REQ-005**: `IndentScopeCache` storage types must use persistent vector-backed structures (`imbl::Vector`) instead of `Vec`.
- [ ] **REQ-006**: The storage-type migration in this feature must be limited to cache storage internals and must not broaden unrelated API or payload contracts.

## Non-Functional Requirements
- Performance: Undo and redo operations that restore cached syntax state must reduce visible restore latency versus current behavior.
- Performance: Cache snapshot cloning for main-thread/worker-thread handoff must reduce copy overhead versus current behavior.
- Reliability: Restored cache state must deterministically match restored buffer text across repeated undo/redo sequences.
- Compatibility: Existing undo/redo semantics for text content and cursor behavior must remain unchanged.

## Acceptance Criteria
- [ ] **AC-001**: Given a buffer with syntax-highlighted content and completed edits, when undo is executed, then text state and syntax-related cache state are both restored and highlighting is immediately correct.
- [ ] **AC-002**: Given a previously undone state, when redo is executed, then text state and syntax-related cache state are both restored and highlighting is immediately correct.
- [ ] **AC-003**: Given repeated undo/redo traversal across multiple history entries, when each step is applied, then syntax highlighting always matches the restored text for that step without waiting for a later catch-up cycle.
- [ ] **AC-004**: Given cache snapshot capture and restore flows, when cloning storage snapshots, then cloning cost is measurably lower than equivalent `Vec`-based cloning in representative editor workloads.
- [ ] **AC-005**: Given non-cache interfaces touched by this feature area, when compiling and running existing behavior checks, then no unrelated API or payload surface expansion is introduced by the vector migration.

## Out of Scope
- Redesigning syntax parsing or highlighting algorithms.
- Broad migration of non-cache editor data structures from `Vec` to `imbl::Vector`.
- New user-facing configuration flags for undo/redo cache restoration behavior.
- Updating completed historical specs.

## Assumptions
- Undo/redo history can carry additional snapshot metadata without changing user-visible command semantics.
- Syntax cache and indent-scope cache snapshots are sufficient to make restored highlighting immediately consistent with restored text.
- Performance validation can be done using representative local workloads or targeted benchmarks in the codebase.

## Dependencies
- Existing undo/redo history model and snapshot lifecycle.
- Syntax cache and `IndentScopeCache` ownership and restore pathways.
- Main-thread and worker-thread coordination paths that exchange syntax-related cache snapshots.
