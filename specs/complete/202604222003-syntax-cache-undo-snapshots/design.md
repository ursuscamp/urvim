# Syntax Cache Undo Snapshots - Technical Design

## Architecture Overview
This feature extends editor history snapshots so syntax-related cache state is restored together with text state during undo and redo traversal. It also migrates cache storage internals from `Vec` to `imbl::Vector` to make snapshot cloning cheap across main-thread and worker-thread boundaries.

The design keeps undo/redo command semantics unchanged from the user perspective and limits persistent-vector migration to cache storage types only.

## Interface Design
### Undo/redo snapshot lifecycle
- Snapshot capture interface (existing history entry creation path):
  - Input: current buffer text snapshot and current syntax-related cache snapshot.
  - Output: history node containing both text snapshot and cache snapshot.
- Undo traversal interface:
  - Input: request to move to previous history node.
  - Output: restored text snapshot and restored cache snapshot applied atomically.
- Redo traversal interface:
  - Input: request to move to next history node.
  - Output: restored text snapshot and restored cache snapshot applied atomically.

Constraints:
- Cache restore is history-driven for both undo and redo.
- Cache restore does not require waiting for asynchronous syntax catch-up to become visually correct.

### Cache storage scope boundary
- Migration to `imbl::Vector` is limited to cache storage internals.
- Existing non-cache API contracts and worker message surface remain unchanged unless required for compatibility with storage internals.

## Data Models
### History entry augmentation
- History entry data model gains syntax-related cache snapshot fields associated with each text snapshot.
- Snapshot fields include syntax cache state and indent-scope cache state required for immediate highlight consistency.

### Cache storage model
- Syntax cache internal collection fields migrate from `Vec<T>` to `imbl::Vector<T>`.
- `IndentScopeCache` internal collection fields migrate from `Vec<T>` to `imbl::Vector<T>`.

Constraints:
- Field semantics and logical ordering remain equivalent to prior `Vec` representation.
- Clone operations must preserve structural sharing behavior expected from persistent vectors.

## Key Components
### History / undo subsystem
Responsibilities:
- Capture cache snapshot together with text snapshot at history boundaries.
- Restore both text and cache snapshot on undo and redo.

Public behavior:
- Undo/redo outcome remains one state transition per command invocation.
- Restored state is immediately renderable with consistent syntax highlighting.

Dependencies:
- Buffer snapshot ownership model.
- Syntax cache and indent-scope cache snapshot providers.

### Syntax cache subsystem
Responsibilities:
- Expose cloneable cache snapshots suitable for history capture.
- Restore snapshot state when history applies undo/redo entries.

Public behavior:
- Cached highlight lookups continue to behave identically for equivalent logical content.

Dependencies:
- Indent scope cache data.
- Existing syntax catch-up pipeline.

### Indent scope cache subsystem
Responsibilities:
- Maintain scope membership/index structures using `imbl::Vector` storage internals.
- Support snapshot/restore semantics required by history restore flow.

Public behavior:
- Scope lookup behavior remains consistent with existing logic.

Dependencies:
- Syntax rebuild/invalidation lifecycle.

### Main/worker handoff path
Responsibilities:
- Continue passing syntax-related snapshot state across thread boundary with lower clone/copy overhead from persistent storage internals.

Public behavior:
- No user-visible protocol changes.

Dependencies:
- Existing background worker framework and job payload serialization/cloning boundaries.

## User Interaction
- After undo, syntax highlighting should already match restored text without visible mismatch windows.
- After redo, syntax highlighting should already match restored text without visible mismatch windows.
- Undo/redo key behavior and editor mode transitions remain unchanged.

## External Dependencies
- Existing `imbl` crate already used by the project (no new third-party dependency expected).
- Existing background worker framework and syntax catch-up system.

## Error Handling
- If a history entry lacks cache snapshot data (e.g., legacy/test-constructed entry), fallback behavior should trigger existing syntax invalidation/rebuild path and still restore text state.
- If restored cache snapshot is structurally incompatible with current buffer shape, treat cache snapshot as invalid, discard it, and trigger normal rebuild safeguards.
- History traversal failures must not leave partially-applied state; restore remains atomic for text plus cache where available.

## Security
- No new network, file I/O, or command execution paths.
- No new privilege boundaries.
- Data handled remains in-process editor state.

## Configuration
No new configuration options.

## Component Interactions
1. Editor commits an undo snapshot boundary.
2. History subsystem captures:
   - text snapshot
   - syntax cache snapshot
   - indent-scope cache snapshot
3. User invokes undo.
4. History subsystem resolves previous node and applies text + cache snapshots atomically.
5. Renderer consumes restored cache immediately for consistent highlighting.
6. User invokes redo.
7. History subsystem resolves next node and applies text + cache snapshots atomically.
8. Worker catch-up pipeline continues as normal for future edits, with reduced copy overhead from persistent cache storage internals.

## Platform Considerations
- Feature is platform-agnostic within supported terminal targets.
- No terminal protocol changes required.
- Threading model assumptions remain aligned with current Rust runtime and editor worker architecture.
