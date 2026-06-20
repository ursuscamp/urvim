//! Generic bordered floating window helpers.
//!
//! This module centralizes bordered floating frame geometry and rendering so
//! multiple overlays (notification banner, command line, etc.) can share one
//! implementation.

use crate::icon;
use crate::screen::Screen;
use crate::ui::text_width::{ClipSide, clip_first_line};
use crate::window::{Position, Size};
use unicode_width::UnicodeWidthStr;
use urvim_terminal::Style;

/// Placement anchor for floating windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatingAnchor {
    /// Place the floating window centered inside the bounds.
    Center,
    /// Place the floating window near the top, centered horizontally.
    TopCenter { top_margin: u16 },
    /// Place the floating window at the top-right corner inside the bounds.
    TopRight,
    /// Place the floating window at the bottom-right corner inside the bounds.
    BottomRight,
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

/// Label rendered into a floating window frame border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatingWindowFrameLabel<'a> {
    /// Text to render inside the border.
    pub text: &'a str,
    /// Frame side where the label should be rendered.
    pub side: FloatingWindowFrameLabelSide,
    /// Label alignment within the non-corner border span.
    pub align: FloatingWindowFrameLabelAlign,
}

impl<'a> FloatingWindowFrameLabel<'a> {
    /// Creates a frame label.
    pub fn new(
        text: &'a str,
        side: FloatingWindowFrameLabelSide,
        align: FloatingWindowFrameLabelAlign,
    ) -> Self {
        Self { text, side, align }
    }

    /// Creates a top-centered frame label.
    pub fn top_center(text: &'a str) -> Self {
        Self::new(
            text,
            FloatingWindowFrameLabelSide::Top,
            FloatingWindowFrameLabelAlign::Center,
        )
    }
}

/// Side of a floating window frame where a label is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatingWindowFrameLabelSide {
    /// Render the label on the top border.
    Top,
    /// Render the label on the bottom border.
    Bottom,
}

