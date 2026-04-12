# Tab Group Owned Jumplist - Implementation Tasks

## Overview

Move jumplist ownership from `Window` to `TabGroup`, keep the existing history semantics intact, and make jump navigation activate or reopen the correct buffer tab before restoring the cursor.

## Backend

- [x] **1.** Move jumplist ownership and recording APIs from `Window` to `TabGroup`.
  - [x] **1.1** Relocate or reuse the jumplist data structure so the tab group owns the active session history. (depends on: design)
  - [x] **1.2** Remove window-level jumplist state and replace it with tab-group-level recording and navigation methods. (depends on: 1.1)
  - [x] **1.3** Update action handling so cursor movement records through the active tab group instead of the active window. (depends on: 1.2)

- [x] **2.** Implement tab resolution and buffer reopening for jumplist playback.
  - [x] **2.1** Add a tab-group helper that finds an already-open tab for a jumplist buffer id. (depends on: design)
  - [x] **2.2** Open a new tab from the existing live buffer when the jumplist target is not currently open. (depends on: 2.1)
  - [x] **2.3** Preserve the active tab group state and tab bar when navigation switches to an existing or reopened destination tab. (depends on: 2.1, 2.2)
  - [x] **2.4** Restore the target cursor through the sync-aware cursor path after the destination tab has been resolved. (depends on: 2.2)

- [x] **3.** Keep the existing jumplist behavior intact after the ownership move.
  - [x] **3.1** Preserve threshold-based refresh, branching, deduplication, and bounded history rules in the moved jumplist logic. (depends on: 1.1)
  - [x] **3.2** Ensure ordinary tab switching does not reset or corrupt jumplist history. (depends on: 1.2)
  - [x] **3.3** Fail safely when a stored jumplist buffer id no longer resolves in the live buffer pool. (depends on: 2.1)

## Testing

- [x] **4.** Add unit tests for tab-group-owned jumplist behavior.
  - [x] **4.1** Verify jumplist recording updates the tab group and not a removed window-local history. (depends on: 1.2)
  - [x] **4.2** Verify jumplist playback selects an already-open tab for the destination buffer. (depends on: 2.1, 2.3)
  - [x] **4.3** Verify jumplist playback reopens a buffer into the tab group when the destination buffer is not currently open. (depends on: 2.2)
  - [x] **4.4** Verify missing live buffers fail safely without changing the active tab selection. (depends on: 3.3)

- [x] **5.** Add integration tests for the user-facing tab navigation flow.
  - [x] **5.1** Verify `Ctrl-O` and `Ctrl-I` move through jump history while activating the correct tab. (depends on: 2.3, 2.4)
  - [x] **5.2** Verify switching tabs does not clear jumplist history. (depends on: 3.2)
  - [x] **5.3** Verify reopened jumplist targets restore their recorded cursor positions after activation. (depends on: 2.4)

## Documentation

- [x] **6.** Update user-facing motion documentation for the new tab-aware jumplist behavior.
  - [x] **6.1** Update `docs/motions.md` so `Ctrl-O` and `Ctrl-I` describe tab-aware jumplist navigation and reopening behavior. (depends on: 2.3)

## Verification

- [x] **7.** Run the focused build and test checks for the affected code paths.
  - [x] **7.1** Run `cargo check` to confirm the refactor builds cleanly. (depends on: 1.2, 2.2)
  - [x] **7.2** Run the jumplist, tab-group, and editor test suites relevant to the new behavior. (depends on: 4.1, 5.1)

## Completion Summary

| Section | Total | Done | Remaining |
|---------|-------|------|-----------|
| Backend | 10 | 10 | 0 |
| Testing | 8 | 8 | 0 |
| Documentation | 2 | 2 | 0 |
| Verification | 2 | 2 | 0 |
| Total | 22 | 22 | 0 |
