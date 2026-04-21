# Partial Indent Scope Cache
## Summary
Introduce partial indent scope tracking so the indent scope cache can be established and invalidated incrementally instead of being fully recomputed on every update. This change is intended to improve cache efficiency while preserving existing syntax highlighting behavior.

## Problem Statement
The current indent scope cache is rebuilt as a whole, even when only part of the buffer needs updated scope information. This makes the cache less efficient than necessary and prevents viewport-limited syntax work from establishing only the scope data it needs. It also forces edits to invalidate more scope state than required.

## User Stories
- As a user editing a large file, I want indent scope data to be computed incrementally, so that cache work stays limited to the lines that actually need it.
- As a user scrolling through a file, I want viewport-driven scope scans to stop once the viewport is covered, so that the editor does not do unnecessary scope work outside the visible area.
- As a user making an edit, I want scope data after the edited line to be invalidated and recomputed, so that later indentation-aware behavior stays correct.

## Functional Requirements
- [ ] **REQ-001**: The indent scope cache shall support partially established scope state rather than requiring a full-buffer recomputation for every refresh.
- [ ] **REQ-002**: The cache shall preserve whether a scope is open or closed so that later scans can resume from existing cached state.
- [ ] **REQ-003**: A viewport-limited syntax ensure pass shall be able to establish indent scope state only up to the viewport end.
- [ ] **REQ-004**: When a viewport pass needs earlier context, it shall resume scanning from the later of the beginning of the file or the end of the invalidated cache region.
- [ ] **REQ-005**: When a line is edited, all indent scope cache state from that line onward shall be invalidated.
- [ ] **REQ-006**: Rebuilding invalidated scope state shall be able to continue from the last valid cached scope state before the invalidation boundary.
- [ ] **REQ-007**: The change shall not alter syntax highlighting results or syntax highlight invalidation behavior.
- [ ] **REQ-008**: Indent scope cache updates shall remain consistent with buffer edits so that later indent-aware features see deterministic scope state.

## Non-Functional Requirements
- [ ] **NFR-001**: The partial update path shall avoid unnecessary recomputation when only a suffix of the buffer needs to be rescanned.
- [ ] **NFR-002**: The cache behavior shall remain reliable across repeated edits, viewport changes, and mixed open/closed scope transitions.
- [ ] **NFR-003**: The change shall preserve compatibility with existing syntax caching behavior and rendering behavior.

## Acceptance Criteria
- [ ] **AC-001**: A viewport-limited syntax update establishes indent scope state only through the visible range, without requiring a full-buffer pass.
- [ ] **AC-002**: Editing a line invalidates scope state from that line through the end of the buffer.
- [ ] **AC-003**: A later viewport or background pass can resume scanning from the first invalidated line boundary using previously cached open/closed scope state.
- [ ] **AC-004**: Existing syntax highlight output remains unchanged before and after the indent scope cache change.
- [ ] **AC-005**: Scope state after repeated edit-and-refresh cycles remains correct for later indentation-aware consumers.

## Out of Scope
- Syntax highlighting changes.
- New user-facing indentation options.
- Changes to the visual appearance of indentation guides or folds.
- Broader syntax cache redesign beyond what is needed to support partial indent scope tracking.

## Assumptions
- The current scope cache can be extended to track enough state to resume scanning from a partial boundary.
- Viewport-limited syntax refreshes already have a clear end line that can act as the upper bound for scope establishment.
- Indent scope data is consumed separately from syntax highlighting, so preserving syntax behavior is feasible without user-visible changes.

## Dependencies
- Existing syntax cache and viewport refresh flow.
- Existing indent scope cache data structures and invalidation path.
- Buffer edit notifications that identify the edited line or line range.
