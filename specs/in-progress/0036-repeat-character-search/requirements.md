# Repeat Character Search Motions

## Summary

Implement Vim's `;` and `,` keys to repeat the previous character search motion (f/F/t/T) forward and backward respectively. The last search state is stored in a global static so it persists across mode switches and window changes.

## Problem Statement

Character scan motions (f, F, t, T) are essential for efficient single-line navigation in vim. After performing a character search, users frequently want to repeat that same search to find the next occurrence. Vim provides `;` to repeat in the same direction and `,` to repeat in the opposite direction.

Currently, urvim implements f/F/t/T motions but lacks the repeat functionality. Users must manually type the full two-key sequence again (e.g., `f x` then `f x`) rather than pressing `;` to quickly find the next `x`.

## User Stories

- **As a** user, **I want to** press `;` after a character search to **repeat the same search in the same direction** (e.g., `f x` then `;` finds next `x`).

- **As a** user, **I want to** press `,` after a character search to **repeat the search in the opposite direction** (e.g., `F x` then `,` finds next `x` going forward).

- **As a** user, **I want** the repeat search to **persist when switching to insert mode and back**, so I can edit text and continue searching without retyping the full sequence.

- **As a** user, **I want** to use count prefixes with repeat searches (e.g., `3;` finds the 3rd occurrence in the same direction).

- **As a** user, **I want** pressing `;` or `,` with no previous search to **do nothing silently**, without error messages or cursor movement.

## Functional Requirements

- [ ] **REQ-001**: `;` repeats the last character search motion (f/F/t/T) in the **same direction**.

- [ ] **REQ-002**: `,` repeats the last character search motion (f/F/t/T) in the **opposite direction**.

- [ ] **REQ-003**: `;` and `,` only read from the stored last search state; they do **not update** the stored state.

- [ ] **REQ-004**: The last search state is stored in a **global static** (`src/globals.rs`) so it persists across:
  - Mode switches (Normal ↔ Insert)
  - Multiple windows (future multi-window support)

- [ ] **REQ-005**: After executing `FindForward`, `FindBackward`, `TillForward`, or `TillBackward`, the search state is updated with:
  - The target character
  - The kind (Find or Till)
  - The direction (Forward or Backward)

- [ ] **REQ-006**: Count prefixes work with `;` and `,` (e.g., `3;` repeats 3 times forward, `3,` repeats 3 times backward).

- [ ] **REQ-007**: If no previous search exists when `;` or `,` is pressed, the cursor does not move (silent fail).

- [ ] **REQ-008**: Pressing `;` after `Fx` goes backward again (same direction).
  - Pressing `,` after `Fx` goes forward (opposite direction).
  - Pressing `;` after `tx` goes till forward again (same direction).
  - Pressing `,` after `tx` goes till backward (opposite direction).

- [ ] **REQ-009**: The terminal key mapping must pass through `;` and `,` as literal keys (not as shifted `:` and `<`).

## Non-Functional Requirements

- **Performance**: Repeat search should be O(n) where n is line length, same as the original search.

- **Thread Safety**: Not required - urvim is single-threaded terminal application with an event loop.

## Acceptance Criteria

- [ ] **AC-001**: After `f x`, pressing `;` moves cursor to the next `x` in the same direction.

- [ ] **AC-002**: After `F x`, pressing `;` moves cursor to the previous `x` (same direction).

- [ ] **AC-003**: After `f x`, pressing `,` moves cursor backward to the previous `x` (opposite direction).

- [ ] **AC-004**: After `F x`, pressing `,` moves cursor forward to the next `x` (opposite direction).

- [ ] **AC-005**: After `t x`, pressing `;` performs till forward again (cursor lands before next `x`).

- [ ] **AC-006**: After `T x`, pressing `,` performs till backward again (cursor lands after previous `x`).

- [ ] **AC-007**: `3;` finds the 3rd occurrence in the same direction as the original search.

- [ ] **AC-008**: `3,` finds the 3rd occurrence in the opposite direction from the original search.

- [ ] **AC-009**: After performing `f x`, switching to insert mode, typing, pressing Escape to return to normal mode, pressing `;` still finds `x` (state persisted).

- [ ] **AC-010**: Pressing `;` with no previous character search does nothing (cursor stays, no error).

- [ ] **AC-011**: The `;` key produces `;` (not `:`) and `,` key produces `,` (not `<`) in the keymap.

## Out of Scope

- Dot repeat (`.`) - separate feature
- Cross-line character search continuation
- Integration with text objects (e.g., `;` after `dt)`)

## Assumptions

- The global static uses `std::sync::Mutex` for interior mutability (simple and sufficient for single-threaded use).
- The `FindState` struct stores: `target_char: char`, `kind: FindKind`, `direction: Direction`.
- `NormalMode` keymap delegates storage updates to `Window::process_action` (which has access to globals).

## Dependencies

- **Internal**:
  - Existing `Action` enum in `src/editor.rs`
  - Existing `Window::process_action` method in `src/window.rs`
  - Existing character scan motions (`FindForward`, `FindBackward`, `TillForward`, `TillBackward`)
  - Existing `CharScanKeymap` in `src/motion/char_scan_keymap.rs`
  - Existing `NormalMode` key handling in `src/editor.rs`
  - Existing terminal key mapping in `src/terminal/keys.rs`
- **Blocked by**: None

## Glossary Terms

**Character Scan Motion**: A motion (f, F, t, T) that takes a target character as a runtime parameter and navigates to or past that character in the current line.

**Repeat Search Forward (`;`)**: An action that repeats the last character scan motion in the same direction, using the stored `FindState`.

**Repeat Search Reverse (`,`)**: An action that repeats the last character scan motion in the opposite direction, using the stored `FindState` and inverting the direction.

**FindState**: A struct stored in a global static that captures the target character, kind (Find/Till), and direction (Forward/Backward) of the last character scan motion.

**Global Static**: A module-level static variable (`src/globals.rs`) that stores the `FindState` and persists across mode switches and windows.
