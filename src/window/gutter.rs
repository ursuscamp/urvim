use super::*;

impl Gutter {
    pub fn new(start_line: usize, visible_rows: u16, total_buffer_lines: usize) -> Self {
        Self {
            start_line,
            visible_rows,
            total_buffer_lines,
            style: Style::new().bg(Color::ansi(236)).fg(Color::ansi(245)),
        }
    }

    /// Creates a gutter renderer with an explicit theme-derived style.
    pub fn new_with_style(
        start_line: usize,
        visible_rows: u16,
        total_buffer_lines: usize,
        style: Style,
    ) -> Self {
        Self {
            start_line,
            visible_rows,
            total_buffer_lines,
            style,
        }
    }

    pub fn calculate_width(&self) -> u16 {
        let digits = Self::digit_count(self.total_buffer_lines);
        let min_width = if self.total_buffer_lines == 0 {
            3
        } else {
            digits + 2
        };
        min_width as u16
    }

    pub(super) fn digit_count(n: usize) -> usize {
        if n == 0 {
            return 1;
        }
        n.ilog10() as usize + 1
    }

    pub fn render(&mut self, screen: &mut Screen, origin: Position) {
        let start_line = self.start_line;
        let total_buffer_lines = self.total_buffer_lines;
        self.render_rows(screen, origin, |screen_row_idx| {
            let buffer_line = start_line + screen_row_idx;
            if buffer_line < total_buffer_lines {
                Some((buffer_line, true))
            } else {
                None
            }
        });
    }

    /// Renders gutter rows aligned to the current render-data rows.
    pub fn render_for_render_data(
        &mut self,
        screen: &mut Screen,
        origin: Position,
        render_data: &RenderData,
    ) {
        let total_buffer_lines = self.total_buffer_lines;
        self.render_rows(screen, origin, |screen_row_idx| {
            render_data
                .line_data
                .get(screen_row_idx)
                .and_then(|line_data| {
                    if line_data.show_gutter_line_number {
                        Some((line_data.buffer_line, true))
                    } else {
                        Some((line_data.buffer_line, false))
                    }
                })
                .filter(|(buffer_line, _)| *buffer_line < total_buffer_lines)
        });
    }

    fn render_rows<F>(&mut self, screen: &mut Screen, origin: Position, line_for_row: F)
    where
        F: Fn(usize) -> Option<(usize, bool)>,
    {
        let width = self.calculate_width();
        let gutter_style = self.style;

        for screen_row in 0..self.visible_rows {
            let screen_row_idx = screen_row as usize;
            let gutter_line = if let Some((buffer_line, show_number)) = line_for_row(screen_row_idx)
            {
                if !show_number {
                    " ".repeat(width as usize)
                } else {
                    let line_num = buffer_line + 1;
                    let line_str = line_num.to_string();
                    let line_width = line_str.len();
                    let left_pad_len = width as usize - 1 - line_width;
                    let left_pad = " ".repeat(left_pad_len);
                    format!("{}{} ", left_pad, line_str)
                }
            } else {
                " ".repeat(width as usize)
            };

            screen.write_string(
                origin.row + screen_row,
                origin.col,
                gutter_style,
                &gutter_line,
            );
        }
    }
}
