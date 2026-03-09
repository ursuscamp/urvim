//! Terminal text styling and color support.
//!
//! This module provides types for styling terminal output using ANSI escape codes,
//! including:
//! - Text decorations (bold, italic, underline, etc.)
//! - Foreground and background colors (ANSI 256-color and RGB)
//! - Underline styles and colors (Kitty protocol extensions)
//!
//! # Example
//!
//! ```
//! use urvim::terminal::style::{Style, Color};
//!
//! let style = Style::new()
//!     .bold()
//!     .fg(Color::ansi(196))
//!     .bg(Color::rgb(0, 0, 0));
//! ```

use crate::terminal::utils::write_decimal;
use std::io::Write;

/// RGB color representation.
///
/// Used for true color (24-bit) terminal support.
/// Values are in the range 0-255 for each channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    /// Red channel (0-255).
    pub r: u8,
    /// Green channel (0-255).
    pub g: u8,
    /// Blue channel (0-255).
    pub b: u8,
}

impl Rgb {
    /// Creates a new RGB color.
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Terminal color representation.
///
/// Colors can be either ANSI 256-color palette values (0-255)
/// or true RGB colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// ANSI 256-color palette (0-255).
    Ansi(u8),
    /// True RGB color (24-bit).
    Rgb(Rgb),
}

impl Color {
    /// Creates an ANSI 256-color.
    ///
    /// The ANSI palette includes:
    /// - 0-7: Standard colors (black, red, green, yellow, blue, magenta, cyan, white)
    /// - 8-15: Bright versions
    /// - 16-231: 6x6x6 color cube
    /// - 232-255: Grayscale
    pub fn ansi(ansi: u8) -> Self {
        Self::Ansi(ansi)
    }

    /// Creates a true RGB color.
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::Rgb(Rgb::new(r, g, b))
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::Ansi(0)
    }
}

/// Underline style for text (Kitty protocol extension).
///
/// Extended underline styles beyond the basic ANSI underline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnderlineStyle {
    /// No underline.
    #[default]
    None,
    /// Straight underline (single).
    Straight,
    /// Double underline.
    Double,
    /// Curly underline.
    Curly,
    /// Dotted underline.
    Dotted,
    /// Dashed underline.
    Dashed,
}

/// Style flag: Bold text.
pub const BOLD: u32 = 1 << 0;
/// Style flag: Faint text (reduced intensity).
pub const FAINT: u32 = 1 << 1;
/// Style flag: Italic text.
pub const ITALIC: u32 = 1 << 2;
/// Style flag: Underlined text.
pub const UNDERLINE: u32 = 1 << 3;
/// Style flag: Slow blink.
pub const SLOW_BLINK: u32 = 1 << 4;
/// Style flag: Rapid blink.
pub const RAPID_BLINK: u32 = 1 << 5;
/// Style flag: Reverse video (swap foreground/background).
pub const REVERSE: u32 = 1 << 6;
/// Style flag: Hidden text (useful for passwords).
pub const HIDDEN: u32 = 1 << 7;
/// Style flag: Strikethrough text.
pub const STRIKETHROUGH: u32 = 1 << 8;
/// Style flag: Overline text.
pub const OVERLINE: u32 = 1 << 9;
/// Style flag: Double underline (distinct from curly/dotted).
pub const DOUBLE_UNDERLINE: u32 = 1 << 10;

/// Terminal text style.
///
/// A builder pattern is used to construct styles:
///
/// ```
/// use urvim::terminal::style::{Style, Color};
///
/// let style = Style::new()
///     .bold()
///     .italic()
///     .fg(Color::ansi(196))
///     .bg(Color::rgb(0, 0, 0));
/// ```
///
/// The escape code can then be obtained via `escape_code()` or
/// written directly to a writer via `write_escape_code()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    flags: u32,
    underline_style: UnderlineStyle,
    underline_color: Option<Color>,
    foreground: Option<Color>,
    background: Option<Color>,
}

impl Style {
    /// Creates a new empty style with all attributes at default values.
    pub const fn new() -> Self {
        Self {
            flags: 0,
            underline_style: UnderlineStyle::None,
            underline_color: None,
            foreground: None,
            background: None,
        }
    }

