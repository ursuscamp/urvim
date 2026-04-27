# Streaming Job Framework

## Summary
Add a general-purpose streaming job framework that supports `start`, `chunk`, and `complete` events while keeping the existing one-shot job API intact. The framework must also support explicit best-effort abort by generation so old streams can be stopped when a newer query supersedes them. The first consumer of the new capability will be the file picker, which should move off its dedicated thread and use the shared job system instead.

## Problem Statement
Urvim already has a background job framework, but it only delivers single completion results. That makes it a poor fit for workloads that naturally produce incremental output, such as directory scans for the fuzzy file picker, and it forces those features to invent their own threading and cancellation behavior.

Without a streaming model, future background tasks either remain awkward to implement or duplicate async infrastructure outside the shared job framework. That increases complexity, fragments cancellation behavior, and makes the picker refactor harder to maintain.

## User Stories
- As a user, I want the file picker to stay responsive while searching, so that large directories do not block typing.
- As a developer, I want a shared streaming job abstraction, so that incremental background work can reuse the editor’s existing job system.
- As a developer, I want to keep the existing one-shot job API, so that current background jobs continue working without churn.
- As a user, I want stale picker results to be ignored, so that only the latest query updates the UI.

## Functional Requirements
- [ ] **REQ-001**: The job framework must support a streaming job mode in addition to the existing one-shot job mode.
- [ ] **REQ-002**: A streaming job must emit `start`, `chunk`, and `complete` events in order.
- [ ] **REQ-003**: Streaming jobs must allow multiple `chunk` events before completion.
- [ ] **REQ-004**: The existing one-shot job API must remain available and unchanged for current callers.
- [ ] **REQ-005**: The streaming job framework must preserve the existing job completion polling and redraw signaling model for the main thread.
- [ ] **REQ-006**: The streaming job framework must support explicit abort by generation and expose that state to running jobs.
- [ ] **REQ-007**: Abort behavior must be best-effort, allowing in-flight work to stop as soon as it observes the abort state.
- [ ] **REQ-008**: The streaming job framework must support stale-result rejection using the same generation/token model as existing jobs.
- [ ] **REQ-009**: The file picker must use the streaming job framework instead of a dedicated thread for filesystem scanning.
- [ ] **REQ-010**: The file picker must abort the previous search generation when a new query supersedes it.
- [ ] **REQ-011**: The file picker must receive streamed search chunks from the job framework and update results incrementally.
- [ ] **REQ-012**: The file picker must continue to ignore stale results from superseded searches.
- [ ] **REQ-013**: The refactor must not change the file picker’s visible selection behavior or file-open behavior.

## Non-Functional Requirements
- **Performance**: Streaming results must arrive incrementally without waiting for the entire filesystem scan to finish.
- **Reliability**: The job framework must ignore stale streaming output when a newer generation supersedes it.
- **Compatibility**: Existing non-streaming jobs must continue to compile and behave as before.
- **Usability**: The picker must remain responsive while results stream in.

## Acceptance Criteria
- [ ] **AC-001**: The job framework can run a streaming job that emits `start`, at least one `chunk`, and `complete`.
- [ ] **AC-002**: A streaming job can emit multiple chunks before completion.
- [ ] **AC-003**: Existing one-shot jobs still compile and complete through the current API.
- [ ] **AC-004**: The job framework can mark a generation as aborted and expose that state to running jobs.
- [ ] **AC-005**: The file picker uses the shared job framework for search instead of a dedicated thread.
- [ ] **AC-006**: The file picker aborts the previous search generation when a new query starts.
- [ ] **AC-007**: The file picker continues to drop stale results and only show the latest query’s results.
- [ ] **AC-008**: The file picker’s selection flow remains unchanged when a result is chosen.

## Out of Scope
- Refactoring all existing non-streaming jobs onto the streaming API.
- Changing the picker UI layout or key bindings.
- Adding progress percentages or other richer job lifecycle states beyond `start`, `chunk`, and `complete`.
- Introducing new public job consumers beyond the file picker for the initial implementation.

## Assumptions
- The current job manager can be extended without breaking existing one-shot job users.
- The picker’s dedicated thread can be removed once the shared streaming path is available.
- The existing generation/token model is sufficient to reject stale streaming results and drive best-effort abort.
- The main loop can continue polling the job framework for accepted events and redraw requests.

## Dependencies
- **Internal**: Existing job framework, main loop polling, layout intent dispatch, file picker overlay.
- **External**: None.
- **Blocked by**: Picker refactor from dedicated thread to streaming job consumer.
