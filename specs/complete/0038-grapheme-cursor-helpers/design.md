# Grapheme-Based Cursor Navigation Helpers - Technical Design

## Architecture Overview

This design refactors cursor navigation to use proper Unicode grapheme cluster handling via `grapheme_indices(true)` instead of ad-hoc byte/character arithmetic.

### Current Issues

1. **`bracket_matcher.rs`**: Uses `chars().nth(cursor.col)` which treats `cursor.col` as a char index. But `cursor.col` is a byte offset, so this produces wrong results for multi-byte characters. Also uses `col_idx += 1` increment which is char-based, not grapheme-based.

2. **`window.rs` `insert_char`**: Uses `cursor.col + c.len_utf8()` which is correct for inserting a single character at cursor position, but the logic doesn't account for grapheme boundaries.

### Data Flow

```
Cursor (byte offset)
    │
    ▼
grapheme_indices(true) ──► Iterates by grapheme clusters, returns (byte_offset, grapheme)
    │
    ▼
Compare byte_offset with cursor.col to find next/prev grapheme
    │
    ▼
Return new Cursor with byte offset of target grapheme
```

## Interface Design

### Buffer Methods

| Method | Input | Output | Description |
|--------|-------|--------|-------------|
| `next_cursor_line(cursor: Cursor)` | `Cursor` | `Option<Cursor>` | Returns cursor at next grapheme in same line, or `None` if at end of line |
| `prev_cursor_line(cursor: Cursor)` | `Option<Cursor>` | Returns cursor at previous grapheme in same line, or `None` if at start of line |
| `next_cursor(cursor: Cursor)` | `Cursor` | `Option<Cursor>` | Next grapheme cursor, wrapping to next line. `None` only at end of last line |
| `prev_cursor(cursor: Cursor)` | `Cursor` | `Option<Cursor>` | Previous grapheme cursor, wrapping to prev line. `None` only at start of first line |

### Method Semantics

**`next_cursor_line(cursor)`:**
- If `cursor.col < line_len`: Returns cursor at byte offset of next grapheme after `cursor.col`
- If `cursor.col >= line_len`: Returns `None` (at end of line, no next grapheme)

**`prev_cursor_line(cursor)`:**
- If `cursor.col > 0`: Returns cursor at byte offset of previous grapheme before `cursor.col`
- If `cursor.col == 0`: Returns `None` (at start of line, no previous grapheme)

**`next_cursor(cursor)`:**
- If `cursor.col < line_len`: Same as `next_cursor_line` - next grapheme in same line
- If `cursor.col == line_len` AND `cursor.line < line_count - 1`: Returns cursor at position 0 of next line
- If `cursor.col == line_len` AND `cursor.line == line_count - 1`: Returns `None` (at end of last line)

**`prev_cursor(cursor)`:**
- If `cursor.col > 0`: Same as `prev_cursor_line` - previous grapheme in same line
- If `cursor.col == 0` AND `cursor.line > 0`: Returns cursor at end of previous line
- If `cursor.col == 0` AND `cursor.line == 0`: Returns `None` (at start of first line)

## Data Models

### Cursor (unchanged)

```rust
/// Cursor position in the buffer.
///
/// Line and column (byte position within line).
/// Column can be from 0 to line byte length (inclusive, meaning cursor is at end of line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}
```

## Key Components

### Buffer Methods

**Location:** `src/buffer.rs`

**Implementations:**

```rust
/// Returns the cursor at the next grapheme cluster in the same line.
///
/// Returns None if cursor is at or past the last grapheme in the line.
/// Does NOT wrap to the next line.
///
/// # Arguments
///
/// * `cursor` - Current cursor position (must be valid within buffer)
///
/// # Example
///
/// ```
/// use urvim::buffer::{Buffer, Cursor};
///
/// let buf = Buffer::from_str("a👨‍👩‍👧‍👦b");
/// let cursor = Cursor::new(0, 0);  // at 'a'
/// let next = buf.next_cursor_line(cursor);
/// // Returns cursor at start of emoji (byte ~1)
/// ```
pub fn next_cursor_line(&self, cursor: Cursor) -> Option<Cursor> {
    let line_len = self.line_len(cursor.line);

    if cursor.col >= line_len {
        return None;
    }

    let line = self.lines.get(cursor.line)?;
    let line_str = line.as_ref();

    // Search only the substring after cursor, starting from cursor.col
    // grapheme_indices(true) on a substring gives us byte offsets relative to that substring
    // so we need to add cursor.col to get absolute byte offsets
    for (relative_offset, _grapheme) in line_str[cursor.col..].grapheme_indices(true) {
        if relative_offset == 0 {
            // We're at the first grapheme of the substring, which is the current grapheme
            continue;
        }
        return Some(Cursor::new(cursor.line, cursor.col + relative_offset));
    }

    // At last grapheme, return end of line
    Some(Cursor::new(cursor.line, line_len))
}

