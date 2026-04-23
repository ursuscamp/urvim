# Syntax Cache Undo Snapshots - Implementation Tasks

## Overview
Implement history-backed syntax cache restoration for undo/redo and migrate syntax cache storage internals to `imbl::Vector` for lower snapshot clone overhead between main and worker threads.

## Core Implementation
- [x] **1.** Extend history snapshots to include syntax-related cache state.
  - [x] **1.1** Identify and document current undo/redo snapshot capture and restore boundaries in the editor/history subsystem.
  - [x] **1.2** Add syntax cache snapshot fields to history entries.
  - [x] **1.3** Add indent-scope cache snapshot fields to history entries.
  - [x] **1.4** Capture cache snapshots at undo boundary creation points without changing existing text/cursor history semantics.
  - [x] **1.5** Restore cache snapshots atomically with text snapshots during undo traversal.
  - [x] **1.6** Restore cache snapshots atomically with text snapshots during redo traversal.

- [x] **2.** Migrate cache storage internals from `Vec` to `imbl::Vector`.
  - [x] **2.1** Convert syntax cache internal vector-backed fields to `imbl::Vector` while preserving ordering semantics.
  - [x] **2.2** Convert `IndentScopeCache` internal vector-backed fields to `imbl::Vector` while preserving lookup behavior.
  - [x] **2.3** Keep migration scoped to cache storage internals only (no broad non-cache API/payload expansion).
  - [x] **2.4** Update constructors/default paths and internal mutation helpers for persistent vector behavior.

- [x] **3.** Integrate restore fallback safeguards.
  - [x] **3.1** Define behavior when cache snapshot data is absent in a history entry (fallback to existing rebuild path).
  - [x] **3.2** Validate cache snapshot compatibility with restored text state before application.
  - [x] **3.3** Ensure restore failure paths do not leave partially applied state.

## Testing
- [x] **4.** Add and update regression tests for history and cache behavior.
  - [x] **4.1** Add undo regression test proving syntax state is immediately consistent after undo.
  - [x] **4.2** Add redo regression test proving syntax state is immediately consistent after redo.
  - [x] **4.3** Add multi-step undo/redo traversal regression coverage for syntax consistency at each step.
  - [x] **4.4** Add tests covering history entries with missing/legacy cache snapshot data and fallback behavior.
  - [x] **4.5** Add tests validating `IndentScopeCache` snapshot/restore correctness after vector migration.

- [x] **5.** Validate the migration on shared editor pathways.
  - [x] **5.1** Run `cargo fmt`.
  - [x] **5.2** Run `cargo check`.
  - [x] **5.3** Run the targeted undo/redo regression and the full library test suite.

## Verification
- [x] **6.** Run project quality gates and confirm behavior.
  - [x] **6.1** Run `cargo fmt`.
  - [x] **6.2** Run `cargo check`.
  - [x] **6.3** Run targeted tests for undo/redo and syntax cache modules.
  - [x] **6.4** Run full test suite if targeted changes touch shared editor pathways.

## Completion Summary
| Metric | Value |
| --- | --- |
| Total Tasks | 6 |
| Completed | 6 |
| Remaining | 0 |
| Progress | 100% |
