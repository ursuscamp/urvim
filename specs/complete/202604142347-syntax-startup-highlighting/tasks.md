# Syntax Startup Highlighting - Implementation Tasks
## Overview
Implement startup-driven syntax warmup so every loaded buffer gets highlighted immediately or in the background, while keeping the first visible top-of-file paint synchronous and preserving the existing `Arc<str>` line storage model.

## Backend
- [x] **1.** Add startup warmup policy for all loaded buffers in the buffer pool.
  - [x] **1.1** Identify the startup path that already has access to every loaded buffer and hook warmup scheduling into it.
  - [x] **1.2** Queue background syntax catch-up for every loaded buffer that is not visible at startup.
  - [x] **1.3** Ensure startup warmup does not requeue duplicate background jobs for the same syntax generation.

- [x] **2.** Add synchronous first-paint warmup for visible buffers starting at line 0.
  - [x] **2.1** Detect the initial visible buffer range before the first render.
  - [x] **2.2** Synchronously warm only the visible top-of-file prefix when the viewport begins at the first line.
  - [x] **2.3** Keep the synchronous path bounded so it does not expand into a full-file scan.
  - [x] **2.4** Promote the visible buffer to foreground-priority catch-up after the synchronous prefix is ready so it warms ahead of hidden buffers.

- [x] **3.** Keep syntax warmup memory-efficient with the existing line representation.
  - [x] **3.1** Reuse existing `Arc<str>` line handles or borrowed references for startup warmup input where possible.
  - [x] **3.2** Avoid introducing new owned per-line copies solely for syntax warmup.
  - [x] **3.3** Keep background syntax snapshots in the persistent line container form where practical instead of flattening them into `Vec<Arc<str>>`.
  - [x] **3.4** Verify the buffer syntax cache and background job snapshot still clone only the handles they need.

## Testing
- [x] **4.** Add regression tests for startup syntax warmup behavior.
  - [x] **4.1** Verify that a loaded but hidden buffer receives background syntax catch-up at startup.
  - [x] **4.2** Verify that a visible buffer starting at line 0 has cached syntax for the first visible lines before the first render-oriented access.
  - [x] **4.3** Verify that stale warmup results are ignored after a buffer edit or invalidation.
  - [x] **4.4** Verify that syntax-disabled configuration still skips startup warmup work.

- [x] **5.** Run project checks and formatting.
  - [x] **5.1** Run targeted tests for buffer and window syntax behavior.
  - [x] **5.2** Run `cargo check` to confirm the build and warnings stay clean.
  - [x] **5.3** Format the code after implementation.

## Completion Summary
| Item | Status |
| --- | --- |
| Startup warmup policy | Done |
| Synchronous first-paint warmup | Done |
| Memory-efficiency validation | Done |
| Regression tests | Done |
| Build and formatting checks | Done |
