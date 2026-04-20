use super::*;
use crate::buffer::{configured_tab_width, display_grapheme_width, display_width_at, expand_tabs};

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

    pub fn render(&self, screen: &mut Screen, origin: Position) {
        self.render_with_base_style(screen, origin, Style::default());
    }

    /// Renders the buffer content using a supplied base style.
    pub fn render_with_base_style(&self, screen: &mut Screen, origin: Position, base_style: Style) {
        let tab_width = configured_tab_width();
        let (_, screen_cols) = screen.size();
        for (row_offset, line_data) in self.line_data.iter().enumerate() {
            let mut line_visual_col = line_data.width_offset;
            let mut col_offset = origin.col;
            for chunk in &line_data.chunks {
                let style = base_style.overlay(line_data.base_style).overlay(chunk.style);
                let rendered = expand_tabs(&chunk.text, line_visual_col, tab_width);
                screen.write_string(origin.row + row_offset as u16, col_offset, style, &rendered);
                let chunk_width = display_width_at(&chunk.text, line_visual_col, tab_width);
                line_visual_col += chunk_width;
                col_offset += chunk_width as u16;
            }

            if line_data.base_style != Style::default() && col_offset < screen_cols {
                let fill_style = base_style.overlay(line_data.base_style);
                screen.fill_region(
                    origin.row + row_offset as u16,
                    col_offset,
                    1,
                    screen_cols - col_offset,
                    fill_style,
                );
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
        for (screen_row, line_data) in self.line_data.iter().enumerate() {
            if line_data.buffer_line == cursor.line {
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
        }
        None
    }
}
