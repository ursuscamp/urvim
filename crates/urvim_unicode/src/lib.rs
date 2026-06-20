//! Utilities for clipping text to terminal display widths.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const ELLIPSIS: &str = "…";

/// Direction used when clipping text to a display width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipSide {
    /// Keep the visible prefix.
    Start,
    /// Keep the visible suffix.
    End,
    /// Keep text from both ends.
    Center,
}

/// Placement used when replacing clipped text with an ellipsis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EllipsisSide {
    /// Place the ellipsis at the start and keep the suffix.
    Start,
    /// Place the ellipsis in the middle and keep both ends.
    Middle,
    /// Place the ellipsis at the end and keep the prefix.
    End,
}

/// Text clipped to fit a terminal display width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClippedText {
    /// Clipped text.
    pub text: String,
    /// Display width of `text`.
    pub width: usize,
    /// Byte offset in the original input where retained text starts.
    pub start_byte: usize,
    /// Byte offset in the original input where retained text ends.
    pub end_byte: usize,
}

/// Returns the terminal display width of text.
pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Clips text to `max_width` terminal display columns.
pub fn clip_text(text: &str, max_width: usize, side: ClipSide) -> ClippedText {
    if max_width == 0 || text.is_empty() {
        return empty_clip();
    }

    match side {
        ClipSide::Start => prefix_by_width(text, max_width),
        ClipSide::End => suffix_by_width(text, max_width),
        ClipSide::Center => center_by_width(text, max_width),
    }
}

/// Clips the first line of text to `max_width` terminal display columns.
pub fn clip_first_line(text: &str, max_width: usize, side: ClipSide) -> ClippedText {
    clip_text(text.lines().next().unwrap_or(""), max_width, side)
}

/// Clips text to `max_width` terminal display columns and inserts an ellipsis.
pub fn ellipsize_text(text: &str, max_width: usize, side: EllipsisSide) -> ClippedText {
    if max_width == 0 || text.is_empty() {
        return empty_clip();
    }

    let text_width = display_width(text);
    if text_width <= max_width {
        return ClippedText {
            text: text.to_string(),
            width: text_width,
            start_byte: 0,
            end_byte: text.len(),
        };
    }

    let ellipsis_width = display_width(ELLIPSIS);
    if max_width <= ellipsis_width {
        return ClippedText {
            text: ELLIPSIS.to_string(),
            width: ellipsis_width,
            start_byte: 0,
            end_byte: 0,
        };
    }

    let content_width = max_width - ellipsis_width;
    match side {
        EllipsisSide::Start => {
            let clipped = suffix_by_width(text, content_width);
            ClippedText {
                text: format!("{ELLIPSIS}{}", clipped.text),
                width: clipped.width + ellipsis_width,
                start_byte: clipped.start_byte,
                end_byte: clipped.end_byte,
            }
        }
        EllipsisSide::Middle => {
            let left_width = content_width.div_ceil(2);
            let right_width = content_width - left_width;
            let prefix = prefix_by_width(text, left_width);
            let suffix = suffix_by_width(text, right_width);
            ClippedText {
                text: format!("{}{ELLIPSIS}{}", prefix.text, suffix.text),
                width: prefix.width + ellipsis_width + suffix.width,
                start_byte: prefix.start_byte,
                end_byte: suffix.end_byte,
            }
        }
        EllipsisSide::End => {
            let clipped = prefix_by_width(text, content_width);
            ClippedText {
                text: format!("{}{ELLIPSIS}", clipped.text),
                width: clipped.width + ellipsis_width,
                start_byte: clipped.start_byte,
                end_byte: clipped.end_byte,
            }
        }
    }
}

fn empty_clip() -> ClippedText {
    ClippedText {
        text: String::new(),
        width: 0,
        start_byte: 0,
        end_byte: 0,
    }
}

fn prefix_by_width(text: &str, max_width: usize) -> ClippedText {
    let mut end_byte = 0usize;
    let mut width = 0usize;
    for (byte_idx, grapheme) in text.grapheme_indices(true) {
        let grapheme_width = display_width(grapheme);
        if width.saturating_add(grapheme_width) > max_width {
            break;
        }
        end_byte = byte_idx + grapheme.len();
        width += grapheme_width;
    }

    ClippedText {
        text: text[..end_byte].to_string(),
        width,
        start_byte: 0,
        end_byte,
    }
}

fn suffix_by_width(text: &str, max_width: usize) -> ClippedText {
    let mut start_byte = text.len();
    let mut width = 0usize;

    for (byte_idx, grapheme) in text.grapheme_indices(true).rev() {
        let grapheme_width = display_width(grapheme);
        if width.saturating_add(grapheme_width) > max_width {
            break;
        }
        start_byte = byte_idx;
        width += grapheme_width;
    }

    ClippedText {
        text: text[start_byte..].to_string(),
        width,
        start_byte,
        end_byte: text.len(),
    }
}

fn center_by_width(text: &str, max_width: usize) -> ClippedText {
    let text_width = display_width(text);
    if text_width <= max_width {
        return ClippedText {
            text: text.to_string(),
            width: text_width,
            start_byte: 0,
            end_byte: text.len(),
        };
    }

    let left_width = max_width.div_ceil(2);
    let right_width = max_width - left_width;
    let prefix = prefix_by_width(text, left_width);
    let suffix = suffix_by_width(text, right_width);
    ClippedText {
        text: format!("{}{}", prefix.text, suffix.text),
        width: prefix.width + suffix.width,
        start_byte: prefix.start_byte,
        end_byte: suffix.end_byte,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_empty_and_zero_width_text() {
        assert_eq!(clip_text("abc", 0, ClipSide::Start).text, "");
        assert_eq!(clip_text("", 3, ClipSide::Start).text, "");
    }

    #[test]
    fn clips_ascii_text_by_side() {
        assert_eq!(clip_text("abcdef", 4, ClipSide::Start).text, "abcd");
        assert_eq!(clip_text("abcdef", 4, ClipSide::End).text, "cdef");
        assert_eq!(clip_text("abcdef", 4, ClipSide::Center).text, "abef");
    }

    #[test]
    fn clips_wide_graphemes_without_exceeding_width() {
        let clipped = clip_text("a你好b", 3, ClipSide::Start);
        assert_eq!(clipped.text, "a你");
        assert_eq!(clipped.width, 3);

        let clipped = clip_text("a你好b", 3, ClipSide::End);
        assert_eq!(clipped.text, "好b");
        assert_eq!(clipped.width, 3);
    }

    #[test]
    fn clips_combining_graphemes_without_splitting() {
        let text = "e\u{301}bc";
        let clipped = clip_text(text, 1, ClipSide::Start);
        assert_eq!(clipped.text, "e\u{301}");
        assert_eq!(clipped.end_byte, "e\u{301}".len());
    }

    #[test]
    fn ellipsizes_text_by_side() {
        assert_eq!(ellipsize_text("abcdef", 4, EllipsisSide::End).text, "abc…");
        assert_eq!(
            ellipsize_text("abcdef", 4, EllipsisSide::Start).text,
            "…def"
        );
        assert_eq!(
            ellipsize_text("abcdef", 4, EllipsisSide::Middle).text,
            "ab…f"
        );
    }

    #[test]
    fn ellipsis_uses_only_marker_when_width_is_one() {
        assert_eq!(ellipsize_text("abcdef", 1, EllipsisSide::End).text, "…");
    }

    #[test]
    fn first_line_clipping_ignores_later_lines() {
        assert_eq!(
            clip_first_line("abcdef\nghi", 3, ClipSide::Start).text,
            "abc"
        );
    }
}
