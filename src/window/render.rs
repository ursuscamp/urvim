use super::*;
use crate::buffer::{configured_tab_width, display_grapheme_width, display_width_at, expand_tabs};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

impl RenderChunk {
    pub fn new(text: &str, style: Style) -> Self {
        Self {
            text: text.to_string(),
            style,
        }
    }

    pub fn default_text(text: &str) -> Self {
        Self {
            text: text.to_string(),
            style: Style::default(),
        }
    }
}

impl RenderData {
    pub fn new(visible_rows: u16) -> Self {
        Self {
            line_data: Vec::with_capacity(visible_rows as usize + 10),
            visible_rows,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_data.len()
    }

    pub fn get_line(&self, screen_line: usize) -> Option<&[RenderChunk]> {
        self.line_data
            .get(screen_line)
            .map(|ld| ld.chunks.as_slice())
    }

    pub fn visible_rows(&self) -> u16 {
        self.visible_rows
    }

    /// Renders the buffer content clipped to the supplied viewport size.
    pub fn render(&self, screen: &mut Screen, origin: Position, size: Size, base_style: Style) {
        let tab_width = configured_tab_width();
        let right_col = origin.col.saturating_add(size.cols);
        for (row_offset, line_data) in self.line_data.iter().take(size.rows as usize).enumerate() {
            let row = origin.row + row_offset as u16;
            let line_base_style = base_style.overlay(line_data.base_style);
            let mut line_visual_col = line_data.width_offset;
            let mut col_offset = origin.col;
            for chunk in &line_data.chunks {
                if col_offset >= right_col {
                    break;
                }

                let style = line_base_style.overlay(chunk.style);
                let rendered = expand_tabs(&chunk.text, line_visual_col, tab_width);
                let remaining_cols = usize::from(right_col.saturating_sub(col_offset));
                let clipped = visible_prefix_by_width(rendered.as_str(), remaining_cols);
                screen.write_string(row, col_offset, style, clipped.as_str());
                let chunk_width = display_width_at(&chunk.text, line_visual_col, tab_width);
                line_visual_col += chunk_width;
                col_offset = col_offset.saturating_add(chunk_width as u16).min(right_col);
            }

            if line_data.base_style != Style::default() && col_offset < right_col {
                screen.fill_region(row, col_offset, 1, right_col - col_offset, line_base_style);
            }
        }
    }

    /// Applies an extra base style to one rendered line.
    pub fn set_line_base_style(&mut self, screen_line: usize, style: Style) {
        if let Some(line_data) = self.line_data.get_mut(screen_line) {
            line_data.base_style = style;
        }
    }

    pub fn cursor_screen_position(&self, cursor: Cursor) -> Option<Position> {
        let tab_width = configured_tab_width();
        for (screen_row, line_data) in self
            .line_data
            .iter()
            .take(self.visible_rows as usize)
            .enumerate()
        {
            if line_data.buffer_line != cursor.line || cursor.col < line_data.byte_offset {
                continue;
            }
            if cursor.col > line_data.end_byte {
                continue;
            }
            if cursor.col == line_data.end_byte
                && let Some(next_line_data) = self.line_data.get(screen_row + 1)
                && next_line_data.buffer_line == cursor.line
                && next_line_data.byte_offset == cursor.col
            {
                continue;
            }

            let mut visual_col = 0;
            let mut byte_pos = line_data.byte_offset;
            for chunk in &line_data.chunks {
                let mut chunk_byte_pos = 0;
                for grapheme in chunk.text.graphemes(true) {
                    if byte_pos + chunk_byte_pos >= cursor.col {
                        break;
                    }
                    visual_col += display_grapheme_width(grapheme, visual_col, tab_width);
                    chunk_byte_pos += grapheme.len();
                }
                byte_pos += chunk.text.len();
            }
            return Some(Position::new(screen_row as u16, visual_col as u16));
        }
        None
    }
}

fn visible_prefix_by_width(text: &str, max_cols: usize) -> String {
    if max_cols == 0 || text.is_empty() {
        return String::new();
    }

    let mut end_byte = 0usize;
    let mut width = 0usize;
    for (byte_idx, grapheme) in text.grapheme_indices(true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if width.saturating_add(grapheme_width) > max_cols {
            break;
        }
        end_byte = byte_idx + grapheme.len();
        width += grapheme_width;
    }

    text[..end_byte].to_string()
}
