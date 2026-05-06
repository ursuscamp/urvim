use super::*;

impl Gutter {
    pub fn new(start_line: usize, visible_rows: u16, total_buffer_lines: usize) -> Self {
        Self {
            start_line,
            visible_rows,
            total_buffer_lines,
            diagnostic_sign_width: 0,
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
            diagnostic_sign_width: 0,
            style,
        }
    }

    /// Sets the reserved width for the diagnostic sign column.
    pub fn with_diagnostic_sign_width(mut self, width: u16) -> Self {
        self.diagnostic_sign_width = width;
        self
    }

    pub fn calculate_width(&self) -> u16 {
        let digits = Self::digit_count(self.total_buffer_lines);
        let min_width = if self.total_buffer_lines == 0 {
            3
        } else {
            digits + 2
        };
        min_width as u16 + self.diagnostic_sign_width
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
        let sign_width = state.diagnostic_sign_width;
        let gutter_width = self.calculate_width();
        let nerdfont_enabled =
            crate::globals::with_config(|config| config.nerdfont_enabled()).unwrap_or(false);

        for screen_row in 0..self.visible_rows {
            let screen_row_idx = screen_row as usize;
            let Some(line_data) = render_data.line_data.get(screen_row_idx) else {
                self.write_gutter_row(
                    screen,
                    origin,
                    screen_row,
                    gutter_style,
                    " ".repeat(gutter_width as usize),
                );
                continue;
            };

            if line_data.buffer_line >= total_buffer_lines {
                self.write_gutter_row(
                    screen,
                    origin,
                    screen_row,
                    gutter_style,
                    " ".repeat(gutter_width as usize),
                );
                continue;
            }

            let row_style = if state.active_screen_row == Some(screen_row_idx) {
                gutter_style.overlay(state.active_line_style.unwrap_or_default())
            } else {
                gutter_style
            };

            let mut gutter_line = String::new();
            let sign = if sign_width == 0 {
                String::new()
            } else {
                state
                    .diagnostic_severities
                    .get(screen_row_idx)
                    .copied()
                    .flatten()
                    .map(|severity| {
                        Self::diagnostic_sign_text(severity, sign_width, nerdfont_enabled)
                    })
                    .unwrap_or_else(|| " ".repeat(sign_width as usize))
            };
            gutter_line.push_str(sign.as_str());
            if line_data.show_gutter_line_number {
                let number_width = gutter_width.saturating_sub(sign_width);
                let number = if state.relative_number && line_data.buffer_line != state.cursor_line
                {
                    Self::format_line_number(
                        line_data.buffer_line.abs_diff(state.cursor_line),
                        number_width,
                    )
                } else {
                    Self::format_line_number(line_data.buffer_line + 1, number_width)
                };
                gutter_line.push_str(number.as_str());
            } else {
                gutter_line.push_str(&" ".repeat(gutter_width.saturating_sub(sign_width) as usize));
            }

            self.write_gutter_row(screen, origin, screen_row, row_style, gutter_line);

            if let Some(severity) = state
                .diagnostic_severities
                .get(screen_row_idx)
                .copied()
                .flatten()
            {
                let sign_style = crate::lsp::diagnostics::diagnostic_style_for(severity, row_style);
                for offset in 0..sign_width {
                    if let Some(cell) =
                        screen.get_cell_mut(origin.row + screen_row, origin.col + offset)
                    {
                        cell.style = sign_style;
                    }
                }
            }
        }
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

    fn diagnostic_sign_text(
        severity: lsp_types::DiagnosticSeverity,
        width: u16,
        nerdfont_enabled: bool,
    ) -> String {
        let marker = crate::lsp::diagnostics::diagnostic_marker(severity, nerdfont_enabled);
        if width <= 1 {
            return marker.to_string();
        }

        let marker_width = unicode_width::UnicodeWidthStr::width(marker);
        let padding = width as usize - marker_width;
        format!("{}{}", marker, " ".repeat(padding))
    }

    fn write_gutter_row(
        &self,
        screen: &mut Screen,
        origin: Position,
        row: u16,
        style: Style,
        gutter_line: String,
    ) {
        screen.write_string(origin.row + row, origin.col, style, gutter_line.as_str());
    }
}
