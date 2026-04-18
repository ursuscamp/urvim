# Basic Register Support

## Summary

Add a simple register system to urvim with three dedicated default registers: yank, delete, and change. Yanks should be able to copy text into the yank register, `p` and `P` should paste from the yank register by default, and a compact register prefix should let users redirect yank/delete/change/paste to another register when needed.

## Problem Statement

urvim can delete and change text, but it does not yet preserve copied text in a reusable register workflow. That makes it hard to copy text once and paste it repeatedly, and it also means delete and change operations have nowhere to store their removed text independently from yanks.

A minimal register system is needed so users can:

- copy text without mutating the buffer
- paste copied text with `p` and `P`
- keep delete and change text separate from the yank register
- override the default register with a small, explicit key sequence

## User Stories

- As a user, I want to yank text without changing the buffer, so that I can paste it later.
- As a user, I want yank, delete, and change to use separate default registers, so that one operation does not overwrite another operation's text.
- As a user, I want to paste with `p` and `P`, so that I can reuse copied text quickly.
- As a user, I want to choose a different register with a short prefix, so that I can direct a copy, delete, change, or paste to a specific slot when needed.

## Functional Requirements

- [ ] **REQ-001**: Urvim shall maintain separate default registers for yank, delete, and change.
- [ ] **REQ-002**: Successful yank operations shall write their captured text to the yank register by default.
- [ ] **REQ-003**: Successful delete operations shall write their removed text to the delete register by default.
- [ ] **REQ-004**: Successful change operations shall write their removed text to the change register by default.
- [ ] **REQ-005**: A compact register prefix shall allow the user to select a non-default register for yank, delete, change, `p`, and `P`.
- [ ] **REQ-005a**: The register prefix shall treat `y`, `d`, and `c` as direct selectors for the default yank, delete, and change registers.
- [ ] **REQ-005b**: Lowercase ASCII letters other than `y`, `d`, and `c` shall remain available as user-named registers.
- [ ] **REQ-006**: Yank commands shall support the same motion, text-object, and linewise targets that delete and change currently support.
- [ ] **REQ-007**: Successful yank operations shall not change the buffer contents or the cursor position.
- [ ] **REQ-008**: `p` shall paste the yank register by default.
- [ ] **REQ-009**: `P` shall paste the yank register by default.
- [ ] **REQ-010**: Register contents shall preserve whether the stored text is characterwise or linewise so paste placement can be resolved correctly.
- [ ] **REQ-011**: Characterwise pastes shall insert inline at the cursor, while linewise pastes shall insert as whole lines relative to the current line.
- [ ] **REQ-012**: Empty or invalid register-targeted operations shall leave the buffer unchanged.
- [ ] **REQ-013**: Startup configuration shall allow remapping the default yank, delete, and change destinations with a `default_registers` table.
- [ ] **REQ-014**: The `default_registers` table shall accept single lowercase ASCII letters for the `yank`, `delete`, and `change` entries and fall back to built-in defaults for omitted entries.

## Non-Functional Requirements

- **Usability**: The default yank/delete/change registers should be easy to reason about and should not require Vim's full register model.
- **Compatibility**: The feature should feel familiar to Vim users without requiring every Vim register behavior.
- **Reliability**: Register-backed editing should remain deterministic across mode switches and window changes within the same editor session.

## Acceptance Criteria

- [ ] **AC-001**: After `yw`, the buffer is unchanged and `p` pastes the copied text using the yank register.
- [ ] **AC-002**: After `yy`, `p` pastes a whole line using the yank register and places it relative to the current line.
- [ ] **AC-003**: After `dd`, the deleted text is stored in the delete register and the yank register is still available for `p`.
- [ ] **AC-004**: After `cc`, the removed text is stored in the change register and the editor enters insert mode.
- [ ] **AC-005**: After selecting an explicit register, such as `"a`, the next yank or paste uses that register instead of the default one.
- [ ] **AC-006**: A characterwise yank pastes inline, while a linewise yank pastes as a full line above or below the current line depending on whether `P` or `p` was used.
- [ ] **AC-007**: Attempting a register-targeted command with an invalid register prefix does not modify the buffer.
- [ ] **AC-008**: A `default_registers` config entry can remap the default yank, delete, and change destinations without changing the register prefix syntax.

## Out of Scope

- Vim's numbered, unnamed, clipboard, and macro registers
- Register persistence across application restarts
- Visual block mode register semantics
- System clipboard integration
- Named register promotion rules beyond the basic explicit register selector

## Assumptions

- The register selector will use a short prefix sequence rather than a full Vim-compatible register model.
- Register contents only need to live for the current editor session.
- Existing delete and change target resolution can be reused for yank and paste workflows.

## Dependencies

- Existing normal-mode key parsing
- Existing delete/change target resolution
- Existing buffer text insertion and deletion helpers
- Existing undo and mode-switching infrastructure
