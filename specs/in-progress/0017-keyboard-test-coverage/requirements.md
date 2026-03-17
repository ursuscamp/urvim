# Comprehensive Keyboard Test Coverage

## Summary
Add missing test coverage for CSI-u (Kitty keyboard protocol) and legacy escape sequences to ensure comprehensive parsing coverage.

## Problem Statement

The keyboard parsing code has significant gaps in test coverage:
- CSI-u (Kitty protocol) keys: Most key codes (Esc, Tab, Enter, Home, End, arrows, etc.) have no test coverage
- Legacy keys: Menu key, alternate Home/End sequences have no tests
- Modifier combinations: Missing edge cases

Without these tests, bugs can go undetected and refactoring is risky.

## User Stories

- **As a** developer, **I want** comprehensive keyboard tests, **so that** I can refactor with confidence and catch regressions.
- **As a** user, **I want** all keyboard keys to work correctly, **so that** my keybindings function as expected.

## Functional Requirements

- [ ] **REQ-001**: Add tests for CSI-u code 2 (Tab) parsing
- [ ] **REQ-002**: Add tests for CSI-u code 4 (Enter) parsing
- [ ] **REQ-003**: Add tests for CSI-u code 5-8 (Home, End, PageUp, PageDown) parsing
- [ ] **REQ-004**: Add tests for CSI-u code 10 (Insert) parsing
- [ ] **REQ-005**: Add tests for CSI-u codes 24-27 (Arrow keys) parsing
- [ ] **REQ-006**: Add tests for CSI-u code 127 (Backspace) parsing
- [ ] **REQ-007**: Add tests for legacy CSI 1~ and 7~ (Home alternates) parsing
- [ ] **REQ-008**: Add tests for legacy CSI 8~ (End alternate) parsing

## Non-Functional Requirements

- **Maintainability**: Tests should follow existing test patterns in `mod.rs`
- **Performance**: Tests should run quickly (unit tests, not integration)

## Acceptance Criteria

- [ ] **AC-001**: All 15+ missing test cases added and passing
- [ ] **AC-002**: Test coverage includes modifier combinations for new keys
- [ ] **AC-003**: All existing tests continue to pass

## Out of Scope

- Testing terminal multiplexer behavior (tmux, screen)
- Testing rare keys (F13-F35, keypad keys, media keys)

## Dependencies

- None - this is a self-contained testing task

## Assumptions

- The parsing code is correct; we're adding tests to verify
- Existing escape parsing code doesn't need modification
