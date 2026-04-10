# Paging Keys

## Summary

Implement support for the terminal `PageUp`, `PageDown`, `Ctrl-U`, and `Ctrl-D` keys so they behave as editor paging motions in urvim. `PageUp` and `PageDown` should move through the buffer by one viewport at a time, while `Ctrl-U` and `Ctrl-D` should move by half a viewport. All of these motions should preserve the current column when possible and work in both normal mode and insert mode.

## Problem Statement

The terminal input layer already recognizes `PageUp` and `PageDown`, but the editor does not currently assign them any behavior. As a result, pressing those keys has no useful effect even though users expect them to page through the document.

## User Stories

- **As a** user in normal mode, **I want** `PageUp` to move me up by one screenful, **so that** I can quickly jump to earlier content.
- **As a** user in normal mode, **I want** `PageDown` to move me down by one screenful, **so that** I can quickly jump to later content.
- **As a** user in normal mode, **I want** `Ctrl-U` to move me up by half a screenful, **so that** I can make smaller jumps while keeping context.
- **As a** user in normal mode, **I want** `Ctrl-D` to move me down by half a screenful, **so that** I can make smaller jumps while keeping context.
- **As a** user in insert mode, **I want** the page keys to move the cursor without leaving insert mode, **so that** I can navigate while typing.
- **As a** user, **I want** paging to keep my column position when possible, **so that** vertical navigation stays aligned.

## Functional Requirements

- [ ] **REQ-001**: `PageUp`, `PageDown`, `Ctrl-U`, and `Ctrl-D` shall be recognized as editor actions in normal mode.
- [ ] **REQ-002**: `PageUp`, `PageDown`, `Ctrl-U`, and `Ctrl-D` shall be recognized as editor actions in insert mode.
- [ ] **REQ-003**: `PageUp` shall move the cursor upward by one viewport height per activation.
- [ ] **REQ-004**: `PageDown` shall move the cursor downward by one viewport height per activation.
- [ ] **REQ-005**: `Ctrl-U` shall move the cursor upward by half of the viewport height per activation.
- [ ] **REQ-006**: `Ctrl-D` shall move the cursor downward by half of the viewport height per activation.
- [ ] **REQ-007**: Paging shall preserve the remembered visual column when the target line is long enough.
- [ ] **REQ-008**: Paging shall clamp to the first or last buffer line when the requested page movement would leave the buffer.
- [ ] **REQ-009**: Paging shall not exit insert mode or insert text.
- [ ] **REQ-010**: Paging shall leave existing `H`, `M`, and `L` motions unchanged.

## Non-Functional Requirements

- **Compatibility**: The behavior should feel consistent with common terminal editor paging shortcuts.
- **Correctness**: Paging must not panic on empty buffers, short buffers, or tiny viewports.
- **Maintainability**: The implementation should reuse existing cursor and viewport helpers where possible.

## Acceptance Criteria

- [ ] **AC-001**: Pressing `PageUp` in normal mode moves the cursor up by one screenful.
- [ ] **AC-002**: Pressing `PageDown` in normal mode moves the cursor down by one screenful.
- [ ] **AC-003**: Pressing `Ctrl-U` in normal mode moves the cursor up by half a screenful.
- [ ] **AC-004**: Pressing `Ctrl-D` in normal mode moves the cursor down by half a screenful.
- [ ] **AC-005**: Pressing any of the four paging keys in insert mode moves the cursor and keeps the editor in insert mode.
- [ ] **AC-006**: Paging keeps the cursor on the same visual column when the destination line is wide enough.
- [ ] **AC-007**: Paging clamps cleanly at the start and end of the buffer.
- [ ] **AC-008**: Existing `H`, `M`, `L`, arrow keys, and other normal-mode motions continue to behave as before.

## Out of Scope

- Adding new scroll-only commands that move the viewport independently of the cursor.
- Changing the behavior of `H`, `M`, or `L`.
- Visual mode paging behavior, since urvim does not currently expose a visual mode.
- Mouse wheel input or touchpad gestures.
- Any special count prefixes for paging motions beyond the default page and half-page movement.

## Assumptions

- One page means the current viewport height in rows.
- Half a page means half of the current viewport height in rows, rounded down but never less than one row.
- The editor should use the current remembered visual column behavior already used by vertical motions.
- The existing terminal parser output for `PageUp` and `PageDown` is the source of truth for those key names.
- `Ctrl-U` and `Ctrl-D` map to canonical key strings already used elsewhere in the editor keymap layer.

## Dependencies

- Existing terminal key parsing for `PageUp` and `PageDown`.
- Existing canonical key handling for control key sequences such as `Ctrl-U` and `Ctrl-D`.
- Existing window cursor movement helpers and viewport sizing.
