# Paragraph Motions

## Summary

Implement `{` and `}` paragraph motions for navigating between blank lines before paragraphs, with count prefix support to move multiple paragraphs at once.

## Problem Statement

Users need efficient navigation between paragraphs in vim-style editing. The `{` and `}` motions move to the blank line before the previous/next paragraph, which is essential for navigating through structured text documents, code with paragraph-style comments, or markdown files.

Currently, no such motion exists in urvim, requiring users to use less efficient line-based navigation.

## User Stories

- **As a** user, **I want to** press `{` to move the cursor to the blank line **before** the previous paragraph, allowing quick upward navigation through document sections.

- **As a** user, **I want to** press `}` to move the cursor to the blank line **before** the next paragraph, allowing quick downward navigation through document sections.

- **As a** user, **I want to** prefix paragraph motions with a count (e.g., `3{`) to move multiple paragraphs at once.

- **As a** user, **I want** paragraph motions to treat an empty line as a paragraph boundary, similar to vim's behavior.

## Functional Requirements

- [ ] **REQ-001**: `{` moves cursor to the blank line **before** the previous paragraph. A paragraph is a sequence of non-empty lines.

- [ ] **REQ-002**: `}` moves cursor to the blank line **before** the next paragraph. A paragraph is a sequence of non-empty lines.

- [ ] **REQ-003**: A "blank line" is a line containing only whitespace characters (or is empty).

- [ ] **REQ-004**: A "paragraph" is defined as a consecutive sequence of non-empty lines (lines with at least one non-whitespace character).

- [ ] **REQ-005**: When `{` is pressed on a non-blank line (inside a paragraph), it searches upward and stops at the first blank line **before** that paragraph.

- [ ] **REQ-006**: When `{` is pressed on a blank line, it searches upward past any non-blank lines, then stops at the first blank line **before** those lines (effectively skipping the previous paragraph).

- [ ] **REQ-007**: When `}` is pressed on a non-blank line (inside a paragraph), it searches downward and stops at the first blank line **after** that paragraph.

- [ ] **REQ-008**: When `}` is pressed on a blank line, it searches downward and stops at the next blank line (if any non-blank lines follow, it finds the blank line after that paragraph).

- [ ] **REQ-009**: Count prefixes work with paragraph motions (e.g., `3{` moves up 3 paragraphs).

- [ ] **REQ-010**: If no blank line/paragraph is found in the search direction, the cursor does not move.

- [ ] **REQ-011**: When `{` lands on a blank line, the cursor should be at column 0 of that blank line.

- [ ] **REQ-012**: When `}` lands on a blank line, the cursor should be at column 0 of that blank line.

- [ ] **REQ-013**: Paragraph motions behave like vertical motions (`j`/`k`) for column preservation - they use and update the remembered visual column.

## Non-Functional Requirements

- **Performance**: Paragraph search should be O(n) where n is the number of lines between cursor and target. No significant slowdown for large files.

- **Usability**: Motion should feel immediate and predictable, matching vim's behavior as closely as possible.

## Acceptance Criteria

- [ ] **AC-001**: In a buffer with paragraphs separated by blank lines:
  ```
  Para 1 line 1
  Para 1 line 2
  
  Para 2 line 1
  ```
  Pressing `{` on "Para 2 line 1" moves to the blank line between Para 1 and Para 2.

- [ ] **AC-002**: Same buffer as AC-001, pressing `}` on "Para 1 line 2" moves to the blank line between Para 1 and Para 2.

- [ ] **AC-003**: `3{` moves up 3 paragraphs (past 3 blank line boundaries).

- [ ] **AC-004**: `2}` moves down 2 paragraphs (past 2 blank line boundaries).

- [ ] **AC-005**: Pressing `{` at the first paragraph with no previous paragraph leaves cursor in place.

- [ ] **AC-006**: Pressing `}` at the last paragraph with no next paragraph leaves cursor in place.

- [ ] **AC-007**: Multiple consecutive blank lines are treated as a single blank line boundary.

- [ ] **AC-008**: A line with only spaces is treated as a blank line.

## Out of Scope

- Section motions (`[[` and `]]`) - these are different motions
- Motion with text objects (e.g., `d{` - delete paragraph)
- Integration with fold methods
- Special handling for comment blocks or other syntax-aware paragraph detection

## Assumptions

- A paragraph is defined purely by blank line separation (standard vim definition)
- Line content is determined by whitespace only (spaces and tabs)
- Cursor lands at column 0 on the target blank line

## Dependencies

- **Internal**:
  - Existing `Buffer` struct for line access
  - Existing `Action` enum and `with_count` trait method
  - Existing `NormalMode` key handling flow
  - Existing `Window::process_action` method
  - Existing vertical motion column preservation logic
- **Blocked by**: None

## Glossary Terms

**Paragraph**: A consecutive sequence of non-empty lines (lines containing at least one non-whitespace character). Paragraphs are separated by one or more blank lines.

**Blank Line**: A line that is either empty or contains only whitespace characters (spaces and/or tabs).

**Paragraph Motion**: A motion (`{` or `}`) that moves the cursor to the blank line before the previous/next paragraph. These are vertical motions that affect the cursor's line position while preserving column position.