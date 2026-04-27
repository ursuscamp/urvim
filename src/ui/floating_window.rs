//! Generic bordered floating window helpers.
//!
//! This module centralizes bordered floating frame geometry and rendering so
//! multiple overlays (notification banner, command line, etc.) can share one
//! implementation.

use crate::globals;
use crate::screen::Screen;
use crate::terminal::Style;
use crate::window::{Position, Size};

/// Placement anchor for floating windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatingAnchor {
    /// Place the floating window centered inside the bounds.
    Center,
    /// Place the floating window near the top, centered horizontally.
    TopCenter { top_margin: u16 },
    /// Place the floating window at the top-right corner inside the bounds.
    TopRight,
}

/// Resolved geometry for a bordered floating window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatingWindowFrame {
    /// Outer frame origin including border.
    pub origin: Position,
    /// Outer frame size including border.
    pub size: Size,
    /// Inner content origin inside the border.
    pub content_origin: Position,
    /// Inner content size inside the border.
    pub content_size: Size,
}

impl FloatingWindowFrame {
    /// Resolves a bordered floating frame from content dimensions and bounds.
    pub fn resolve(
        bounds_origin: Position,
        bounds_size: Size,
        content_rows: u16,
        content_cols: u16,
        anchor: FloatingAnchor,
    ) -> Option<Self> {
        if bounds_size.rows < 3 || bounds_size.cols < 3 || content_rows == 0 || content_cols == 0 {
            return None;
        }

        let frame_rows = content_rows.saturating_add(2).min(bounds_size.rows);
        let frame_cols = content_cols.saturating_add(2).min(bounds_size.cols);
        if frame_rows < 3 || frame_cols < 3 {
            return None;
        }

        let origin = match anchor {
            FloatingAnchor::Center => Position::new(
                bounds_origin
                    .row
                    .saturating_add(bounds_size.rows.saturating_sub(frame_rows) / 2),
                bounds_origin
                    .col
                    .saturating_add(bounds_size.cols.saturating_sub(frame_cols) / 2),
            ),
            FloatingAnchor::TopCenter { top_margin } => Position::new(
                bounds_origin
                    .row
                    .saturating_add(top_margin.min(bounds_size.rows.saturating_sub(frame_rows))),
                bounds_origin
                    .col
                    .saturating_add(bounds_size.cols.saturating_sub(frame_cols) / 2),
            ),
            FloatingAnchor::TopRight => Position::new(
                bounds_origin.row,
                bounds_origin
                    .col
                    .saturating_add(bounds_size.cols.saturating_sub(frame_cols)),
            ),
        };

        Some(Self {
            origin,
            size: Size::new(frame_rows, frame_cols),
            content_origin: Position::new(origin.row + 1, origin.col + 1),
            content_size: Size::new(frame_rows - 2, frame_cols - 2),
        })
    }

    /// Draws the bordered floating frame and fills its body region.
    pub fn render_bordered(self, screen: &mut Screen, border_style: Style, body_style: Style) {
        if self.size.rows < 3 || self.size.cols < 3 {
            return;
        }

        if self.content_size.rows > 0 && self.content_size.cols > 0 {
            screen.fill_region(
                self.content_origin.row,
                self.content_origin.col,
                self.content_size.rows,
                self.content_size.cols,
                body_style,
            );
        }

        let glyphs = FloatingWindowGlyphs::active();

        let top_row = self.origin.row;
        let bottom_row = self.origin.row + self.size.rows - 1;
        let left_col = self.origin.col;
        let right_col = self.origin.col + self.size.cols - 1;

        screen.write_string(top_row, left_col, border_style, glyphs.top_left);
        screen.write_string(top_row, right_col, border_style, glyphs.top_right);
        screen.write_string(bottom_row, left_col, border_style, glyphs.bottom_left);
        screen.write_string(bottom_row, right_col, border_style, glyphs.bottom_right);

        for col in left_col + 1..right_col {
            screen.write_string(top_row, col, border_style, glyphs.horizontal);
            screen.write_string(bottom_row, col, border_style, glyphs.horizontal);
        }

        for row in top_row + 1..bottom_row {
            screen.write_string(row, left_col, border_style, glyphs.vertical);
            screen.write_string(row, right_col, border_style, glyphs.vertical);
        }
    }

