# Insert Mode Escape Binding
## Summary
Add a user-configurable insert-mode escape binding so users can leave insert mode without reaching for the physical Escape key. The configured binding should work in addition to the built-in `<Esc>` key.

## Problem Statement
Leaving insert mode currently requires pressing `<Esc>`, which can be awkward on compact keyboards or for users who prefer home-row workflows. Urvim should let users define an alternate insert-mode escape binding in config so they can switch back to normal mode more ergonomically.

## User Stories
1. As a user, I want to configure an alternate insert-mode escape key or key sequence, so that I can leave insert mode without moving my hand to the Escape key.
2. As a user, I want the built-in `<Esc>` binding to keep working, so that existing workflows do not change.
3. As a user, I want invalid alternate escape bindings to be rejected clearly, so that I can fix configuration mistakes quickly.

## Functional Requirements
- [ ] **REQ-001**: The editor must allow the user to define an alternate insert-mode escape binding in the configuration file.
- [ ] **REQ-002**: The configured alternate escape binding must be additive to the built-in `<Esc>` binding, not a replacement for it.
- [ ] **REQ-003**: When the configured alternate escape binding is pressed in insert mode, the editor must switch to normal mode.
- [ ] **REQ-004**: The configured alternate escape binding must not insert text into the buffer when it is recognized as an exact match.
- [ ] **REQ-005**: The configured alternate escape binding must be expressed in the same canonical key format used by existing key bindings.
- [ ] **REQ-006**: If the configured alternate escape binding is empty, whitespace only, or otherwise invalid, configuration loading must fail with a clear error.
- [ ] **REQ-007**: If no alternate escape binding is configured, insert mode behavior must remain unchanged from the current default.
- [ ] **REQ-008**: The alternate escape binding must only apply in insert mode.
- [ ] **REQ-009**: Partial key sequences for the alternate escape binding must not exit insert mode until the full binding is matched.

## Non-Functional Requirements
- [ ] **REQ-010**: The feature must preserve current insert-mode responsiveness and not introduce noticeable input lag.
- [ ] **REQ-011**: The feature must remain compatible with existing configuration loading behavior and error reporting.
- [ ] **REQ-012**: The feature must preserve backward compatibility for users who do not add the new config option.

## Acceptance Criteria
- [ ] **AC-001**: With no new config value set, pressing `<Esc>` in insert mode still returns the editor to normal mode.
- [ ] **AC-002**: With an alternate escape binding configured, pressing that binding in insert mode returns the editor to normal mode.
- [ ] **AC-003**: With an alternate escape binding configured, pressing `<Esc>` in insert mode still returns the editor to normal mode.
- [ ] **AC-004**: An invalid alternate escape binding in config causes startup configuration loading to fail with a clear validation error.
- [ ] **AC-005**: The alternate escape binding does not affect normal mode key handling.
- [ ] **AC-006**: The alternate escape binding does not insert literal characters when it is used to leave insert mode.

## Out of Scope
- Remapping or overriding arbitrary insert-mode keys beyond the alternate escape binding.
- Introducing a separate interactive keybinding editor.
- Changing normal-mode escape or command handling.
- Adding platform-specific keyboard shortcuts outside the config file.

## Assumptions
- The alternate escape binding will be configured through the existing TOML startup config.
- The binding will use urvim's existing canonical key string format.
- The user may choose a single key or a multi-key sequence, as long as it can be represented by the existing insert-mode keymap system.
- The built-in `<Esc>` binding will remain unconditional.

## Dependencies
- Insert mode keymap support.
- Startup configuration parsing and validation.
- Canonical key string parsing and trie keymap lookup.
