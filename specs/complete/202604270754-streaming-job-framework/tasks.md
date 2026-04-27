# Streaming Job Framework - Implementation Tasks

## Overview
Refactor the shared job framework to support streaming jobs with `start` / `chunk` / `complete` delivery and explicit best-effort abort by generation, while preserving the existing one-shot API. Migrate the file picker to the streaming job path and remove its dedicated background thread. Total: 8 tasks.

## Job Framework
- [x] **1.** Add streaming job core types and submission APIs (test: unit tests for type behavior and API compatibility)
  - [x] **1.1** Add `StreamingJob` and `StreamingJobEvent` types for ordered event emission.
  - [x] **1.2** Add `JobManager::submit_streaming()` without changing the existing one-shot submission methods.
  - [x] **1.3** Add `JobManager::abort_generation()` for best-effort abort by generation.
  - [x] **1.4** Preserve existing one-shot job behavior and tests while adding the streaming API.

- [x] **2.** Extend the worker and completion path for streaming delivery (test: unit tests for `start`, `chunk`, `complete`, and abort handling)
  - [x] **2.1** Update the worker to accept and execute streaming jobs on the shared background thread.
  - [x] **2.2** Forward streaming events to the main-thread completion queue in order.
  - [x] **2.3** Reject stale or aborted streaming generations on the acceptance path.
  - [x] **2.4** Preserve redraw signaling and existing completion polling semantics.

## Picker Migration
- [x] **3.** Convert the file picker to a streaming job consumer (test: integration tests for chunked search and abort-on-query-change)
  - [x] **3.1** Replace the picker’s dedicated background thread with a streaming job submission.
  - [x] **3.2** Abort the previous generation when the picker query changes.
  - [x] **3.3** Keep incrementally updating the picker as chunks arrive.
  - [x] **3.4** Preserve stale-result rejection and selection behavior.

- [x] **4.** Remove picker-specific threading and stale-thread cleanup (test: integration tests that the picker no longer spawns its own worker)
  - [x] **4.1** Remove the dedicated picker thread spawn path.
  - [x] **4.2** Remove now-unused picker thread state and cleanup code.

## UI and Integration
- [x] **5.** Keep picker and layout integration stable (test: regression tests for open, cancel, select, and redraw behavior)
  - [x] **5.1** Ensure the picker still opens from `F1` and closes from `Esc` / `Ctrl-C`.
  - [x] **5.2** Ensure `Enter` / `Ctrl-Y` still select the highlighted file.
  - [x] **5.3** Ensure `Layout` still opens or focuses the selected file tab.

## Testing and Verification
- [x] **6.** Add unit tests for streaming job framework behavior (test: targeted job framework tests)
  - [x] **6.1** Test streaming event ordering and multi-chunk delivery.
  - [x] **6.2** Test generation abort behavior and stale-result rejection.
  - [x] **6.3** Test one-shot job API compatibility remains intact.

- [x] **7.** Add integration tests for picker search through the shared job framework (test: picker and layout integration tests)
  - [x] **7.1** Verify chunked file search still updates the picker incrementally.
  - [x] **7.2** Verify aborting a previous query prevents old results from winning.
  - [x] **7.3** Verify picker selection still opens or focuses the file tab.

- [x] **8.** Run project quality gates after the refactor (test: `cargo fmt`, `cargo check`, and `cargo test`)
  - [x] **8.1** Run `cargo fmt` and fix formatting issues.
  - [x] **8.2** Run `cargo check` and resolve build or warning issues.
  - [x] **8.3** Run the full test suite.

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Job Framework | 2 | 2 | 100% |
| Picker Migration | 2 | 2 | 100% |
| UI and Integration | 1 | 1 | 100% |
| Testing and Verification | 3 | 3 | 100% |
| **Total** | **8** | **8** | **100%** |
