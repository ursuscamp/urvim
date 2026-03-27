use super::*;

impl Gutter {
    pub fn new(start_line: usize, visible_rows: u16, total_buffer_lines: usize) -> Self {
        Self {
            start_line,
            visible_rows,
            total_buffer_lines,
            last_buffer_line: None,
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
            last_buffer_line: None,
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
        let width = self.calculate_width();
        let gutter_style = self.style;

        for screen_row in 0..self.visible_rows {
            let screen_row_idx = screen_row as usize;
            let buffer_line = self.start_line + screen_row_idx;

            if Some(buffer_line) == self.last_buffer_line && buffer_line < self.total_buffer_lines {
                let gutter_line = " ".repeat(width as usize);
                screen.write_string(
                    origin.row + screen_row,
                    origin.col,
                    gutter_style,
                    &gutter_line,
                );
                continue;
            }

            let gutter_line = if buffer_line < self.total_buffer_lines {
                let line_num = buffer_line + 1;
                let line_str = line_num.to_string();
                let line_width = line_str.len();
                let left_pad_len = width as usize - 1 - line_width;
                let left_pad = " ".repeat(left_pad_len);
                format!("{}{} ", left_pad, line_str)
            } else {
                " ".repeat(width as usize)
            };

            screen.write_string(
                origin.row + screen_row,
                origin.col,
                gutter_style,
                &gutter_line,
            );

            if buffer_line < self.total_buffer_lines {
                self.last_buffer_line = Some(buffer_line);
            }
        }
    }
}
