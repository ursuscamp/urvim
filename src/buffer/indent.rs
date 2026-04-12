use super::*;
use crate::config::TabInsertion;
use crate::globals;

/// Direction used when shifting line indentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentDirection {
    /// Remove one indentation step from the start of the line.
    Decrease,
    /// Add one indentation step to the start of the line.
    Increase,
}

impl Buffer {
    /// Returns the leading whitespace prefix for the given line, even if the line is blank.
    pub fn line_leading_whitespace_prefix(&self, line_idx: usize) -> Option<String> {
        let line = self.line_at(line_idx)?.as_ref();
        let prefix: String = line.chars().take_while(|ch| ch.is_whitespace()).collect();
        Some(prefix)
    }

    /// Returns the visual width of the leading whitespace prefix for the given line.
    pub fn line_leading_whitespace_width(&self, line_idx: usize) -> Option<usize> {
        let prefix = self.line_leading_whitespace_prefix(line_idx)?;
        Some(leading_whitespace_width(&prefix))
    }

    /// Returns the indentation step that should be used when shifting lines in this buffer.
    pub fn resolved_indent_step_prefix(&self) -> String {
        let tab_insertion = self
            .inferred_tab_insertion()
            .or_else(|| globals::with_config(|config| Some(config.tab_insertion)).flatten())
            .unwrap_or_default();

        match tab_insertion {
            TabInsertion::Tabs => "\t".to_string(),
            TabInsertion::Spaces => " ".repeat(crate::buffer::configured_tab_width().max(1)),
        }
    }

    /// Shifts the indentation of the given line by one step and returns the number of bytes changed.
    pub fn shift_line_indentation(
        &mut self,
        line_idx: usize,
        direction: IndentDirection,
    ) -> Option<usize> {
        let _ = self.line_at(line_idx)?;
        match direction {
            IndentDirection::Increase => self.increase_line_indentation(line_idx),
            IndentDirection::Decrease => self.decrease_line_indentation(line_idx),
        }
    }

    /// Inserts one indentation step at the start of the line and returns the inserted byte length.
    pub fn increase_line_indentation(&mut self, line_idx: usize) -> Option<usize> {
        self.line_at(line_idx)?;
        let prefix = self.resolved_indent_step_prefix();
        if prefix.is_empty() {
            return Some(0);
        }

        self.insert_text(Cursor::new(line_idx, 0), &prefix);
        Some(prefix.len())
    }

    /// Removes one indentation step from the start of the line and returns the removed byte length.
    pub fn decrease_line_indentation(&mut self, line_idx: usize) -> Option<usize> {
        let line = self.line_at(line_idx)?.as_ref();
        let prefix = line
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .collect::<String>();
        let remove_len = indentation_step_prefix_byte_len(&prefix);
        if remove_len > 0 {
            self.remove(Cursor::new(line_idx, 0), Cursor::new(line_idx, remove_len));
        }
        Some(remove_len)
    }

    /// Returns the leading-whitespace prefix that should be inserted for a new line at `cursor`.
    ///
    /// The helper inspects nearby non-blank lines and returns the exact leading whitespace
    /// sequence from the most-indented relevant neighbor when one is available.
    pub fn inferred_auto_indent_prefix(&self, cursor: Cursor) -> Option<String> {
        let line_count = self.line_count();
        if line_count == 0 || cursor.line >= line_count {
            return None;
        }

        let mut best: Option<(usize, usize, String)> = None;

        let consider =
            |line_idx: usize, order: usize, best: &mut Option<(usize, usize, String)>| {
                let Some(line) = self.line_at(line_idx).map(|line| line.as_ref()) else {
                    return;
                };
                let Some((prefix, width)) = leading_whitespace_prefix(line) else {
                    return;
                };

                let replace = match best {
                    Some((best_width, best_order, _)) => {
                        width > *best_width || (width == *best_width && order < *best_order)
                    }
                    None => true,
                };

                if replace {
                    *best = Some((width, order, prefix));
                }
            };

        consider(cursor.line, 0, &mut best);
        if cursor.line > 0 {
            consider(cursor.line - 1, 1, &mut best);
        }
        if cursor.line + 1 < line_count {
            consider(cursor.line + 1, 2, &mut best);
        }

        best.map(|(_, _, prefix)| prefix)
    }
}

fn indentation_step_prefix_byte_len(prefix: &str) -> usize {
    let step_width = crate::buffer::configured_tab_width().max(1);
    let mut removed_width = 0;
    let mut removed_bytes = 0;

    for (byte_idx, ch) in prefix.char_indices() {
        let ch_width = if ch == '\t' {
            step_width
        } else {
            crate::buffer::char_width(ch).max(1)
        };
        removed_width += ch_width;
        removed_bytes = byte_idx + ch.len_utf8();
        if removed_width >= step_width {
            break;
        }
    }

    removed_bytes
}

fn leading_whitespace_width(prefix: &str) -> usize {
    let tab_width = crate::buffer::configured_tab_width().max(1);
    prefix.chars().fold(0, |acc, ch| {
        acc + if ch == '\t' {
            tab_width
        } else {
            crate::buffer::char_width(ch).max(1)
        }
    })
}

fn leading_whitespace_prefix(line: &str) -> Option<(String, usize)> {
    if line.chars().all(|ch| ch.is_whitespace()) {
        return None;
    }

    let tab_width = configured_tab_width().max(1);
    let prefix = line
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .collect::<String>();
    let width = prefix
        .chars()
        .fold(0, |acc, ch| acc + if ch == '\t' { tab_width } else { 1 });
    if prefix.is_empty() {
        None
    } else {
        Some((prefix, width))
    }
}
