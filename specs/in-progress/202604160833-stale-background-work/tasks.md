# Stale Background Work - Implementation Tasks
## Overview
Add a reusable latest-only cancellation path for background editor work so newer jobs can invalidate older queued work before it burns CPU. Use syntax highlighting as the first consumer, while keeping the last completed highlight visible until fresher work finishes.

## Backend
- [x] **1.** Extend the job framework with replaceable-job cancellation semantics.
  - [x] **1.1** Add a way to identify jobs that should behave as latest-only work for a shared scope, such as a buffer-specific background task.
  - [x] **1.2** Ensure queued stale jobs are skipped before expensive execution, not just rejected after completion.
  - [x] **1.3** Preserve the existing priority ordering, shutdown behavior, and stale-completion filtering for non-replaceable work.
  - [x] **1.4** Add debug logging that records when stale background work is canceled, skipped, or discarded so cancellation behavior is visible in `debug.log`.

- [x] **2.** Wire syntax catch-up through the latest-only cancellation path.
  - [x] **2.1** Update the syntax background request flow so a newer request supersedes any older queued request for the same buffer.
  - [x] **2.2** Keep the last completed syntax cache visible while a newer background result is pending.
  - [x] **2.3** Make sure edits and syntax invalidations resubmit fresh work without leaving old queued jobs eligible to run.

## Testing
- [x] **3.** Add unit tests for latest-only background job cancellation.
  - [x] **3.1** Verify that multiple queued jobs for the same replaceable scope only allow the newest job to execute.
  - [x] **3.2** Verify that non-replaceable jobs still obey the existing FIFO and priority rules.
  - [x] **3.3** Verify that stale queued work never produces an accepted completion.

- [x] **4.** Add regression coverage for syntax highlighting under rapid edits.
  - [x] **4.1** Verify that repeated syntax invalidation only leaves one current background request eligible for the affected buffer.
  - [x] **4.2** Verify that the editor continues showing the last completed syntax state until the newer result arrives.
  - [x] **4.3** Verify that syntax-disabled buffers remain on the existing no-op path.

## Documentation
- [x] **5.** Update the background-work and syntax docs to describe latest-only cancellation.
  - [x] **5.1** Document that background work can be superseded before it starts, not merely discarded after completion.
  - [x] **5.2** Document that syntax highlighting keeps the last good styling visible while fresher background work is pending.
  - [x] **5.3** Note that the same cancellation model is intended for future deferred editor tasks.

## Validation
- [x] **6.** Run project validation for the cancellation change.
  - [x] **6.1** Run `cargo check` and fix any build or warning issues introduced by the change.
  - [x] **6.2** Run the focused job and syntax tests that cover stale-work cancellation and highlight freshness.

## Completion Summary
| Area | Tasks | Status |
|---|---|---|
| Backend | 2 | Complete |
| Testing | 2 | Complete |
| Documentation | 1 | Complete |
| Validation | 1 | Complete |
| Total | 6 | Complete |
