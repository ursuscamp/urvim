# Canonical Key String Representation

## Summary

This feature defines a canonical string representation for all keypresses in urvim. Character keys (including emojis and wide characters) are represented as their character. Special keys use angle bracket notation (e.g., `<C-a>`). Three characters that have string representation (space, `<`, `>`) are instead represented as specials: `<Space>`, `<LessThan>`, `<GreaterThan>`. Modifiers follow a canonical order, and shift-modified letter keys are normalized to uppercase.

## Problem Statement

Urvim needs a standardized way to represent any keypress as a unique, consistent string. This is essential for:
- Keybinding systems that map key combinations to actions
- User configuration that allows customizing key bindings
- Command recursion and macros that need to serialize key sequences
- Debugging and logging of key inputs

Currently, there is no canonical representation, leading to inconsistent handling of the same key combination across different parts of the application.

## User Stories

- **As a** developer, **I want** a function that converts any `Key` to a canonical string, **so that** I can implement keybinding lookup consistently.

- **As a** user, **I want** my keybindings to work regardless of which order I press modifiers, **so that** `Ctrl+Shift+a` and `Shift+Ctrl+a` both map to the same action.

- **As a** macro system, **I want** to serialize key sequences as strings and replay them later, **so that** I can record and playback user actions.

## Functional Requirements

- [ ] **REQ-001**: Implement a method `canonical_string(&self) -> String` on the `Key` type that returns the canonical string representation
- [ ] **REQ-002**: For `KeyCode::Char(c)` where `c` is a printable character (including emojis and wide characters), return the character itself as the string
- [ ] **REQ-003**: For special keys (Enter, Tab, Backspace, Delete, Esc, Arrow keys, Function keys, etc.), return the special notation format `<KeyName>` (e.g., `<Enter>`, `<Tab>`, `<Up>`, `<F1>`)
- [ ] **REQ-004**: For the three special characters that DO have string representation, return the special notation instead:
  - Space character (`' '`) → `<Space>`
  - Less-than character (`'<'`) → `<LessThan>`
  - Greater-than character (`'>'`) → `<GreaterThan>`
- [ ] **REQ-005**: For Ctrl+letter combinations (e.g., Ctrl+a, Ctrl+b), return `<C-letter>` format (e.g., `<C-a>`, `<C-b>`)
- [ ] **REQ-006**: Define a canonical modifier order: Ctrl → Alt → Shift → Super → Hyper → Meta
- [ ] **REQ-007**: Apply the canonical modifier order in the output string (e.g., `<C-A-a>` for Ctrl+Alt+a, not `<A-C-a>`)
- [ ] **REQ-008**: For Shift + letter (a-z), return the uppercase letter directly (e.g., Shift+a → "A", Shift+b → "B") instead of `<S-a>`
- [ ] **REQ-009**: For Shift + non-letter characters that have shifted representations (e.g., Shift+1 = '!', Shift+2 = '@'), return the shifted character
- [ ] **REQ-010**: For Shift + special characters that don't have shifted representations, use the special notation with Shift (e.g., Shift+Enter → `<S-Enter>`)
- [ ] **REQ-011**: For modifier combinations with non-printable characters, include all modifiers in the canonical order (e.g., Ctrl+Shift+Enter → `<C-S-Enter>`)

## Non-Functional Requirements

- **Performance**: The canonical string conversion should be O(1) for most key types, with minimal allocations
- **Reliability**: The conversion must be deterministic - same keypress always produces the same output

## Acceptance Criteria

- [ ] **AC-001**: `Key::new(KeyCode::Char('a')).canonical_string()` returns "a"
- [ ] **AC-002**: `Key::new(KeyCode::Char('A')).canonical_string()` returns "A"
- [ ] **AC-003**: `Key::new(KeyCode::Char(' ')).canonical_string()` returns "<Space>"
- [ ] **AC-004**: `Key::new(KeyCode::Char('<')).canonical_string()` returns "<LessThan>"
- [ ] **AC-005**: `Key::new(KeyCode::Char('>')).canonical_string()` returns "<GreaterThan>"
- [ ] **AC-006**: `Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL).canonical_string()` returns "<C-a>"
- [ ] **AC-007**: `Key::with_modifiers(KeyCode::Char('a'), Modifiers::SHIFT).canonical_string()` returns "A"
- [ ] **AC-008**: `Key::with_modifiers(KeyCode::Char('1'), Modifiers::SHIFT).canonical_string()` returns "!"
- [ ] **AC-009**: `Key::with_modifiers(KeyCode::Enter, Modifiers::CTRL).canonical_string()` returns "<C-Enter>"
- [ ] **AC-010**: `Key::with_modifiers(KeyCode::Enter, Modifiers::SHIFT).canonical_string()` returns "<S-Enter>"
- [ ] **AC-011**: `Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL | Modifiers::ALT).canonical_string()` returns "<A-C-a>"
- [ ] **AC-012**: `Key::new(KeyCode::Up).canonical_string()` returns "<Up>"
- [ ] **AC-013**: `Key::new(KeyCode::F1).canonical_string()` returns "<F1>"
- [ ] **AC-014**: `Key::new(KeyCode::Esc).canonical_string()` returns "<Esc>"
- [ ] **AC-015**: `Key::new(KeyCode::Char('😀')).canonical_string()` returns "😀" (emoji works)

## Out of Scope

- Keybinding configuration UI
- Loading/saving keybindings from configuration files
- Key sequence parsing (converting string back to Key)
- Mouse events
- Composite key sequences (multiple keys pressed in sequence)

## Assumptions

- The `Key` struct is the primary type for key representation and won't change significantly
- Modifiers are represented using the existing `Modifiers` bitflags
- The implementation will be in the `terminal/keys.rs` module or a new module it creates

## Dependencies

- **Internal**: Depends on the existing `Key`, `KeyCode`, and `Modifiers` types in `src/terminal/keys.rs`
- **Blocked by**: None - these types are already defined
