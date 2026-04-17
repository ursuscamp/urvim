# Split Pane Focus History - Implementation Tasks

## Overview

Implement split-local pane focus memory in `Layout` so directional navigation restores the last focused descendant pane when re-entering a split subtree, with safe fallback after pane closure or pruning.

## Backend

- [ ] **1.** Extend split-tree metadata to track remembered descendant focus.
  - [ ] **1.1** Add a remembered descendant `PaneId` field to `SplitNode`. (depends on: design)
  - [ ] **1.2** Initialize remembered focus when creating new split nodes so the new parent subtree has deterministic state. (depends on: 1.1)
  - [ ] **1.3** Add or update doc comments for any public layout types touched by the change. (depends on: 1.1)

- [ ] **2.** Add tree helpers for maintaining and resolving split-local focus history.
  - [ ] **2.1** Implement subtree membership or lookup helpers needed to determine whether a pane belongs to a split subtree. (depends on: 1.1)
  - [ ] **2.2** Implement a helper that records a focused pane into every ancestor split that contains it. (depends on: 2.1)
  - [ ] **2.3** Implement a helper that resolves the preferred surviving pane for a subtree, preferring remembered descendants and falling back to another surviving pane in that subtree. (depends on: 2.1)
  - [ ] **2.4** Update pane-removal and prune paths to discard or recompute stale remembered pane ids. (depends on: 2.3)

- [ ] **3.** Update layout focus transitions to use split-local focus memory.
  - [ ] **3.1** Route focus changes through a shared path that updates `focused_pane` and records ancestor split history. (depends on: 2.2)
  - [ ] **3.2** Update directional navigation so geometry still picks the destination side, but the final focused pane is resolved from the destination subtree's remembered descendant when applicable. (depends on: 2.3, 3.1)
  - [ ] **3.3** Preserve current behavior when no valid remembered descendant exists or when navigation does not cross into a different split subtree. (depends on: 3.2)

- [ ] **4.** Update documentation for pane navigation semantics.
  - [ ] **4.1** Update [docs/motions.md](/Users/ryan/Dev/urvim/docs/motions.md) to note that directional pane navigation restores the last focused pane within the destination split subtree when applicable. (depends on: 3.2)

## Testing

- [ ] **5.** Add regression coverage for remembered split focus behavior.
  - [ ] **5.1** Add a layout test where re-entering a cousin split restores the last focused pane inside that split subtree. (depends on: 3.2)
  - [ ] **5.2** Add a layout test showing unrelated pane visits elsewhere do not change the remembered pane for another split subtree. (depends on: 3.2)
  - [ ] **5.3** Add a layout test where the remembered pane is closed and navigation falls back to another surviving pane in that subtree. (depends on: 2.4, 3.2)
  - [ ] **5.4** Add a layout test covering nested mixed-axis panes to verify geometry still determines the destination subtree before focus restoration occurs. (depends on: 3.2)
  - [ ] **5.5** Add a layout test confirming simple layouts without remembered subtree state retain current directional behavior. (depends on: 3.3)

## Verification

- [ ] **6.** Verify the feature and clean up touched surfaces.
  - [ ] **6.1** Run `cargo fmt` after implementation. (depends on: 3.2)
  - [ ] **6.2** Run `cargo check` and fix any build or warning regressions. (depends on: 3.2)
  - [ ] **6.3** Run the focused layout test suite covering split navigation and pane removal. (depends on: 5.5)

## Completion Summary

| Section | Total | Done | Remaining |
|---------|-------|------|-----------|
| Backend | 4 | 0 | 4 |
| Testing | 1 | 0 | 1 |
| Verification | 1 | 0 | 1 |
| Total | 6 | 0 | 6 |
