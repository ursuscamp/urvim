use super::*;

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
        for (row_offset, line_data) in self.line_data.iter().enumerate() {
            let mut col_offset = origin.col;
            for chunk in &line_data.chunks {
                screen.write_string(
                    origin.row + row_offset as u16,
                    col_offset,
                    chunk.style,
                    &chunk.text,
                );
                col_offset += UnicodeWidthStr::width(chunk.text.as_str()) as u16;
            }
        }
    }

    pub fn cursor_screen_position(&self, cursor: Cursor) -> Option<Position> {
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
                        visual_col += UnicodeWidthStr::width(grapheme);
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
