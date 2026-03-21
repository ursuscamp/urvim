# Text Objects: Inner Word and Around Word

## Summary

Implement vim-style text objects using a compositional `Operation(Operator, TextObject)` action. This feature adds delete operation combined with inner word (`diw`) and around word (`daw`) text objects, establishing the foundation for operator-pending motions.

## Problem Statement

Currently urvim supports:
- Simple operators (`x`, `dd`, `cc`) that act on fixed targets
- Motions (`w`, `b`, `e`) that move the cursor independently

Missing: **Operator-pending motions** - vim's compositional editing where an operator (like `d`) waits for a motion/text-object to define the target region. For example, `diw` (delete inner word) combines delete operator + inner word text object.

Text objects enable precise, repeatable text manipulation without visual selection mode.

## User Stories

- **As a** vim user, **I want** to delete the word under my cursor without having to manually move to its end, **so that** I can make quick edits with fewer keypresses.

- **As a** vim user, **I want** to delete a word including its surrounding whitespace, **so that** I don't leave extra spaces when removing words.

- **As a** urvim developer, **I want** to use a compositional Operation(Operator, TextObject) architecture, **so that** future operators and text objects can be added without modifying the action enum repeatedly.

## Functional Requirements

- [ ] **REQ-001**: `Action::Operation(Operator, TextObject)` variant handles all operator+text-object combinations
- [ ] **REQ-002**: `diw` (delete inner word) deletes the word/whitespace under the cursor:
  - If cursor is inside a word: deletes the word
  - If cursor is inside whitespace: deletes the whitespace region
- [ ] **REQ-003**: `daw` (delete around word):
  - If cursor is inside a word: deletes the word and ALL trailing whitespace after it
  - If cursor is inside whitespace: deletes the whitespace and the trailing word after it
- [ ] **REQ-004**: Count prefixes work with text objects multiplicatively:
  - Leading count: `3diw` deletes 3 inner words
  - Sub-count: `d3iw` deletes 3 inner words  
  - Combined: `3d3iw` deletes 9 inner words (3 × 3)
  - The existing `CountParser::parse` already handles this correctly
- [ ] **REQ-005**: If the text object motion doesn't complete (e.g., user presses Escape), the operation is cancelled and no text is deleted
- [ ] **REQ-006**: `daw` with cursor on word: trailing whitespace is defined as all consecutive whitespace characters immediately following the word

## Non-Functional Requirements

- **Performance**: Text object resolution should be O(1) - cursor position determines object boundaries without scanning
- **Compatibility**: Behavior matches vim text objects for `iw` and `aw`

## Acceptance Criteria

- [ ] **AC-001**: `Action::Operation(Delete, InnerWord)` is the result of key sequence `d` → `i` → `w`
- [ ] **AC-002**: `diw` with cursor inside "hello" in "hello world" deletes "hello" (cols 0-4)
- [ ] **AC-003**: `diw` with cursor inside "    " (whitespace between words) deletes all whitespace
- [ ] **AC-004**: `daw` with cursor inside "hello" in "hello   world" deletes "hello   " (word + all trailing spaces)
- [ ] **AC-005**: `daw` with cursor inside "    " (whitespace between words) deletes "    world" (whitespace + trailing word)
- [ ] **AC-006**: `d3iw` deletes three consecutive inner words
- [ ] **AC-007**: Pressing Escape during operator-pending state returns to normal mode without deleting
- [ ] **AC-008**: After operation, cursor is positioned at the start of the deleted region
- [ ] **AC-009**: The delete operation creates an undo snapshot

## Out of Scope

- Visual mode text objects (requires visual mode implementation)
- Other text objects: `i(`, `a(`, `i{`, `a{`, `i"`, `a"`, etc.
- Change operator with text objects (`ciw`, `caw`)
- Yank operator with text objects (`yiw`, `yaw`)
- Operator-pending motions without text objects (`dw`, `db` as pure motions)

## Assumptions

- Cursor is positioned somewhere within or adjacent to the word being targeted
- A "word" is defined by the existing `Boundary::Word` semantics (alphanumeric + underscore)
- Inner word (`iw`) selects from word start to word end (exclusive of boundaries)
- Around word (`aw`) includes one whitespace character after the word (if present)

## Dependencies

- `Boundary` enum and word boundary detection logic (existing in `src/buffer.rs`)
- `TrieKeymap` for key sequence matching (existing in `src/editor.rs`)
- Undo/redo system (existing in `src/buffer.rs`)

## Technical Notes

### Architecture: Operation(Operator, TextObject)

Instead of many action variants, use a compositional design:

```rust
pub enum Operator {
    Delete,
    // Change, Yank, etc. (future)
}

pub enum TextObject {
    InnerWord,
    AroundWord,
    // InnerSentence, AroundParagraph, etc. (future)
}

pub enum Action {
    // ... existing actions ...
    Operation(Operator, TextObject),
}
```

### Vim Behavior Reference

| Command | Cursor Position | Deleted Text |
|---------|-----------------|--------------|
| `diw` | inside word "hello" | "hello" |
| `diw` | inside whitespace "    " | all whitespace |
| `daw` | inside word "hello   " | "hello   " (word + all trailing spaces) |
| `daw` | inside whitespace "    wo" | "    wo" (whitespace + trailing word) |

### Key Sequence Flow

```
d → operator-pending (waiting for motion/text-object)
  i → still pending (waiting for text-object completion)
    w → complete: Operation(Delete, InnerWord)
```

If at any point Escape is pressed:
```
d → operator-pending
  <Esc> → cancel, return to normal mode
```
d → operator-pending (waiting for motion)
  i → still pending (waiting for text object completion)
    w → complete: execute DeleteInnerWord
```

If at any point Escape is pressed:
```
d → operator-pending
  <Esc> → cancel, return to normal mode
```
