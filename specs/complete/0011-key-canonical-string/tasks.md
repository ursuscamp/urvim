# Canonical Key String Representation - Implementation Tasks

## Overview

Total: 12 tasks
Key milestones:
1. Add helper functions for keycode-to-special-name and modifier prefix conversion
2. Implement the main `canonical_string()` method on `Key`
3. Add comprehensive unit tests
Dependencies: None (uses existing types)

## Implementation

- [x] **1.** Add helper function `keycode_to_special_name()` to convert non-char KeyCode to special name string
  - [x] **1.1** Map all KeyCode variants (Enter, Backspace, Tab, Esc, Delete, Insert, Up, Down, Left, Right, Home, End, PageUp, PageDown, F1-F12, Null) to their string names (test: verify each mapping)
  - [x] **1.2** Return `None` for `KeyCode::Char` (test: verify None returned for Char)

- [x] **2.** Add helper function `modifiers_to_prefixes()` to get modifier prefixes in canonical order
  - [x] **2.1** Define modifier prefix constants (C for CTRL, A for ALT, S for SHIFT, Su for SUPER, H for HYPER, M for META) (test: verify prefix values)
  - [x] **2.2** Return prefixes in order: Ctrl → Alt → Shift → Super → Hyper → Meta (test: verify order with multiple modifiers)

- [x] **3.** Add helper functions for special character handling
  - [x] **3.1** Add `needs_special_representation(c: char) -> bool` to check if char is space, <, or > (test: verify returns true for ' ', '<', '>')
  - [x] **3.2** Add `special_name_for_char(c: char) -> Option<&'static str>` to get special name (test: verify "Space", "LessThan", "GreaterThan" returned)

- [x] **4.** Add helper function `is_shiftable_letter(c: char) -> bool` to check if character is a letter
  - [x] **4.1** Returns true for 'a'-'z' and 'A'-'Z' (test: verify letters return true)
  - [x] **4.2** Returns false for all other characters (test: verify non-letters return false)

- [x] **5.** Add helper function `get_shifted_char(c: char) -> Option<char>` for Shift+character on US keyboard
  - [x] **5.1** Map number row: 1→!, 2→@, 3→#, 4→$, 5→%, 6→^, 7→&, 8→*, 9→(, 0→) (test: verify each mapping)
  - [x] **5.2** Map punctuation row: `→~, `-→_, =→+, [→{, ]→}, \→|, ;→:, '→", ,→<, .→>, /→? (test: verify each mapping)
  - [x] **5.3** Return None for characters without shifted representation (test: verify None for letters, numbers)

- [x] **6.** Implement main `canonical_string(&self) -> String` method on `Key`
  - [x] **6.1** Handle Char keys with no modifiers - return character as-is (test: "a" → "a", "Z" → "Z")
  - [x] **6.2** Handle Char keys that are special exceptions (space, <, >) - return special notation (test: " " → "<Space>", "<" → "<LessThan>", ">" → "<GreaterThan>")
  - [x] **6.3** Handle Char keys with Shift only - apply shift normalization (test: 'a'+Shift → "A", '1'+Shift → "!")
  - [x] **6.4** Handle Char keys with Ctrl (with or without other modifiers) - return `<C-x>` format (test: Ctrl+a → "<C-a>")
  - [x] **6.5** Handle special keys (non-Char KeyCode) with no modifiers - return `<KeyName>` format (test: Up → "<Up>", F1 → "<F1>")
  - [x] **6.6** Handle special keys with modifiers - return `<M-KeyName>` format (test: Ctrl+Enter → "<C-Enter>")
  - [x] **6.7** Handle modifier combinations - apply canonical modifier order (test: Ctrl+Alt+a → "<C-A-a>", not "<A-C-a>")

## Testing

- [x] **7.** Add unit tests for basic characters
  - [x] **7.1** Test lowercase letters a-z (test: each returns itself)
  - [x] **7.2** Test uppercase letters A-Z (test: each returns itself)
  - [x] **7.3** Test digits 0-9 (test: each returns itself)
  - [x] **7.4** Test punctuation marks (test: each returns itself)
  - [x] **7.5** Test Unicode/emoji characters (test: emoji like 😀 returns itself)

- [x] **8.** Add unit tests for special exceptions
  - [x] **8.1** Test space character returns "<Space>" (test: verify)
  - [x] **8.2** Test less-than character returns "<LessThan>" (test: verify)
  - [x] **8.3** Test greater-than character returns "<GreaterThan>" (test: verify)

- [x] **9.** Add unit tests for modifier combinations
  - [x] **9.1** Test CTRL alone with letters (test: "<C-a>", "<C-z>")
  - [x] **9.2** Test ALT alone with letters (test: "<A-a>")
  - [x] **9.3** Test SHIFT alone with letters normalizes to uppercase (test: "A", "B")
  - [x] **9.4** Test multiple modifiers in various combinations (test: verify canonical order)
  - [x] **9.5** Test SUPER, HYPER, META modifiers (test: "<Su-x>", "<H-x>", "<M-x>")

- [x] **10.** Add unit tests for special keys
  - [x] **10.1** Test all navigation keys (Up, Down, Left, Right, Home, End, PageUp, PageDown) (test: each returns "<KeyName>")
  - [x] **10.2** Test all function keys F1-F12 (test: each returns "<F1>" through "<F12>")
  - [x] **10.3** Test control keys (Enter, Tab, Backspace, Delete, Insert, Esc, Null) (test: each returns "<KeyName>")
  - [x] **10.4** Test special keys with modifiers (test: "<C-Up>", "<S-F1>")

- [x] **11.** Add unit tests for shift normalization edge cases
  - [x] **11.1** Test Shift+letter = uppercase (test: 'a'+Shift → "A")
  - [x] **11.2** Test Shift+number = shifted character (test: '1'+Shift → "!")
  - [x] **11.3** Test Shift+punctuation = shifted character (test: '['+Shift → "{")
  - [x] **11.4** Test Shift+special key = modifier prefix (test: Shift+Enter → "<S-Enter>")

- [x] **12.** Run full test suite and verify all tests pass
  - [x] **12.1** Run `cargo test` (test: all tests pass)
  - [x] **12.2** Check for clippy warnings (test: no new warnings)
  - [x] **12.3** Verify build succeeds with `cargo check` (test: no errors)

---

## Completion Summary

| Phase | Tasks | Completed | Progress |
|-------|-------|-----------|----------|
| Implementation | 6 | 6 | 100% |
| Testing | 6 | 6 | 100% |
| **Total** | **12** | **12** | **100%** |
