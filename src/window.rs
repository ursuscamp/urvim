//! Window rendering module.
//!
//! This module provides the Window type, which owns a Buffer and is responsible
//! for rendering its content to the screen. A window has a screen position (origin)
//! and size, and renders the buffer content starting from its origin.

use crate::buffer::{Buffer, Cursor};
use crate::screen::Screen;
use crate::terminal::Style;
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
}

impl BufferView {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffer,
            scroll_offset: Position::new(0, 0),
            cursor: Cursor::new(0, 0),
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

    /// Adjusts scroll offset to ensure the cursor is visible in the viewport.
    ///
    /// This function implements auto-scrolling by:
    /// 1. Vertical scrolling: If cursor is above or below the visible area, adjust row offset
    /// 2. Horizontal scrolling: If cursor's visual column is left or right of visible area, adjust column offset
    /// 3. Bounds clamping: Ensure scroll offset doesn't go beyond buffer/line boundaries
    ///
    /// The viewport shows content starting from `scroll_offset.row` rows and `scroll_offset.col` visual columns.
    /// Cursor is positioned at the last visible row/column when scrolling brings it into view.
    pub fn scroll_to_cursor(&mut self, viewport_size: Size) {
        let cursor = self.cursor;

        let buffer_line_count = self.buffer.line_count();
        if buffer_line_count == 0 {
            self.scroll_offset = Position::new(0, 0);
            return;
        }

        let visible_rows = viewport_size.rows as usize;
        let visible_cols = viewport_size.cols as usize;

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
}

impl Window {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffer_view: BufferView::new(buffer),
            render_data: RenderData::new(0),
        }
    }

    pub fn buffer_view(&self) -> &BufferView {
        &self.buffer_view
    }

    pub fn buffer_view_mut(&mut self) -> &mut BufferView {
        &mut self.buffer_view
    }

    pub fn render_data(&self) -> &RenderData {
        &self.render_data
    }

    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.render_data = self.buffer_view.build_render_data(size);
        self.render_data.render(screen, origin);
    }

    pub fn visual_cursor(&self) -> Option<Position> {
        self.render_data
            .cursor_screen_position(self.buffer_view.cursor())
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

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "l");
        assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, "l");
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
}
