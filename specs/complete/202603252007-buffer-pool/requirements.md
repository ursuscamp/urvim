# Global Buffer Pool

## Summary
urvim should store all buffer data in a single global buffer pool and identify buffers by a stable `BufferId` instead of embedding `Buffer` values inside views and windows. Opening or creating a buffer should return a `BufferId`, and repeated opens of the same absolute path should reuse the existing buffer entry.

## Problem Statement
Today, buffer data is owned directly by individual views and windows. That makes the same file easy to load more than once, keeps multiple copies of the same buffer in memory, and ties rendering and editing logic too closely to local buffer ownership. The editor needs a single source of truth for buffer data so that files are deduplicated by path and all buffer I/O flows through one shared pool.

## User Stories
- As a user, I want opening the same file multiple times to reuse the existing buffer, so that the editor does not waste memory or create diverging copies of the file.
- As a user, I want windows to render from the current buffer contents at render time, so that edits are visible everywhere the buffer is referenced.
- As a user, I want new and opened buffers to be addressed by an ID, so that the editor can share buffer state safely across views.

## Functional Requirements
- [ ] **REQ-001**: The editor must assign every buffer a `BufferId` value that is unique within the running process.
- [ ] **REQ-002**: The first generated `BufferId` must be `0`, and each subsequent generated ID must increase monotonically by one.
- [ ] **REQ-003**: The editor must store all buffer contents in a global buffer pool keyed by `BufferId`.
- [ ] **REQ-004**: Buffer views must store only a `BufferId` and view-local state such as cursor position and scroll offset.
- [ ] **REQ-005**: Windows must resolve their buffer through the global buffer pool at render time instead of storing a buffer directly.
- [ ] **REQ-006**: Any action that reads or mutates buffer contents must do so through the global buffer pool.
- [ ] **REQ-007**: Creating a new buffer without a path must return a fresh `BufferId` and register an empty buffer in the pool.
- [ ] **REQ-008**: Opening a buffer from a path must resolve that path to an absolute path before the buffer is created or looked up.
- [ ] **REQ-009**: If the global buffer pool already contains a buffer for the resolved absolute path, opening that path must return the existing `BufferId` instead of creating a duplicate buffer.
- [ ] **REQ-010**: If opening a path fails, the editor must surface the failure instead of creating an unnamed or partial buffer entry.
- [ ] **REQ-011**: Buffer identity must remain stable for the lifetime of the buffer pool entry, even if multiple views reference the same buffer.
- [ ] **REQ-012**: Buffer operations that depend on file metadata, such as file name display and file-backed save behavior, must continue to work when accessed through a `BufferId`.

## Non-Functional Requirements
- **Performance**: Reusing buffers by absolute path should avoid duplicate allocations for the same file.
- **Reliability**: Buffer lookups through the pool must return consistent data for all views referencing the same `BufferId`.
- **Compatibility**: Existing editor workflows for opening files, rendering windows, and editing text should continue to behave as before from the user’s perspective, aside from deduplication.
- **Usability**: The editor should still present the same visible buffer content and cursor behavior after the ownership model changes.

## Acceptance Criteria
- [ ] **AC-001**: Opening the same file twice in the editor returns the same `BufferId`.
- [ ] **AC-002**: A buffer opened from a relative path is deduplicated against the same file opened through a different relative path that resolves to the same absolute path.
- [ ] **AC-003**: A new unsaved buffer receives a distinct `BufferId` and does not reuse an existing file-backed buffer.
- [ ] **AC-004**: Rendering a window after editing a shared buffer shows the updated content in every view that references that buffer.
- [ ] **AC-005**: The editor does not hold more than one in-memory buffer entry for the same absolute file path.
- [ ] **AC-006**: Failed opens do not create entries in the buffer pool.

## Out of Scope
- Persistence of the buffer pool across editor restarts.
- Background garbage collection or automatic eviction of unused buffers.
- Changing the text editing behavior of `Buffer` itself.
- Adding user-visible commands for listing or manually managing buffer IDs.

## Assumptions
- The editor will keep a process-global buffer pool for the lifetime of the application.
- `BufferId` will be a lightweight newtype wrapper around `usize`.
- Absolute path resolution will use the existing path utilities already present in the codebase.
- Rendering and editing code can be updated to fetch buffers from the pool without changing the visible editor behavior.

## Dependencies
- The existing `Buffer` type and its file I/O logic.
- The existing `AbsolutePath` type and path resolution behavior.
- Window and tab-group ownership code that currently stores buffers directly.
