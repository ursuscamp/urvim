# Streaming Job Framework - Technical Design

## Architecture Overview
The streaming job framework extends the existing deferred job system with ordered event emission for long-running tasks. The current job framework remains the execution core for single-result jobs, while a new streaming layer adds `start`, `chunk`, and `complete` event delivery for jobs that produce incremental output.

The file picker becomes the first consumer of the streaming path. Instead of spawning a dedicated background thread, it submits a streaming job to the shared job framework and receives chunked filesystem results through the main-thread completion poll path.

Data flow is:
`F1` -> picker opens -> picker submits streaming search job -> background worker emits `start` -> worker emits one or more `chunk` events -> worker emits `complete` -> main thread polls accepted events -> picker updates UI and ignores stale generations

Key architectural decisions:
- keep the existing one-shot `Job` API intact for current callers
- add a streaming job variant rather than replacing the current model
- reuse the existing generation/token rejection logic so stale picker searches remain cheap to discard
- add an explicit best-effort abort signal keyed by generation so superseded streams can stop quickly
- keep streaming delivery on the same main-thread polling path already used for job completions

## Interface Design

| Interface | Input | Output | Description |
|-----------|-------|--------|-------------|
| `StreamingJob` | job implementation | streaming events | A job that emits `start`, `chunk`, and `complete` events |
| `StreamingJobEvent` | event payload | event wrapper | Represents an emitted streaming event |
| `JobManager::submit_streaming()` | kind, priority, token, job | `Result<(), JobSubmitError>` | Submits a streaming job to the shared worker |
| `JobManager::abort_generation()` | kind, token | `()` | Marks a generation as aborted for best-effort cancellation |
| `JobManager::process_streaming_completed()` | callback | accepted streaming events | Polls and forwards accepted streaming events to the caller |
| `PickerSearchJob` | query, root path, generation | streaming picker events | Filesystem search job used by the file picker |

Proposed signatures:
- `trait StreamingJob { type Event: Send + 'static; fn run(self, context: &JobContext, emit: &mut dyn FnMut(Self::Event)); }`
- `enum StreamingJobEvent { Start, Chunk, Complete }`
- `impl JobManager { pub fn submit_streaming<J>(&self, kind: JobKind, priority: JobPriority, token: JobToken, job: J) -> Result<(), JobSubmitError> where J: StreamingJob; }`
- `impl JobManager { pub fn abort_generation(&self, kind: JobKind, token: JobToken); }`

## Data Models

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `StreamingJobEvent::Start` | unit-like | first event only | Marks the start of a streaming job |
| `StreamingJobEvent::Chunk` | `Box<dyn Any + Send>` or typed payload | zero or more | Carries one incremental payload |
| `StreamingJobEvent::Complete` | unit-like | last event only | Marks job completion |
| `StreamingJobEnvelope.kind` | `JobKind` | required | Identifies the stream type |
| `StreamingJobEnvelope.token` | `JobToken` | required | Generation token used for stale-result rejection |
| `AbortState` | `{ kind, token, aborted }` | runtime-only | Best-effort abort marker for streaming jobs |

Schema changes:
- No persisted data schema changes.
- The job framework gains runtime-only streaming state and event delivery.

## Key Components

### JobManager
**Responsibilities:**
- Submit one-shot jobs using the existing API
- Submit streaming jobs using the new API
- Poll and dispatch accepted job results/events
- Preserve stale-result rejection and redraw signaling
- Register generation aborts for superseded streams

**Public API:**
- `submit(kind, priority, token, job)`
- `submit_latest_only(kind, priority, token, job)`
- `submit_streaming(kind, priority, token, job)`
- `process_completed(on_accepted)`
- `process_streaming_completed(on_accepted)`

### Streaming Job Worker
**Responsibilities:**
- Execute streaming jobs on the shared background worker
- Forward emitted events to the completion queue in order
- Stop delivery for stale or cancelled generations

**Algorithm example:**
```text
run job
  emit Start
  while job has more output
    if context is stale or stopping: emit Complete? no, stop delivery
    emit Chunk(payload)
  emit Complete
```

### PickerSearchJob
**Responsibilities:**
- Walk the filesystem from the current working directory
- Apply case-insensitive file matching
- Emit chunked file results to the picker
- Signal completion when traversal ends

