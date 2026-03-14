# Boundary-Based Vim Motions - Technical Design

## Architecture Overview

This feature extends the existing buffer module with boundary detection capabilities and adds new motion actions to the editor. The design follows a layered approach:

1. **Boundary enum** - Defines boundary types as a closed set of variants
2. **Buffer methods** - Core boundary detection and navigation logic
3. **Action variants** - Editor integration for user-triggered motions

The boundary system works by examining graphemes around a cursor position to determine if it satisfies a particular boundary condition.

## Interface Design

### Boundary Enum

```rust
/// Represents different boundary types for text navigation.
/// 
/// - `Word`: Start of a word (alphanumeric or underscore)
/// - `WordEnd`: End of a word
/// - `BigWord`: Start of a BigWord (non-whitespace sequence)
/// - `BigWordEnd`: End of a BigWord
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// Word boundary (alphanumeric + underscore)
    Word,
    /// End of word boundary
    WordEnd,
    /// BigWord boundary (non-whitespace)
    BigWord,
    /// End of BigWord boundary
    BigWordEnd,
}
```

### Buffer Methods

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| `is_at_boundary` | `cursor: Cursor`, `boundary: Boundary` | `bool` | Check if cursor satisfies boundary |
| `next_boundary` | `cursor: Cursor`, `boundary: Boundary` | `Option<Cursor>` | Find next boundary forward |
| `prev_boundary` | `cursor: Cursor`, `boundary: Boundary` | `Option<Cursor>` | Find previous boundary backward |
| `is_word_char` | `grapheme: &str` | `bool` | Check if grapheme is word character |
| `is_whitespace_char` | `grapheme: &str` | `bool` | Check if grapheme is whitespace |

### Action Variants

| Action | Key | Description |
|--------|-----|-------------|
| `ForwardTo(Boundary)` | `w` / `W` / `e` | Move forward to boundary |
| `BackTo(Boundary)` | `b` / `B` | Move backward to boundary |

### Editor Extension

**Responsibilities:**
- Map key presses to motion actions
- Execute motion actions on the buffer

**Action Variants Added:**
```rust
pub enum Action {
    // ... existing variants ...
    
    // Boundary motions
    ForwardTo(Boundary),   // w, W, e
    BackTo(Boundary),      // b, B
}
```

**Key Mappings (NormalMode):**
```rust
(KeyCode::Char('w'), _) => Action::ForwardTo(Boundary::Word),
(KeyCode::Char('b'), _) => Action::BackTo(Boundary::Word),
(KeyCode::Char('e'), _) => Action::ForwardTo(Boundary::WordEnd),
(KeyCode::Char('W'), _) => Action::ForwardTo(Boundary::BigWord),
(KeyCode::Char('B'), _) => Action::BackTo(Boundary::BigWord),
(KeyCode::Char('E'), _) => Action::ForwardTo(Boundary::BigWordEnd),
```

### Character Classification

| Character Type | Contains | Example |
|---------------|----------|---------|
| Word char | `[a-zA-Z0-9_]` | `'a'`, `'Z'`, `'5'`, `'_'` |
| Whitespace | `[\t\n\r ]` (tab, newline, carriage return, space) | `' '`, `'\n'` |

## Key Components

### Buffer Extension

**Responsibilities:**
- Character classification (word vs non-word)
- Boundary detection at cursor position
- Boundary navigation (find next/previous)

**Public API:**
```rust
impl Buffer {
    /// Check if cursor is at the specified boundary
    pub fn is_at_boundary(&self, cursor: Cursor, boundary: Boundary) -> bool;
    
    /// Find next boundary position forward from cursor
    pub fn next_boundary(&self, cursor: Cursor, boundary: Boundary) -> Option<Cursor>;
    
    /// Find previous boundary position backward from cursor
    pub fn prev_boundary(&self, cursor: Cursor, boundary: Boundary) -> Option<Cursor>;
    
    /// Check if grapheme is a word character (alphanumeric + underscore)
    fn is_word_char(grapheme: &str) -> bool;
    
    /// Check if grapheme is whitespace
    fn is_whitespace_char(grapheme: &str) -> bool;
}
```

**Dependencies:**
- `unicode_segmentation::UnicodeSegmentation` (already in use)
- Existing Buffer methods (`line_at`, `line_len`, `cursor_left`, `cursor_right`)

### Boundary Detection Logic

#### Word Boundary Detection

For `Word` boundary (cursor at start of word):
- Current grapheme is a word character
- Previous grapheme is either:
  - Whitespace, OR
  - Non-existent (start of line/buffer), OR
  - Different category (not word char)

For `WordEnd` boundary (cursor at end of word):
- Current position is AFTER a word character
- Next grapheme is either:
  - Whitespace, OR
  - Non-existent (end of line/buffer), OR
  - Different category (not word char)

#### BigWord Boundary Detection

For `BigWord` boundary (cursor at start of BigWord):
- Current grapheme is NOT whitespace
- Previous grapheme is either:
  - Whitespace, OR
  - Non-existent (start of line/buffer)

