# Background Worker Framework - Technical Design
## Architecture Overview
The editor will gain a small internal background job system centered on one serial worker thread. Jobs are submitted by the main editor thread, executed asynchronously in submission order within priority tiers, and return results that the main thread applies only if they are still current.

The first built-in job will be syntax catch-up. Rendering will continue to request syntax spans for the visible viewport immediately, but any remaining highlight work for offscreen lines will be scheduled as background work and computed incrementally afterward.

This design keeps the worker framework general enough for future deferred tasks while avoiding a broad concurrency model up front.

## Interface Design
### Job submission
The framework should expose an internal submission API that can enqueue a job with a priority and a cancellation token.

Conceptually:

```rust
pub enum JobPriority {
    Foreground,
    Background,
}

pub struct JobToken {
    pub generation: u64,
}

pub trait Job {
    type Output;

    fn run(self, context: &JobContext) -> Self::Output;
}

pub struct JobHandle;

impl JobHandle {
    pub fn submit<J>(&self, priority: JobPriority, token: JobToken, job: J);
    pub fn poll_completion(&self) -> Option<JobEvent>;
}
```

### Completion signaling
The editor loop should be able to observe a completion signal from the worker subsystem and convert it into a repaint opportunity. The exact transport can stay internal, but it must be safe to wake the main loop after new syntax data arrives.

### Syntax job shape
Syntax catch-up should be expressed as a background job that receives:
- a snapshot of the buffer lines needed for scanning
- the current syntax identity
- the earliest invalidated line or equivalent start point
- a generation token tied to the current buffer state

The job returns completed span chunks plus enough metadata for the main thread to verify that the work still applies.

## Data Models
### Background job envelope
Each enqueued job should carry:
- job kind
- priority tier
- cancellation or generation token
- buffered input snapshot or equivalent immutable payload

### Background result envelope
Each completed job should carry:
- originating job kind
- generation token
- result payload
- a monotonic or comparable ordering marker if needed for stale-result detection

### Syntax catch-up state
Syntax catch-up needs a per-buffer generation or version number that increments when edits or syntax changes invalidate prior work. That version is used to reject stale worker output.

## Key Components
### Background scheduler
Responsibilities:
- own the worker thread and its queue
- order jobs by priority and then FIFO within each tier
- provide a wakeup/completion signal for the main loop
- keep stale jobs from applying after invalidation

Dependencies:
- standard library synchronization primitives
- existing editor globals and buffer ownership patterns

### Syntax catch-up job
Responsibilities:
- compute highlight spans for lines beyond the initially visible viewport
- reuse the existing tokenizer and syntax registry
- stop or restart cleanly when the tracked buffer generation changes

Dependencies:
- `src/buffer/syntax.rs`
- `src/syntax/`

### Main-thread result applier
Responsibilities:
- receive worker completions
- verify generation/token compatibility
- merge accepted results into live buffer or cache state
- request a redraw when new visible styling becomes available

Dependencies:
- the main event loop in `src/main.rs`
- rendering and buffer access code in `src/window/`

## User Interaction
There should be no new direct user command for the worker framework in v1.

Visible behavior:
- visible content appears immediately
- offscreen syntax fill-in may lag slightly behind for large files
- after the worker finishes a chunk, the screen updates automatically on the next repaint opportunity

## External Dependencies
No new external crates are required if the current standard library synchronization and threading primitives are sufficient.

## Error Handling
Expected failure cases:
- a worker job sees stale buffer state and is ignored
- a background job cannot acquire the state it needs and should fail without crashing the editor
- a result arrives after the buffer changed and is discarded
- the worker thread exits unexpectedly and the editor falls back to synchronous behavior for that job path or leaves syntax cache behavior unchanged, depending on the failure point

Failures should be contained and observable through existing logging, not surfaced as a user-facing crash unless the current syntax path would already fail on that input.

## Security
The background worker framework does not expand the trust boundary. It processes editor-owned buffer content and internal job metadata only. Shared-state access must remain synchronized and bounded to prevent data races.

## Configuration
No new user-facing configuration is required for the initial version.

Existing syntax enable/disable configuration must still be honored. When syntax highlighting is disabled, syntax catch-up work should not be scheduled.

## Component Interactions
1. The editor renders the visible viewport on the main thread.
2. The syntax path identifies that additional offscreen work is needed.
3. The main thread submits a background job with a generation token and priority tier.
4. The worker thread computes highlight spans for the requested range.
5. The worker returns a completion event.
6. The main thread verifies that the result still matches the active buffer generation.
7. Accepted results are merged into the buffer syntax cache.
8. The editor loop repaints, and the newly available highlight data becomes visible.

## Platform Considerations
The design should remain portable across the supported terminal platforms because it relies only on standard threading and synchronization primitives.

The only notable platform-sensitive behavior is the existing editor event loop timing. The worker framework should integrate with that loop without assuming platform-specific wakeup APIs.
