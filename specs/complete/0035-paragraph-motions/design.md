# Paragraph Motions - Technical Design

## 2. Architecture Overview

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│ NormalMode                                                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ TrieKeymap (existing)                                        ││
│  │  - Added: "{" → MoveToPreviousParagraph                      ││
│  │  - Added: "}" → MoveToNextParagraph                          ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│ Window::process_action()                                         │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ Action::MoveToPreviousParagraph  → cursor_paragraph_back()  ││
│  │ Action::MoveToNextParagraph    → cursor_paragraph_forward() ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow

1. User presses `{` → buffer becomes `["{"]` → `TrieKeymap.get_action(["{"])` → `MoveToPreviousParagraph` → complete
2. User presses `5{` → buffer becomes `["5", "{"]` → `CountParser` extracts count=5 → complete → `Count(5, MoveToPreviousParagraph)`
3. `process_action(Count(5, MoveToPreviousParagraph))` → `handle_count_repeatable(5, MoveToPreviousParagraph)` → calls `cursor_paragraph_back()` 5 times

### Key Architectural Decisions

- **Simple trie addition**: Unlike character scan motions, paragraph motions are single-key sequences with no parameters, so they fit naturally in the existing TrieKeymap.
- **Count via repeat mechanism**: Since paragraph motions are repeatable (like `j`, `k`, `w`), the existing `handle_count_repeatable()` mechanism handles count prefixes automatically.
- **Buffer methods for search**: The actual paragraph boundary detection logic lives in the `Buffer` struct as `cursor_paragraph_backward()` and `cursor_paragraph_forward()`.

---

## 3. Interface Design

### Action Enum (new variants)

```rust
pub enum Action {
    // ... existing variants ...

    /// Move cursor to blank line before the previous paragraph
    MoveToPreviousParagraph,
    /// Move cursor to blank line before the next paragraph
    MoveToNextParagraph,
}
```

### Buffer Methods (new)

```rust
impl Buffer {
    /// Move cursor to the blank line before the previous paragraph.
    ///
    /// Returns None if no previous blank line/paragraph is found.
    /// A paragraph is a consecutive sequence of non-empty lines.
    pub fn cursor_paragraph_backward(&self, cursor: Cursor) -> Option<Cursor>;

    /// Move cursor to the blank line before the next paragraph.
    ///
    /// Returns None if no next blank line/paragraph is found.
    /// A paragraph is a consecutive sequence of non-empty lines.
    pub fn cursor_paragraph_forward(&self, cursor: Cursor) -> Option<Cursor>;
}
```

### Window Methods (new)

```rust
impl Window {
    /// Move cursor to the blank line before the previous paragraph.
    pub fn move_cursor_to_previous_paragraph(&mut self);

    /// Move cursor to the blank line before the next paragraph.
    pub fn move_cursor_to_next_paragraph(&mut self);
}
```

---

## 4. Data Models

### Paragraph Detection Rules

| Line Type | Example | Is Paragraph Start | Is Blank |
|-----------|---------|-------------------|----------|
| Empty | `` | No | Yes |
| Whitespace only | `   ` | No | Yes |
| Non-empty | `hello` | Yes | No |

### Paragraph Search Algorithm

**For `{` (backward):**

1. Start from current cursor line
2. If current line is non-blank (inside paragraph), search UPWARD for a blank line
3. Once blank line found, STOP and return cursor at that blank line (column 0)
4. If current line is blank, search UPWARD for the previous non-blank line
5. Search upward for the previous blank line
6. STOP at that blank line (column 0)
7. If no blank line found (at file start), return None

**For `}` (forward):**

1. Start from current cursor line
2. If current line is non-blank (inside paragraph), search DOWNWARD for a blank line
3. Once blank line found, STOP and return cursor at that blank line (column 0)
4. If current line is blank, search DOWNWARD for the next blank line
5. STOP at that blank line (column 0)
6. If no blank line found (at file end), return None

### Edge Cases

