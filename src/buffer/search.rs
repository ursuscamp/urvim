use super::*;

impl Buffer {
    pub fn find_char_forward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let line_str = line.as_ref();
        let start_col = cursor.col + 1;
        let mut occurrences: Vec<usize> = Vec::new();
        for (grapheme_idx, grapheme) in line_str.grapheme_indices(true) {
            if grapheme_idx >= start_col && grapheme.starts_with(target) {
                occurrences.push(grapheme_idx);
            }
        }
        let target_idx = occurrences.get(count.saturating_sub(1))?;
        Some(Cursor::new(line_idx, *target_idx))
    }

    pub fn find_char_backward(&self, cursor: Cursor, target: char, count: usize) -> Option<Cursor> {
        let line_idx = cursor.line;
        let line = self.line_at(line_idx)?;
        let line_str = line.as_ref();
        let occurrences: Vec<usize> = line_str
            .grapheme_indices(true)
            .filter(|&(idx, grapheme)| idx < cursor.col && grapheme.starts_with(target))
            .map(|(idx, _)| idx)
            .collect();
        let target_idx = occurrences.len().saturating_sub(count);
        let idx = *occurrences.get(target_idx)?;
        Some(Cursor::new(line_idx, idx))
    }
}
