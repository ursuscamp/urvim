# Split Pane Focus History

## Summary
Add split-local focus restoration for pane navigation so that moving back into a previously visited split subtree restores the pane that was last focused in that subtree instead of always landing on the first pane in traversal order.

## Problem Statement
urvim's nested split navigation currently chooses the next focused pane purely from rendered geometry. When a user leaves a split subtree that contains multiple panes and later navigates back into that subtree from a cousin split, focus can land on the first pane in that subtree instead of the pane the user last worked in. This breaks navigation continuity and makes nested split workflows feel unpredictable.

## User Stories
- As a user working in nested splits, I want returning to a split subtree to restore the pane I last used there, so that pane navigation preserves my working context.
- As a user moving between cousin splits, I want focus restoration to stay local to the destination split subtree, so that unrelated navigation elsewhere does not change where I land.
- As a user closing panes in a remembered split subtree, I want navigation to fall back to a surviving pane in that subtree, so that focus remains stable even after layout changes.

## Functional Requirements
- [ ] **REQ-001**: The layout must remember the most recently focused pane within each split subtree.
- [ ] **REQ-002**: When directional pane navigation enters a split subtree from outside that subtree, the layout must prefer the subtree's remembered pane if that pane still exists and is directionally reachable.
- [ ] **REQ-003**: Remembered pane selection must be local to the destination split subtree and must not be replaced by visits to panes outside that subtree.
- [ ] **REQ-004**: When focus changes to a pane, the layout must refresh remembered-pane state for every ancestor split that contains that pane.
- [ ] **REQ-005**: If the remembered pane for a split subtree no longer exists, the layout must fall back to another surviving pane within that same subtree before falling back to generic geometric selection.
- [ ] **REQ-006**: Creating a new split must initialize remembered-pane state so immediate subsequent navigation behaves deterministically.
- [ ] **REQ-007**: Closing or pruning panes must keep remembered-pane state valid and must not leave stale pane references behind.
- [ ] **REQ-008**: Pane navigation that does not enter a different split subtree must preserve the existing directional behavior.
- [ ] **REQ-009**: Split-local focus restoration must work across nested mixed-axis layouts, including cousin-split navigation.
- [ ] **REQ-010**: Split-local focus restoration must not change buffer contents, cursor positions, mode state, or tab selection in unrelated panes.

## Non-Functional Requirements
- [ ] **NFR-001**: Directional pane navigation must remain deterministic for the same layout state and sequence of focus changes.
- [ ] **NFR-002**: Focus restoration bookkeeping must remain internal to the layout and must not require new user-facing configuration.
- [ ] **NFR-003**: Additional focus-memory logic must remain responsive during ordinary pane navigation and rendering.
- [ ] **NFR-004**: The behavior must be covered by unit tests for nested split navigation, remembered-pane fallback, and pane removal.

## Acceptance Criteria
- [ ] **AC-001**: In a layout where one split subtree contains multiple panes, navigating out of that subtree and then back into it restores the pane that was last focused there.
- [ ] **AC-002**: Visiting panes in unrelated split subtrees does not change which pane is restored when re-entering a previously visited split subtree.
- [ ] **AC-003**: If the remembered pane in a split subtree is closed, re-entering that subtree restores another surviving pane in that subtree rather than an invalid or removed pane.
- [ ] **AC-004**: In nested mixed-axis layouts, directional pane navigation still respects geometric direction while restoring the remembered pane inside the destination subtree.
- [ ] **AC-005**: Existing pane navigation in layouts without an eligible remembered subtree continues to behave as it does today.

## Out of Scope
- Global pane most-recently-used history shared across the entire layout.
- New key bindings or user-facing commands for pane focus history.
- Persisting split focus history across editor restarts.
- Pane rearrangement or interactive split resizing.

## Assumptions
- A split subtree is the set of pane descendants reachable from a `SplitNode`.
- "Moving back into a split" means directional navigation crosses from outside a split subtree into that subtree.
- The destination subtree may contain nested splits, and the remembered pane may be any descendant pane in that subtree.
- If no valid remembered pane exists in the destination subtree, current geometric navigation remains the final fallback.

## Dependencies
- Existing nested split layout tree and directional pane navigation.
- Stable `PaneId` tracking in `Layout`.
- Existing pane close and prune behavior that collapses empty split nodes.