**Dependencies:**
- `ignore::WalkBuilder`
- job framework streaming APIs
- picker state/reconciliation logic

## User Interaction
### Invocation Patterns
- The file picker still opens from `F1`.
- Search still updates as the user types.
- Background results arrive incrementally while the picker remains open.

### Flows
1. User opens the picker.
2. Picker submits a streaming search job to the shared job manager.
3. Job emits `start`.
4. Job emits one or more `chunk` events with partial matches.
5. Job emits `complete`.
6. Main thread polls accepted events and updates picker results.
7. User selects a result or cancels as before.

### Error and Recovery Paths
- If a streaming job is superseded, stale events are rejected by token/generation checks.
- If a streaming job is superseded, the framework marks that generation aborted and the running job should stop when it next observes the abort state.
- If a streaming job fails or is stopped, the picker keeps its last valid results or shows an empty state.
- Existing one-shot jobs continue to use current completion handling.

## External Dependencies
| Dependency | Purpose | Version/Notes |
|------------|---------|---------------|
| `ignore` | gitignore-aware traversal for picker search | existing or added dependency |
| `std::sync::mpsc` | queue event delivery | existing stdlib channel |
| `std::thread` | shared worker thread execution | existing job framework worker |

## Error Handling
| Error Code | Condition | Error Data | Recovery |
|------------|-----------|------------|----------|
| `STREAMING_JOB_STOPPED` | job manager shutting down or generation aborted | `{ kind, token }` | stop emitting, let main thread discard |
| `STREAMING_JOB_STALE` | token/generation superseded | `{ kind, token }` | drop accepted events |
| `STREAMING_JOB_PANIC` | streaming job panics | `{ kind, token }` | surface as failed job event |

Logging requirements:
- start/chunk/complete delivery should be debug-level
- stale event rejection should be debug-level
- job panics should remain warn-level

## Security
- Streaming job payloads are in-process data only.
- No new secrets or external execution paths are introduced.
- File picker search still treats paths as data, not commands.

## Configuration
No new user-facing configuration.

Fixed behavior:
- existing one-shot jobs remain unchanged
- streaming jobs use the same worker lifecycle and shutdown behavior
- picker search is the initial streaming consumer
- abort remains best-effort rather than a hard kill

## Component Interactions
```text
Picker -> JobManager::submit_streaming -> shared worker
shared worker -> emit start/chunk/complete -> completion queue
main thread -> JobManager::process_streaming_completed -> Picker
Picker -> layout intent dispatch -> open file/focus existing tab
```

Streaming events use the same accepted-event filtering model as current jobs so the picker can ignore stale generations without adding a separate ad hoc thread. Abort is best-effort: late chunks may still be produced briefly, but they are discarded if they arrive after abort registration.

## Platform Considerations
| Platform | Consideration | Approach |
|----------|---------------|----------|
| Linux | large directory trees | stream chunks incrementally |
| macOS | cwd/path behavior | use process cwd and native paths |
| Windows | path separator display | preserve native paths in labels |

## Trade-offs
**Decision**: Add streaming as a first-class job mode instead of overloading one-shot completion.

**Reasoning**:
- keeps one-shot callers stable
- makes incremental work explicit
- keeps streaming semantics reusable beyond the picker

**Impact**:
- adds API surface to the job framework
- requires new event handling paths on the main thread

**Decision**: Keep the existing job worker thread.

**Reasoning**:
- avoids duplicate threading infrastructure
- preserves shutdown and generation behavior
- minimizes picker-specific code

**Impact**:
- streaming jobs still share the same worker bottleneck
- long-running streams must be carefully chunked

## Risks and Mitigations
| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Streaming API complicates the job framework | Medium | High | Keep one-shot APIs unchanged and isolate streaming types |
| Picker still receives stale chunks after query changes | Medium | High | Reuse generation/token filtering in acceptance path |
| Shared worker becomes chatty with many chunks | Medium | Medium | Chunk results and keep events coarse enough for the UI |
| Panic handling becomes harder with mid-stream events | Low | Medium | Wrap streaming jobs with the same panic capture path as one-shot jobs |
