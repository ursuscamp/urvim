use super::*;

impl Window {
    pub(super) fn set_cursor_to_visual_col_on_line(
        &mut self,
        target_line: usize,
        visual_col: usize,
    ) {
        let target_col = self
            .buffer_view
            .with_buffer(|buffer| buffer.byte_pos_at_visual_col(target_line, visual_col))
            .unwrap_or(0);
        self.buffer_view
            .set_cursor(Cursor::new(target_line, target_col));
        self.buffer_view.set_remembered_visual_col(visual_col);
    }

    pub fn move_cursor_left(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.prev_cursor(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_right(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.next_cursor(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_up(&mut self, target_col: usize) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.cursor_up(cursor, target_col))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_down(&mut self, target_col: usize) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.cursor_down(cursor, target_col))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_forward_to(&mut self, boundary: Boundary) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.next_boundary(cursor, boundary))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_back_to(&mut self, boundary: Boundary) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.prev_boundary(cursor, boundary))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_to_char_forward(&mut self, target: char, count: usize) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.find_char_forward(cursor, target, count))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_to_char_backward(&mut self, target: char, count: usize) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.find_char_backward(cursor, target, count))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_till_forward(&mut self, target: char, count: usize) {
        let cursor = self.buffer_view.cursor();
        let new_cursor = self.buffer_view.with_buffer(|buffer| {
            let line = buffer
                .line_at(cursor.line)
                .map(|l| l.as_ref())
                .unwrap_or("");
            let search_start_col = if cursor.col == 0 {
                0
            } else {
                let mut col = cursor.col;
                for (byte_offset, grapheme) in line.grapheme_indices(true) {
                    if byte_offset >= cursor.col {
                        col = byte_offset + grapheme.len();
                        break;
                    }
                }
                col
            };

            let search_cursor = Cursor::new(cursor.line, search_start_col);
            if let Some(new_cursor) = buffer.find_char_forward(search_cursor, target, count) {
                if let Some(prev_cursor) = buffer.prev_cursor_line(new_cursor) {
                    Some(prev_cursor)
                } else {
                    Some(new_cursor)
                }
            } else {
                None
            }
        });

        if let Some(new_cursor) = new_cursor.flatten() {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_till_backward(&mut self, target: char, count: usize) {
        let cursor = self.buffer_view.cursor();
        let new_cursor = self.buffer_view.with_buffer(|buffer| {
            let search_cursor = Cursor::new(cursor.line, cursor.col.saturating_sub(1));

            if let Some(new_cursor) = buffer.find_char_backward(search_cursor, target, count) {
                if let Some(next_cursor) = buffer.next_cursor_line(new_cursor) {
                    Some(next_cursor)
                } else {
                    let line_len = self.buffer_view.line_len(new_cursor.line);
                    Some(Cursor::new(new_cursor.line, line_len))
                }
            } else {
                None
            }
        });

        if let Some(new_cursor) = new_cursor.flatten() {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_to_line_end(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.cursor_end_of_line(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_to_line_start(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.cursor_start_of_line(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_to_line_content_start(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.cursor_content_start_of_line(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_to_previous_paragraph(&mut self) {
        let cursor = self.buffer_view.cursor();
        let target_col = self.buffer_view.get_or_compute_target_col();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.cursor_paragraph_backward(cursor))
            .flatten()
        {
            self.set_cursor_to_visual_col_on_line(new_cursor.line, target_col);
        }
    }

    pub fn move_cursor_to_next_paragraph(&mut self) {
        let cursor = self.buffer_view.cursor();
        let target_col = self.buffer_view.get_or_compute_target_col();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.cursor_paragraph_forward(cursor))
            .flatten()
        {
            self.set_cursor_to_visual_col_on_line(new_cursor.line, target_col);
        }
    }

    pub fn visual_cursor(&self) -> Option<Position> {
        if let Some(mut pos) = self
            .render_data
            .cursor_screen_position(self.buffer_view.cursor())
        {
            let total_lines = self.buffer_view.line_count();
            let gutter = Gutter::new(
                self.buffer_view.scroll_offset().row as usize,
                self.render_data.visible_rows(),
                total_lines,
            );
            pos.col += gutter.calculate_width();
            Some(pos)
        } else {
            None
        }
    }
}