/// Alignment for a frame label within the non-corner border span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatingWindowFrameLabelAlign {
    /// Align the label to the left edge of the border span.
    Left,
    /// Center the label in the border span.
    Center,
    /// Align the label to the right edge of the border span.
    Right,
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
            FloatingAnchor::BottomRight => Position::new(
                bounds_origin
                    .row
                    .saturating_add(bounds_size.rows.saturating_sub(frame_rows)),
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

    /// Resolves a bordered floating frame near a cursor position.
    pub fn resolve_near_cursor(
        bounds_origin: Position,
        bounds_size: Size,
        cursor: Position,
        content_rows: u16,
        content_cols: u16,
    ) -> Option<Self> {
        let frame = Self::resolve(
            bounds_origin,
            bounds_size,
            content_rows,
            content_cols,
            FloatingAnchor::Center,
        )?;
        let max_row = bounds_origin
            .row
            .saturating_add(bounds_size.rows.saturating_sub(frame.size.rows));
        let max_col = bounds_origin
            .col
            .saturating_add(bounds_size.cols.saturating_sub(frame.size.cols));

        let below_row = cursor.row.saturating_add(1);
        let row =
            if below_row.saturating_add(frame.size.rows) <= bounds_origin.row + bounds_size.rows {
                below_row
            } else if cursor.row >= frame.size.rows {
                cursor.row - frame.size.rows
            } else {
                cursor.row.saturating_add(1).min(max_row)
            }
            .min(max_row);
        let col = cursor.col.min(max_col);

        Some(Self {
            origin: Position::new(row, col),
            content_origin: Position::new(row.saturating_add(1), col.saturating_add(1)),
            ..frame
        })
    }

    /// Draws the bordered floating frame and fills its body region.
    pub fn render_bordered(self, screen: &mut Screen, border_style: Style, body_style: Style) {
        self.render_bordered_with_label(screen, border_style, body_style, None);
    }

    /// Draws the bordered floating frame with an optional border label.
    pub fn render_bordered_with_label(
        self,
        screen: &mut Screen,
        border_style: Style,
        body_style: Style,
        label: Option<FloatingWindowFrameLabel<'_>>,
    ) {
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

        let glyphs = icon::BorderGlyphs::active();

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

        if let Some(label) = label {
            self.render_label(screen, border_style, label);
        }
    }

    fn render_label(
        self,
        screen: &mut Screen,
        border_style: Style,
        label: FloatingWindowFrameLabel<'_>,
    ) {
        let Some((row, col, text)) = self.resolve_label(label) else {
            return;
        };

        screen.write_str(row, col, border_style, text.as_str());
    }

    fn resolve_label(self, label: FloatingWindowFrameLabel<'_>) -> Option<(u16, u16, String)> {
        let available_cols = self.size.cols.checked_sub(2)? as usize;
        if available_cols == 0 {
            return None;
        }

        let text = clip_first_line(label.text, available_cols, ClipSide::Start).text;
        let label_cols = UnicodeWidthStr::width(text.as_str());
        if label_cols == 0 {
            return None;
        }

        let offset = match label.align {
            FloatingWindowFrameLabelAlign::Left => 0,
            FloatingWindowFrameLabelAlign::Center => available_cols.saturating_sub(label_cols) / 2,
            FloatingWindowFrameLabelAlign::Right => available_cols.saturating_sub(label_cols),
        } as u16;
        let row = match label.side {
            FloatingWindowFrameLabelSide::Top => self.origin.row,
            FloatingWindowFrameLabelSide::Bottom => self.origin.row + self.size.rows - 1,
        };
        let col = self.origin.col + 1 + offset;

        Some((row, col, text))
    }

    /// Draws a horizontal separator connected to this frame's side borders.
    pub fn render_separator(self, screen: &mut Screen, row: u16, style: Style) {
        if row <= self.origin.row || row >= self.origin.row + self.size.rows - 1 {
            return;
        }

        let glyphs = icon::BorderGlyphs::active();
        let right_col = self.origin.col + self.size.cols - 1;

        screen.write_string(row, self.origin.col, style, glyphs.separator_left);
        for col in self.content_origin.col..right_col {
            screen.write_string(row, col, style, glyphs.horizontal);
        }
        screen.write_string(row, right_col, style, glyphs.separator_right);
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
    fn resolve_bottom_right_frame() {
        let frame = FloatingWindowFrame::resolve(
            Position::new(2, 3),
            Size::new(8, 12),
            1,
            4,
            FloatingAnchor::BottomRight,
        )
        .expect("frame should resolve");

        assert_eq!(frame.origin, Position::new(7, 9));
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
    fn resolve_near_cursor_prefers_below_when_space_allows() {
        let frame = FloatingWindowFrame::resolve_near_cursor(
            Position::new(0, 0),
            Size::new(20, 40),
            Position::new(4, 10),
            4,
            10,
        )
        .expect("frame should resolve");

        assert_eq!(frame.origin.row, 5);
    }

    #[test]
    fn resolve_near_cursor_falls_back_above_when_needed() {
        let frame = FloatingWindowFrame::resolve_near_cursor(
            Position::new(0, 0),
            Size::new(8, 40),
            Position::new(6, 10),
            4,
            10,
        )
        .expect("frame should resolve");

        assert!(frame.origin.row < 6);
    }

    #[test]
    fn glyphs_follow_ascii_border_capability() {
        let glyphs = icon::BorderGlyphs::for_unicode_borders(false);

        assert_eq!(glyphs.horizontal, "-");
        assert_eq!(glyphs.separator_left, "|");
        assert_eq!(glyphs.separator_right, "|");
    }

    #[test]
    fn glyphs_follow_unicode_border_capability() {
        let glyphs = icon::BorderGlyphs::for_unicode_borders(true);

        assert_eq!(glyphs.horizontal, "─");
        assert_eq!(glyphs.separator_left, "├");
        assert_eq!(glyphs.separator_right, "┤");
    }

    #[test]
    fn label_resolves_top_center_inside_corners() {
        let frame = frame_at_origin(Size::new(3, 12));
        let label = FloatingWindowFrameLabel::top_center("Name");

        assert_eq!(frame.resolve_label(label), Some((0, 4, "Name".to_string())));
    }

    #[test]
    fn label_resolves_bottom_left_inside_corners() {
        let frame = frame_at_origin(Size::new(5, 12));
        let label = FloatingWindowFrameLabel::new(
            "Name",
            FloatingWindowFrameLabelSide::Bottom,
            FloatingWindowFrameLabelAlign::Left,
        );

        assert_eq!(frame.resolve_label(label), Some((4, 1, "Name".to_string())));
    }

    #[test]
    fn label_resolves_top_right_inside_corners() {
        let frame = frame_at_origin(Size::new(3, 12));
        let label = FloatingWindowFrameLabel::new(
            "Name",
            FloatingWindowFrameLabelSide::Top,
            FloatingWindowFrameLabelAlign::Right,
        );

        assert_eq!(frame.resolve_label(label), Some((0, 7, "Name".to_string())));
    }

    #[test]
    fn label_clips_to_non_corner_span() {
        let frame = frame_at_origin(Size::new(3, 6));
        let label = FloatingWindowFrameLabel::top_center("abcdef");

        assert_eq!(frame.resolve_label(label), Some((0, 1, "abcd".to_string())));
    }

    #[test]
    fn label_clips_wide_graphemes_without_exceeding_span() {
        let frame = frame_at_origin(Size::new(3, 5));
        let label = FloatingWindowFrameLabel::top_center("ab🙂");

        assert_eq!(frame.resolve_label(label), Some((0, 1, "ab".to_string())));
    }

    #[test]
    fn empty_label_does_not_resolve() {
        let frame = frame_at_origin(Size::new(3, 6));
        let label = FloatingWindowFrameLabel::top_center("");

        assert_eq!(frame.resolve_label(label), None);
    }

    fn frame_at_origin(size: Size) -> FloatingWindowFrame {
        FloatingWindowFrame {
            origin: Position::new(0, 0),
            size,
            content_origin: Position::new(1, 1),
            content_size: Size::new(size.rows.saturating_sub(2), size.cols.saturating_sub(2)),
        }
    }
}
