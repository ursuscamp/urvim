# Vim-Like Jumplist - Implementation Tasks

## Overview

Implement a per-window, session-only jumplist with Vim-style backward/forward navigation, threshold-based history updates, deduplication, bounded history, and cursor syncing for stored/restored positions.

## Backend

- [x] **1.** Add jumplist data structures and ownership to the window layer.
  - [x] **1.1** Introduce a window-local jumplist type that stores ordered cursor history and a current position pointer. (depends on: design)
  - [x] **1.2** Add a jumplist entry type that records buffer identity and cursor position for each history point. (depends on: 1.1)
  - [x] **1.3** Attach jumplist state to `Window` so each window maintains its own session-local history. (depends on: 1.1)
  - [x] **1.4** Enforce a fixed maximum size and drop the oldest entry when the history grows past that limit. (depends on: 1.1)

- [x] **2.** Add cursor-sync helpers for safe storage and restoration of recorded positions.
  - [x] **2.1** Implement a grapheme-aware cursor normalization helper in the buffer layer that clamps stored positions to valid boundaries. (depends on: design)
  - [x] **2.2** Expose a sync-aware cursor restoration path from `BufferView` or `Window` so stored positions are normalized before being written back. (depends on: 2.1)
  - [x] **2.3** Route every stored-cursor restore path through the sync-aware helper, including jumplist playback and other cursor-restoring flows. (depends on: 2.2)

- [x] **3.** Extend the action and keymap layers with jumplist navigation actions.
  - [x] **3.1** Add dedicated action variants for jumplist backward and forward navigation. (depends on: design)
  - [x] **3.2** Bind `<C-o>` and `<C-i>` in normal mode to the new jumplist actions. (depends on: 3.1)
  - [x] **3.3** Ensure the new actions are treated as navigation actions and do not interfere with insert mode, repeat capture, or edit snapshot logic. (depends on: 3.1)

- [x] **4.** Implement jumplist recording and branching behavior in the window action pipeline.
  - [x] **4.1** Refresh the current jumplist head in place when the active file or buffer is already the current history target and cursor movement stays within the distance threshold. (depends on: 1.1, 2.1)
  - [x] **4.2** Create a new jumplist entry when cursor movement within the active file or buffer crosses the distance threshold. (depends on: 1.1, 2.1)
  - [x] **4.3** Deduplicate existing buffer/cursor pairs by removing the older occurrence and moving the entry to the front. (depends on: 1.2)
  - [x] **4.4** Discard forward history only when a threshold-crossing move occurs after the user has navigated backward in the jumplist. (depends on: 1.1, 4.2)
  - [x] **4.5** Update the current jumplist entry in place for small backward-navigation cursor moves without discarding forward history. (depends on: 1.1, 4.1)
  - [x] **4.6** Restore the correct buffer and cursor position when handling backward and forward jumplist navigation. (depends on: 2.2, 3.1)

- [x] **5.** Integrate jumplist recording with other cursor-restoring paths that already exist in the editor.
  - [x] **5.1** Update undo/redo restoration to use the sync-aware cursor path. (depends on: 2.2)
  - [x] **5.2** Update window or tab restoration paths that write stored cursor positions directly so they also sync before restoring. (depends on: 2.2)
  - [x] **5.3** Audit direct `set_cursor` restore sites and replace them with the sync-aware helper where the cursor originates from stored state rather than live motion. (depends on: 2.2)

- [x] **6.** Add documentation for the new navigation behavior.
  - [x] **6.1** Update `docs/motions.md` with `Ctrl-O` and `Ctrl-I` jumplist behavior and any user-facing semantics that matter for navigation. (depends on: 3.2)

## Testing

- [x] **7.** Add unit tests for jumplist data handling and threshold behavior.
  - [x] **7.1** Test that recording a new qualifying jump adds an entry and respects the fixed maximum size. (depends on: 1.1, 4.2)
  - [x] **7.2** Test that repeated visits to the same buffer/cursor pair move the entry to the front rather than duplicating it. (depends on: 4.3)
  - [x] **7.3** Test that backward navigation followed by a threshold-crossing move discards forward history. (depends on: 4.4)
  - [x] **7.4** Test that backward navigation followed by a small move updates the current entry without dropping forward history. (depends on: 4.5)

- [x] **8.** Add unit tests for cursor-sync behavior.
  - [x] **8.1** Test that stored cursor positions are normalized to valid grapheme boundaries before being recorded. (depends on: 2.1)
  - [x] **8.2** Test that restored cursor positions are normalized when the buffer has changed since recording. (depends on: 2.2)
  - [x] **8.3** Test that sync-aware restoration preserves the nearest valid position instead of failing. (depends on: 2.2)

- [x] **9.** Add integration tests for user-facing jumplist navigation.
  - [x] **9.1** Verify `<C-o>` and `<C-i>` navigate backward and forward through jumplist entries in normal mode. (depends on: 3.2, 4.6)
  - [x] **9.2** Verify ordinary cursor movement within the threshold refreshes the current entry in the active file or buffer. (depends on: 4.1)
  - [x] **9.3** Verify threshold-crossing movement creates a new entry for the active file or buffer. (depends on: 4.2)
  - [x] **9.4** Verify cursor restoration remains valid after the underlying buffer text changes. (depends on: 2.2, 5.1)
  - [x] **9.5** Verify existing cursor-restoring flows outside jumplist playback also remain grapheme-safe. (depends on: 5.2, 5.3)

## Completion Summary

| Section | Total | Done | Remaining |
|---------|-------|------|-----------|
| Backend | 19 | 19 | 0 |
| Testing | 10 | 10 | 0 |
| Total | 29 | 29 | 0 |
