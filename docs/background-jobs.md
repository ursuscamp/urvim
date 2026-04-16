# Background Jobs

urvim includes a small internal job framework for deferred editor work that should not block the main input/render loop.

The current design is intentionally simple:

- one serial worker thread
- priority-aware queueing
- latest-only submissions that can supersede older queued work for the same scope
- generation tokens to reject stale results
- a completion queue drained by the main thread
- a tick-based wakeup path so completed work can surface without extra input

## Core Types

- `JobHandle` owns the worker thread and the completion channel.
- `JobManager` sits on the main thread, tracks the latest generation per job kind, and filters stale completions.
- `JobKind` labels the work being done, such as syntax catch-up for a particular buffer.
- `JobToken` carries the generation number used to reject stale work.
- `JobPriority` distinguishes foreground work from lower-priority maintenance work.
- `JobSubmissionMode::LatestOnly` allows the queue to drop older queued jobs before they run.

## How It Is Used

The syntax highlighter is the first built-in user of this framework:

1. The editor renders whatever cached syntax spans it already has.
2. If the cache is incomplete, it queues a background catch-up job.
3. The worker computes the rest of the cache off-thread.
4. The main thread applies the result only if the generation still matches.
5. A tick event gives the editor loop a chance to repaint with the updated data.
6. When a newer latest-only submission arrives for the same scope, older queued work is skipped before it can consume more worker time.

## Extension Point

This framework is meant to host future deferred work that does not need to run inline with user input, such as:

- indexing
- cache warming
- diagnostics refresh
- file scans

When adding a new job type, prefer keeping the job payload self-contained and versioned so stale completions can be dropped safely. If the work is replaceable, submit it as latest-only so older queued jobs can be pruned before execution.
