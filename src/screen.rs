//! Screen rendering module.
//!
//! This module provides a double-buffered screen renderer for terminal output.
//! It maintains two buffers - the current frame being built and the previous
//! frame for diff-based rendering.
//!
//! # Features
//!
//! - Double-buffered rendering for efficient terminal updates
//! - Grapheme cluster support with proper Unicode handling
//! - Wide character (emoji, CJK) support with automatic cell clearing
//! - Diff-based rendering: only writes changed cells to the terminal
//! - Zero-allocation clearing: reuses existing String capacity

use crate::terminal::{Style, Terminal};
use rustix::fd::AsFd;
use std::io;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A single cell in the screen grid.
///
/// Each cell contains a style and a grapheme cluster (String, since some
/// grapheme clusters have width > 1).
#[derive(Clone)]
pub struct Cell {
    /// The style to apply to this cell's text.
    pub style: Style,
    /// The grapheme cluster content of this cell.
    pub text: String,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            style: Style::default(),
            text: " ".to_string(),
        }
    }
}

impl Cell {
    /// Clears the cell, resetting style to default and text to a space.
    pub fn clear(&mut self) {
        self.text.clear();
        self.text.push(' ');
        self.style = Style::default();
    }
}

/// A double-buffered screen renderer.
///
/// Screen maintains two buffers: the current frame being built and the previous
/// frame for comparison. When rendering, it only updates cells that have changed.
pub struct Screen {
    rows: u16,
    cols: u16,
    buffer: Vec<Cell>,
    old_buffer: Vec<Cell>,
}

impl Screen {
    /// Creates a new screen with the specified dimensions.
    ///
    /// Both buffers are initialized with default cells containing a space.
    pub fn new(rows: u16, cols: u16) -> Self {
        let size = (rows * cols) as usize;
        let buffer = vec![Cell::default(); size];
        let old_buffer = buffer.clone();
        Self {
            rows,
            cols,
            buffer,
            old_buffer,
        }
    }

