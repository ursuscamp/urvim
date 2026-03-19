//! Window rendering module.
//!
//! This module provides the Window type, which owns a Buffer and is responsible
//! for rendering its content to the screen. A window has a screen position (origin)
//! and size, and renders the buffer content starting from its origin.

use crate::action::{ActionResult, ActionResult::NotHandled};
use crate::buffer::{Boundary, Buffer, Cursor};
use crate::editor::Action;
use crate::screen::Screen;
use crate::terminal::Color;
use crate::terminal::Style;
use crate::widget::Widget;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Position {
    pub row: u16,
    pub col: u16,
}

impl Position {
    pub fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}

impl Size {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols }
    }
}

/// Gutter renders line numbers on the left side of the editor window.
///
/// The gutter displays line numbers for the visible buffer content,
/// with a distinct background color to separate it from the content.
#[derive(Debug, Clone)]
pub struct Gutter {
    /// First visible buffer line (scroll offset)
    start_line: usize,
    /// Number of visible rows in the window
    visible_rows: u16,
    /// Total number of lines in the buffer (for width calculation)
    total_buffer_lines: usize,
    /// Last rendered buffer line number (for wrapping detection)
    last_buffer_line: Option<usize>,
    /// Background color for gutter (ANSI 236 = dark gray)
    background_color: Color,
    /// Foreground color for line numbers (ANSI 245 = light gray)
    foreground_color: Color,
}

impl Gutter {
    /// Creates a new Gutter with viewport info.
    ///
    /// # Arguments
    /// * `start_line` - First visible buffer line (0-indexed)
    /// * `visible_rows` - Number of visible rows in window
    /// * `total_buffer_lines` - Total lines in buffer (for width calculation)
    pub fn new(start_line: usize, visible_rows: u16, total_buffer_lines: usize) -> Self {
        Self {
            start_line,
            visible_rows,
            total_buffer_lines,
            last_buffer_line: None,
            background_color: Color::ansi(236),
            foreground_color: Color::ansi(245),
        }
    }

    /// Calculates the required width for the gutter.
    /// Width = digits(total_buffer_lines) + 2 (1 space padding each side)
    pub fn calculate_width(&self) -> u16 {
        let digits = Self::digit_count(self.total_buffer_lines);
        // Minimum 3 columns: 1 digit + 1 space left + 1 space right
        // (even for empty buffer, show at least "1")
        let min_width = if self.total_buffer_lines == 0 {
            3
        } else {
            digits + 2
        };
        min_width as u16
    }

    /// Counts the number of digits in a number using mathematical calculation.
    /// Returns 1 for 0 (edge case).
    fn digit_count(n: usize) -> usize {
        if n == 0 {
            return 1;
        }
        // Use ilog10 for efficient digit counting
        n.ilog10() as usize + 1
    }

