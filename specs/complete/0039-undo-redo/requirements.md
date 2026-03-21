# Undo/Redo

## Summary

Implement undo and redo functionality for text buffers using a snapshot-based approach. Each buffer stores its own undo history as a list of clones, allowing cheap snapshots due to the `imbl::Vector<Arc<str>>` data structure.

## Problem Statement

Currently, the editor lacks any undo or redo capability. Users cannot revert accidental text changes, making the editor unreliable for serious editing work. The editor needs a way to go back to previous states of the buffer text.

## User Stories

- **As a** user, **I want to** press `u` to undo my last change **so that** I can revert accidental edits.
- **As a** user, **I want to** press `U` to redo a previously undone change **so that** I can restore changes I had undone.
- **As a** user, **I want each buffer** to maintain its own undo history **so that** undoing in one file doesn't affect another.

## Functional Requirements

- [ ] **REQ-001**: Buffer shall store a history of text snapshots (Vector clones)
- [ ] **REQ-002**: Undo action shall restore the previous snapshot and advance the undo position backward
- [ ] **REQ-003**: Redo action shall restore the next snapshot and advance the undo position forward
- [ ] **REQ-004**: Normal mode shall bind `u` key to trigger Undo action
- [ ] **REQ-005**: Normal mode shall bind `U` key (Shift+u) to trigger Redo action
- [ ] **REQ-006**: New changes after an undo shall clear any redo history (standard undo behavior)
- [ ] **REQ-007**: Taking a snapshot shall occur before any text-modifying operation
- [ ] **REQ-008**: Undo/Redo shall preserve cursor position as it was at the time of the snapshot
- [ ] **REQ-009**: First keystroke after mode switch shall trigger a snapshot before modification

## Non-Functional Requirements

- **Performance**: Cloning `imbl::Vector<Arc<str>>` is O(1) due to `Arc` internals - it only increments a reference count without copying data
- **Memory**: Each snapshot shares immutable data with previous snapshots via `Arc`, so memory usage is proportional to changes rather than full file size

## Acceptance Criteria

- [ ] **AC-001**: Typing "hello" then pressing `u` results in empty buffer
- [ ] **AC-002**: After undo, pressing `U` restores "hello"
- [ ] **AC-003**: After undo then new typing "x", pressing `u` results in "hello" (redo lost)
- [ ] **AC-004**: Pressing `u` in an empty buffer (nothing to undo) has no effect
- [ ] **AC-005**: Pressing `U` when at the latest state (nothing to redo) has no effect
- [ ] **AC-006**: Multiple undos (e.g., "hello", "world" typed separately, then two `u`) restores first state
- [ ] **AC-007**: Multiple redos work correctly after multiple undos
- [ ] **AC-008**: Each buffer has independent undo history

## Out of Scope

- Vim's "undo branch" concept - we only have linear undo/redo
- Undo across sessions (persistence)
- Unlimited undo stack (practical limit may be imposed)
- `Ctrl-r` as redo (only `U` is specified)

## Assumptions

- The `imbl::Vector<Arc<str>>` data structure clones in O(1) time since it only increments `Arc` reference counts
- Snapshot shall be taken before text-modifying actions
- Cursor position does not need to be restored during undo/redo (only text state)

## Dependencies

- **Internal**: Buffer text operations (insert_text, remove, delete_lines, etc.)
- **External**: `imbl` crate (already in use)
