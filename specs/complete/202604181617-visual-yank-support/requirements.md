# Visual Yank Support

## Summary

Add yank support to character-wise visual mode and visual-line mode so users can copy selections with `y`, preserve the selection kind in registers, and return to normal mode after a successful yank.

## Problem Statement

urvim's visual modes can delete or change selections, but they cannot currently copy them with `y`. That breaks the standard Vim visual workflow for selecting text, yanking it, and pasting it later with `p` or `P`.

## User Stories

- As a Vim user, I want to press `y` in character-wise visual mode, so that I can copy a selected fragment without changing the buffer.
- As a Vim user, I want to press `y` in visual-line mode, so that I can copy whole lines and paste them as lines later.
- As a Vim user, I want visual yanks to return me to normal mode, so that I can continue editing immediately after copying.
- As a Vim user, I want yanked text to retain whether it was characterwise or linewise, so that pasting behaves correctly.

## Functional Requirements

- [ ] **REQ-001**: Urvim shall support yank in character-wise visual mode.
- [ ] **REQ-002**: Urvim shall support yank in visual-line mode.
- [ ] **REQ-003**: A successful visual yank shall capture the active selection and write it to the resolved yank register target without mutating the buffer.
- [ ] **REQ-004**: A visual yank shall preserve whether the copied text is characterwise or linewise.
- [ ] **REQ-005**: A successful visual yank shall exit the editor to normal mode.
- [ ] **REQ-006**: Visual yanks shall use the current visual selection boundaries, including motion-adjusted ranges, and capture exactly the selected text.
- [ ] **REQ-007**: Visual yanks shall respect an explicit register prefix when one is active and shall default to the yank register otherwise.
- [ ] **REQ-008**: Failed, empty, or unresolved visual yank operations shall leave the buffer unchanged and shall not overwrite register contents.
- [ ] **REQ-009**: Visual yank support shall not change the behavior of visual delete or visual change.

## Non-Functional Requirements

- **Compatibility**: Visual yanks should match Vim's `v`/`V` + `y` workflow closely enough that common copy-and-paste sequences feel familiar.
- **Reliability**: Successful yanks should leave the editor in a predictable mode and should not disturb the current buffer contents.
- **Usability**: The selected text kind should remain visible to later paste operations so `p` and `P` behave as users expect.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `y` in character-wise visual mode copies the selection, leaves the buffer unchanged, and returns the editor to normal mode.
- [ ] **AC-002**: Pressing `y` in visual-line mode copies the selected whole lines, leaves the buffer unchanged, and returns the editor to normal mode.
- [ ] **AC-003**: A characterwise visual yank pastes inline, while a linewise visual yank pastes as whole lines.
- [ ] **AC-004**: An explicit register prefix used before a visual yank receives the copied text and preserves the copied text kind.
- [ ] **AC-005**: A failed or unresolved visual yank does not modify the buffer or overwrite any register contents.
- [ ] **AC-006**: Existing visual delete and visual change behavior continues to work after yank support is added.

## Out of Scope

- Blockwise visual mode
- Changing the semantics of normal-mode `y` outside visual selection workflows
- Register persistence across application restarts
- Clipboard or macro register integration
- Visual dot repeat

## Assumptions

- urvim already has visual and visual-line mode selection state that can be reused for yank capture.
- The register system already records whether stored text is characterwise or linewise.
- Existing paste behavior already uses the stored text kind to decide between inline and linewise placement.

## Dependencies

- Existing character-wise and linewise visual mode infrastructure
- Existing register storage and paste behavior
- Existing normal-mode and visual-mode key parsing
- Existing selection extraction helpers for buffer text