    /// Renders the gutter to the screen at the given position.
    /// Renders ALL visible_rows with background, line numbers for content rows.
    pub fn render(&mut self, screen: &mut Screen, origin: Position) {
        let width = self.calculate_width();
        let gutter_style = Style::new()
            .bg(self.background_color)
            .fg(self.foreground_color);

        // Render each row: create full gutter line string and write it
        for screen_row in 0..self.visible_rows {
            let screen_row_idx = screen_row as usize;
            let buffer_line = self.start_line + screen_row_idx;

            // Skip line number if same buffer line would repeat (for wrapping support)
            if Some(buffer_line) == self.last_buffer_line && buffer_line < self.total_buffer_lines {
                // Still need to render background for this row
                let gutter_line = " ".repeat(width as usize);
                screen.write_string(
                    origin.row + screen_row,
                    origin.col,
                    gutter_style,
                    &gutter_line,
                );
                continue;
            }

            // Create the gutter line: left_pad + line_number + right_pad
            let gutter_line = if buffer_line < self.total_buffer_lines {
                let line_num = buffer_line + 1;
                let line_str = line_num.to_string();
                let line_width = line_str.len();

                // Right-align: left_pad fills to (width - line_width - 1) for right padding
                let left_pad_len = width as usize - 1 - line_width;
                let left_pad = " ".repeat(left_pad_len);
                let right_pad = " "; // 1 space on the right

                format!("{}{}{}", left_pad, line_str, right_pad)
            } else {
                // No valid buffer line - just background
                " ".repeat(width as usize)
            };

            // Write the entire gutter line at once
            screen.write_string(
                origin.row + screen_row,
                origin.col,
                gutter_style,
                &gutter_line,
            );

            // Track last buffer line for wrapping detection
            if buffer_line < self.total_buffer_lines {
                self.last_buffer_line = Some(buffer_line);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderChunk {
    pub text: String,
    pub style: Style,
}

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

#[derive(Debug, Clone)]
pub struct LineData {
    pub buffer_line: usize,
    pub byte_offset: usize,
    pub width_offset: usize,
    pub chunks: Vec<RenderChunk>,
}

#[derive(Debug, Clone)]
pub struct RenderData {
    line_data: Vec<LineData>,
    visible_rows: u16,
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
        use unicode_segmentation::UnicodeSegmentation;
        use unicode_width::UnicodeWidthStr;

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

#[derive(Debug, Clone)]
pub struct BufferView {
    buffer: Buffer,
    scroll_offset: Position,
    cursor: Cursor,
    remembered_visual_col: Option<usize>,
}

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

    /// Get the target column for vertical movement.
    /// Returns remembered column if set, otherwise calculates from current position.
    pub fn get_or_compute_target_col(&self) -> usize {
        if let Some(col) = self.remembered_visual_col {
            return col;
        }
        // First vertical move: use current position
        self.buffer.visual_col_at(self.cursor)
    }

    /// Update remembered column from current cursor position.
    pub fn update_remembered_to_current(&mut self) {
        self.remembered_visual_col = Some(self.buffer.visual_col_at(self.cursor));
    }

    /// Set remembered column to a specific value.
    pub fn set_remembered_visual_col(&mut self, col: usize) {
        self.remembered_visual_col = Some(col);
    }

    /// Adjusts scroll offset to ensure the cursor is visible in the viewport.
    ///
    /// This function implements auto-scrolling by:
    /// 1. Vertical scrolling: If cursor is above or below the visible area, adjust row offset
    /// 2. Horizontal scrolling: If cursor's visual column is left or right of visible area, adjust column offset
    /// 3. Bounds clamping: Ensure scroll offset doesn't go beyond buffer/line boundaries
    ///
    /// The viewport shows content starting from `scroll_offset.row` rows and `scroll_offset.col` visual columns.
    /// Cursor is positioned at the last visible row/column when scrolling brings it into view.
    ///
    /// The `gutter_width` parameter should be the width of the gutter in characters, which is
    /// subtracted from the visible columns to ensure horizontal scrolling accounts for the gutter.
    pub fn scroll_to_cursor(&mut self, viewport_size: Size, gutter_width: u16) {
        let cursor = self.cursor;

        let buffer_line_count = self.buffer.line_count();
        if buffer_line_count == 0 {
            self.scroll_offset = Position::new(0, 0);
            return;
        }

        let visible_rows = viewport_size.rows as usize;
        // Subtract gutter width from visible columns to match render calculation
        let visible_cols = viewport_size.cols.saturating_sub(gutter_width) as usize;

        // Vertical scrolling: Ensure cursor.line is within [scroll_offset.row, scroll_offset.row + visible_rows)
        // If cursor is above viewport, scroll up to show it at the top
        if cursor.line < self.scroll_offset.row as usize {
            self.scroll_offset.row = cursor.line as u16;
        // If cursor is below viewport, scroll down to show it at the bottom
        // cursor.line >= scroll_offset.row + visible_rows means cursor is past the last visible row
        } else if cursor.line >= self.scroll_offset.row as usize + visible_rows {
            // Position cursor at the last visible row: scroll_offset.row = cursor.line - visible_rows + 1
            self.scroll_offset.row = (cursor.line + 1 - visible_rows) as u16;
        }

        // Clamp scroll_offset.row to valid range [0, buffer_line_count - visible_rows]
        // Use saturating_sub to handle case when buffer has fewer lines than viewport
        let max_row = buffer_line_count.saturating_sub(visible_rows);
        if self.scroll_offset.row as usize > max_row {
            self.scroll_offset.row = max_row as u16;
        }

        // Horizontal scrolling: Ensure cursor's visual column is within [scroll_offset.col, scroll_offset.col + visible_cols)
        // Calculate the cursor's visual column position in the line (not screen column)
        let cursor_visual_col = self.buffer.visual_col_at(cursor);

        // If cursor is left of viewport, scroll left to show it at the left edge
        if cursor_visual_col < self.scroll_offset.col as usize {
            self.scroll_offset.col = cursor_visual_col as u16;
        // If cursor is right of viewport, scroll right to show it at the right edge
        // cursor_visual_col >= scroll_offset.col + visible_cols means cursor is past the last visible column
        } else if cursor_visual_col >= self.scroll_offset.col as usize + visible_cols {
            // Position cursor at the last visible column: scroll_offset.col = cursor_visual_col - visible_cols + 1
            self.scroll_offset.col = (cursor_visual_col + 1 - visible_cols) as u16;
        }

        // Clamp scroll_offset.col to not exceed line width minus viewport width
        // This prevents scrolling past the end of the current line
        if let Some(line) = self.buffer.line_at(cursor.line) {
            let line_width = UnicodeWidthStr::width(line.as_ref());
            // Maximum scroll offset is line_width - visible_cols (last visible column is at line_width - 1)
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

    /// Calculates the horizontal byte offset and returns visible text for rendering.
    ///
    /// Given a line of text and a visual width offset (scroll position), this function:
    /// 1. Iterates through graphemes to find the byte position where cumulative visual width exceeds the offset
    /// 2. Returns the byte offset, width at that position, and the sliced visible text
    ///
    /// # Arguments
    /// - `line_text`: The full line of text to process
    /// - `visual_width_offset`: The visual column position to start rendering from (scroll offset)
    ///
    /// # Returns
    /// - `byte_offset`: Byte position in the line where visible text starts
    /// - `width_offset`: Visual width at byte_offset (for cursor position calculation)
    /// - `visible_text`: The substring to render (line[byte_offset..])
    ///
    /// # Example
    /// For line "Hello世" with visual_width_offset = 5:
    /// - "H"(1) + "e"(1) + "l"(1) + "l"(1) + "o"(1) = 5, next char "世"(2) would make 7 > 5
    /// - So byte_offset = 5 (after "Hello"), width_offset = 5, visible_text = "世"
    fn calculate_horizontal_offset(
        line_text: &str,
        visual_width_offset: usize,
    ) -> (usize, usize, String) {
        // Special case: no horizontal scrolling needed
        if visual_width_offset == 0 {
            return (0, 0, line_text.to_string());
        }

        // Iterate through graphemes to find where to start
        // current_width tracks accumulated visual width
        // byte_offset tracks byte position in original string
        let mut current_width = 0;
        let mut byte_offset = 0;

        for grapheme in line_text.graphemes(true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            // Break when adding this grapheme would exceed the scroll offset
            // Using > (not >=) ensures we include the grapheme at exactly the boundary
            if current_width + grapheme_width > visual_width_offset {
                break;
            }
            current_width += grapheme_width;
            byte_offset += grapheme.len();
        }

        // If byte_offset reached end of line, return empty visible text
        let actual_line_width = UnicodeWidthStr::width(line_text);
        if byte_offset >= line_text.len() {
            return (line_text.len(), actual_line_width, String::new());
        }

        // Slice the text from byte_offset to get visible portion
        let visible_text = line_text[byte_offset..].to_string();
        (byte_offset, current_width, visible_text)
    }
}

#[derive(Debug)]
pub struct Window {
    buffer_view: BufferView,
    render_data: RenderData,
    size: Size,
}

impl Window {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffer_view: BufferView::new(buffer),
            render_data: RenderData::new(0),
            size: Size::default(),
        }
    }

    pub fn buffer_view(&self) -> &BufferView {
        &self.buffer_view
    }

    pub fn buffer_view_mut(&mut self) -> &mut BufferView {
        &mut self.buffer_view
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn render_data(&self) -> &RenderData {
        &self.render_data
    }

    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.size = size;
        // Get buffer info for gutter
        let buffer = self.buffer_view.buffer();
        let total_lines = buffer.line_count();
        let start_line = self.buffer_view.scroll_offset().row as usize;

        // Create gutter with needed info (no buffer reference)
        let mut gutter = Gutter::new(start_line, size.rows, total_lines);
        let gutter_width = gutter.calculate_width();

        // Scroll to make cursor visible before rendering
        self.buffer_view.scroll_to_cursor(size, gutter_width);

        // Render gutter at origin position
        gutter.render(screen, origin);

        // Render buffer content offset by gutter width
        let content_origin = Position::new(origin.row, origin.col + gutter_width);
        let content_size = Size::new(size.rows, size.cols.saturating_sub(gutter_width));

        self.render_data = self.buffer_view.build_render_data(content_size);
        self.render_data.render(screen, content_origin);
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.buffer_view.set_cursor(cursor);
    }

    pub fn move_cursor_left(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer().cursor_left(cursor) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_right(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer().cursor_right(cursor) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_up(&mut self, target_col: usize) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer().cursor_up(cursor, target_col) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_down(&mut self, target_col: usize) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer().cursor_down(cursor, target_col) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_forward_to(&mut self, boundary: Boundary) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer().next_boundary(cursor, boundary) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn move_cursor_back_to(&mut self, boundary: Boundary) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer().prev_boundary(cursor, boundary) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    /// Move cursor to end of current line.
    ///
    /// If already at end of line, moves to end of next line.
    pub fn move_cursor_to_line_end(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer().cursor_end_of_line(cursor) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    /// Move cursor to absolute start of line (column 0).
    ///
    /// If already at column 0, does nothing.
    pub fn move_cursor_to_line_start(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer().cursor_start_of_line(cursor) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    /// Move cursor to first non-whitespace of current line.
    ///
    /// If already at first non-whitespace, wraps to previous line's first non-whitespace.
    pub fn move_cursor_to_line_content_start(&mut self) {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .buffer()
            .cursor_content_start_of_line(cursor)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn insert_char(&mut self, c: char) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        buffer.insert_char(cursor, c);
        let new_cursor = match c {
            '\n' => Cursor::new(cursor.line + 1, 0),
            _ => Cursor::new(cursor.line, cursor.col + c.len_utf8()),
        };
        self.buffer_view.set_cursor(new_cursor);
    }

    /// Delete the character before the cursor (backspace).
    pub fn delete_char_before_cursor(&mut self) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        if let Some(new_cursor) = buffer.delete_char_before_cursor(cursor) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    /// Delete the character at the cursor (delete key).
    pub fn delete_char_at_cursor(&mut self) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        if let Some(new_cursor) = buffer.delete_char_at_cursor(cursor) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    /// Join current line with next line, inserting a space between them.
    pub fn join_lines_with_space(&mut self) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        // Join 2 lines (current and next) with space
        if let Some(new_cursor) = buffer.join_lines(cursor.line, 2, true) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    /// Join current line with next line without inserting a space.
    pub fn join_lines_without_space(&mut self) {
        let cursor = self.buffer_view.cursor();
        let buffer = self.buffer_view.buffer_mut();
        // Join 2 lines (current and next) without space
        if let Some(new_cursor) = buffer.join_lines(cursor.line, 2, false) {
            self.buffer_view.set_cursor(new_cursor);
        }
    }

    pub fn visual_cursor(&self) -> Option<Position> {
        // Get cursor position from render data
        if let Some(mut pos) = self
            .render_data
            .cursor_screen_position(self.buffer_view.cursor())
        {
            // Add gutter width to column to get screen position
            let total_lines = self.buffer_view.buffer().line_count();
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

    /// Main dispatcher for Count actions - routes to appropriate handler based on inner action type.
    pub(crate) fn handle_count(&mut self, count: usize, inner: &Action) -> ActionResult {
        match inner {
            // gg and G with count: go to specified line (count-1, clamped to file bounds)
            Action::MoveToFirstLine | Action::MoveToLastLine => {
                self.handle_count_line_motion(count, inner)
            }

            // H with count: go to N lines from top of viewport
            Action::MoveToScreenTop => self.handle_count_screen_motion(count, inner),

            // L with count: go to N lines from bottom of viewport
            Action::MoveToScreenBottom => self.handle_count_screen_motion(count, inner),

            // Join with count: join count+1 lines at once
            Action::JoinWithSpace | Action::JoinWithoutSpace => {
                self.handle_count_join(count, inner)
            }

            // dd with count: delete N lines
            Action::DeleteLine => self.handle_count_delete_line(count),

            // cc with count: change N lines
            Action::ChangeLine => self.handle_count_change_line(count),

            // C with count: change from cursor to end of N lines
            Action::ChangeToLineEnd => self.handle_count_change_to_line_end(count),

            // o with count: create N lines below
            Action::OpenLineBelow => self.handle_count_open_line_below(count),

            // O with count: create N lines above
            Action::OpenLineAbove => self.handle_count_open_line_above(count),

            // Line actions: go to target absolute line, then perform action
            // (must check after specific action types above)
            _ if inner.is_line_action() => self.handle_count_line_action(count, inner),

            // Default: repeatable action - execute count times
            _ => self.handle_count_repeatable(count, inner),
        }
    }

    /// Handles line motions (gg, G) with count - go to absolute line.
    fn handle_count_line_motion(&mut self, count: usize, _action: &Action) -> ActionResult {
        let line_count = self.buffer_view.buffer.line_count();
        if line_count == 0 {
            return ActionResult::Handled;
        }
        let target_line = (count - 1).min(line_count - 1);
        let target_col = self.buffer_view.get_or_compute_target_col();
        self.buffer_view
            .set_cursor(Cursor::new(target_line, target_col));
        // Update remembered column like vertical motions do
        self.buffer_view.set_remembered_visual_col(target_col);
        ActionResult::Handled
    }

    /// Handles screen-relative motions (H, L) with count - N lines from top/bottom of viewport.
    fn handle_count_screen_motion(&mut self, count: usize, action: &Action) -> ActionResult {
        let viewport_rows = self.size.rows as usize;
        if viewport_rows == 0 {
            return ActionResult::Handled;
        }
        let start_line = self.buffer_view.scroll_offset().row as usize;
        let line_count = self.buffer_view.buffer().line_count();
        if line_count == 0 {
            return ActionResult::Handled;
        }

        let target_line = if matches!(action, Action::MoveToScreenTop) {
            // H: go to N lines from top of viewport
            let offset = count.saturating_sub(1);
            (start_line + offset)
                .min(start_line + viewport_rows - 1)
                .min(line_count - 1)
        } else {
            // L: go to N lines from bottom of viewport
            let end_line = (start_line + viewport_rows - 1).min(line_count - 1);
            let offset = count.saturating_sub(1);
            end_line.saturating_sub(offset).max(start_line)
        };

        let target_col = self.buffer_view.get_or_compute_target_col();
        self.buffer_view
            .set_cursor(Cursor::new(target_line, target_col));
        self.buffer_view.set_remembered_visual_col(target_col);
        ActionResult::Handled
    }

    /// Handles line actions (0, $, ^, A, I) with count - go to target line then execute.
    fn handle_count_line_action(&mut self, count: usize, action: &Action) -> ActionResult {
        // Lines are 0-indexed internally, count is 1-indexed
        let target_line = (count as isize - 1).max(0) as usize;
        let current_cursor = self.buffer_view.cursor();
        // Move to target line, preserving column if possible
        self.buffer_view
            .set_cursor(Cursor::new(target_line, current_cursor.col));
        // Then execute the line action
        self.process_action(action)
    }

    /// Handles join motions (J, gJ) with count - join N+1 lines.
    fn handle_count_join(&mut self, count: usize, action: &Action) -> ActionResult {
        // e.g., 2J joins 3 lines (current + 2 more)
        let with_space = matches!(action, Action::JoinWithSpace);
        let cursor = self.buffer_view.cursor();
        let actual_count = count + 1;

        if let Some(new_cursor) =
            self.buffer_view
                .buffer
                .join_lines(cursor.line, actual_count, with_space)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    /// Handles DeleteLine (dd) with count - delete N lines starting from cursor.
    fn handle_count_delete_line(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer.delete_lines(cursor.line, count) {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    /// Handles ChangeLine (cc) with count - change N lines starting from cursor.
    fn handle_count_change_line(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer.change_lines(cursor.line, count) {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    /// Handles ChangeToLineEnd (C) with count - change from cursor to end of N lines.
    fn handle_count_change_to_line_end(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self.buffer_view.buffer.change_to_line_end(cursor, count) {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    /// Handles OpenLineBelow (o) with count - create N lines below current.
    fn handle_count_open_line_below(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .buffer
            .insert_lines_after(cursor.line, count)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    /// Handles OpenLineAbove (O) with count - create N lines above current.
    fn handle_count_open_line_above(&mut self, count: usize) -> ActionResult {
        let cursor = self.buffer_view.cursor();
        if let Some(new_cursor) = self
            .buffer_view
            .buffer
            .insert_lines_before(cursor.line, count)
        {
            self.buffer_view.set_cursor(new_cursor);
        }
        ActionResult::Handled
    }

    /// Default handler: execute repeatable action N times.
    fn handle_count_repeatable(&mut self, count: usize, action: &Action) -> ActionResult {
        for _ in 0..count {
            self.process_action(action);
        }
        ActionResult::Handled
    }
}

impl Widget for Window {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        let result = match action {
            Action::MoveLeft => {
                self.move_cursor_left();
                ActionResult::Handled
            }
            Action::MoveDown => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_down(target_col);
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveUp => {
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.move_cursor_up(target_col);
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveRight => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            Action::InsertChar(c) => {
                self.insert_char(*c);
                ActionResult::Handled
            }
            Action::ForwardTo(boundary) => {
                self.move_cursor_forward_to(*boundary);
                ActionResult::Handled
            }
            Action::BackTo(boundary) => {
                self.move_cursor_back_to(*boundary);
                ActionResult::Handled
            }
            Action::MoveToLineEnd => {
                self.move_cursor_to_line_end();
                ActionResult::Handled
            }
            Action::MoveToLineStart => {
                self.move_cursor_to_line_start();
                ActionResult::Handled
            }
            Action::MoveToLineContentStart => {
                self.move_cursor_to_line_content_start();
                ActionResult::Handled
            }
            Action::MoveToFirstLine => {
                // Go to first line (or specified line with count - handled in Count branch)
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.buffer_view.set_cursor(Cursor::new(0, target_col));
                // Update remembered column like vertical motions do
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveToLastLine => {
                // Go to last line (or specified line with count - handled in Count branch)
                let target_line = self.buffer_view.buffer.line_count().saturating_sub(1);
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.buffer_view
                    .set_cursor(Cursor::new(target_line, target_col));
                // Update remembered column like vertical motions do
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveToScreenTop => {
                // H: Move to top of viewport (or N lines from top)
                // Viewport info comes from the Window's size
                let viewport_rows = self.size.rows as usize;
                if viewport_rows == 0 {
                    return ActionResult::Handled;
                }
                let start_line = self.buffer_view.scroll_offset().row as usize;
                let target_line =
                    start_line.min(self.buffer_view.buffer().line_count().saturating_sub(1));
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.buffer_view
                    .set_cursor(Cursor::new(target_line, target_col));
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveToScreenMiddle => {
                // M: Move to middle of viewport
                let viewport_rows = self.size.rows as usize;
                if viewport_rows == 0 {
                    return ActionResult::Handled;
                }
                let start_line = self.buffer_view.scroll_offset().row as usize;
                let line_count = self.buffer_view.buffer().line_count();
                if line_count == 0 {
                    return ActionResult::Handled;
                }
                let target_line = (start_line + viewport_rows / 2).min(line_count - 1);
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.buffer_view
                    .set_cursor(Cursor::new(target_line, target_col));
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::MoveToScreenBottom => {
                // L: Move to bottom of viewport (or N lines from bottom)
                let viewport_rows = self.size.rows as usize;
                if viewport_rows == 0 {
                    return ActionResult::Handled;
                }
                let start_line = self.buffer_view.scroll_offset().row as usize;
                let line_count = self.buffer_view.buffer().line_count();
                if line_count == 0 {
                    return ActionResult::Handled;
                }
                let end_line = (start_line + viewport_rows - 1).min(line_count - 1);
                let target_line = end_line;
                let target_col = self.buffer_view.get_or_compute_target_col();
                self.buffer_view
                    .set_cursor(Cursor::new(target_line, target_col));
                self.buffer_view.set_remembered_visual_col(target_col);
                ActionResult::Handled
            }
            Action::DeleteBackward => {
                self.delete_char_before_cursor();
                ActionResult::Handled
            }
            Action::DeleteForward => {
                self.delete_char_at_cursor();
                ActionResult::Handled
            }
            Action::AppendAfterCursor => {
                self.move_cursor_right();
                ActionResult::Handled
            }
            Action::AppendToLineEnd => {
                // A should move to after the last character (line_len), not last non-whitespace
                let cursor = self.buffer_view.cursor();
                let line_len = self.buffer_view.buffer.line_len(cursor.line);
                self.buffer_view
                    .set_cursor(Cursor::new(cursor.line, line_len));
                ActionResult::Handled
            }
            Action::InsertAtLineStart => {
                self.move_cursor_to_line_content_start();
                ActionResult::Handled
            }
            Action::JoinWithSpace => {
                self.join_lines_with_space();
                ActionResult::Handled
            }
            Action::JoinWithoutSpace => {
                self.join_lines_without_space();
                ActionResult::Handled
            }
            Action::DeleteLine => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = self.buffer_view.buffer.delete_lines(cursor.line, 1) {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Action::ChangeLine => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = self.buffer_view.buffer.change_lines(cursor.line, 1) {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Action::ChangeToLineEnd => {
                self.handle_count_change_to_line_end(1);
                ActionResult::Handled
            }
            Action::OpenLineBelow => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = self.buffer_view.buffer.insert_lines_after(cursor.line, 1)
                {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Action::OpenLineAbove => {
                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) =
                    self.buffer_view.buffer.insert_lines_before(cursor.line, 1)
                {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Action::MoveToMatchingBracket => {
                use crate::motion::bracket_matcher::find_matching_bracket;

                let cursor = self.buffer_view.cursor();
                if let Some(new_cursor) = find_matching_bracket(&self.buffer_view.buffer, cursor) {
                    self.buffer_view.set_cursor(new_cursor);
                }
                ActionResult::Handled
            }
            Action::Count(count, inner) => {
                return self.handle_count(*count, inner);
            }
            // All other actions are not handled by window
            _ => NotHandled,
        };

        // Centralized column preservation logic
        if action.resets_remembered_column() {
            self.buffer_view.update_remembered_to_current();
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_default() {
        let pos = Position::default();
        assert_eq!(pos.row, 0);
        assert_eq!(pos.col, 0);
    }

    #[test]
    fn test_position_new() {
        let pos = Position::new(5, 10);
        assert_eq!(pos.row, 5);
        assert_eq!(pos.col, 10);
    }

    #[test]
    fn test_size_default() {
        let size = Size::default();
        assert_eq!(size.rows, 0);
        assert_eq!(size.cols, 0);
    }

    #[test]
    fn test_size_new() {
        let size = Size::new(24, 80);
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }

    #[test]
    fn test_buffer_view_new() {
        let buffer = Buffer::from_str("test");
        let view = BufferView::new(buffer);

        assert_eq!(view.scroll_offset(), Position::default());
        assert_eq!(view.cursor(), Cursor::new(0, 0));
    }

    #[test]
    fn test_buffer_view_cursor() {
        let buffer = Buffer::from_str("test");
        let mut view = BufferView::new(buffer);

        view.set_cursor(Cursor::new(0, 2));
        assert_eq!(view.cursor(), Cursor::new(0, 2));
    }

    #[test]
    fn test_buffer_view_scroll_offset() {
        let buffer = Buffer::from_str("test");
        let mut view = BufferView::new(buffer);

        view.set_scroll_offset(Position::new(5, 10));
        assert_eq!(view.scroll_offset(), Position::new(5, 10));
    }

    #[test]
    fn test_window_new() {
        let buffer = Buffer::from_str("test");
        let window = Window::new(buffer);

        assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
    }

    #[test]
    fn test_window_render() {
        let buffer = Buffer::from_str("line1\nline2\nline3");
        let mut window = Window::new(buffer);

        let mut screen = crate::screen::Screen::new(3, 80);
        window.render(&mut screen, Position::new(0, 0), Size::new(3, 80));

        // With gutter (3 columns for 3 lines: digits(3) + 2 = 3), buffer starts at col 3
        // Check gutter background is rendered
        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
        // Check buffer content starts after gutter
        assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "l");
        assert_eq!(screen.get_cell_mut(1, 3).unwrap().text, "l");
    }

    #[test]
    fn test_render_chunk_new() {
        let chunk = RenderChunk::new("test", crate::terminal::Style::default());
        assert_eq!(chunk.text, "test");
    }

    #[test]
    fn test_render_chunk_default_text() {
        let chunk = RenderChunk::default_text("test");
        assert_eq!(chunk.text, "test");
        assert_eq!(chunk.style, crate::terminal::Style::default());
    }

    #[test]
    fn test_render_data_new() {
        let data = RenderData::new(10);
        assert_eq!(data.line_count(), 0);
        assert_eq!(data.visible_rows(), 10);
    }

    #[test]
    fn test_render_data_get_line() {
        let buffer = Buffer::from_str("line1\nline2\nline3");
        let view = BufferView::new(buffer);
        let render_data = view.build_render_data(Size::new(3, 80));

        let line = render_data.get_line(0).unwrap();
        assert!(!line.is_empty());
        assert_eq!(line[0].text, "line1");
    }

    #[test]
    fn test_render_data_out_of_bounds() {
        let buffer = Buffer::from_str("line1\nline2\nline3");
        let view = BufferView::new(buffer);
        let render_data = view.build_render_data(Size::new(3, 80));

        assert!(render_data.get_line(10).is_none());
    }

    // Gutter tests
    #[test]
    fn test_gutter_width_calculation() {
        // 1-9 lines: 1 digit + 2 padding = 3 columns
        let gutter = Gutter::new(0, 10, 9);
        assert_eq!(gutter.calculate_width(), 3);

        // 1-99 lines: 2 digits + 2 padding = 4 columns
        let gutter = Gutter::new(0, 10, 99);
        assert_eq!(gutter.calculate_width(), 4);

        // 1-999 lines: 3 digits + 2 padding = 5 columns
        let gutter = Gutter::new(0, 10, 999);
        assert_eq!(gutter.calculate_width(), 5);

        // Empty buffer: minimum 3 columns
        let gutter = Gutter::new(0, 10, 0);
        assert_eq!(gutter.calculate_width(), 3);
    }

    #[test]
    fn test_gutter_digit_count() {
        assert_eq!(Gutter::digit_count(0), 1);
        assert_eq!(Gutter::digit_count(9), 1);
        assert_eq!(Gutter::digit_count(10), 2);
        assert_eq!(Gutter::digit_count(99), 2);
        assert_eq!(Gutter::digit_count(100), 3);
        assert_eq!(Gutter::digit_count(999), 3);
        assert_eq!(Gutter::digit_count(1000), 4);
    }

    #[test]
    fn test_gutter_render_background() {
        // Use 10 lines so gutter width is 4 (digits(10) + 2 = 4)
        let mut gutter = Gutter::new(0, 5, 10);
        let mut screen = crate::screen::Screen::new(5, 80);

        gutter.render(&mut screen, Position::new(0, 0));

        let gutter_width = gutter.calculate_width();
        assert_eq!(gutter_width, 4); // Verify expected width

        // Check background is rendered for all visible rows in gutter area
        for row in 0..5 {
            for col in 0..gutter_width {
                let _cell = screen.get_cell_mut(row, col).unwrap();
                // Most cells should be spaces (background or padding)
                // Only specific columns should have line numbers
            }
        }

        // Specifically check that gutter cells have spaces (not line numbers)
        // Column 0 should always be space (left padding)
        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, " ");
    }

    #[test]
    fn test_gutter_render_line_numbers() {
        // For 10 lines: digits(10) + 2 = 4 columns
        // Layout: col0=left_pad, col1=empty/1st_digit, col2=2nd_digit/last_digit, col3=right_pad
        let mut gutter = Gutter::new(0, 3, 10);
        let mut screen = crate::screen::Screen::new(3, 80);

        gutter.render(&mut screen, Position::new(0, 0));

        // Width is digits(10) + 2 = 4
        // Line "1": col0=space, col1=space, col2="1", col3=space
        let cell_left_pad = screen.get_cell_mut(0, 0).unwrap();
        assert_eq!(cell_left_pad.text, " "); // left padding
        let cell_empty = screen.get_cell_mut(0, 1).unwrap();
        assert_eq!(cell_empty.text, " "); // empty for 1-digit
        let cell_num = screen.get_cell_mut(0, 2).unwrap();
        assert_eq!(cell_num.text, "1"); // line number right-aligned
        let cell_right_pad = screen.get_cell_mut(0, 3).unwrap();
        assert_eq!(cell_right_pad.text, " "); // right padding

        // Line "2": col0=space, col1=space, col2="2", col3=space
        assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(1, 1).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(1, 2).unwrap().text, "2");
        assert_eq!(screen.get_cell_mut(1, 3).unwrap().text, " ");

        // Line "3": col0=space, col1=space, col2="3", col3=space
        assert_eq!(screen.get_cell_mut(2, 0).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(2, 1).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(2, 2).unwrap().text, "3");
        assert_eq!(screen.get_cell_mut(2, 3).unwrap().text, " ");
    }

    #[test]
    fn test_gutter_wrap_detection() {
        // Simulate scrolling where same buffer line appears in multiple screen rows
        // start_line=5, visible_rows=2 would show buffer lines 5 and 6
        // With 10 lines: width = 4
        // Row 0: buffer line 5 -> "6" at column 2, right padding at column 3
        // Row 1: buffer line 6 -> "7" at column 2, right padding at column 3
        let mut gutter = Gutter::new(5, 2, 10);
        let mut screen = crate::screen::Screen::new(2, 80);

        gutter.render(&mut screen, Position::new(0, 0));

        // Row 0: buffer line 5 -> "6" (1-indexed)
        // Line "6" at column 2 (right-aligned for 1-digit)
        let cell_0 = screen.get_cell_mut(0, 2).unwrap();
        assert_eq!(cell_0.text, "6");

        // Row 1: buffer line 6 -> "7" (1-indexed)
        let cell_1 = screen.get_cell_mut(1, 2).unwrap();
        assert_eq!(cell_1.text, "7");
    }

    #[test]
    fn test_gutter_scroll_offset() {
        // Test gutter with scroll offset
        // With 20 total lines: digits(20) + 2 = 4 columns
        // start_line=10 means first visible is buffer line 10 (display 11, 2 digits)
        let mut gutter = Gutter::new(10, 5, 20);
        let mut screen = crate::screen::Screen::new(5, 80);

        gutter.render(&mut screen, Position::new(0, 0));

        // Verify gutter width
        assert_eq!(gutter.calculate_width(), 4);

        // First visible line is buffer line 10 (1-indexed: 11, 2 digits)
        // Layout: col0=left_pad, col1="1", col2="1", col3=right_pad
        let cell_left_pad = screen.get_cell_mut(0, 0).unwrap();
        assert_eq!(cell_left_pad.text, " "); // left padding
        let cell_digit1 = screen.get_cell_mut(0, 1).unwrap();
        assert_eq!(cell_digit1.text, "1"); // first digit of "11"
        let cell_digit2 = screen.get_cell_mut(0, 2).unwrap();
        assert_eq!(cell_digit2.text, "1"); // second digit of "11"
        let cell_right_pad = screen.get_cell_mut(0, 3).unwrap();
        assert_eq!(cell_right_pad.text, " "); // right padding
    }

    #[test]
    fn test_window_visual_cursor_with_gutter() {
        let buffer = Buffer::from_str("line1\nline2\nline3");
        let mut window = Window::new(buffer);

        // Set cursor to line 0, column 2 (within "line1")
        window.buffer_view_mut().set_cursor(Cursor::new(0, 2));

        // Need to call render to build render_data first
        let size = Size::new(3, 80);
        let mut screen = crate::screen::Screen::new(3, 80);
        window.render(&mut screen, Position::new(0, 0), size);

        // Get visual cursor position
        let cursor_pos = window.visual_cursor();

        assert!(cursor_pos.is_some());
        let pos = cursor_pos.unwrap();

        // Cursor should be offset by gutter width (3 columns for 3 lines)
        // The cursor is at column 2 in the content, plus 3 for gutter = column 5
        let gutter_width = 3; // digits(3) + 2 = 3
        assert_eq!(pos.col, 2 + gutter_width);
    }

    #[test]
    fn test_gutter_scroll_and_rerender() {
        // Simulate scrolling and re-rendering
        // First render at start_line=0
        let mut gutter = Gutter::new(0, 5, 20);
        let mut screen = crate::screen::Screen::new(5, 80);

        gutter.render(&mut screen, Position::new(0, 0));

        // Verify initial render - line 1 should have gutter style
        // For 20 lines, width = digits(20) + 2 = 2 + 2 = 4
        // Line "1" (digit 1): col0=space, col1=space, col2="1", col3=space
        let cell_line1 = screen.get_cell_mut(0, 2).unwrap();
        assert_eq!(cell_line1.text, "1");

        // Now simulate scrolling - create new gutter at start_line=3
        let mut gutter2 = Gutter::new(3, 5, 20);
        let mut screen2 = crate::screen::Screen::new(5, 80);

        gutter2.render(&mut screen2, Position::new(0, 0));

        // After scrolling to line 3, row 0 should show line 4 (buffer line 3 + 1)
        // Line "4": col0=space, col1=space, col2="4", col3=space
        let cell_scrolled = screen2.get_cell_mut(0, 2).unwrap();
        assert_eq!(cell_scrolled.text, "4");

        // Verify gutter background is rendered for ALL rows including empty ones
        // Row 4 would be buffer line 7 which doesn't exist in 20 lines, but background should still be there
        let cell_empty_row = screen2.get_cell_mut(4, 0).unwrap();
        assert_eq!(cell_empty_row.text, " ");
    }

    #[test]
    fn test_gutter_then_buffer_render() {
        // Test that buffer content doesn't overwrite gutter
        // This simulates what happens in Window::render
        let gutter_width = 4; // digits(20) + 2 = 4

        // First render gutter
        let mut gutter = Gutter::new(0, 5, 20);
        let mut screen = crate::screen::Screen::new(5, 80);
        gutter.render(&mut screen, Position::new(0, 0));

        // Verify gutter cells have correct content
        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
        assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");

        // Now simulate buffer content rendering at offset
        let content_origin = Position::new(0, gutter_width);
        let content_size = Size::new(5, 80 - gutter_width);

        // Create some buffer content to render
        let buffer = crate::buffer::Buffer::from_str("line1\nline2\nline3");
        let view = BufferView::new(buffer);
        let render_data = view.build_render_data(content_size);
        render_data.render(&mut screen, content_origin);

        // After buffer rendering, gutter cells should STILL have correct gutter content
        // Gutter is at columns 0-3, buffer is at column 4+
        // Column 0 should still be gutter left padding
        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
        // Column 2 should still have line number "1" (not overwritten by buffer)
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
        // Column 3 should still be gutter right padding
        assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");

        // But buffer content should be at column 4+
        assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, "l"); // "line1"
    }

    #[test]
    fn test_gutter_width_change() {
        // Test gutter when width changes (e.g., file grows from 99 to 100 lines)
        // Old gutter width = 4 (digits(99) + 2 = 2 + 2)
        // New gutter width = 5 (digits(100) + 2 = 3 + 2)

        // Simulate first render with width=4
        let mut screen = crate::screen::Screen::new(3, 80);
        let mut gutter = Gutter::new(0, 3, 99);
        gutter.render(&mut screen, Position::new(0, 0));

        // With width=4 and line "1":
        // col0=space, col1=space, col2="1", col3=space
        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
        assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");

        // Now simulate re-render with width=5 (simulating file grew)
        // The screen still has old content, but we re-render with new width
        let mut gutter2 = Gutter::new(0, 3, 100);
        gutter2.render(&mut screen, Position::new(0, 0));

        // With width=5 and line "1" (1 digit):
        // col0=space, col1=space, col2=space, col3="1", col4=space
        // Because: right_padding at col4, line at col4-1=3
        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "1");
        assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, " ");

        // Also verify multi-digit line number
        // Line "11" would be at columns 2-3
        let mut gutter3 = Gutter::new(9, 3, 100); // start at line 9, showing 10, 11
        gutter3.render(&mut screen, Position::new(0, 0));

        // Line "10" at row 0: col2="1", col3="0", col4=space
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
        assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "0");
        assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, " ");
    }

    // Column preservation tests

    #[test]
    fn test_column_preservation_first_vertical_move() {
        // First vertical move should use current column and remember it
        let buffer = Buffer::from_str("abcdefgh\nij");
        let mut window = Window::new(buffer);

        // Position at column 5 on first line
        window.buffer_view.set_cursor(Cursor::new(0, 5));

        // First move down via Window - should use current column (5), remember it
        window.process_action(&Action::MoveDown);
        assert_eq!(window.buffer_view.cursor().line, 1);
        // Line 2 is "ij" (length 2), so column 5 should clamp to 2
        assert_eq!(window.buffer_view.cursor().col, 2);
    }

    #[test]
    fn test_column_preservation_consecutive_vertical_moves() {
        // Consecutive vertical moves should preserve remembered column
        let buffer = Buffer::from_str("abcdefgh\nabcdefgh\nabcdefgh");
        let mut window = Window::new(buffer);

        // Position at column 5 on first line
        window.buffer_view.set_cursor(Cursor::new(0, 5));

        // Move down - remembers column 5
        window.process_action(&Action::MoveDown);
        assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 5));

        // Move down again - should use remembered column 5
        window.process_action(&Action::MoveDown);
        assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 5));

        // Move up - should use remembered column 5
        window.process_action(&Action::MoveUp);
        assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 5));
    }

    #[test]
    fn test_column_preservation_horizontal_resets() {
        // Horizontal movement should reset remembered column
        use crate::editor::Action;

        let buffer = Buffer::from_str("abcdefgh\nabcdefgh\nabcdefgh");
        let mut window = Window::new(buffer);

        // Position at column 5 on first line
        window.buffer_view.set_cursor(Cursor::new(0, 5));

        // Move down - remembers column 5
        window.process_action(&Action::MoveDown);
        assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 5));

        // Move right - should reset remembered column to current (now at column 6)
        window.process_action(&Action::MoveRight);
        // Now at column 6 on line 1

        // Move down again - should use new column 6 and go to line 2
        window.process_action(&Action::MoveDown);
        assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 6));
    }

    #[test]
    fn test_column_preservation_clamp_on_short_line() {
        // Moving to shorter line should clamp to end of line
        let buffer = Buffer::from_str("abcdefgh\nij\nabcdefgh");
        let mut window = Window::new(buffer);

        // Position at column 5 on first line
        window.buffer_view.set_cursor(Cursor::new(0, 5));

        // Move down to shorter line "ij" (length 2)
        window.process_action(&Action::MoveDown);
        // Should clamp to column 2 (end of "ij")
        assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 2));

        // Move down to longer line - should use remembered column 5
        window.process_action(&Action::MoveDown);
        assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 5));
    }

    #[test]
    fn test_action_resets_remembered_column() {
        use crate::buffer::Boundary;
        use crate::editor::Action;

        // Horizontal movements should reset
        assert!(Action::MoveLeft.resets_remembered_column());
        assert!(Action::MoveRight.resets_remembered_column());
        assert!(Action::ForwardTo(Boundary::Word).resets_remembered_column());
        assert!(Action::BackTo(Boundary::Word).resets_remembered_column());
        assert!(Action::MoveToLineEnd.resets_remembered_column());
        assert!(Action::MoveToLineStart.resets_remembered_column());
        assert!(Action::MoveToLineContentStart.resets_remembered_column());

        // Vertical movements should NOT reset
        assert!(!Action::MoveUp.resets_remembered_column());
        assert!(!Action::MoveDown.resets_remembered_column());

        // Other actions should not reset
        assert!(!Action::SwitchToInsert.resets_remembered_column());
        assert!(Action::InsertChar('a').resets_remembered_column());
        assert!(Action::DeleteBackward.resets_remembered_column());
        assert!(Action::DeleteForward.resets_remembered_column());
    }

    #[test]
    fn test_action_uses_remembered_column() {
        use crate::editor::Action;

        // Vertical movements should use remembered column
        assert!(Action::MoveUp.uses_remembered_column());
        assert!(Action::MoveDown.uses_remembered_column());

        // Other movements should NOT
        assert!(!Action::MoveLeft.uses_remembered_column());
        assert!(!Action::MoveRight.uses_remembered_column());
    }
}