    /// Draws a horizontal separator connected to this frame's side borders.
    pub fn render_separator(self, screen: &mut Screen, row: u16, style: Style) {
        if row <= self.origin.row || row >= self.origin.row + self.size.rows - 1 {
            return;
        }

        let glyphs = FloatingWindowGlyphs::active();
        let right_col = self.origin.col + self.size.cols - 1;

        screen.write_string(row, self.origin.col, style, glyphs.separator_left);
        for col in self.content_origin.col..right_col {
            screen.write_string(row, col, style, glyphs.horizontal);
        }
        screen.write_string(row, right_col, style, glyphs.separator_right);
    }
}

/// Glyph set used to draw bordered floating windows and internal separators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatingWindowGlyphs {
    /// Top-left corner glyph.
    pub top_left: &'static str,
    /// Top-right corner glyph.
    pub top_right: &'static str,
    /// Bottom-left corner glyph.
    pub bottom_left: &'static str,
    /// Bottom-right corner glyph.
    pub bottom_right: &'static str,
    /// Horizontal line glyph.
    pub horizontal: &'static str,
    /// Vertical line glyph.
    pub vertical: &'static str,
    /// Left separator junction glyph.
    pub separator_left: &'static str,
    /// Right separator junction glyph.
    pub separator_right: &'static str,
}

impl FloatingWindowGlyphs {
    /// Returns the floating window glyphs enabled by the active configuration.
    pub fn active() -> Self {
        let unicode_borders =
            globals::with_config(|config| config.unicode_borders_enabled()).unwrap_or(false);
        Self::for_unicode_borders(unicode_borders)
    }

    /// Returns floating window glyphs for the requested border capability.
    pub fn for_unicode_borders(unicode_borders: bool) -> Self {
        if unicode_borders {
            return Self {
                top_left: "┌",
                top_right: "┐",
                bottom_left: "└",
                bottom_right: "┘",
                horizontal: "─",
                vertical: "│",
                separator_left: "├",
                separator_right: "┤",
            };
        }

        Self {
            top_left: "+",
            top_right: "+",
            bottom_left: "+",
            bottom_right: "+",
            horizontal: "-",
            vertical: "|",
            separator_left: "|",
            separator_right: "|",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_centered_frame() {
        let frame = FloatingWindowFrame::resolve(
            Position::new(0, 0),
            Size::new(10, 20),
            1,
            6,
            FloatingAnchor::Center,
        )
        .expect("frame should resolve");

        assert_eq!(frame.size, Size::new(3, 8));
        assert_eq!(frame.origin, Position::new(3, 6));
        assert_eq!(frame.content_origin, Position::new(4, 7));
        assert_eq!(frame.content_size, Size::new(1, 6));
    }

    #[test]
    fn resolve_top_right_frame() {
        let frame = FloatingWindowFrame::resolve(
            Position::new(2, 3),
            Size::new(8, 12),
            1,
            4,
            FloatingAnchor::TopRight,
        )
        .expect("frame should resolve");

        assert_eq!(frame.origin, Position::new(2, 9));
        assert_eq!(frame.size, Size::new(3, 6));
    }

    #[test]
    fn resolve_top_center_frame() {
        let frame = FloatingWindowFrame::resolve(
            Position::new(2, 3),
            Size::new(20, 40),
            6,
            10,
            FloatingAnchor::TopCenter { top_margin: 5 },
        )
        .expect("frame should resolve");

        assert_eq!(frame.origin, Position::new(7, 17));
        assert_eq!(frame.size, Size::new(8, 12));
    }

    #[test]
    fn resolve_top_center_clamps_margin_to_bounds() {
        let frame = FloatingWindowFrame::resolve(
            Position::new(2, 3),
            Size::new(10, 40),
            6,
            10,
            FloatingAnchor::TopCenter { top_margin: 20 },
        )
        .expect("frame should resolve");

        assert_eq!(frame.origin, Position::new(4, 17));
        assert_eq!(frame.size, Size::new(8, 12));
    }

    #[test]
    fn glyphs_follow_ascii_border_capability() {
        let glyphs = FloatingWindowGlyphs::for_unicode_borders(false);

        assert_eq!(glyphs.horizontal, "-");
        assert_eq!(glyphs.separator_left, "|");
        assert_eq!(glyphs.separator_right, "|");
    }

    #[test]
    fn glyphs_follow_unicode_border_capability() {
        let glyphs = FloatingWindowGlyphs::for_unicode_borders(true);

        assert_eq!(glyphs.horizontal, "─");
        assert_eq!(glyphs.separator_left, "├");
        assert_eq!(glyphs.separator_right, "┤");
    }
}