/// Returns the cursor at the previous grapheme cluster in the same line.
///
/// Returns None if cursor is at the first grapheme in the line.
/// Does NOT wrap to the previous line.
///
/// # Arguments
///
/// * `cursor` - Current cursor position (must be valid within buffer)
///
/// # Example
///
/// ```
/// use urvim::buffer::{Buffer, Cursor};
///
/// let buf = Buffer::from_str("a👨‍👩‍👧‍👦b");
/// let cursor = Cursor::new(0, 5);  // after emoji
/// let prev = buf.prev_cursor_line(cursor);
/// // Returns cursor at 'a' (byte 0)
/// ```
pub fn prev_cursor_line(&self, cursor: Cursor) -> Option<Cursor> {
    if cursor.col == 0 {
        return None;
    }

    let line = self.lines.get(cursor.line)?;
    let line_str = line.as_ref();

    // Search only the substring before cursor using double-ended iterator
    // The grapheme iterator is DoubleEndedIterator, so we can use .rev().next()
    // to efficiently find the last grapheme in the prefix without O(n) iteration
    let prefix = &line_str[..cursor.col];

    // Get the last grapheme's byte offset in the prefix
    // .rev() reverses the iterator (O(1) for double-ended)
    // .next() gets the last element (now first in reversed order)
    let last_grapheme_offset = prefix
        .grapheme_indices(true)
        .rev()
        .next()
        .map(|(offset, _)| offset)?;

    Some(Cursor::new(cursor.line, last_grapheme_offset))
}

/// Returns the cursor at the next grapheme cluster.
///
/// If at end of line, wraps to start of next line.
/// Returns None only if at end of last line.
///
/// # Arguments
///
/// * `cursor` - Current cursor position
pub fn next_cursor(&self, cursor: Cursor) -> Option<Cursor> {
    let line_len = self.line_len(cursor.line);

    if cursor.col < line_len {
        // Move within current line
        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();

        // Search only the substring after cursor
        for (relative_offset, _grapheme) in line_str[cursor.col..].grapheme_indices(true) {
            if relative_offset == 0 {
                continue;
            }
            return Some(Cursor::new(cursor.line, cursor.col + relative_offset));
        }
        // At last grapheme, return end of line
        Some(Cursor::new(cursor.line, line_len))
    } else if cursor.line < self.lines.len() - 1 {
        // Move to start of next line
        Some(Cursor::new(cursor.line + 1, 0))
    } else {
        // At end of last line, stay in place
        None
    }
}