For `BigWordEnd` boundary (cursor at end of BigWord):
- Current position is AFTER a non-whitespace character
- Next grapheme is either:
  - Whitespace, OR
  - Non-existent (end of line/buffer)

### Edge Cases

1. **Cursor at buffer start**: `b` returns None, stays in place
2. **Cursor at buffer end**: `w`, `e` return None, stays in place
3. **Empty line**: Motion skips to next/previous non-empty line
4. **Consecutive whitespace**: Skips all whitespace to find next boundary
5. **Cursor in whitespace**: Moves to first non-whitespace in direction
6. **Line wrapping**: When reaching end of line, continues from start of next line (and vice versa for backward)

### Examples

```
Text: "hello world foo"
Cursor: at 'h' (0,0)

- Press w: cursor moves to 'w' (start of "world")
- Press w: cursor moves to 'f' (start of "foo")
- Press b: cursor moves to 'w' (start of "world")  
- Press b: cursor moves to 'h' (start of "hello")
- Press e: cursor moves to 'o' (end of "hello")
- Press e: cursor moves to 'd' (end of "world")

Text: "hello  world"
Cursor: at first 'h'

- Press W: cursor moves to 'w' (start of "world")
- Press E: cursor moves to 'd' (end of "world")
```

## External Dependencies

| Dependency | Purpose | Status |
|------------|---------|--------|
| `unicode_segmentation` | Grapheme iteration | Already in use |
| `unicode-width` | Character width calculation | Already in use |

No new external dependencies required.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| No boundary in direction | Return None, cursor stays |
| Cursor at buffer boundary | Return None |
| Empty buffer | Return None for all motions |
| Invalid cursor position | Return None (defensive) |

## Security

No security concerns - this is purely text navigation logic with no external input handling.

## Configuration

No configuration required - all behavior is hardcoded to match Vim semantics.

## Component Interactions

```
User presses 'w' key
        │
        ▼
NormalMode::handle_key()
        │
        ▼
Action::ForwardTo(Boundary::Word)
        │
        ▼
Editor applies action
        │
        ▼
Buffer::next_boundary(cursor, Boundary::Word)
        │
        ├── is_word_char() checks
        ├── is_whitespace_char() checks
        └── Returns Option<Cursor>
        │
        ▼
Cursor position updated
```

## Trade-offs

**Decision**: Use grapheme-based boundary detection

**Reasoning**:
- Buffer already uses grapheme-based cursor movement
- Unicode characters (emoji, CJK) should be treated as single units
- Matches existing buffer behavior

**Impact**:
- Performance is O(n) where n is distance to next boundary
- For typical use cases (words are short), this is negligible

**Alternative considered**: Byte-based or char-based detection
- Rejected because it would break Unicode handling consistency

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Unicode edge cases | Low | Medium | Extensive test coverage with Unicode strings |
| Performance on long lines | Low | Low | O(n) is acceptable for typical word lengths |
| Inconsistent Vim behavior | Medium | Low | Document differences, make behavior adjustable |

## Testing Strategy

### Unit Tests

1. **Character classification**
   - `is_word_char`: alphanumeric, underscore, non-word chars
   - `is_whitespace_char`: space, tab, newline, non-whitespace

2. **Boundary detection**
   - `is_at_boundary` for each boundary type
   - Edge cases: start/end of line, start/end of buffer
   - Unicode: emoji, CJK, combining characters

3. **Boundary navigation**
   - `next_boundary` / `prev_boundary` for each type
   - Line wrapping behavior
   - Empty lines
   - Multiple consecutive boundaries

### Integration Tests

1. Key mappings in NormalMode
2. Action execution flow
3. Cursor position updates

### Test Cases

```rust
// Word forward (w)
"hello world" cursor at 'h' -> w -> cursor at 'w' (start of "world")
"hello world foo" cursor at 'w' -> w -> cursor at 'f' (start of "foo")
"hello-world" cursor at 'h' -> w -> cursor at '-'

// Word backward (b)
"hello world" cursor at 'd' -> b -> cursor at 'w' (start of "world")
"hello world" cursor at 'w' -> b -> cursor at 'h' (start of "hello")

// Word end forward (e)
"hello" cursor at 'h' -> e -> cursor at 'o' (end of "hello")
"hello world" cursor at 'h' -> e -> cursor at 'o' (end of "hello")
"hello world" cursor at 'o' -> e -> cursor at 'd' (end of "world")

// BigWord forward (W)
"hello  world" cursor at 'h' -> W -> cursor at 'w' (skipping multiple spaces)

// BigWord backward (B)
"hello  world" cursor at 'd' -> B -> cursor at 'w' (start of "world")

// BigWord end forward (E)
"hello world" cursor at 'h' -> E -> cursor after 'o' (end of "hello")

// Edge: cursor in whitespace
"hello world" cursor at space between words -> w -> cursor at 'w'

// Edge: line wrapping
"hello\nworld" cursor at 'o' (end of line 0) -> w -> cursor at 'w' (start of line 1)

// Edge: backward line wrapping  
"hello\nworld" cursor at 'w' (start of line 1) -> b -> cursor at 'h' (start of line 0)
```
