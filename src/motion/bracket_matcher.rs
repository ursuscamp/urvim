//! Bracket matching module for % key navigation.
//!
//! This module provides functionality to find matching brackets (parentheses,
//! square brackets, and curly braces) for the percent key motion.

use crate::buffer::{Buffer, Cursor, TextRef};

/// Mapping from opening brackets to their matching closing brackets.
const OPENING_BRACKETS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];

/// Mapping from closing brackets to their matching opening brackets.
const CLOSING_BRACKETS: &[(char, char)] = &[(')', '('), (']', '['), ('}', '{')];

/// Find the matching bracket position for the character at the given cursor.
///
/// Returns `None` if:
/// - The character at cursor is not a bracket
/// - No matching bracket exists in the buffer
///
/// # Arguments
///
/// * `buffer` - The text buffer to search in
/// * `cursor` - The cursor position (must be valid)
///
/// # Examples
///
/// ```
/// use urvim::buffer::{Buffer, Cursor};
/// use urvim::motion::bracket_matcher::find_matching_bracket;
///
/// let buf = Buffer::from_str("function(foo) { }");
/// // Cursor at position of '(' (index 8)
/// let result = find_matching_bracket(&buf, Cursor::new(0, 8));
/// assert!(result.is_some());
/// ```
pub fn find_matching_bracket(buffer: &Buffer, cursor: Cursor) -> Option<Cursor> {
    let line = buffer.line_at(cursor.line)?;
    let ch = line.char_at(cursor.col)?;

    // Check if it's an opening bracket
    for (open, close) in OPENING_BRACKETS {
        if ch == *open {
            return find_matching_forward(buffer, cursor, *open, *close);
        }
    }

    // Check if it's a closing bracket
    for (close, open) in CLOSING_BRACKETS {
        if ch == *close {
            return find_matching_backward(buffer, cursor, *close, *open);
        }
    }

    // Not a bracket
    None
}

/// Search forward from cursor to find the matching closing bracket.
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

        // On the first line, start searching after the opening bracket
        // On subsequent lines, search from the beginning of the line
        let search_start = if line_idx == start.line {
            start.col + 1
        } else {
            0
        };

        for (byte_offset, ch) in line.char_indices() {
            if byte_offset < search_start {
                continue;
            }
            if ch == open {
                depth += 1;
            } else if ch == close {
                if depth == 0 {
                    return Some(Cursor::new(line_idx, byte_offset));
                }
                depth -= 1;
            }
        }

        // Move to next line
        line_idx += 1;
    }

    None
}

/// Search backward from cursor to find the matching opening bracket.
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
    let mut search_end = start.col;

    while line_idx > 0 || search_end > 0 {
        // Get the line to search
        let line = buffer.line_at(line_idx)?;

        // On the first iteration (start line), search before start.col
        // On subsequent iterations (previous lines), search from end of line
        let current_search_end = if line_idx == start.line {
            search_end
        } else {
            // After moving to previous line, search from end of that line
            line.len()
        };

        let mut byte_idx = current_search_end;
        while let Some((byte_offset, ch)) = line.previous_char(byte_idx) {
            if ch == close {
                depth += 1;
            } else if ch == open {
                if depth == 0 {
                    return Some(Cursor::new(line_idx, byte_offset));
                }
                depth -= 1;
            }
            byte_idx = byte_offset;
        }

        // Move to previous line for next iteration
        if line_idx == 0 {
            break;
        }
        line_idx -= 1;
        // For the next iteration, search_end will be the full length of the new current line
        search_end = buffer.line_at(line_idx).map(|l| l.len()).unwrap_or(0);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matching_paren_forward() {
        let buf = Buffer::from_str("function(foo)");
        // Position at '(' (index 8)
        let result = find_matching_bracket(&buf, Cursor::new(0, 8));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 12));
    }

    #[test]
    fn test_find_matching_paren_backward() {
        let buf = Buffer::from_str("function(foo)");
        // Position at ')' (index 12)
        let result = find_matching_bracket(&buf, Cursor::new(0, 12));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 8));
    }

    #[test]
    fn test_find_matching_bracket_forward() {
        let buf = Buffer::from_str("[foo, bar]");
        // Position at '[' (index 0)
        let result = find_matching_bracket(&buf, Cursor::new(0, 0));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 9));
    }

    #[test]
    fn test_find_matching_bracket_backward() {
        let buf = Buffer::from_str("[foo, bar]");
        // Position at ']' (index 9)
        let result = find_matching_bracket(&buf, Cursor::new(0, 9));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 0));
    }

    #[test]
    fn test_find_matching_brace_forward() {
        let buf = Buffer::from_str("{ a: 1 }");
        // Position at '{' (index 0)
        let result = find_matching_bracket(&buf, Cursor::new(0, 0));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 7));
    }

    #[test]
    fn test_find_matching_brace_backward() {
        let buf = Buffer::from_str("{ a: 1 }");
        // Position at '}' (index 7)
        let result = find_matching_bracket(&buf, Cursor::new(0, 7));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 0));
    }

    #[test]
    fn test_nested_brackets() {
        let buf = Buffer::from_str("((foo))");
        // Position at first '('
        let result = find_matching_bracket(&buf, Cursor::new(0, 0));
        assert!(result.is_some());
        // Should jump to the last ')' (the outermost match)
        assert_eq!(result.unwrap(), Cursor::new(0, 6));

        // Position at second '('
        let result = find_matching_bracket(&buf, Cursor::new(0, 1));
        assert!(result.is_some());
        // Should jump to the second to last ')' (the inner match)
        assert_eq!(result.unwrap(), Cursor::new(0, 5));
    }

    #[test]
    fn test_non_bracket_character() {
        let buf = Buffer::from_str("hello world");
        // Position at 'h'
        let result = find_matching_bracket(&buf, Cursor::new(0, 0));
        assert!(result.is_none());
    }

    #[test]
    fn test_no_matching_bracket() {
        let buf = Buffer::from_str("(hello");
        // Position at '('
        let result = find_matching_bracket(&buf, Cursor::new(0, 0));
        assert!(result.is_none());
    }

    #[test]
    fn test_multiline_brackets() {
        let buf = Buffer::from_str("(\n  foo\n)");
        // Position at '(' on line 0
        let result = find_matching_bracket(&buf, Cursor::new(0, 0));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(2, 0));

        // Position at ')' on line 2
        let result = find_matching_bracket(&buf, Cursor::new(2, 0));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 0));
    }

    #[test]
    fn test_nested_multiline_brackets() {
        let buf = Buffer::from_str("({[()]})");
        // Position at '{'
        let result = find_matching_bracket(&buf, Cursor::new(0, 0));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 7));

        // Position at '('
        let result = find_matching_bracket(&buf, Cursor::new(0, 1));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 6));

        // Position at '['
        let result = find_matching_bracket(&buf, Cursor::new(0, 2));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), Cursor::new(0, 5));
    }
}