| Scenario | `{` Behavior | `}` Behavior |
|----------|--------------|--------------|
| At file start, no previous blank line | Stay in place (return None) | - |
| At file end, no next blank line | - | Stay in place (return None) |
| On blank line | Skip non-blank above, find blank line before them | Find next blank line, stop there |
| On non-blank line (paragraph) | Find blank line BEFORE it, stop there | Find blank line AFTER current paragraph, stop there |
| Multiple consecutive blank lines | Treat as single boundary, stop at first one found | Treat as single boundary, skip to find next after paragraph |
| Line with only spaces | Treat as blank line | Treat as blank line |

---

## 5. Key Components

### Buffer::is_blank_line()

```rust
/// Check if a line is blank (empty or whitespace only).
fn is_blank_line(&self, line_idx: usize) -> bool {
    let line = self.line_at(line_idx)?;
    line.chars().all(|c| c.is_whitespace())
}
```

### Buffer::cursor_paragraph_backward()

```rust
pub fn cursor_paragraph_backward(&self, cursor: Cursor) -> Option<Cursor> {
    let total_lines = self.line_count();
    let mut line_idx = cursor.line;

    // If on non-blank line (inside paragraph), find blank line BEFORE it
    if !self.is_blank_line(line_idx) {
        while line_idx > 0 && !self.is_blank_line(line_idx) {
            line_idx -= 1;
        }
        // Now at blank line or line 0
        if self.is_blank_line(line_idx) {
            return Some(Cursor::new(line_idx, 0));
        }
        return None; // No blank line found
    }

    // On blank line - find previous blank line
    while line_idx > 0 && self.is_blank_line(line_idx) {
        line_idx -= 1;
    }

    // Skip any non-blank lines to find the blank line before them
    while line_idx > 0 && !self.is_blank_line(line_idx) {
        line_idx -= 1;
    }

    // line_idx is now at a blank line or 0
    if self.is_blank_line(line_idx) {
        Some(Cursor::new(line_idx, 0))
    } else {
        None // No blank line found
    }
}
```

### Buffer::cursor_paragraph_forward()

```rust
pub fn cursor_paragraph_forward(&self, cursor: Cursor) -> Option<Cursor> {
    let total_lines = self.line_count();
    let mut line_idx = cursor.line;

    // If on non-blank line (inside paragraph), find blank line AFTER it
    if !self.is_blank_line(line_idx) {
        while line_idx < total_lines && !self.is_blank_line(line_idx) {
            line_idx += 1;
        }
        // Skip any additional blank lines
        while line_idx < total_lines && self.is_blank_line(line_idx) {
            line_idx += 1;
        }
        // Now at non-blank or past EOF
        if line_idx < total_lines {
            // We found the start of next paragraph, find blank line after it
            while line_idx < total_lines && !self.is_blank_line(line_idx) {
                line_idx += 1;
            }
            if line_idx < total_lines {
                return Some(Cursor::new(line_idx, 0));
            }
        }
        return None;
    }

    // On blank line - find next blank line
    while line_idx < total_lines && self.is_blank_line(line_idx) {
        line_idx += 1;
    }

    // Skip non-blank paragraph lines
    while line_idx < total_lines && !self.is_blank_line(line_idx) {
        line_idx += 1;
    }

    // line_idx is now at blank line or past EOF
    if line_idx < total_lines && self.is_blank_line(line_idx) {
        Some(Cursor::new(line_idx, 0))
    } else {
        None // No blank line found
    }
}
```

---

## 6. User Interaction

### Key Sequence Flow

```
Normal Mode (no prefix):
  [no buffer] -- press '{' --> "{" recognized, execute MoveToPreviousParagraph
  [no buffer] -- press '}' --> "}" recognized, execute MoveToNextParagraph

Normal Mode (with count):
  [no buffer] -- press '3' --> buffer = ["3"], waiting = true
  buffer = ["3"] -- press '{' --> execute Count(3, MoveToPreviousParagraph)
  -- handle_count_repeatable calls cursor_paragraph_backward() 3 times --
```

### Column Preservation

Paragraph motions are vertical motions (like `j` and `k`), so they:
- Use the remembered visual column when moving (via `get_or_compute_target_col()`)
- Update the remembered visual column after moving (via `set_remembered_visual_col()`)

