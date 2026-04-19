# Visual Mode Text Objects

## Summary
urvim will support text objects in character-wise visual mode, using the same text object families that are already available in normal-mode operator-pending flows. When a user invokes a text object while visual mode is active, the current selection will be set to the resolved object. Repeating the same text object on an already-matching selection will leave the selection unchanged.

## Problem Statement
Users can already use text objects such as `iw`, `aw`, `iW`, `aW`, bracket objects, and quote objects with operators in normal mode, but they cannot use the same object vocabulary to retarget an active visual selection. This makes selection workflows inconsistent and forces users to leave visual mode or re-select text manually when they want to change a selection.

## User Stories
- As a user editing text, I want to start a visual selection with a text object and then invoke the same object again without changing the selection, so that the selection stays stable and predictable.
- As a user familiar with Vim, I want urvim to recognize the same text object families in visual mode as it does in normal mode, so that my editing habits transfer naturally.
- As a user selecting text, I want repeated text object commands to behave predictably, so that I can anticipate how the selection will change before I press the next key.

## Functional Requirements
- [ ] **REQ-001**: urvim must allow text objects to be invoked while character-wise visual mode is active.
- [ ] **REQ-002**: urvim must support the same text object families in character-wise visual mode that are available in normal-mode operator-pending flows.
- [ ] **REQ-003**: Supported visual-mode text objects must include inner and around word objects (`iw`, `aw`), inner and around BigWord objects (`iW`, `aW`), bracket objects, and quote objects.
- [ ] **REQ-004**: When a visual-mode text object is resolved successfully, urvim must update the active selection to the resolved object instead of applying a motion-like expansion.
- [ ] **REQ-005**: Repeating the same visual-mode text object on an already-matching selection must leave the selection unchanged.
- [ ] **REQ-006**: Visual-mode text object selection updates must preserve visual mode as the active mode after the selection changes.
- [ ] **REQ-007**: If a visual-mode text object cannot be resolved at the current cursor location, urvim must leave the existing selection unchanged.
- [ ] **REQ-008**: Different visual-mode text objects may retarget the active selection to a different resolved object.
- [ ] **REQ-009**: Visual-mode text object behavior must not change the existing normal-mode operator-pending text object behavior.
- [ ] **REQ-010**: The initial implementation may limit this feature to character-wise visual mode; linewise visual mode text objects are not required in the first pass.

## Non-Functional Requirements
- [ ] **NFR-001**: The feature must remain responsive during interactive selection updates.
- [ ] **NFR-002**: The feature must preserve existing text-object behavior for users who do not enter visual mode.
- [ ] **NFR-003**: The feature must remain compatible with urvim's Unicode-aware cursor and selection handling.

## Acceptance Criteria
- [ ] **AC-001**: Starting from normal mode, `viw` enters visual mode and selects the inner word under the cursor.
- [ ] **AC-002**: After `viw`, invoking the same supported text object in visual mode leaves the current selection unchanged.
- [ ] **AC-003**: After invoking a different supported text object in visual mode, the selection updates to the new resolved object.
- [ ] **AC-004**: After a visual text-object update, the editor remains in visual mode.
- [ ] **AC-005**: Visual-mode `iw`, `aw`, `iW`, `aW`, bracket objects, and quote objects are all accepted when the feature is enabled.
- [ ] **AC-006**: Attempting a visual-mode text object at an invalid location leaves the current selection unchanged.
- [ ] **AC-007**: Normal-mode operator-pending text objects continue to behave as they do today.

## Out of Scope
- Visual line mode text-object selection.
- Adding new text object families beyond those already supported in normal mode.
- Changing operator-pending counts or deletion/change semantics.
- Changing the syntax or meaning of existing normal-mode motion keys.

## Assumptions
- The existing normal-mode text object set is the authoritative list for the first pass.
- "Update" means the active visual selection becomes the resolved text object range.
- Repeating the same visual-mode text object should be treated as idempotent.
- The user-facing workflow is limited to character-wise visual mode for the first implementation.
- Existing bracket and quote matching semantics remain the source of truth for those object families.

## Dependencies
- Existing normal-mode text object resolution logic.
- Existing character-wise visual selection state and rendering.
- Existing bracket and quote matching helpers.
- Existing Unicode-safe cursor and range normalization behavior.
