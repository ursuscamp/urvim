# Flash on Yank

## Summary
When the user yanks text in Normal Mode, the editor briefly flashes the yanked region using the same colors as a visual selection. This flash is a visual confirmation only and does not switch the editor into Visual Mode.

## Problem Statement
It can be difficult to verify exactly what text was yanked after a yank command completes, especially when the intended region was not selected correctly. The editor should provide a short-lived visual cue that highlights the yanked range so the user can confirm the result immediately.

## User Stories
- As a user, I want the yanked text to flash briefly after a yank, so that I can confirm what was copied.
- As a user, I want this feedback to happen without entering Visual Mode, so that my editing flow is not interrupted.
- As a user, I want repeated yanks to restart the flash, so that the most recent yank is always the one I see.

## Functional Requirements
- [ ] **REQ-001**: After a yank completes in Normal Mode, the editor must briefly display the yanked region as a highlighted selection.
- [ ] **REQ-002**: The highlight used for the yank flash must match the editor's existing Visual Mode selection colors.
- [ ] **REQ-003**: The yank flash must not switch the editor into Visual Mode.
- [ ] **REQ-004**: The yank flash must only occur when the yank action is performed from Normal Mode.
- [ ] **REQ-005**: The yank flash must support all yank variants, including linewise and blockwise yanks.
- [ ] **REQ-006**: The initial flash duration must be 200 milliseconds.
- [ ] **REQ-007**: If another yank occurs before the current flash expires, the current flash must be interrupted and restarted for the newest yanked region.

## Non-Functional Requirements
- [ ] **NFR-001**: The flash behavior must be responsive enough that it feels immediate after the yank completes.
- [ ] **NFR-002**: The feature must not change the meaning or contents of yanks and registers.
- [ ] **NFR-003**: The feature must preserve existing Visual Mode behavior without introducing new modal transitions.

## Acceptance Criteria
- [ ] **AC-001**: Yanking a character-wise region in Normal Mode shows the yanked region highlighted for approximately 200 milliseconds.
- [ ] **AC-002**: The highlight uses the same styling as a normal Visual Mode selection.
- [ ] **AC-003**: The editor remains in Normal Mode before, during, and after the flash.
- [ ] **AC-004**: Performing a linewise or blockwise yank in Normal Mode also shows the correct flashed region.
- [ ] **AC-005**: Performing a second yank while the first flash is visible replaces the old flash with the new one.
- [ ] **AC-006**: Yanks triggered outside Normal Mode do not start the flash.

## Out of Scope
- Configurable flash duration.
- Custom styling for the yank flash distinct from Visual Mode colors.
- Any changes to how yank ranges are determined.
- Any new yank commands or new register semantics.

## Assumptions
- The editor already has a way to render a temporary highlighted region using the same visual selection style.
- The yank action flow can identify the final yanked region after an operation completes.
- Normal Mode is the only context in which this feedback should appear.

## Dependencies
- Existing yank operation handling.
- Existing Visual Mode selection styling and rendering path.
- Existing mode management so the feature can confirm the current mode before showing the flash.
