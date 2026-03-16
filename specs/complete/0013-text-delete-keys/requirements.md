# Text Delete Keys

## Summary

Add text deletion functionality in both insert and normal modes using backspace and delete keys. Backspace removes characters backward, while delete removes the current character without moving the cursor. Both operations work on Unicode grapheme clusters and join lines when appropriate.

## Problem Statement

The editor currently lacks basic text deletion capabilities. Users need standard text editing operations:
- In insert mode: backspace to delete backward, delete to delete forward without moving cursor
- In normal mode: x to act as delete (forward), X to act as backspace (backward)
- All operations should work with Unicode grapheme clusters, not raw bytes
- When deleting at line boundaries, lines should join together

## User Stories

1. **As a** user in insert mode, **I want** to press Backspace to delete the character before the cursor, **so that** I can correct typos as I type.

2. **As a** user in insert mode, **I want** to press Delete to remove the character at the cursor position without moving the cursor, **so that** I can delete characters without losing my place.

3. **As a** user in normal mode, **I want** to press x to delete the character under the cursor, **so that** I can quickly remove characters without entering insert mode.

4. **As a** user in normal mode, **I want** to press X to delete the character before the cursor, **so that** I can quickly delete backward without entering insert mode.

5. **As a** user editing text with multi-byte characters (e.g., emojis, accented characters), **I want** backspace and delete to remove whole grapheme clusters, **so that** I don't corrupt Unicode text.

6. **As a** user with multiple lines of text, **I want** backspace at the start of a line to join it with the previous line, **so that** I can naturally join lines while editing.

## Functional Requirements

- [ ] **REQ-001**: In insert mode, Backspace removes the grapheme cluster before the cursor and moves cursor backward by one grapheme
- [ ] **REQ-002**: In insert mode, Delete removes the grapheme cluster at the cursor position without moving the cursor
- [ ] **REQ-003**: In normal mode, x removes the grapheme cluster at the cursor position (identical to insert mode Delete)
- [ ] **REQ-004**: In normal mode, X removes the grapheme cluster before the cursor (identical to insert mode Backspace)
- [ ] **REQ-005**: All delete operations must work on Unicode grapheme clusters, not individual bytes or code points
- [ ] **REQ-006**: Backspace at the start of a line joins the current line with the previous line
- [ ] **REQ-007**: Delete at the end of a line joins the current line with the next line
- [ ] **REQ-008**: When at the beginning of the document, Backspace has no effect (no previous line to join)
- [ ] **REQ-009**: When at the end of the document and last line, Delete has no effect (nothing to join)
- [ ] **REQ-010**: The cursor position should be properly clamped after deletions to prevent invalid positions
- [ ] **REQ-011**: Key bindings should be properly registered in the keybinding system

## Non-Functional Requirements

- **Performance**: Delete operations should be O(1) for grapheme traversal
- **Reliability**: Operations must not panic or cause undefined behavior at document boundaries
- **Compatibility**: Behavior may differ slightly from vim for consistency between modes

## Acceptance Criteria

- [ ] **AC-001**: Pressing Backspace in insert mode at position 5 removes character at position 4, cursor moves to position 4
- [ ] **AC-002**: Pressing Delete in insert mode at position 5 removes character at position 5, cursor stays at position 5
- [ ] **AC-003**: Pressing x in normal mode at position 5 removes character at position 5, cursor stays at position 5
- [ ] **AC-004**: Pressing X in normal mode at position 5 removes character at position 4, cursor moves to position 4
- [ ] **AC-005**: Backspace at line start (column 0) joins current line with previous line
- [ ] **AC-006**: Delete at line end joins current line with next line
- [ ] **AC-007**: Grapheme clusters like "é" (single grapheme, 2 bytes) are deleted as a single unit
- [ ] **AC-008**: Emojis like "👍" (may be multiple code points) are deleted as a single unit
- [ ] **AC-009**: Backspace at document start does nothing
- [ ] **AC-010**: Delete at document end does nothing

## Out of Scope

- Visual mode delete operations (will be handled separately)
- Repeat operations with "."
- Integration with registers for deleted text
- Case transformation keys (D, C, etc.)

## Dependencies

- Keybinding system (must be functional)
- Document/text buffer with grapheme-aware operations
- Cursor position management

## Assumptions

- The editor uses a grapheme-aware text representation
- Key events for Backspace and Delete are already captured by the keybinding system
- Cursor position can be properly clamped to valid positions
