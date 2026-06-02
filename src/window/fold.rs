use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldRange {
    pub start_line: usize,
    pub end_line: usize,
}

impl FoldRange {
    fn contains_hidden_line(self, line: usize) -> bool {
        line > self.start_line && line <= self.end_line
    }
}

impl BufferView {
    pub(super) fn folded_range_starting_at(&self, start_line: usize) -> Option<FoldRange> {
        if !self.folded_lines.contains(&start_line) {
            return None;
        }

        self.fold_range_for_line(start_line)
            .filter(|range| range.start_line == start_line)
    }

    pub(super) fn folded_render_ranges(&self) -> Vec<FoldRange> {
        self.folded_lines
            .iter()
            .copied()
            .filter_map(|start_line| self.folded_range_starting_at(start_line))
            .collect()
    }

    pub(super) fn is_line_hidden_by_fold(&self, line: usize) -> bool {
        self.folded_range_containing_hidden_line(line).is_some()
    }

    pub(super) fn next_visible_line_from_hidden(&self, line: usize) -> usize {
        self.folded_range_containing_hidden_line(line)
            .map(|range| range.end_line.saturating_add(1))
            .unwrap_or(line)
    }

    fn folded_range_containing_hidden_line(&self, line: usize) -> Option<FoldRange> {
        self.folded_lines
            .iter()
            .copied()
            .filter_map(|start_line| self.folded_range_starting_at(start_line))
            .find(|range| range.contains_hidden_line(line))
    }

    pub(super) fn open_folds_containing_hidden_line(&mut self, line: usize) -> bool {
        let mut changed = false;
        loop {
            let start_line = self.folded_lines.iter().copied().find(|start_line| {
                self.folded_range_starting_at(*start_line)
                    .is_some_and(|range| range.contains_hidden_line(line))
            });
            match start_line {
                Some(start_line) => {
                    self.folded_lines.remove(&start_line);
                    changed = true;
                }
                None => break,
            }
        }
        if changed {
            crate::session::mark_dirty();
        }
        changed
    }

    pub(super) fn folded_range_at_visible_start(&self, line: usize) -> Option<FoldRange> {
        self.folded_range_starting_at(line)
    }

    pub(super) fn open_fold_starting_at(&mut self, line: usize) -> bool {
        let changed = self.folded_lines.remove(&line);
        if changed {
            crate::session::mark_dirty();
        }
        changed
    }

    pub(super) fn folded_range_before_visible_line(&self, line: usize) -> Option<FoldRange> {
        self.folded_lines
            .iter()
            .copied()
            .filter_map(|start_line| self.folded_range_starting_at(start_line))
            .find(|range| range.end_line.saturating_add(1) == line)
    }

    pub(super) fn next_visible_line_after(&self, line: usize) -> usize {
        if let Some(range) = self.folded_range_starting_at(line) {
            range.end_line.saturating_add(1)
        } else {
            line.saturating_add(1)
        }
    }

    pub(super) fn previous_visible_line_before(&self, line: usize) -> usize {
        if line == 0 {
            return 0;
        }

        let candidate = line - 1;
        self.folded_range_containing_hidden_line(candidate)
            .map(|range| range.start_line)
            .unwrap_or(candidate)
    }

    pub(super) fn visible_row_for_line(&self, line: usize) -> usize {
        let mut row = 0usize;
        let mut current = 0usize;
        while current < line {
            current = self.next_visible_line_after(current);
            row += 1;
        }
        row
    }

    pub(super) fn line_for_visible_row(&self, row: usize) -> usize {
        let line_count = self.line_count();
        if line_count == 0 {
            return 0;
        }

        let mut current_row = 0usize;
        let mut line = 0usize;
        while current_row < row && line + 1 < line_count {
            line = self.next_visible_line_after(line).min(line_count - 1);
            current_row += 1;
        }
        line
    }

    pub(super) fn visible_line_count(&self) -> usize {
        let line_count = self.line_count();
        let mut visible = 0usize;
        let mut line = 0usize;
        while line < line_count {
            visible += 1;
            line = self.next_visible_line_after(line);
        }
        visible.max(1)
    }

    pub(super) fn toggle_fold_at_cursor(&mut self) -> bool {
        let Some(range) = self.fold_range_for_cursor() else {
            return false;
        };

        if !self.folded_lines.remove(&range.start_line) {
            self.close_fold_range(range);
        }
        crate::session::mark_dirty();
        true
    }

    pub(super) fn open_fold_at_cursor(&mut self) -> bool {
        let Some(range) = self.fold_range_for_cursor() else {
            return false;
        };

        let removed = self.folded_lines.remove(&range.start_line);
        if removed {
            crate::session::mark_dirty();
        }
        removed
    }

    pub(super) fn close_fold_at_cursor(&mut self) -> bool {
        let Some(range) = self.fold_range_for_cursor() else {
            return false;
        };

        if self.folded_lines.contains(&range.start_line) {
            return false;
        }

        self.close_fold_range(range);
        crate::session::mark_dirty();
        true
    }

    fn close_fold_range(&mut self, range: FoldRange) {
        self.folded_lines.insert(range.start_line);
        if range.contains_hidden_line(self.cursor.line) {
            self.cursor = Cursor::new(range.start_line, 0);
        }
    }

    fn fold_range_for_cursor(&self) -> Option<FoldRange> {
        let cursor_line = self.cursor.line;
        self.with_buffer(|buffer| {
            if buffer.line_count() == 0 {
                return None;
            }
            let line = cursor_line.min(buffer.line_count().saturating_sub(1));
            fold_range_for_line(buffer, line)
        })
        .flatten()
    }

    fn fold_range_for_line(&self, line: usize) -> Option<FoldRange> {
        self.with_buffer(|buffer| fold_range_for_line(buffer, line))
            .flatten()
    }
}

pub(super) fn fold_range_for_line(buffer: &Buffer, line: usize) -> Option<FoldRange> {
    if buffer.line_count() == 0 {
        return None;
    }

    fold_range_starting_at(buffer, line).or_else(|| containing_indent_fold_range(buffer, line))
}

pub(super) fn fold_range_starting_at(buffer: &Buffer, start_line: usize) -> Option<FoldRange> {
    if buffer
        .line_at(start_line)
        .is_some_and(|text| text.to_string().chars().all(char::is_whitespace))
    {
        return None;
    }

    let base_indent = buffer.line_leading_whitespace_width(start_line)?;
    let mut saw_more_indented_line = false;
    let mut end_line = None;
    for line in start_line + 1..buffer.line_count() {
        if buffer
            .line_at(line)
            .is_some_and(|text| text.to_string().chars().all(char::is_whitespace))
        {
            if saw_more_indented_line {
                end_line = Some(line);
            }
            continue;
        }

        let width = buffer.line_leading_whitespace_width(line)?;
        if width <= base_indent {
            break;
        }
        saw_more_indented_line = true;
        end_line = Some(line);
    }

    end_line.map(|end_line| FoldRange {
        start_line,
        end_line,
    })
}

fn containing_indent_fold_range(buffer: &Buffer, line: usize) -> Option<FoldRange> {
    if line == 0 {
        return None;
    }

    (0..line)
        .rev()
        .filter_map(|candidate| fold_range_starting_at(buffer, candidate))
        .find(|range| range.contains_hidden_line(line))
}