/// Returns the cursor at the previous grapheme cluster.
///
/// If at start of line, wraps to end of previous line.
/// Returns None only if at start of first line.
///
/// # Arguments
///
/// * `cursor` - Current cursor position
pub fn prev_cursor(&self, cursor: Cursor) -> Option<Cursor> {
    if cursor.col > 0 {
        // Move within current line
        let line = self.lines.get(cursor.line)?;
        let line_str = line.as_ref();

        let prefix = &line_str[..cursor.col];

        let last_grapheme_offset = prefix
            .grapheme_indices(true)
            .rev()
            .next()
            .map(|(offset, _)| offset)?;

        Some(Cursor::new(cursor.line, last_grapheme_offset))
    } else if cursor.line > 0 {
        // Move to end of previous line
        let prev_line_len = self.line_len(cursor.line - 1);
        Some(Cursor::new(cursor.line - 1, prev_line_len))
    } else {
        // At start of first line, stay in place
        None
    }
}
```

### bracket_matcher Module (refactoring)

**Location:** `src/motion/bracket_matcher.rs`

**Changes:**

Refactor `find_matching_forward` and `find_matching_backward` to use grapheme-based iteration:

**Current (incorrect):**
```rust
let chars: Vec<char> = line.chars().collect();
while col_idx < chars.len() {
    let ch = chars[col_idx];
    // ... match logic
    col_idx += 1;  // Wrong: increments by char, not grapheme
}
```

**Corrected approach using efficient substring iteration:**

```rust
fn find_matching_forward(
    buffer: &Buffer,
    start: Cursor,
    open: char,
    close: char,
) -> Option<Cursor> {
    let mut depth = 0;
    let mut line_idx = start.line;

    let total_lines = buffer.line_count();

    while line_idx < total_lines {
        let line = buffer.line_at(line_idx)?;

        // Search from byte position after the opening bracket
        // Use substring from start.col to avoid iterating entire line
        let search_start = start.col + 1;
        let search_range = line[search_start..].grapheme_indices(true);

        // Track absolute byte offset as we iterate
        let mut abs_byte_offset = search_start;

        for (rel_offset, grapheme) in search_range {
            abs_byte_offset = search_start + rel_offset;

            // Check if this grapheme starts with open or close bracket
            if let Some(ch) = grapheme.chars().next() {
                if ch == open {
                    depth += 1;
                } else if ch == close {
                    if depth == 0 {
                        return Some(Cursor::new(line_idx, abs_byte_offset));
                    }
                    depth -= 1;
                }
            }
        }

        // Move to next line
        line_idx += 1;
    }

    None
}
```

**Key improvements:**
1. Uses `line[search_start..].grapheme_indices(true)` to iterate only from the search position, not the entire line
2. Correctly handles byte offsets (not char indices) since `cursor.col` is a byte offset
3. Tracks absolute byte position by adding the substring offset to the base search position
4. Extracts first char from each grapheme cluster with `grapheme.chars().next()` to check for brackets

**Backward search refactoring:**

```rust
fn find_matching_backward(
    buffer: &Buffer,
    start: Cursor,
    close: char,
    open: char,
) -> Option<Cursor> {
    let mut depth = 0;
    let mut line_idx = start.line;

    // Search from byte position before the closing bracket
    // Use substring up to start.col to avoid iterating entire line
    let search_end = start.col;

    while line_idx > 0 || search_end > 0 {
        // Get the line to search
        let line = if search_end > 0 {
            buffer.line_at(line_idx)?
        } else {
            // Move to previous line
            line_idx -= 1;
            if line_idx == 0 && start.col == 0 {
                break;
            }
            let prev_line = buffer.line_at(line_idx)?;
            search_end = prev_line.len();
            prev_line
        };

        // Use reversed grapheme iterator to search backward
        // .take(search_end) limits to graphemes before start.col
        for (rel_offset, grapheme) in line[..search_end].grapheme_indices(true).rev() {
            let abs_byte_offset = rel_offset;

            if let Some(ch) = grapheme.chars().next() {
                if ch == close {
                    depth += 1;
                } else if ch == open {
                    if depth == 0 {
                        return Some(Cursor::new(line_idx, abs_byte_offset));
                    }
                    depth -= 1;
                }
            }
        }

        // Move to previous line for next iteration
        if line_idx == 0 {
            break;
        }
        line_idx -= 1;
        // Get length of new current line for next iteration
        if let Some(prev_line) = buffer.line_at(line_idx) {
            search_end = prev_line.len();
        }
    }

    None
}
```

**Why not use `next_cursor` helpers?**

The `next_cursor` and `prev_cursor` helpers move by ONE grapheme. Using them in bracket_matcher would require calling them repeatedly (O(n²) for n graphemes). Instead, bracket_matcher iterates through all graphemes directly using the same efficient pattern as the helpers - using `grapheme_indices(true)` on a substring.

## User Interaction

### Invoking Bracket Matching

1. User presses `%` key
2. `find_matching_bracket(buffer, cursor)` is called
3. If character at cursor is opening bracket, `find_matching_forward` is called
4. If character at cursor is closing bracket, `find_matching_backward` is called
5. Cursor moves to matching bracket

### Edge Cases

- **Nested brackets**: Depth tracking works correctly with grapheme iteration
- **Multi-byte brackets**: ASCII brackets are always single-byte and their own grapheme, so no issues
- **Emoji/CJK before bracket**: Grapheme iteration correctly skips multi-byte characters
- **Line wrapping**: When reaching end of line, iteration moves to `line_idx + 1` and continues from byte 0 of next line

## External Dependencies

| Dependency | Purpose | Notes |
|------------|---------|-------|
| `unicode_segmentation::UnicodeSegmentation` | Grapheme iteration | Already in use via `grapheme_indices(true)` |
| `unicode_segmentation::UnicodeSegmentation::grapheme_indices` | Get byte offsets of grapheme starts | Already used in buffer.rs |

No new external dependencies required.

## Error Handling

### Cursor Validation

All methods assume a valid cursor (line < line_count, col <= line_len). If invalid:
- Return `None` for navigation methods
- For `find_matching_bracket`, the cursor is validated before calling

### Edge Cases

| Case | Behavior |
|------|----------|
| Cursor at end of last line | `next_cursor` returns `None`, `prev_cursor` goes to end of previous line |
| Cursor at start of first line | `prev_cursor` returns `None`, `next_cursor` goes to start of next line |
| Cursor at end of non-last line | `next_cursor` goes to start of next line |
| Cursor at start of non-first line | `prev_cursor` goes to end of previous line |
| Empty line | `next_cursor_line` returns `None`, `prev_cursor_line` returns `None` |

## Security

No security concerns - this is pure text navigation logic with no external input or sensitive data handling.

## Trade-offs

**Decision**: Create separate `*_cursor_line` methods instead of adding parameters to existing methods

**Reasoning**:
- Clearer intent at call sites: `next_cursor_line` explicitly says "don't cross lines"
- Avoids boolean parameter smell (`cross_line_boundary: bool`)
- Having both versions allows bracket_matcher and similar code to explicitly control boundary behavior

**Impact**:
- Slight code duplication between `next_cursor` and `next_cursor_line` (both iterate using `grapheme_indices`)
- More methods to document, but each is simpler to understand

**Efficiency Note**: The implementations use partial string slicing (`line[cursor.col..]` and `line[..cursor.col]`) to avoid iterating the entire line. For `prev_cursor_line` and `prev_cursor`, the double-ended nature of the grapheme iterator is leveraged with `.last()` to efficiently find the previous grapheme without scanning from the start.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Performance regression from new iteration | Low | Low | Methods use same `grapheme_indices(true)` pattern as existing working code |
| Breaking existing cursor behavior | Low | High | Rename `cursor_right`→`next_cursor` and `cursor_left`→`prev_cursor`, update all call sites |
| Missing bracket matcher edge cases | Medium | Medium | Existing tests + new multi-byte character tests |

## Testing Strategy

### Unit Tests

1. **Basic grapheme navigation** - ASCII characters
2. **Emoji navigation** - Single emoji, emoji sequences, emoji + text
3. **CJK character navigation** - Chinese, Japanese, Korean characters
4. **Combining character navigation** - Characters with combining diacritics
5. **Line boundary crossing** - At end/start of lines with content before/after
6. **Document boundary** - At start of first line, end of last line

### Integration Tests

1. **Bracket matching with emojis** - `foo(👨‍👩‍👧‍👦bar)` - % should match correctly
2. **Bracket matching with CJK** - `函数(参数)` - % should match correctly
3. **Nested brackets with multi-byte** - `(a👨‍👩‍👧‍👦(b)c)` - % should match outer parens

## Files to Modify

| File | Change |
|------|--------|
| `src/buffer.rs` | Rename `cursor_right` → `next_cursor`, `cursor_left` → `prev_cursor`. Add `next_cursor_line`, `prev_cursor_line`. Remove old method names. |
| `src/motion/bracket_matcher.rs` | Refactor `find_matching_forward` and `find_matching_backward` to use grapheme iteration |
| `src/window.rs` | Update all call sites from `cursor_right`/`cursor_left` to `next_cursor`/`prev_cursor`. Review `insert_char` and other methods for incorrect cursor arithmetic |

## Call Sites to Update

### window.rs
- `cursor_right` → `next_cursor`:
  - Line ~1130: in cursor navigation
  - All other usages of `buffer.cursor_right()` and `cursor_right()`
- `cursor_left` → `prev_cursor`:
  - Line ~1173-1174: in cursor navigation
  - All other usages of `buffer.cursor_left()` and `cursor_left()`

### Any other files using cursor_right/cursor_left (grep to find all)

## Implementation Order

1. Add `next_cursor_line` and `prev_cursor_line` to buffer.rs
2. Add `next_cursor` and `prev_cursor` to buffer.rs (using similar logic to existing `cursor_right`/`cursor_left`)
3. Rename `cursor_right` → `next_cursor` and `cursor_left` → `prev_cursor` (remove old methods)
4. Find and update all call sites in window.rs and other files
5. Refactor `bracket_matcher.rs` to use grapheme iteration
6. Add unit tests for all new methods
7. Run full test suite to verify no regressions