    /// Sets the foreground color.
    pub const fn fg(mut self, color: Color) -> Self {
        self.foreground = Some(color);
        self
    }

    /// Sets the background color.
    pub const fn bg(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// Enables bold text.
    pub const fn bold(mut self) -> Self {
        self.flags |= BOLD;
        self
    }

    /// Enables faint (reduced intensity) text.
    pub const fn faint(mut self) -> Self {
        self.flags |= FAINT;
        self
    }

    /// Enables italic text.
    pub const fn italic(mut self) -> Self {
        self.flags |= ITALIC;
        self
    }

    /// Enables underlined text.
    pub const fn underline(mut self) -> Self {
        self.flags |= UNDERLINE;
        self
    }

    /// Enables slow blinking text.
    pub const fn blink(mut self) -> Self {
        self.flags |= SLOW_BLINK;
        self
    }

    /// Enables reverse video (swaps foreground and background).
    pub const fn reverse(mut self) -> Self {
        self.flags |= REVERSE;
        self
    }

    /// Enables hidden text (useful for passwords).
    pub const fn hidden(mut self) -> Self {
        self.flags |= HIDDEN;
        self
    }

    /// Enables strikethrough text.
    pub const fn strikethrough(mut self) -> Self {
        self.flags |= STRIKETHROUGH;
        self
    }

    /// Enables overline text.
    pub const fn overline(mut self) -> Self {
        self.flags |= OVERLINE;
        self
    }

    /// Enables double underline.
    pub const fn double_underline(mut self) -> Self {
        self.flags |= DOUBLE_UNDERLINE;
        self
    }

    /// Sets the underline style (Kitty protocol).
    ///
    /// This extends basic underline to include curly, dotted, and dashed variants.
    pub const fn underline_style(mut self, style: UnderlineStyle) -> Self {
        self.underline_style = style;
        self
    }

    /// Sets the underline color (Kitty protocol).
    ///
    /// Allows the underline to have a different color than the foreground text.
    pub const fn underline_color(mut self, color: Color) -> Self {
        self.underline_color = Some(color);
        self
    }

    /// Generates the ANSI escape code for this style as a String.
    ///
    /// Returns an empty string if no style attributes are set.
    ///
    /// # Example
    ///
    /// ```
    /// use urvim::terminal::style::{Style, Color};
    ///
    /// let style = Style::new().bold().fg(Color::ansi(196));
    /// assert_eq!(style.escape_code(), "\x1b[1;38;5;196m");
    /// ```
    pub fn escape_code(&self) -> String {
        let mut buf = Vec::new();
        self.write_escape_code(&mut buf).ok();
        // Safe: escape codes only contain ASCII digits (0-9), semicolons (;),
        // colons (:), and escape sequences (\x1b, [), all valid UTF-8.
        String::from_utf8(buf).unwrap()
    }

    /// Writes the ANSI escape code for this style to a writer.
    ///
    /// This is the core function that generates the escape sequence.
    /// The format is `\x1b[paramsm` where params is a semicolon-separated
    /// list of SGR (Select Graphic Rendition) codes.
    ///
    /// The function generates codes in a specific order:
    /// 1. Text decorations (bold, italic, etc.) - codes 1-9, 21, 53
    /// 2. Underline style (4:n) - Kitty protocol extension
    /// 3. Underline color (58:...) - Kitty protocol extension
    /// 4. Foreground color (38:...)
    /// 5. Background color (48:...)
    ///
    /// Color codes:
    /// - ANSI: 38;5;n or 48;5;n
    /// - RGB: 38;2;r;g;b or 48;2;r;g;b
    pub fn write_escape_code<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut buf = [0u8; 128];
        let mut i = 0;

        // SGR introducer: ESC [
        buf[i] = b'\x1b';
        i += 1;
        buf[i] = b'[';
        i += 1;

        let mut first = true;

        // Text decorations are written first, in numeric order.
        // We track 'first' to determine whether to prepend a semicolon.

        // Bold (1), Faint (2), Italic (3), Underline (4), Blink (5,6), Reverse (7), Hidden (8), Strikethrough (9)
        if self.flags & BOLD != 0 {
            i = write_decimal(1, &mut buf, i);
            first = false;
        }
        if self.flags & FAINT != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(2, &mut buf, i);
            first = false;
        }
        if self.flags & ITALIC != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(3, &mut buf, i);
            first = false;
        }
        if self.flags & UNDERLINE != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(4, &mut buf, i);
            first = false;
        }
        if self.flags & SLOW_BLINK != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(5, &mut buf, i);
            first = false;
        }
        if self.flags & RAPID_BLINK != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(6, &mut buf, i);
            first = false;
        }
        if self.flags & REVERSE != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(7, &mut buf, i);
            first = false;
        }
        if self.flags & HIDDEN != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(8, &mut buf, i);
            first = false;
        }
        if self.flags & STRIKETHROUGH != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(9, &mut buf, i);
            first = false;
        }
        // Double underline (21) - note: this disables bold in some terminals
        if self.flags & DOUBLE_UNDERLINE != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(21, &mut buf, i);
            first = false;
        }
        // Overline (53)
        if self.flags & OVERLINE != 0 {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            i = write_decimal(53, &mut buf, i);
            first = false;
        }

        // Extended underline style: 4:style (Kitty protocol)
        // Format: 4:n where n is 0=none, 1=straight, 2=double, 3=curly, 4=dotted, 5=dashed
        if self.underline_style != UnderlineStyle::None {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            let style_val = match self.underline_style {
                UnderlineStyle::None => 0,
                UnderlineStyle::Straight => 1,
                UnderlineStyle::Double => 2,
                UnderlineStyle::Curly => 3,
                UnderlineStyle::Dotted => 4,
                UnderlineStyle::Dashed => 5,
            };
            i = write_decimal(4, &mut buf, i);
            buf[i] = b':';
            i += 1;
            i = write_decimal(style_val, &mut buf, i);
            first = false;
        }

        // Underline color (58) - Kitty protocol
        // Format: 58;5;n (ANSI) or 58;2;r;g;b (RGB)
        if let Some(color) = &self.underline_color {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            match color {
                Color::Ansi(ansi) => {
                    // 58;5;n - underline color ANSI
                    i = write_decimal(58, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(5, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(*ansi as u32, &mut buf, i);
                }
                Color::Rgb(rgb) => {
                    // 58;2;r;g;b - underline color RGB
                    i = write_decimal(58, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(2, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(rgb.r as u32, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(rgb.g as u32, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(rgb.b as u32, &mut buf, i);
                }
            }
            first = false;
        }

        // Foreground color (38) - must come after decorations
        // Format: 38;5;n (ANSI) or 38;2;r;g;b (RGB)
        if let Some(fg) = &self.foreground {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            match fg {
                Color::Ansi(ansi) => {
                    i = write_decimal(38, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(5, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(*ansi as u32, &mut buf, i);
                }
                Color::Rgb(rgb) => {
                    i = write_decimal(38, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(2, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(rgb.r as u32, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(rgb.g as u32, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(rgb.b as u32, &mut buf, i);
                }
            }
            first = false;
        }

        // Background color (48) - must come after foreground
        // Format: 48;5;n (ANSI) or 48;2;r;g;b (RGB)
        if let Some(bg) = &self.background {
            if !first {
                buf[i] = b';';
                i += 1;
            }
            match bg {
                Color::Ansi(ansi) => {
                    i = write_decimal(48, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(5, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(*ansi as u32, &mut buf, i);
                }
                Color::Rgb(rgb) => {
                    i = write_decimal(48, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(2, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(rgb.r as u32, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(rgb.g as u32, &mut buf, i);
                    buf[i] = b';';
                    i += 1;
                    i = write_decimal(rgb.b as u32, &mut buf, i);
                }
            }
            first = false;
        }

        // If no attributes were set, return empty string
        if first {
            return Ok(());
        }

        // SGR terminator
        buf[i] = b'm';
        i += 1;

        writer.write_all(&buf[..i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_creation() {
        let rgb = Rgb::new(255, 128, 0);
        assert_eq!(rgb.r, 255);
        assert_eq!(rgb.g, 128);
        assert_eq!(rgb.b, 0);
    }

    #[test]
    fn test_color_ansi() {
        let color = Color::ansi(196);
        assert_eq!(color, Color::Ansi(196));
    }

    #[test]
    fn test_color_rgb() {
        let color = Color::rgb(255, 128, 64);
        assert_eq!(color, Color::Rgb(Rgb::new(255, 128, 64)));
    }

    #[test]
    fn test_style_empty() {
        let style = Style::new();
        assert_eq!(style.escape_code(), "");
    }

    #[test]
    fn test_style_bold() {
        let style = Style::new().bold();
        assert_eq!(style.escape_code(), "\x1b[1m");
    }

    #[test]
    fn test_style_italic() {
        let style = Style::new().italic();
        assert_eq!(style.escape_code(), "\x1b[3m");
    }

    #[test]
    fn test_style_underline() {
        let style = Style::new().underline();
        assert_eq!(style.escape_code(), "\x1b[4m");
    }

    #[test]
    fn test_style_multiple() {
        let style = Style::new().bold().italic().underline();
        assert_eq!(style.escape_code(), "\x1b[1;3;4m");
    }

    #[test]
    fn test_style_fg_ansi() {
        let style = Style::new().fg(Color::ansi(196));
        assert_eq!(style.escape_code(), "\x1b[38;5;196m");
    }

    #[test]
    fn test_style_fg_rgb() {
        let style = Style::new().fg(Color::rgb(255, 128, 64));
        assert_eq!(style.escape_code(), "\x1b[38;2;255;128;64m");
    }

    #[test]
    fn test_style_bg_ansi() {
        let style = Style::new().bg(Color::ansi(21));
        assert_eq!(style.escape_code(), "\x1b[48;5;21m");
    }

    #[test]
    fn test_style_bg_rgb() {
        let style = Style::new().bg(Color::rgb(0, 255, 128));
        assert_eq!(style.escape_code(), "\x1b[48;2;0;255;128m");
    }

    #[test]
    fn test_style_fg_bg_decoration() {
        let style = Style::new()
            .fg(Color::ansi(196))
            .bg(Color::rgb(0, 0, 0))
            .bold();
        assert_eq!(style.escape_code(), "\x1b[1;38;5;196;48;2;0;0;0m");
    }

    #[test]
    fn test_style_kitty_underline_style() {
        let style = Style::new().underline_style(UnderlineStyle::Curly);
        assert_eq!(style.escape_code(), "\x1b[4:3m");
    }

    #[test]
    fn test_style_kitty_underline_color_ansi() {
        let style = Style::new().underline_color(Color::ansi(208));
        assert_eq!(style.escape_code(), "\x1b[58;5;208m");
    }

    #[test]
    fn test_style_kitty_underline_color_rgb() {
        let style = Style::new().underline_color(Color::rgb(255, 0, 0));
        assert_eq!(style.escape_code(), "\x1b[58;2;255;0;0m");
    }

    #[test]
    fn test_style_kitty_underline_style_and_color() {
        let style = Style::new()
            .underline_style(UnderlineStyle::Double)
            .underline_color(Color::rgb(0, 255, 0));
        assert_eq!(style.escape_code(), "\x1b[4:2;58;2;0;255;0m");
    }

    #[test]
    fn test_style_builder_pattern() {
        let style = Style::new()
            .fg(Color::rgb(255, 200, 100))
            .bg(Color::ansi(236))
            .bold()
            .italic()
            .underline();
        assert_eq!(style.escape_code(), "\x1b[1;3;4;38;2;255;200;100;48;5;236m");
    }

    #[test]
    fn test_style_copy() {
        let style1 = Style::new().bold().fg(Color::ansi(196));
        let style2 = style1;
        assert_eq!(style1.escape_code(), "\x1b[1;38;5;196m");
        assert_eq!(style2.escape_code(), "\x1b[1;38;5;196m");
    }

    #[test]
    fn test_style_all_decorations() {
        let style = Style::new()
            .bold()
            .faint()
            .italic()
            .underline()
            .blink()
            .reverse()
            .hidden()
            .strikethrough()
            .overline()
            .double_underline();
        assert_eq!(style.escape_code(), "\x1b[1;2;3;4;5;7;8;9;21;53m");
    }

    #[test]
    fn test_style_underline_and_underline_style_together() {
        let style = Style::new()
            .underline()
            .underline_style(UnderlineStyle::Double);
        assert_eq!(style.escape_code(), "\x1b[4;4:2m");
    }

    #[test]
    fn test_write_escape_code_empty() {
        let style = Style::new();
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"");
    }

    #[test]
    fn test_write_escape_code_bold() {
        let style = Style::new().bold();
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[1m");
    }

    #[test]
    fn test_write_escape_code_multiple() {
        let style = Style::new().bold().italic().underline();
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[1;3;4m");
    }

    #[test]
    fn test_write_escape_code_fg_ansi() {
        let style = Style::new().fg(Color::ansi(196));
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[38;5;196m");
    }

    #[test]
    fn test_write_escape_code_fg_rgb() {
        let style = Style::new().fg(Color::rgb(255, 128, 64));
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[38;2;255;128;64m");
    }

    #[test]
    fn test_write_escape_code_bg_ansi() {
        let style = Style::new().bg(Color::ansi(236));
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[48;5;236m");
    }

    #[test]
    fn test_write_escape_code_bg_rgb() {
        let style = Style::new().bg(Color::rgb(0, 255, 128));
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[48;2;0;255;128m");
    }

    #[test]
    fn test_write_escape_code_fg_bg() {
        let style = Style::new()
            .fg(Color::rgb(255, 200, 100))
            .bg(Color::ansi(236));
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[38;2;255;200;100;48;5;236m");
    }

    #[test]
    fn test_write_escape_code_full() {
        let style = Style::new()
            .fg(Color::rgb(255, 200, 100))
            .bg(Color::ansi(236))
            .bold()
            .italic()
            .underline();
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[1;3;4;38;2;255;200;100;48;5;236m");
    }

    #[test]
    fn test_write_escape_code_kitty_underline_style() {
        let style = Style::new().underline_style(UnderlineStyle::Double);
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[4:2m");
    }

    #[test]
    fn test_write_escape_code_kitty_underline_color_ansi() {
        let style = Style::new().underline_color(Color::ansi(208));
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[58;5;208m");
    }

    #[test]
    fn test_write_escape_code_kitty_underline_color_rgb() {
        let style = Style::new().underline_color(Color::rgb(255, 0, 0));
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[58;2;255;0;0m");
    }

    #[test]
    fn test_write_escape_code_all_decorations() {
        let style = Style::new()
            .bold()
            .faint()
            .italic()
            .underline()
            .blink()
            .reverse()
            .hidden()
            .strikethrough()
            .overline()
            .double_underline();
        let mut buf = Vec::new();
        style.write_escape_code(&mut buf).unwrap();
        assert_eq!(buf, b"\x1b[1;2;3;4;5;7;8;9;21;53m");
    }

    #[test]
    fn test_write_escape_code_matches_escape_code() {
        let styles = [
            Style::new(),
            Style::new().bold(),
            Style::new().italic(),
            Style::new().underline(),
            Style::new().bold().italic().underline(),
            Style::new().fg(Color::ansi(196)),
            Style::new().fg(Color::rgb(255, 128, 64)),
            Style::new().bg(Color::ansi(236)),
            Style::new().bg(Color::rgb(0, 255, 128)),
            Style::new()
                .fg(Color::rgb(255, 200, 100))
                .bg(Color::ansi(236)),
            Style::new()
                .fg(Color::rgb(255, 200, 100))
                .bg(Color::ansi(236))
                .bold()
                .italic()
                .underline(),
            Style::new().underline_style(UnderlineStyle::Double),
            Style::new().underline_color(Color::ansi(208)),
            Style::new().underline_color(Color::rgb(255, 0, 0)),
            Style::new()
                .underline_style(UnderlineStyle::Double)
                .underline_color(Color::rgb(0, 255, 0)),
            Style::new()
                .bold()
                .faint()
                .italic()
                .underline()
                .blink()
                .reverse()
                .hidden()
                .strikethrough()
                .overline()
                .double_underline(),
        ];

        for style in styles {
            let mut buf = Vec::new();
            style.write_escape_code(&mut buf).unwrap();
            let expected = style.escape_code();
            assert_eq!(buf, expected.as_bytes(), "failed for style: {:?}", style);
        }
    }
}
