# Undo/Redo - Implementation Tasks

## Overview

Total: 28 tasks
Estimated completion: 1 day
Prerequisites: None (imbl crate already in use)

## Data Structures

- [x] **1.** Create `Snapshot` struct in buffer.rs
  - [x] **1.1** Add `lines: Vector<Arc<str>>` field
  - [x] **1.2** Add `cursor: Cursor` field
  - [x] **1.3** Add `#[derive(Debug, Clone)]`

- [x] **2.** Create `UndoState` struct in buffer.rs
  - [x] **2.1** Add `history: Vector<Snapshot>` field
  - [x] **2.2** Add `position: usize` field
  - [x] **2.3** Add `#[derive(Debug, Clone)]`
  - [x] **2.4** Implement `new() -> Self`
  - [x] **2.5** Implement `push_snapshot()` (test: see Interaction Flows in design)
  - [x] **2.6** Implement `update_cursor()` (test: see Interaction Flows in design)
  - [x] **2.7** Implement `undo() -> Option<(Vector<Arc<str>>, Cursor)>` (test: basic undo/redo)
  - [x] **2.8** Implement `redo() -> Option<(Vector<Arc<str>>, Cursor)>` (test: basic undo/redo)
  - [x] **2.9** Implement `can_undo() -> bool`
  - [x] **2.10** Implement `can_redo() -> bool`
  - [x] **2.11** Implement `clear()`

## Buffer Integration

- [x] **3.** Add `undo_state: Option<UndoState>` field to `Buffer` struct

- [x] **4.** Add public undo/redo methods to `Buffer`
  - [x] **4.1** `undo(&mut self) -> bool` (test: undo restores previous state)
  - [x] **4.2** `redo(&mut self) -> bool` (test: redo restores next state)
  - [x] **4.3** `can_undo(&self) -> bool`
  - [x] **4.4** `can_redo(&self) -> bool`
  - [x] **4.5** `push_snapshot(&mut self, cursor: Cursor)` (test: snapshot creation)
  - [x] **4.6** `update_cursor(&mut self, cursor: Cursor)` (test: cursor tracking)

## Action Enum Changes

- [x] **5.** Add `Undo` and `Redo` variants to `Action` enum in editor.rs

- [x] **6.** Implement `is_snapshottable(&self) -> bool` on `Action`
  - [x] **6.1** Return `true` for `SwitchToInsert` and `SwitchToNormal`
  - [x] **6.2** Return `true` for text-modifying actions (Delete*, Change*, Join*, Append*, InsertAt*, Open*)
  - [x] **6.3** Return `false` for `InsertChar`
  - [x] **6.4** Return `false` for `Undo` and `Redo`
  - [x] **6.5** Return `false` for movement actions
  - [x] **6.6** Handle `Count` by delegating to inner action

- [x] **7.** Implement `updates_snapshot_cursor(&self) -> bool` on `Action`
  - [x] **7.1** Return `true` for all movement actions
  - [x] **7.2** Handle `Count` by delegating to inner action
  - [x] **7.3** Return `false` for all other actions

## Key Bindings

- [x] **8.** Add key bindings in NormalMode
  - [x] **8.1** Bind `u` to `Action::Undo` (test: pressing u triggers undo)
  - [x] **8.2** Bind `U` (Shift+u) to `Action::Redo` (test: pressing U triggers redo)

## Window Integration

- [x] **9.** Modify `Window::process_action()` to handle Undo/Redo
  - [x] **9.1** Handle `Action::Undo` - call `buffer.undo()` (test: u key reverts buffer)
  - [x] **9.2** Handle `Action::Redo` - call `buffer.redo()` (test: U key restores buffer)

- [x] **10.** Modify action processing to call snapshot/cursor methods
  - [x] **10.1** Before processing action, check `action.is_snapshottable()` - if true, call `buffer.push_snapshot(current_cursor)` (test: insert-exit-undo cycle)
  - [x] **10.2** After processing action, check `action.updates_snapshot_cursor()` - if true, call `buffer.update_cursor(new_cursor)` (test: cursor position restored after undo)

## Testing

- [x] **11.** Write unit tests for UndoState
  - [x] **11.1** Test empty undo/redo returns None
  - [x] **11.2** Test push_snapshot creates snapshot
  - [x] **11.3** Test undo restores previous state
  - [x] **11.4** Test redo restores next state
  - [x] **11.5** Test text deduplication (same text only updates cursor)
  - [x] **11.6** Test update_cursor updates active snapshot
  - [x] **11.7** Test can_undo/can_redo return correct values

- [x] **12.** Write unit tests for Action methods
  - [x] **12.1** Test is_snapshottable for all action types
  - [x] **12.2** Test updates_snapshot_cursor for all action types

- [x] **13.** Write integration tests
  - [x] **13.1** Test insert session undo (i -> type -> Esc -> u)
  - [x] **13.2** Test cursor preservation after undo
  - [x] **13.3** Test redo after undo
  - [x] **13.4** Test new edit clears redo history
  - [x] **13.5** Test multiple undos
  - [x] **13.6** Test multiple redos

## Documentation

- [x] **14.** Update docs/motions.md if needed (unlikely) - Not needed, undo/redo is not a motion

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Data Structures | 11 | 11 | 100% |
| Buffer Integration | 6 | 6 | 100% |
| Action Enum | 3 | 3 | 100% |
| Key Bindings | 2 | 2 | 100% |
| Window Integration | 2 | 2 | 100% |
| Testing | 3 | 3 | 100% |
| Documentation | 1 | 1 | 100% |
| **Total** | **28** | **28** | **100%** |
