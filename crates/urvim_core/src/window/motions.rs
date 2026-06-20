use super::*;

#[derive(Clone, Copy)]
enum ViewportAnchor {
    Top,
    Center,
    Bottom,
}

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

    fn move_cursor_by_lines(&mut self, line_delta: usize, upwards: bool) {
        let line_count = self.buffer_view.line_count();
        if line_count == 0 || line_delta == 0 {
            return;
        }

        let cursor = self.buffer_view.cursor();
        let current_line = cursor.line.min(line_count - 1);
        let mut target_line = current_line;
        for _ in 0..line_delta {
            target_line = if upwards {
                self.buffer_view.previous_visible_line_before(target_line)
            } else {
                self.buffer_view
                    .next_visible_line_after(target_line)
                    .min(line_count - 1)
            };
        }
        let target_col = self.buffer_view.get_or_compute_target_col();
        self.set_cursor_to_visual_col_on_line(target_line, target_col);
    }

    /// Moves the cursor up by one viewport height.
    pub fn move_cursor_page_up(&mut self, viewport_rows: usize) {
        self.move_cursor_by_lines(viewport_rows, true);
    }

    /// Moves the cursor down by one viewport height.
    pub fn move_cursor_page_down(&mut self, viewport_rows: usize) {
        self.move_cursor_by_lines(viewport_rows, false);
    }

    /// Moves the cursor up by half of the viewport height.
    pub fn move_cursor_half_page_up(&mut self, viewport_rows: usize) {
        let delta = (viewport_rows / 2).max(1);
        self.move_cursor_by_lines(delta, true);
    }

    /// Moves the cursor down by half of the viewport height.
    pub fn move_cursor_half_page_down(&mut self, viewport_rows: usize) {
        let delta = (viewport_rows / 2).max(1);
        self.move_cursor_by_lines(delta, false);
    }

    fn align_viewport_to_cursor(&mut self, anchor: ViewportAnchor, viewport_rows: usize) {
        if viewport_rows == 0 {
            return;
        }

        let line_count = self.buffer_view.line_count();
        if line_count == 0 {
            return;
        }

        let cursor_line = self.buffer_view.cursor().line.min(line_count - 1);
        let cursor_row = self.buffer_view.visible_row_for_line(cursor_line);
        let unclamped_top_row = match anchor {
            ViewportAnchor::Top => cursor_row,
            ViewportAnchor::Center => cursor_row.saturating_sub(viewport_rows / 2),
            ViewportAnchor::Bottom => cursor_row.saturating_add(1).saturating_sub(viewport_rows),
        };
        let max_top_row = self
            .buffer_view
            .visible_line_count()
            .saturating_sub(viewport_rows);
        let clamped_top_line = self
            .buffer_view
            .line_for_visible_row(unclamped_top_row.min(max_top_row));
        let row = u16::try_from(clamped_top_line).unwrap_or(u16::MAX);
        self.buffer_view
            .set_scroll_offset(Position::new(row, self.buffer_view.scroll_offset().col));
    }

    /// Aligns the viewport so the cursor line is shown on the top row.
    pub fn align_viewport_cursor_top(&mut self, viewport_rows: usize) {
        self.align_viewport_to_cursor(ViewportAnchor::Top, viewport_rows);
    }

    /// Aligns the viewport so the cursor line is shown on the center row.
    pub fn align_viewport_cursor_center(&mut self, viewport_rows: usize) {
        self.align_viewport_to_cursor(ViewportAnchor::Center, viewport_rows);
    }

    /// Aligns the viewport so the cursor line is shown on the bottom row.
    pub fn align_viewport_cursor_bottom(&mut self, viewport_rows: usize) {
        self.align_viewport_to_cursor(ViewportAnchor::Bottom, viewport_rows);
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

    pub fn move_cursor_left_within_line(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.prev_cursor_line(cursor))
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
            let line = if self.buffer_view.is_line_hidden_by_fold(new_cursor.line) {
                self.buffer_view
                    .previous_visible_line_before(new_cursor.line)
            } else {
                new_cursor.line
            };
            self.set_cursor_to_visual_col_on_line(line, target_col);
        }
    }

    pub fn move_cursor_down(&mut self, target_col: usize) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.cursor_down(cursor, target_col))
            .flatten()
        {
            let line = if self.buffer_view.is_line_hidden_by_fold(new_cursor.line) {
                self.buffer_view
                    .next_visible_line_from_hidden(new_cursor.line)
            } else {
                new_cursor.line
            }
            .min(self.buffer_view.line_count().saturating_sub(1));
            self.set_cursor_to_visual_col_on_line(line, target_col);
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
        let new_cursor = self
            .buffer_view
            .with_buffer(|buffer| buffer.find_till_forward(cursor, target, count));

        if let Some(new_cursor) = new_cursor.flatten() {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_till_backward(&mut self, target: char, count: usize) {
        let cursor = self.buffer_view.cursor();
        let new_cursor = self
            .buffer_view
            .with_buffer(|buffer| buffer.find_till_backward(cursor, target, count));

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

    pub fn move_cursor_to_previous_diff_hunk(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.previous_diff_hunk_cursor(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_to_next_diff_hunk(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.next_diff_hunk_cursor(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_to_previous_diff_hunk_end(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.previous_diff_hunk_end_cursor(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_to_next_diff_hunk_end(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .with_buffer(|buffer| buffer.next_diff_hunk_end_cursor(cursor))
            .flatten()
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn visual_cursor(&self) -> Option<Position> {
        if let Some(mut pos) = self
            .render_data
            .cursor_screen_position(self.buffer_view.cursor())
        {
            let total_lines = self.buffer_view.line_count();
            let diagnostic_sign_width =
                diagnostic_sign_width_for_buffer(self.buffer_view.buffer_id_opt());
            let gutter = Gutter::new(
                self.buffer_view.scroll_offset().row as usize,
                self.render_data.visible_rows(),
                total_lines,
            )
            .with_diagnostic_sign_width(diagnostic_sign_width)
            .with_diff_sign_width(diff_sign_width_for_buffer(self.buffer_view.buffer_id_opt()))
            .with_fold_sign_width(FOLD_SIGN_WIDTH);
            pos.col += gutter.calculate_width();
            Some(pos)
        } else {
            None
        }
    }
}
