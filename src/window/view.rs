use super::*;

impl BufferView {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffer,
            scroll_offset: Position::new(0, 0),
            cursor: Cursor::new(0, 0),
            remembered_visual_col: None,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    pub fn scroll_offset(&self) -> Position {
        self.scroll_offset
    }

    pub fn set_scroll_offset(&mut self, offset: Position) {
        self.scroll_offset = offset;
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
    }

    pub fn get_or_compute_target_col(&self) -> usize {
        if let Some(col) = self.remembered_visual_col {
            return col;
        }
        self.buffer.visual_col_at(self.cursor)
    }

    pub fn update_remembered_to_current(&mut self) {
        self.remembered_visual_col = Some(self.buffer.visual_col_at(self.cursor));
    }

    pub fn set_remembered_visual_col(&mut self, col: usize) {
        self.remembered_visual_col = Some(col);
    }

    pub fn scroll_to_cursor(&mut self, viewport_size: Size, gutter_width: u16) {
        let cursor = self.cursor;
        let buffer_line_count = self.buffer.line_count();
        if buffer_line_count == 0 {
            self.scroll_offset = Position::new(0, 0);
            return;
        }

        let visible_rows = viewport_size.rows as usize;
        let visible_cols = viewport_size.cols.saturating_sub(gutter_width) as usize;

        if cursor.line < self.scroll_offset.row as usize {
            self.scroll_offset.row = cursor.line as u16;
        } else if cursor.line >= self.scroll_offset.row as usize + visible_rows {
            self.scroll_offset.row = (cursor.line + 1 - visible_rows) as u16;
        }

        let max_row = buffer_line_count.saturating_sub(visible_rows);
        if self.scroll_offset.row as usize > max_row {
            self.scroll_offset.row = max_row as u16;
        }

        let cursor_visual_col = self.buffer.visual_col_at(cursor);
        if cursor_visual_col < self.scroll_offset.col as usize {
            self.scroll_offset.col = cursor_visual_col as u16;
        } else if cursor_visual_col >= self.scroll_offset.col as usize + visible_cols {
            self.scroll_offset.col = (cursor_visual_col + 1 - visible_cols) as u16;
        }

        if let Some(line) = self.buffer.line_at(cursor.line) {
            let line_width = UnicodeWidthStr::width(line.as_ref());
            let max_col = line_width.saturating_sub(visible_cols);
            if self.scroll_offset.col as usize > max_col {
                self.scroll_offset.col = max_col as u16;
            }
        }
    }

    pub fn build_render_data(&self, size: Size) -> RenderData {
        let mut render_data = RenderData::new(size.rows);
        let buffer = &self.buffer;
        let start_line = self.scroll_offset.row as usize;
        let total_lines_needed = size.rows as usize + 10;
        let horizontal_offset = self.scroll_offset.col as usize;

        for screen_line in 0..total_lines_needed {
            let buffer_line_idx = start_line + screen_line;
            if let Some(line) = buffer.line_at(buffer_line_idx) {
                let line_text = line.as_ref();
                let (byte_offset, width_offset, visible_text) =
                    Self::calculate_horizontal_offset(line_text, horizontal_offset);

                let chunk = RenderChunk::default_text(&visible_text);
                let line_data = LineData {
                    buffer_line: buffer_line_idx,
                    byte_offset,
                    width_offset,
                    chunks: vec![chunk],
                };
                render_data.line_data.push(line_data);
            } else {
                break;
            }
        }

        render_data
    }

    fn calculate_horizontal_offset(
        line_text: &str,
        visual_width_offset: usize,
    ) -> (usize, usize, String) {
        if visual_width_offset == 0 {
            return (0, 0, line_text.to_string());
        }

        let mut current_width = 0;
        let mut byte_offset = 0;

        for grapheme in line_text.graphemes(true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            if current_width + grapheme_width > visual_width_offset {
                break;
            }
            current_width += grapheme_width;
            byte_offset += grapheme.len();
        }

        let actual_line_width = UnicodeWidthStr::width(line_text);
        if byte_offset >= line_text.len() {
            return (line_text.len(), actual_line_width, String::new());
        }

        let visible_text = line_text[byte_offset..].to_string();
        (byte_offset, current_width, visible_text)
    }
}
