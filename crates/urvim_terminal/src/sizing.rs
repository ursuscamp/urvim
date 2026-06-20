//! Text sizing and scaling support (Kitty Terminal Protocol).
//!
//! This module provides types for the Kitty terminal's text sizing feature,
//! which allows applications to render text at custom scales and widths.
//! The protocol uses OSC 66 sequence: `\x1b]66;params\x07`
//!
//! Supported parameters:
//! - `s=N`: Scale factor (e.g., 2 for double height)
//! - `w=N`: Width multiplier (e.g., 2 for double width)
//! - `n=N`: Numerator for fractional scaling
//! - `d=N`: Denominator for fractional scaling
//! - `v=N`: Vertical alignment (0=top, 1=bottom, 2=centered)
//! - `h=N`: Horizontal alignment (0=left, 1=right, 2=centered)

use crate::utils::write_decimal;
#[allow(dead_code)]
use std::io::Write;

/// Vertical text alignment for sized text.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    /// Align to the top.
    #[default]
    Top,
    /// Align to the bottom.
    Bottom,
    /// Center vertically.
    Centered,
}

#[allow(dead_code)]
impl VerticalAlign {
    /// Converts the alignment to its numeric value.
    pub fn as_u8(self) -> u8 {
        match self {
            VerticalAlign::Top => 0,
            VerticalAlign::Bottom => 1,
            VerticalAlign::Centered => 2,
        }
    }
}

/// Horizontal text alignment for sized text.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HorizontalAlign {
    /// Align to the left.
    #[default]
    Left,
    /// Align to the right.
    Right,
    /// Center horizontally.
    Centered,
}

#[allow(dead_code)]
impl HorizontalAlign {
    /// Converts the alignment to its numeric value.
    pub fn as_u8(self) -> u8 {
        match self {
            HorizontalAlign::Left => 0,
            HorizontalAlign::Right => 1,
            HorizontalAlign::Centered => 2,
        }
    }
}

/// Text sizing parameters for the Kitty terminal protocol.
///
/// This allows rendering text at custom scales, widths, and alignments.
/// All fields have sensible defaults (scale=1, width=0, etc.).
///
/// # Example
///
/// ```
/// use urvim_terminal::sizing::{TextSizing, VerticalAlign, HorizontalAlign};
///
/// let sizing = TextSizing::new()
///     .scale(2)
///     .width(2)
///     .vertical(VerticalAlign::Centered)
///     .horizontal(HorizontalAlign::Right);
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextSizing {
    /// Scale factor (1 = normal, 2 = double height, etc.)
    pub scale: u8,
    /// Width multiplier (0 = normal, 2 = double width)
    pub width: u8,
    /// Numerator for fractional scaling.
    pub numerator: u8,
    /// Denominator for fractional scaling.
    pub denominator: u8,
    /// Vertical alignment.
    pub vertical: VerticalAlign,
    /// Horizontal alignment.
    pub horizontal: HorizontalAlign,
}

#[allow(dead_code)]
impl TextSizing {
    /// Creates a new TextSizing with default values (normal text).
    pub const fn new() -> Self {
        Self {
            scale: 1,
            width: 0,
            numerator: 0,
            denominator: 0,
            vertical: VerticalAlign::Top,
            horizontal: HorizontalAlign::Left,
        }
    }

    /// Sets the scale factor.
    ///
    /// Value of 1 is normal size, 2 is double height, etc.
    pub const fn scale(mut self, scale: u8) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the width multiplier.
    ///
    /// Value of 0 is normal width, 2 is double width, etc.
    pub const fn width(mut self, width: u8) -> Self {
        self.width = width;
        self
    }

    /// Sets the numerator for fractional scaling.
    ///
    /// Used together with `denominator` for precise scaling (e.g., 3/2).
    pub const fn numerator(mut self, numerator: u8) -> Self {
        self.numerator = numerator;
        self
    }

    /// Sets the denominator for fractional scaling.
    ///
    /// Used together with `numerator` for precise scaling (e.g., 3/2).
    pub const fn denominator(mut self, denominator: u8) -> Self {
        self.denominator = denominator;
        self
    }

    /// Sets the vertical alignment.
    pub const fn vertical(mut self, vertical: VerticalAlign) -> Self {
        self.vertical = vertical;
        self
    }

    /// Sets the horizontal alignment.
    pub const fn horizontal(mut self, horizontal: HorizontalAlign) -> Self {
        self.horizontal = horizontal;
        self
    }

    /// Writes the OSC 66 escape sequence for this text sizing to a writer.
    ///
    /// The format is: `\x1b]66;params\x07`
    pub fn write_escape_code<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut buf = [0u8; 64];
        let mut i = 0;

        // OSC 66 prefix
        buf[i] = b'\x1b';
        i += 1;
        buf[i] = b']';
        i += 1;
        buf[i] = b'6';
        i += 1;
        buf[i] = b'6';
        i += 1;
        buf[i] = b';';
        i += 1;

