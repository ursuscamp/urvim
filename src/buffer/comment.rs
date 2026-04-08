use super::*;
use crate::syntax::builtin_syntax_registry;

impl Buffer {
    /// Returns the active syntax's canonical comment prefix, if one is defined.
    pub fn comment_prefix(&self) -> Option<SmolStr> {
        builtin_syntax_registry()
            .ok()?
            .get_by_name(self.syntax_name())
            .and_then(|definition| definition.metadata.comment_prefix.clone())
    }

    /// Toggles a single line comment prefix at the provided cursor line.
    pub fn toggle_line_comment(&mut self, cursor: Cursor, comment_prefix: &str) -> Option<Cursor> {
        self.toggle_line_comments(cursor, 1, comment_prefix)
    }

    /// Toggles line comments across a consecutive line range using one shared comment column.
    pub fn toggle_line_comments(
        &mut self,
        cursor: Cursor,
        line_count: usize,
        comment_prefix: &str,
    ) -> Option<Cursor> {
        let prefix = comment_prefix.trim();
        if prefix.is_empty() {
            return None;
        }

        let line_idx = cursor.line;
        let total_lines = self.lines.len();
        if line_idx >= total_lines {
            return None;
        }

        let end_line = (line_idx + line_count).min(total_lines);
        let mut target_column: Option<usize> = None;
        let mut uncomment = true;

        for current_line in line_idx..end_line {
            let Some(line) = self.lines.get(current_line) else {
                continue;
            };
            let line = line.as_ref();
            if line.trim().is_empty() {
                continue;
            }

            let indent_end = leading_whitespace_end(line);
            target_column = Some(match target_column {
                Some(existing) => existing.min(indent_end),
                None => indent_end,
            });

            if !line[indent_end..].starts_with(prefix) {
                uncomment = false;
            }
        }

        let Some(target_column) = target_column else {
            return Some(cursor);
        };

        let mut new_cursor = cursor;
        let mut changed_any = false;

        for current_line in line_idx..end_line {
            let Some(line) = self.lines.get(current_line) else {
                continue;
            };
            let line = line.as_ref().to_string();
            if line.trim().is_empty() {
                continue;
            }

            let (new_line, cursor_col) = if uncomment {
                toggle_comment_off(
                    line,
                    target_column,
                    prefix,
                    current_line == line_idx,
                    cursor.col,
                )
            } else {
                toggle_comment_on(
                    line,
                    target_column,
                    prefix,
                    current_line == line_idx,
                    cursor.col,
                )
            };

            self.lines = self.lines.update(current_line, Arc::from(new_line));
            self.invalidate_syntax_from(current_line);
            if current_line == line_idx {
                new_cursor = Cursor::new(line_idx, cursor_col);
            }
            changed_any = true;
        }

        if changed_any {
            Some(new_cursor)
        } else {
            Some(cursor)
        }
    }
}

fn leading_whitespace_end(line: &str) -> usize {
    line.char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, _)| idx)
        .unwrap_or(line.len())
}

fn toggle_comment_off(
    line: String,
    target_column: usize,
    prefix: &str,
    is_cursor_line: bool,
    cursor_col: usize,
) -> (String, usize) {
    if !line[target_column..].starts_with(prefix) {
        return toggle_comment_on(line, target_column, prefix, is_cursor_line, cursor_col);
    }

    let mut removal_end = target_column + prefix.len();
    if line[removal_end..].starts_with(' ') {
        removal_end += 1;
    }

    let removed_len = removal_end - target_column;
    let mut new_line = line;
    new_line.drain(target_column..removal_end);

    let new_col = if is_cursor_line {
        if cursor_col <= target_column {
            cursor_col
        } else if cursor_col < removal_end {
            target_column
        } else {
            cursor_col.saturating_sub(removed_len)
        }
    } else {
        cursor_col
    };

    (new_line, new_col)
}

fn toggle_comment_on(
    line: String,
    target_column: usize,
    prefix: &str,
    is_cursor_line: bool,
    cursor_col: usize,
) -> (String, usize) {
    let insert_text = if line[target_column..].is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix} ")
    };
    let insert_len = insert_text.len();
    let mut new_line = line;
    new_line.insert_str(target_column, &insert_text);

    let new_col = if is_cursor_line {
        if cursor_col < target_column {
            cursor_col
        } else {
            cursor_col + insert_len
        }
    } else {
        cursor_col
    };

    (new_line, new_col)
}
