# Background Jobs

urvim includes a small internal job framework for deferred editor work that should not block the main input/render loop.

## Core Types

- `JobHandle` owns the worker thread pool and the completion channel.
- `JobManager` sits on the main thread, tracks the latest generation per job kind, and filters stale events.
- `BackgroundJob` is the enum of built-in background work.
- `JobPayload` is the enum of outputs produced by background work.
- `JobKind` labels the work being done.
- `JobToken` carries the generation number used to reject stale work.
- `` describes whether a job produces one final result or streamed chunks.
- `JobEvent` carries lifecycle events back to the main thread.
- `JobContext` lets a job check whether it is current, aborted, or shutting down.

## Current built-in jobs

- buffer syntax cache refresh
- buffer indent scope cache refresh
- file picker search
- live grep search
- picker preview syntax refresh

## How it works

1. Main-thread code submits a `BackgroundJob` through `JobManager` or `JobHandle`.
2. Submission tags the job with a `JobKind` and `JobToken`.
3. `JobHandle` queues the job and wakes the worker.
4. The worker builds a `JobContext` and runs the concrete job variant.
5. The job emits `JobEvent` values back to the main thread.
6. The main thread accepts only events whose generation still matches.
7. Accepted work can request a redraw.

## Notes

- `JobSubmissionMode::LatestOnly` prunes older queued work for the same kind.
- Streaming jobs should check `JobContext::is_aborted()` and stop early.
- The framework uses a thread pool (default: 4 workers) so independent jobs like syntax refresh and indent scope refresh can run in parallel.
- `JobHandle::new()` creates 1 worker (used in tests); production code goes through `JobManager::new()` which creates 4 workers.