        let mut first = true;

        // Scale: s=N
        if self.scale != 1 {
            first = false;
            buf[i] = b's';
            i += 1;
            buf[i] = b'=';
            i += 1;
            i = write_decimal(self.scale, &mut buf, i);
        }

        // Width: w=N
        if self.width != 0 {
            if !first {
                buf[i] = b':';
                i += 1;
            }
            first = false;
            buf[i] = b'w';
            i += 1;
            buf[i] = b'=';
            i += 1;
            i = write_decimal(self.width, &mut buf, i);
        }

        // Numerator: n=N
        if self.numerator != 0 {
            if !first {
                buf[i] = b':';
                i += 1;
            }
            first = false;
            buf[i] = b'n';
            i += 1;
            buf[i] = b'=';
            i += 1;
            i = write_decimal(self.numerator, &mut buf, i);
        }

        // Denominator: d=N
        if self.denominator != 0 {
            if !first {
                buf[i] = b':';
                i += 1;
            }
            first = false;
            buf[i] = b'd';
            i += 1;
            buf[i] = b'=';
            i += 1;
            i = write_decimal(self.denominator, &mut buf, i);
        }

        // Vertical alignment: v=N
        if self.vertical != VerticalAlign::Top {
            if !first {
                buf[i] = b':';
                i += 1;
            }
            first = false;
            buf[i] = b'v';
            i += 1;
            buf[i] = b'=';
            i += 1;
            i = write_decimal(self.vertical.as_u8(), &mut buf, i);
        }

        // Horizontal alignment: h=N
        if self.horizontal != HorizontalAlign::Left {
            if !first {
                buf[i] = b':';
                i += 1;
            }
            buf[i] = b'h';
            i += 1;
            buf[i] = b'=';
            i += 1;
            i = write_decimal(self.horizontal.as_u8(), &mut buf, i);
        }

        // Terminator
        buf[i] = b';';
        i += 1;

        writer.write_all(&buf[..i])
    }
}

/// Indicates the level of text sizing support in the terminal.
///
/// Returned by `Terminal::detect_text_sizing_support()` after probing
/// the terminal's capabilities.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSizingSupport {
    /// Text sizing is not supported.
    None,
    /// Only width scaling is supported (scale is ignored).
    WidthOnly,
    /// Full text sizing support (scale and width both work).
    Full,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_sizing_default() {
        let ts = TextSizing::new();
        assert_eq!(ts.scale, 1);
        assert_eq!(ts.width, 0);
        assert_eq!(ts.numerator, 0);
        assert_eq!(ts.denominator, 0);
        assert_eq!(ts.vertical, VerticalAlign::Top);
        assert_eq!(ts.horizontal, HorizontalAlign::Left);
    }

    #[test]
    fn test_text_sizing_builder() {
        let ts = TextSizing::new()
            .scale(2)
            .width(3)
            .numerator(1)
            .denominator(2)
            .vertical(VerticalAlign::Centered)
            .horizontal(HorizontalAlign::Right);

        assert_eq!(ts.scale, 2);
        assert_eq!(ts.width, 3);
        assert_eq!(ts.numerator, 1);
        assert_eq!(ts.denominator, 2);
        assert_eq!(ts.vertical, VerticalAlign::Centered);
        assert_eq!(ts.horizontal, HorizontalAlign::Right);
    }

    #[test]
    fn test_text_sizing_escape_code_scale() {
        let ts = TextSizing::new().scale(2);
        let mut buf = Vec::new();
        ts.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b]66;s=2;");
    }

    #[test]
    fn test_text_sizing_escape_code_width() {
        let ts = TextSizing::new().width(2);
        let mut buf = Vec::new();
        ts.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b]66;w=2;");
    }

    #[test]
    fn test_text_sizing_escape_code_fractional() {
        let ts = TextSizing::new().numerator(1).denominator(2);
        let mut buf = Vec::new();
        ts.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b]66;n=1:d=2;");
    }

    #[test]
    fn test_text_sizing_escape_code_full() {
        let ts = TextSizing::new()
            .scale(2)
            .width(3)
            .numerator(1)
            .denominator(2)
            .vertical(VerticalAlign::Centered)
            .horizontal(HorizontalAlign::Right);
        let mut buf = Vec::new();
        ts.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b]66;s=2:w=3:n=1:d=2:v=2:h=1;");
    }

    #[test]
    fn test_text_sizing_escape_code_vertical() {
        let ts = TextSizing::new().vertical(VerticalAlign::Bottom);
        let mut buf = Vec::new();
        ts.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b]66;v=1;");
    }

    #[test]
    fn test_text_sizing_escape_code_horizontal() {
        let ts = TextSizing::new().horizontal(HorizontalAlign::Centered);
        let mut buf = Vec::new();
        ts.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b]66;h=2;");
    }

    #[test]
    fn test_text_sizing_escape_code_default() {
        let ts = TextSizing::new();
        let mut buf = Vec::new();
        ts.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b]66;;");
    }
}