---

## 7. External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| None | - | Uses existing Buffer and Window infrastructure |

---

## 8. Error Handling

| Scenario | Behavior |
|----------|----------|
| No previous paragraph (at start) | Cursor stays in place, returns None |
| No next paragraph (at end) | Cursor stays in place, returns None |
| Count exceeds available paragraphs | Executes as many as possible, cursor may land on last available blank line |

---

## 9. Security

Not applicable - this is a local text editor with no network access or external data flow.

---

## 10. Configuration

No new configuration options required.

---

## 11. Component Interactions

### Flow: `{` execution

```
User presses '{'
  → NormalMode::handle_key({)
  → keymap.get_action(["{"]) → Some(MoveToPreviousParagraph)
  → return Complete(MoveToPreviousParagraph)
  → Window::process_action(MoveToPreviousParagraph)
  → Window::move_cursor_to_previous_paragraph()
  → buffer.cursor_paragraph_backward(cursor) → Some(new_cursor)
  → cursor moved to blank line
  → buffer_view.update_remembered_to_current() (column preservation)
```

### Flow: `5}` execution

```
User presses '5}'
  → NormalMode::handle_key(5)
  → buffer = ["5"], waiting = true
  → NormalMode::handle_key(})
  → keymap.get_action(["5", "}"]) → CountParser extracts count=5, action=MoveToNextParagraph
  → return Complete(Count(5, MoveToNextParagraph))
  → Window::process_action(Count(5, MoveToNextParagraph))
  → handle_count_repeatable(5, MoveToNextParagraph)
  → for _ in 0..5 { process_action(MoveToNextParagraph) }
  → each call moves cursor to next paragraph's preceding blank line
```

---

## 12. Platform Considerations

Not applicable - all components are platform-independent Rust code.

---

## 13. Trade-offs

**Decision**: Implement paragraph detection directly in Buffer rather than creating a separate ParagraphNavigator

**Reasoning**:
- Buffer already has all line access methods needed
- Adding cursor movement methods to Buffer follows existing patterns (e.g., `cursor_up`, `cursor_down`, `cursor_end_of_line`)
- Keeps related functionality together

**Alternative considered**: Create a separate `ParagraphNavigator` struct
- Rejected: Would be overkill for simple backward/forward line searches

---

## 14. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Incorrect handling of whitespace-only lines | Low | Medium | Test with lines containing only spaces/tabs |
| Cursor landing in wrong position when multiple blank lines exist | Low | Medium | Treat consecutive blank lines as single boundary |
| Count prefix not working correctly | Low | High | Use existing handle_count_repeatable which is well-tested |

---

## 15. Testing Strategy

### Unit Tests

**Buffer::is_blank_line():**
- Empty line returns true
- Line with only spaces returns true
- Line with only tabs returns true
- Line with content returns false
- Line with content and trailing spaces returns false

**Buffer::cursor_paragraph_backward():**
- From middle of paragraph, moves to blank line before it
- From blank line, moves to blank line before previous paragraph
- From first line of paragraph at file start, returns None
- With count, moves count paragraphs backward

**Buffer::cursor_paragraph_forward():**
- From middle of paragraph, moves to blank line after it
- From blank line, moves to blank line after next paragraph
- From last line of paragraph at file end, returns None
- With count, moves count paragraphs forward

**Window integration:**
- `process_action(MoveToPreviousParagraph)` calls correct buffer method
- `process_action(MoveToNextParagraph)` calls correct buffer method
- Column preservation is applied

**NormalMode:**
- `{` key produces `MoveToPreviousParagraph` action
- `}` key produces `MoveToNextParagraph` action
- Count prefix works (e.g., `5{` → `Count(5, MoveToPreviousParagraph)`)
- Both motions are countable

### Integration Tests

- Navigate through document with multiple paragraphs using `{` and `}`
- Count prefix works correctly through multiple paragraphs
- Cursor stays in place when no more paragraphs in direction
- Multiple blank lines treated as single boundary
