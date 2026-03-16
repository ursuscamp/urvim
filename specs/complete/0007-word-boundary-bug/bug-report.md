# BUG-001: Small word motions (w/e/b) skip non-word characters

## Summary

The small boundary motions (w, e, b) skip over non-word characters entirely instead of treating them as separate words. For example, with the text "hello---world", pressing `w` goes directly from "hello" to "world", skipping over "---" completely.

## Severity: Medium

- Affects text editing productivity when working with punctuation or special characters
- Workaround: use character-by-character navigation (h/l)
- Not data-corrupting, but significant usability impact for certain text patterns

## Environment

| Field | Value |
|-------|-------|
| App Version | Latest |
| OS | macOS / Linux |
| Terminal | Any |

## Reproduction Steps

1. Open a buffer with text containing non-word characters between words, e.g., "hello---world"
2. Place cursor at the start of the line (position 0)
3. Press `w` to move to the next word start
4. Observe the cursor position

## Expected Behavior

For text "hello---world":
- Position 0 ("h"): pressing `w` should move to position 5 (first "-")
- Position 5 ("-"): pressing `w` should move to position 8 (first "w" of "world")
- Each "word" of non-word characters should be treated as a separate word

## Actual Behavior

For text "hello---world":
- Position 0 ("h"): pressing `w` moves to position 8 (first "w" of "world")
- The "---" is completely skipped

## Impact

- Users cannot efficiently navigate text with punctuation or special characters as separators
- Inconsistent with Vim behavior where "---" would be treated as separate words
- Affects workflows involving code (e.g., "foo->bar"), URLs, or special notation

## Root Cause

The `next_boundary` function in `src/buffer.rs` skips all non-word characters when searching for the next word boundary. After skipping word characters, it continues scanning through any non-word characters (including "---") without treating them as word boundaries.

Location: `src/buffer.rs:951-967`

The problematic code:
```rust
// Skip any whitespace to find the next word
while check_col < line_len {
    let g = line_str
        .get(check_col..)
        .and_then(|s| s.graphemes(true).next());
    match g {
        Some(gg) if Self::is_word_char(gg) => {
            // Found start of next word - return this position
            return Some(Cursor::new(line_idx, check_col));
        }
        Some(gg) => {
            check_col += gg.len();  // BUG: Skips ALL non-word chars
        }
        None => break,
    }
}
```

The fix should check: if after skipping word characters, the next character is a non-word, non-whitespace character, that position is a word boundary and should be returned.

## Solution Approach

**Chosen**: Modify `next_boundary` to treat non-word, non-whitespace characters as separate word boundaries

**Reasoning**:
- Matches Vim's behavior for small word motions
- Minimal code change - only need to add a check after skipping word chars
- Doesn't affect BigWord motions (W, B, E) which already work correctly

**Rejected alternatives**:
- Use `is_at_boundary` inside `next_boundary`: Both approaches have O(n) complexity, but `is_at_boundary` is designed to check a specific position, not scan forward. Using it would require calling it at every position until a boundary is found, adding unnecessary complexity and overhead. The direct fix is cleaner.
- Skip only whitespace: Would not fix the issue since it only addresses whitespace, not punctuation/non-word chars

## Code Changes

| File | Change | Description |
|------|--------|-------------|
| `src/buffer.rs` | Modify | Update `next_boundary` to detect non-word char boundaries |

## Edge Cases

- Empty lines: should continue to next line (existing behavior should work)
- Multiple non-word characters in a row: each should be treated as a separate word boundary
- Non-word chars at start of line: should be treated as first "word"
- Line with only non-word chars: should navigate correctly
- Mixed case: "hello_world---test" should treat each segment correctly
