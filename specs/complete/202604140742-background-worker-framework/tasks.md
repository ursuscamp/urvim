# Background Worker Framework - Implementation Tasks
## Overview
Build a reusable internal background worker framework, then wire syntax catch-up through it as the first real job. Keep the change set split by concern so the worker infrastructure, syntax integration, and redraw signaling can be reviewed independently.

## Backend
- [x] **1.** Add a job module that owns a single serial worker thread and a priority-aware job queue.
  - [x] **1.1** Define the internal job envelope, priority tiers, and cancellation token or generation model.
  - [x] **1.2** Implement job submission, worker wakeup, and completion delivery.
  - [x] **1.3** Add shutdown behavior so the worker exits cleanly when the editor stops.
- [x] **2.** Add main-thread result handling for completed jobs.
  - [x] **2.1** Add stale-result checks so old work is ignored after edits or syntax changes.
  - [x] **2.2** Add a redraw notification path so background completion can trigger a repaint.
  - [x] **2.3** Add logging for job start, completion, cancellation, and rejection.
- [x] **3.** Integrate syntax catch-up with the job framework.
  - [x] **3.1** Split the syntax path so visible-line rendering remains immediate while offscreen work is queued separately.
  - [x] **3.2** Submit background syntax work with a generation token tied to the active buffer state.
  - [x] **3.3** Restart or cancel syntax jobs when edits invalidate cached syntax state.
  - [x] **3.4** Keep the existing syntax-disabled path from scheduling background work.

## Event Loop
- [x] **4.** Update the editor event loop so worker completion can surface as a repaint opportunity.
  - [x] **4.1** Add or adapt a non-input tick/event path that can wake the loop for background completions.
  - [x] **4.2** Ensure normal keyboard and resize handling still behaves unchanged.

## Testing
- [x] **5.** Add unit tests for the background worker framework.
  - [x] **5.1** Verify priority ordering and FIFO behavior within each tier.
  - [x] **5.2** Verify cancellation or generation mismatch prevents stale results from being applied.
  - [x] **5.3** Verify worker shutdown does not panic and leaves the editor in a clean state.
- [x] **6.** Add regression tests for syntax catch-up behavior.
  - [x] **6.1** Verify visible-line highlighting still appears immediately for a large buffer.
  - [x] **6.2** Verify background results eventually populate offscreen syntax spans.
  - [x] **6.3** Verify edits invalidate stale background syntax output.
  - [x] **6.4** Verify syntax-disabled mode does not schedule background syntax work.
- [x] **7.** Add coverage for redraw signaling.
  - [x] **7.1** Verify a completed background job causes the next repaint to use the new data.
  - [x] **7.2** Verify the editor does not require extra user input to show completed background syntax work.

## Documentation
- [x] **8.** Update syntax and developer docs to describe the new background catch-up behavior.
  - [x] **8.1** Document that visible syntax is immediate while offscreen highlighting may complete later.
  - [x] **8.2** Document the worker framework as an internal extension point for future deferred tasks.

## Completion Summary
| Area | Status | Notes |
|---|---|---|
| Worker framework | In progress | Core job module and result handling are in place |
| Syntax integration | Complete | Background syntax catch-up is wired through the job framework |
| Event loop wakeup | Complete | Tick events wake the loop for background completions |
| Tests | Complete | Queue, stale-result, syntax catch-up, redraw, and tick coverage added |
| Documentation | Complete | Syntax and deferred-work docs describe the new behavior |
