# Boundary-Based Vim Motions

## Summary

This feature implements a flexible boundary-based motion system for the urvim editor, enabling advanced text navigation including word, WORD, and related motions. The system uses an enum to represent different boundary types and provides methods to check if a cursor position satisfies a boundary and to navigate between boundaries.

## Problem Statement

The current urvim implementation only supports basic character-by-character cursor movement (left, right, up, down). Users need more advanced text navigation capabilities similar to Vim, including:
- Moving by words (alphanumeric + underscore)
- Moving by WORDs (non-whitespace sequences)
- Moving to word/WORD boundaries (start or end)
- Moving forward and backward between these boundaries

Without these motions, users cannot efficiently navigate text, which significantly reduces editing productivity compared to standard Vim behavior.

## User Stories

- **As a** Vim user, **I want** to press `w` to move forward to the start of the next word, **so that** I can quickly skip over words while editing.

- **As a** Vim user, **I want** to press `b` to move backward to the start of the previous word, **so that** I can navigate backwards through text efficiently.

- **As a** Vim user, **I want** to press `e` to move forward to the end of the current/next word, **so that** I can position the cursor at word endings.

- **As a** Vim user, **I want** to press `W` (uppercase) to move forward by WORDs (non-whitespace sequences), **so that** I can navigate past whitespace-separated tokens quickly.

- **As a** Vim user, **I want** to press `B` to move backward by WORDs, **so that** I can navigate backwards past whitespace-separated tokens.

- **As a** Vim user, **I want** to press `E` to move to the end of the current/next WORD, **so that** I can position at WORD endings.

## Functional Requirements

- [ ] **REQ-001**: Create a `Boundary` enum that represents different boundary types for text navigation
- [ ] **REQ-002**: Implement `Buffer::is_at_boundary(cursor, boundary)` method to check if cursor satisfies a boundary
- [ ] **REQ-003**: Implement `Buffer::next_boundary(cursor, boundary)` to find next boundary position forward
- [ ] **REQ-004**: Implement `Buffer::prev_boundary(cursor, boundary)` to find previous boundary position backward
- [ ] **REQ-005**: Support `Word` boundary (alphanumeric + underscore characters)
- [ ] **REQ-006**: Support `WordEnd` boundary (end of word)
- [ ] **REQ-007**: Support `BigWord` boundary (non-whitespace sequences)
- [ ] **REQ-008**: Support `BigWordEnd` boundary (end of BigWord)
- [ ] **REQ-009**: Create action `ForwardTo(Boundary)` for moving to boundary (w, W, e, E)
- [ ] **REQ-010**: Create action `BackTo(Boundary)` for moving backward to boundary (b, B)
- [ ] **REQ-011**: Handle/execute ForwardTo and BackTo actions to update cursor position
- [ ] **REQ-012**: Handle edge cases: cursor at buffer start/end, empty lines, consecutive whitespace
- [ ] **REQ-013**: Handle multiline navigation correctly (wrapping between lines)

## Non-Functional Requirements

- **Performance**: Boundary checks should be O(n) where n is distance to next boundary. Should handle lines with 10,000+ characters efficiently.
- **Unicode Support**: Must properly handle Unicode characters, emoji, and combining characters as single graphemes
- **Consistency**: Behavior should match Vim's motion semantics as closely as possible

## Acceptance Criteria

- [ ] **AC-001**: Pressing `w` in normal mode moves cursor to start of next word
- [ ] **AC-002**: Pressing `b` in normal mode moves cursor to start of previous word
- [ ] **AC-003**: Pressing `e` in normal mode moves cursor to end of current/next word
- [ ] **AC-004**: Pressing `W` moves cursor to start of next BigWord
- [ ] **AC-005**: Pressing `B` moves cursor to start of previous BigWord
- [ ] **AC-006**: Pressing `E` moves cursor to end of current/next BigWord
- [ ] **AC-007**: When reaching end of line while moving forward, continues from start of next line (and vice versa for backward)
- [ ] **AC-008**: Cursor stays in place if no boundary exists in the direction of motion
- [ ] **AC-009**: Buffer boundary methods correctly identify boundaries with proper Unicode handling
- [ ] **AC-010**: All motions have corresponding unit tests

## Out of Scope

- Count prefixes (e.g., `3w` for three words) - future enhancement
- Motions with operators (e.g., `dw` to delete word) - future enhancement
- Sentence (`(`, `)`) and paragraph (`{`, `}`) motions - future enhancement
- Search patterns (`/`, `?`) - future enhancement

## Assumptions

- Grapheme cluster handling is already properly implemented in Buffer (confirmed by existing cursor movement code)
- Unicode segmentation library (`unicode_segmentation`) is available and handles grapheme iteration correctly
- The existing action system in editor.rs can be extended with new variants

## Dependencies

- **Internal**: Buffer module (cursor navigation), Editor module (action handling)
- **External**: `unicode_segmentation` crate (already in use for cursor movement)
