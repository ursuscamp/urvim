use super::*;

impl Buffer {
    /// Returns the leading whitespace prefix for the given line, even if the line is blank.
    pub fn line_leading_whitespace_prefix(&self, line_idx: usize) -> Option<String> {
        let line = self.line_at(line_idx)?.as_ref();
        let prefix: String = line.chars().take_while(|ch| ch.is_whitespace()).collect();
        Some(prefix)
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

        let consider = |line_idx: usize, order: usize, best: &mut Option<(usize, usize, String)>| {
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

fn leading_whitespace_prefix(line: &str) -> Option<(String, usize)> {
    if line.chars().all(|ch| ch.is_whitespace()) {
        return None;
    }

    let tab_width = configured_tab_width().max(1);
    let prefix = line
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .collect::<String>();
    let width = prefix.chars().fold(0, |acc, ch| {
        acc + if ch == '\t' { tab_width } else { 1 }
    });
    if prefix.is_empty() {
        None
    } else {
        Some((prefix, width))
    }
}