    /// Resizes the screen to new dimensions.
    ///
    /// This creates new buffers with the new size, initialized with spaces.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        let size = (rows * cols) as usize;
        let buffer = vec![Cell::default(); size];
        let old_buffer = buffer.clone();
        self.buffer = buffer;
        self.old_buffer = old_buffer;
        self.rows = rows;
        self.cols = cols;
    }

    /// Returns the screen dimensions.
    pub fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    /// Gets a mutable reference to the cell at the specified position.
    ///
    /// Returns `None` if the position is out of bounds.
    pub fn get_cell_mut(&mut self, row: u16, col: u16) -> Option<&mut Cell> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let idx = (row * self.cols + col) as usize;
        self.buffer.get_mut(idx)
    }

    /// Clears the current buffer.
    ///
    /// This resets all cells to default style with a space character.
    pub fn clear(&mut self) {
        self.clear_with_style(Style::default());
    }

    /// Clears the current buffer using an explicit style.
    ///
    /// Every cell is reset to a space character and assigned the provided
    /// style. This is useful when a renderer wants blank space to inherit a
    /// theme or component-specific base style instead of the screen default.
    pub fn clear_with_style(&mut self, style: Style) {
        self.fill_region(0, 0, self.rows, self.cols, style);
    }

    /// Fills a rectangular region with a space character and explicit style.
    ///
    /// Coordinates outside the screen bounds are clipped to the visible area.
    pub fn fill_region(
        &mut self,
        row: u16,
        col: u16,
        rows: u16,
        cols: u16,
        style: Style,
    ) {
        let row_end = row.saturating_add(rows).min(self.rows);
        let col_end = col.saturating_add(cols).min(self.cols);

        for current_row in row.min(self.rows)..row_end {
            for current_col in col.min(self.cols)..col_end {
                if let Some(cell) = self.get_cell_mut(current_row, current_col) {
                    cell.text.clear();
                    cell.text.push(' ');
                    cell.style = style;
                }
            }
        }
    }

    /// Writes a string to the screen at the specified position.
    ///
    /// The string is written grapheme cluster by grapheme cluster. Each
    /// grapheme cluster goes into its own cell.
    ///
    /// Wide characters (width > 1) automatically clear the cells they occupy
    /// before being written to prevent rendering issues.
    ///
    /// Writing stops early if the edge of the screen is reached.
    pub fn write_string(&mut self, row: u16, col: u16, style: Style, text: &str) {
        self.write_graphemes(row, col, style, std::iter::once(text));
    }

    /// Writes a string to the screen at the specified position.
    ///
    /// This iterates over the graphemes of the string and writes each grapheme.
    ///
    /// Writing stops early if the edge of the screen is reached.
    pub fn write_str(&mut self, row: u16, col: u16, style: Style, text: &str) {
        self.write_graphemes(row, col, style, text.graphemes(true));
    }

    /// Internal helper that writes graphemes from an iterator of string chunks.
    ///
    /// Stops early if the edge of the screen is reached.
    fn write_graphemes<'a, I: Iterator<Item = &'a str>>(
        &mut self,
        row: u16,
        col: u16,
        style: Style,
        chunks: I,
    ) {
        let mut current_col = col;

        for chunk in chunks {
            for grapheme in chunk.graphemes(true) {
                if current_col >= self.cols {
                    return;
                }

                let width = grapheme.width();

                if width > 1 {
                    let cells_to_clear = width.min((self.cols - current_col) as usize);
                    for offset in 0..cells_to_clear {
                        let clear_col = current_col + offset as u16;
                        if let Some(cell) = self.get_cell_mut(row, clear_col) {
                            cell.clear();
                        }
                    }
                }

                if let Some(cell) = self.get_cell_mut(row, current_col) {
                    cell.text.clear();
                    cell.text.push_str(grapheme);
                    cell.style = style;
                }

                current_col += width as u16;
            }
        }
    }

    /// Renders the screen buffer to the terminal using diff-based updates.
    ///
    /// This method compares the current frame with the previous frame and only
    /// writes cells that have changed. This minimizes terminal I/O for efficiency.
    ///
    /// ## Wide Character Handling
    ///
    /// Terminal cells are typically 1 character wide, but some Unicode grapheme
    /// clusters (emoji, CJK characters) occupy 2+ cells. When rendering a wide
    /// character, we must:
    /// 1. First clear all cells the character will occupy with spaces (to remove
    ///    any stale data from the previous frame)
    /// 2. Then write the character
    /// 3. Skip past the remaining cells to avoid redundant writes
    ///
    /// This mirrors how `write_graphemes()` populates the screen buffer - it
    /// clears adjacent cells before writing wide characters. Without this, the
    /// terminal would display leftover characters from the previous frame in the
    /// cells adjacent to wide characters.
    ///
    /// ## Algorithm
    ///
    /// The render loop uses a column cursor (`col`) that advances by the visual
    /// width of each cell when changes are detected, rather than always advancing
    /// by 1. This ensures we process all cells a wide character occupies in one
    /// pass, preventing the loop from re-visiting those cells and potentially
    /// overwriting the wide character we just wrote.
    ///
    /// Example: Writing "😀" at column 0 (width=2)
    /// - Clear cells 0 and 1 with spaces
    /// - Write "😀" to cell 0
    /// - Skip to column 2 (not column 1)
    pub fn render<I: io::Read + AsFd, O: io::Write + AsFd>(
        &mut self,
        terminal: &mut Terminal<I, O>,
    ) -> io::Result<()> {
        terminal.hide_cursor()?;

        for row in 0..self.rows {
            let mut col: u16 = 0;
            while col < self.cols {
                let idx = (row * self.cols + col) as usize;
                let cell = &self.buffer[idx];
                let old_cell = &self.old_buffer[idx];

                if cell.style != old_cell.style || cell.text != old_cell.text {
                    terminal.set_cursor_position(row + 1, col + 1)?;

                    let width = UnicodeWidthStr::width(cell.text.as_str()).max(1);

                    if width > 1 {
                        let cells_to_clear = width.min((self.cols - col) as usize);
                        for clear_offset in 0..cells_to_clear {
                            terminal.set_cursor_position(row + 1, col + 1 + clear_offset as u16)?;
                            terminal.write_text(" ")?;
                        }
                        // Reposition after clearing wide character cells
                        terminal.set_cursor_position(row + 1, col + 1)?;
                    }

                    // Always apply the style to ensure correctness, regardless of whether
                    // it changed from old_buffer. The terminal state may differ from buffer
                    // state when we skip unchanged cells in the else branch.
                    terminal.set_style(&cell.style)?;
                    terminal.write_text(&cell.text)?;
                    terminal.reset_style()?;

                    col += width as u16;
                } else {
                    col += 1;
                }
            }
        }

        terminal.show_cursor()?;

        std::mem::swap(&mut self.buffer, &mut self.old_buffer);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_new() {
        let screen = Screen::new(10, 20);
        assert_eq!(screen.size(), (10, 20));
    }

    #[test]
    fn test_screen_resize() {
        let mut screen = Screen::new(10, 20);
        screen.resize(15, 30);
        assert_eq!(screen.size(), (15, 30));
    }

    #[test]
    fn test_screen_resize_to_zero_dimensions() {
        let mut screen = Screen::new(2, 2);
        screen.resize(0, 0);
        assert_eq!(screen.size(), (0, 0));
    }

    #[test]
    fn test_get_cell_mut() {
        let mut screen = Screen::new(3, 3);
        let cell = screen.get_cell_mut(1, 2);
        assert!(cell.is_some());

        let cell = screen.get_cell_mut(3, 0);
        assert!(cell.is_none());

        let cell = screen.get_cell_mut(0, 3);
        assert!(cell.is_none());
    }

    #[test]
    fn test_clear() {
        let mut screen = Screen::new(2, 2);
        if let Some(cell) = screen.get_cell_mut(0, 0) {
            cell.text.push_str("hello");
            cell.style = Style::new().bold();
        }
        screen.clear();
        let cell = screen.get_cell_mut(0, 0).unwrap();
        assert_eq!(cell.text, " ");
        assert_eq!(cell.style, Style::default());
    }

    #[test]
    fn test_clear_with_style() {
        let mut screen = Screen::new(2, 2);
        let style = Style::new().bold().fg(crate::terminal::Color::ansi(196));

        screen.clear_with_style(style);

        let cell = screen.get_cell_mut(0, 0).unwrap();
        assert_eq!(cell.text, " ");
        assert_eq!(cell.style, style);
    }

    #[test]
    fn test_fill_region() {
        let mut screen = Screen::new(3, 3);
        let style = Style::new().bg(crate::terminal::Color::ansi(30));

        screen.fill_region(1, 1, 2, 2, style);

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().style, Style::default());
        assert_eq!(screen.get_cell_mut(1, 1).unwrap().style, style);
        assert_eq!(screen.get_cell_mut(2, 2).unwrap().style, style);
    }

    #[test]
    fn test_write_string_ascii() {
        let mut screen = Screen::new(2, 5);
        screen.write_string(0, 0, Style::default(), "abc");

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "a");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "b");
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "c");
        assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");
    }

    #[test]
    fn test_write_string_stops_at_edge() {
        let mut screen = Screen::new(2, 3);
        screen.write_string(0, 0, Style::default(), "abcd");

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "a");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "b");
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "c");
    }

    #[test]
    fn test_write_string_wide_character() {
        let mut screen = Screen::new(2, 4);

        screen.write_string(0, 0, Style::default(), "😀");

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "😀");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
    }

    #[test]
    fn test_write_string_mixed() {
        let mut screen = Screen::new(2, 5);

        screen.write_string(0, 0, Style::default(), "a😀b");

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "a");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "😀");
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, " ");
        assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "b");
    }

    #[test]
    fn test_write_string_with_style() {
        let mut screen = Screen::new(2, 2);
        let style = Style::new().bold().fg(crate::terminal::Color::ansi(196));

        screen.write_string(0, 0, style, "ab");

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().style, style);
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().style, style);
    }

    #[test]
    fn test_render_compares_old_buffer() {
        let mut screen = Screen::new(2, 2);

        screen.write_string(0, 0, Style::default(), "a");
        screen.write_string(0, 1, Style::default(), "b");

        std::mem::swap(&mut screen.buffer, &mut screen.old_buffer);

        screen.write_string(0, 0, Style::default(), "a");
        screen.write_string(0, 1, Style::default(), "c");

        let idx00 = 0usize;
        let idx01 = 1usize;

        assert_eq!(screen.buffer[idx00].text, "a");
        assert_eq!(screen.old_buffer[idx00].text, "a");

        assert_eq!(screen.buffer[idx01].text, "c");
        assert_eq!(screen.old_buffer[idx01].text, "b");
    }

    #[test]
    fn test_cell_default() {
        let cell = Cell::default();
        assert_eq!(cell.text, " ");
        assert_eq!(cell.style, Style::default());
    }

    #[test]
    fn test_cell_clone() {
        let mut cell = Cell::default();
        cell.text.push_str("test");
        cell.style = Style::new().bold();

        let cloned = cell.clone();
        assert_eq!(cloned.text, " test");
    }

    #[test]
    fn test_write_str_ascii() {
        let mut screen = Screen::new(2, 5);

        screen.write_str(0, 0, Style::default(), "abc");

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "a");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "b");
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "c");
    }

    #[test]
    fn test_write_str_wide_character() {
        let mut screen = Screen::new(2, 4);

        screen.write_str(0, 0, Style::default(), "😀");

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "😀");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
    }

    #[test]
    fn test_write_str_stops_at_edge() {
        let mut screen = Screen::new(2, 3);

        screen.write_str(0, 0, Style::default(), "abcd");

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, "a");
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, "b");
        assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "c");
    }

    #[test]
    fn test_write_str_with_style() {
        let mut screen = Screen::new(2, 2);
        let style = Style::new().bold().fg(crate::terminal::Color::ansi(196));

        screen.write_str(0, 0, style, "ab");

        assert_eq!(screen.get_cell_mut(0, 0).unwrap().style, style);
        assert_eq!(screen.get_cell_mut(0, 1).unwrap().style, style);
    }
}
