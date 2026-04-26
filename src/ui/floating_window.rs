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
}

/// Draws a bordered floating frame and fills its body region.
pub fn render_bordered_frame(
    screen: &mut Screen,
    frame: FloatingWindowFrame,
    border_style: Style,
    body_style: Style,
) {
    if frame.size.rows < 3 || frame.size.cols < 3 {
        return;
    }

    if frame.content_size.rows > 0 && frame.content_size.cols > 0 {
        screen.fill_region(
            frame.content_origin.row,
            frame.content_origin.col,
            frame.content_size.rows,
            frame.content_size.cols,
            body_style,
        );
    }

    let unicode_borders =
        globals::with_config(|config| config.unicode_borders_enabled()).unwrap_or(false);
    let (top_left, top_right, bottom_left, bottom_right, horizontal, vertical) =
        border_glyphs(unicode_borders);

    let top_row = frame.origin.row;
    let bottom_row = frame.origin.row + frame.size.rows - 1;
    let left_col = frame.origin.col;
    let right_col = frame.origin.col + frame.size.cols - 1;

    screen.write_string(top_row, left_col, border_style, top_left);
    screen.write_string(top_row, right_col, border_style, top_right);
    screen.write_string(bottom_row, left_col, border_style, bottom_left);
    screen.write_string(bottom_row, right_col, border_style, bottom_right);

    for col in left_col + 1..right_col {
        screen.write_string(top_row, col, border_style, horizontal);
        screen.write_string(bottom_row, col, border_style, horizontal);
    }

    for row in top_row + 1..bottom_row {
        screen.write_string(row, left_col, border_style, vertical);
        screen.write_string(row, right_col, border_style, vertical);
    }
}

fn border_glyphs(
    unicode_borders: bool,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    if unicode_borders {
        ("┌", "┐", "└", "┘", "─", "│")
    } else {
        ("+", "+", "+", "+", "-", "|")
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
}
