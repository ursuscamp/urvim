//! Candidate generation for insert-mode completion sources.

pub(super) mod buffer_words;
pub(super) mod paths;

use crate::buffer::{Buffer, Cursor, TextObjectRange, TextRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PathPrefixKind {
    Absolute,
    CurrentDir,
    ParentDir,
    HomeDir,
}

pub(super) fn current_path_prefix(
    buffer: &Buffer,
    cursor: Cursor,
) -> Option<(Cursor, PathPrefixKind, String)> {
    let Some(line) = buffer.line_at(cursor.line) else {
        return None;
    };

    let cursor_col = cursor.col.min(line.len());
    let mut start = cursor_col;

    while start > 0 {
        let Some((prev_start, prev)) = line.previous_char(start) else {
            break;
        };
        if !is_path_char(prev) {
            break;
        }
        start = prev_start;
    }

    if start >= cursor_col {
        return None;
    }

    let kind = if line.range_starts_with(start, cursor_col, "~/") == Some(true) {
        Some(PathPrefixKind::HomeDir)
    } else if line.range_starts_with(start, cursor_col, "../") == Some(true) {
        Some(PathPrefixKind::ParentDir)
    } else if line.range_starts_with(start, cursor_col, "./") == Some(true) {
        Some(PathPrefixKind::CurrentDir)
    } else if line.range_starts_with(start, cursor_col, "/") == Some(true) {
        Some(PathPrefixKind::Absolute)
    } else {
        None
    }?;
    let prefix = line.range_text(start, cursor_col)?;

    Some((Cursor::new(cursor.line, start), kind, prefix))
}

pub(super) fn current_word_prefix(buffer: &Buffer, cursor: Cursor) -> (Cursor, String) {
    let Some(line) = buffer.line_at(cursor.line) else {
        return (cursor, String::new());
    };

    let cursor_col = cursor.col.min(line.len());
    let mut start = cursor_col;

    while start > 0 {
        let Some((prev_start, prev)) = line.previous_char(start) else {
            break;
        };
        if !is_word_char(prev) {
            break;
        }
        start = prev_start;
    }

    if start >= cursor_col {
        return (Cursor::new(cursor.line, cursor_col), String::new());
    }

    let prefix = line.range_text(start, cursor_col).unwrap_or_default();
    (Cursor::new(cursor.line, start), prefix)
}

fn current_word_range(buffer: &Buffer, cursor: Cursor) -> TextObjectRange {
    let (start, _) = current_word_prefix(buffer, cursor);
    TextObjectRange {
        start,
        end: Cursor::new(
            cursor.line,
            cursor.col.min(
                buffer
                    .line_at(cursor.line)
                    .map(|line| line.len())
                    .unwrap_or(0),
            ),
        ),
    }
}

fn unique_words_in_buffer(buffer: &Buffer) -> Vec<String> {
    let mut words = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for line in buffer.lines() {
        let mut offset = 0usize;
        while offset < line.len() {
            let Some((start, ch)) = line.next_char(offset) else {
                break;
            };
            if !is_word_char(ch) {
                offset = start + ch.len_utf8();
                continue;
            }

            let mut end = start + ch.len_utf8();
            while end < line.len() {
                let Some((next_idx, next_ch)) = line.next_char(end) else {
                    break;
                };
                if !is_word_char(next_ch) {
                    break;
                }
                end = next_idx + next_ch.len_utf8();
            }

            let Some(word) = line.range_text(start, end) else {
                break;
            };
            let key = word.to_lowercase();
            if seen.insert(key) {
                words.push(word);
            }
            offset = end;
        }
    }

    words
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn is_path_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '~')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;

    #[test]
    fn current_word_prefix_uses_generic_word_boundaries() {
        let buffer = Buffer::from_str("hello world");
        let (start, prefix) = current_word_prefix(&buffer, Cursor::new(0, 3));

        assert_eq!(start, Cursor::new(0, 0));
        assert_eq!(prefix, "hel");
    }
}
