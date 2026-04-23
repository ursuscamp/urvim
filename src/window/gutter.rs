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
        let gutter_style = self.style;
        let width = self.calculate_width();
        self.render_rows(screen, origin, |screen_row_idx| {
            let buffer_line = start_line + screen_row_idx;
            if buffer_line < total_buffer_lines {
                Some((
                    Self::format_line_number(buffer_line + 1, width),
                    gutter_style,
                ))
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
        state: GutterRenderState,
    ) {
        let total_buffer_lines = self.total_buffer_lines;
        let gutter_style = self.style;
        let gutter_width = self.calculate_width();
        self.render_rows(screen, origin, |screen_row_idx| {
            let line_data = render_data.line_data.get(screen_row_idx)?;

            if line_data.buffer_line >= total_buffer_lines {
                return None;
            }

            let row_style = if state.active_screen_row == Some(screen_row_idx) {
                gutter_style.overlay(state.active_line_style.unwrap_or_default())
            } else {
                gutter_style
            };

            let gutter_line = if !line_data.show_gutter_line_number {
                " ".repeat(gutter_width as usize)
            } else if state.relative_number && line_data.buffer_line != state.cursor_line {
                Self::format_line_number(
                    line_data.buffer_line.abs_diff(state.cursor_line),
                    gutter_width,
                )
            } else {
                Self::format_line_number(line_data.buffer_line + 1, gutter_width)
            };

            Some((gutter_line, row_style))
        });
    }

    fn render_rows<F>(&mut self, screen: &mut Screen, origin: Position, line_for_row: F)
    where
        F: Fn(usize) -> Option<(String, Style)>,
    {
        let width = self.calculate_width();
        let gutter_style = self.style;

        for screen_row in 0..self.visible_rows {
            let screen_row_idx = screen_row as usize;
            let (gutter_line, row_style) = line_for_row(screen_row_idx)
                .unwrap_or_else(|| (" ".repeat(width as usize), gutter_style));

            screen.write_string(origin.row + screen_row, origin.col, row_style, &gutter_line);
        }
    }

    fn format_line_number(line_number: usize, width: u16) -> String {
        let line_str = line_number.to_string();
        let line_width = line_str.len();
        let left_pad_len = width as usize - 1 - line_width;
        let left_pad = " ".repeat(left_pad_len);
        format!("{}{} ", left_pad, line_str)
    }
}
