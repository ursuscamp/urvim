# Menu Key Support

## Summary
Add support for the Menu key (also known as the Application key or context menu key) in the keyboard handling system, mapping from the Kitty keyboard protocol CSI 29~ escape sequence.

## Problem Statement

The Menu key is a standard keyboard key found on many keyboards, particularly in the editing cluster near the arrow keys. When users press this key in terminals that support the Kitty keyboard protocol, urvim does not recognize it and falls back to the Escape key. This creates a poor user experience for users with this key.

## User Stories

- **As a** user with a keyboard containing a Menu/Application key, **I want** urvim to recognize my Menu key presses, **so that** I can use it for custom keybindings or it doesn't accidentally trigger escape actions.
- **As a** terminal application developer, **I want** comprehensive keyboard protocol support, **so that** urvim works correctly with all modern terminals.

## Functional Requirements

- [ ] **REQ-001**: Add `Menu` variant to the `KeyCode` enum in `keys.rs`
- [ ] **REQ-002**: Implement parsing for CSI 29~ escape sequence in `escape.rs`
- [ ] **REQ-003**: Ensure the Menu key produces a distinct canonical string representation
- [ ] **REQ-004**: Add unit tests for Menu key parsing from CSI 29~ sequence

## Non-Functional Requirements

- **Compatibility**: Must work with legacy terminals that don't send CSI 29~ (falls through to Escape, which is acceptable)
- **Performance**: No performance impact - parsing is O(1)

## Acceptance Criteria

- [ ] **AC-001**: `\x1b[29~` sequence parses to `KeyCode::Menu`
- [ ] **AC-002**: `KeyCode::Menu.canonical_string()` returns `"<Menu>"`
- [ ] **AC-003**: Unit tests pass for Menu key parsing
- [ ] **AC-004**: Menu key with modifiers (e.g., Shift+Menu) parses correctly

## Out of Scope

- Adding Menu key to any default keybindings (configuration issue)
- Supporting the Menu key in terminal multiplexers (tmux, screen)
- Handling platform-specific Menu key variants

## Assumptions

- The CSI 29~ sequence is the standard way to send Menu key in Kitty protocol
- Most modern terminals (Kitty, WezTerm, iTerm2) support this sequence

## Dependencies

- None - this is a self-contained feature
