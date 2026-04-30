# Background Jobs

urvim includes a small internal job framework for deferred editor work that should not block the main input/render loop.

The current design is intentionally simple:

- one serial worker thread
- priority-aware queueing
- latest-only submissions that can supersede older queued work for the same scope
- generation tokens to reject stale results
- one unified job API with `Once` and `Streaming` delivery modes
- ordered `start` / `chunk` / `complete` job events
- best-effort abort by generation for superseded streaming work
- a completion queue drained by the main thread
- a tick-based wakeup path so completed work can surface without extra input

## Core Types

- `JobHandle` owns the worker thread and the completion channel.
- `JobManager` sits on the main thread, tracks the latest generation per job kind, and filters stale events.
- `JobManager` also accepts streaming deliveries and marks generations aborted when newer work supersedes them.
- `JobKind` labels the work being done, such as syntax catch-up for a particular buffer.
- `JobToken` carries the generation number used to reject stale work.
- `JobPriority` distinguishes foreground work from lower-priority maintenance work.
- `JobDelivery::Once` and `JobDelivery::Streaming` describe how job output is delivered.
- `JobSubmissionMode::LatestOnly` allows the queue to drop older queued jobs before they run.
- `Job` describes background work that can emit one or many ordered outputs.
- `JobEvent` carries lifecycle events back to the main thread.

## How It Is Used

The syntax highlighter is one built-in user of this framework:

1. The editor renders whatever cached syntax spans it already has.
2. If the cache is incomplete, it queues a background catch-up job.
3. The worker computes the rest of the cache off-thread.
4. The main thread applies the result only if the generation still matches.
5. A tick event gives the editor loop a chance to repaint with the updated data.
6. When a newer latest-only submission arrives for the same scope, older queued work is skipped before it can consume more worker time.

Picker previews also use the same pattern for syntax warmup:

1. The picker loads raw file contents immediately.
2. If the preview cache is incomplete, it queues a background preview syntax refresh job.
3. The worker fills in the syntax cache off-thread.
4. The preview keeps rendering plain text until the job result arrives.
5. The main thread applies the result only if the generation still matches.

Streaming jobs follow the same main-thread polling model as one-shot jobs, but they can deliver progress incrementally:

1. The worker emits `Started` first.
2. The worker emits one or more `Chunk` events while it runs.
3. The worker emits `Completed` when the job finishes normally.
4. The main thread applies only the events whose generation still matches.
5. If a newer query supersedes the current one, the old generation is marked aborted and the job stops the next time it observes `JobContext::is_aborted()`.

The file picker is the first built-in streaming consumer:

1. The picker increments its generation when search text changes.
2. The picker submits a streaming file-scan job to the shared job manager.
3. The scan walks the filesystem and emits matching files in chunks.
4. The layout loop drains streaming events and forwards accepted chunks back into the picker.
5. Older generations are aborted and stale chunks are discarded.

## Extension Point

This framework is meant to host future deferred work that does not need to run inline with user input, such as:

- indexing
- cache warming
- diagnostics refresh
- file scans

When adding a new job type, prefer keeping the job payload self-contained and versioned so stale completions can be dropped safely. If the work is replaceable, submit it as latest-only so older queued jobs can be pruned before execution.

For streaming work, also check `JobContext::is_aborted()` periodically and exit early when it becomes true.
