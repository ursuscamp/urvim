//! Shared conversions between LSP positions and byte offsets,
//! plus LSP–text-crate conversion helpers.

use lsp_types::{Position, PositionEncodingKind};
use urvim_text::{TextEncoding, TextPosition, TextRange};

// ---------------------------------------------------------------------------
// Raw byte-level helpers
// ---------------------------------------------------------------------------

/// Converts a byte index within a line to an LSP `character` value.
pub fn byte_index_to_position_character(
    line: &str,
    byte_index: usize,
    encoding: PositionEncodingKind,
) -> Option<u32> {
    if byte_index > line.len() {
        return None;
    }

    if encoding == PositionEncodingKind::UTF8 {
        return Some(byte_index as u32);
    }

    if encoding == PositionEncodingKind::UTF16 {
        let mut units = 0u32;
        for (offset, ch) in line.char_indices() {
            if offset >= byte_index {
                return Some(units);
            }
            units = units.saturating_add(ch.len_utf16() as u32);
        }
        return Some(units);
    }

    if encoding == PositionEncodingKind::UTF32 {
        let mut chars = 0u32;
        for (offset, _) in line.char_indices() {
            if offset >= byte_index {
                return Some(chars);
            }
            chars = chars.saturating_add(1);
        }
        return Some(chars);
    }

    None
}

/// Converts an LSP `character` value to a byte index within a line.
pub fn position_character_to_byte_index(
    line: &str,
    character: u32,
    encoding: PositionEncodingKind,
) -> Option<usize> {
    if encoding == PositionEncodingKind::UTF8 {
        return Some(character as usize).filter(|byte_index| *byte_index <= line.len());
    }

    if encoding == PositionEncodingKind::UTF16 {
        let target = character as usize;
        let mut units = 0usize;
        for (offset, ch) in line.char_indices() {
            if units == target {
                return Some(offset);
            }
            units = units.saturating_add(ch.len_utf16());
            if units > target {
                return None;
            }
        }
        return if units == target {
            Some(line.len())
        } else {
            None
        };
    }

    if encoding == PositionEncodingKind::UTF32 {
        let target = character as usize;
        let count = line.chars().count();
        if target > count {
            return None;
        }
        if target == count {
            return Some(line.len());
        }
        return line.char_indices().nth(target).map(|(offset, _)| offset);
    }

    None
}

/// Converts an LSP position to a byte offset within a multi-line string.
pub fn position_to_byte_offset(
    text: &str,
    position: Position,
    encoding: PositionEncodingKind,
) -> Option<usize> {
    let mut offset = 0usize;
    let total_lines = text.split('\n').count();

    for (line_idx, line) in text.split('\n').enumerate() {
        if line_idx == position.line as usize {
            let line_offset = position_character_to_byte_index(line, position.character, encoding)?;
            return Some(offset + line_offset);
        }

        offset = offset.saturating_add(line.len());
        if line_idx + 1 < total_lines {
            offset = offset.saturating_add(1);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// LSP ↔ text-crate conversion helpers
// ---------------------------------------------------------------------------

/// Converts an LSP position to a text-crate `TextPosition`.
pub fn text_position_from_lsp(position: lsp_types::Position) -> TextPosition {
    TextPosition {
        line: position.line as usize,
        character: position.character as usize,
    }
}

/// Converts a text-crate `TextPosition` to an LSP position.
pub fn lsp_position_from_text(position: TextPosition) -> lsp_types::Position {
    lsp_types::Position::new(position.line as u32, position.character as u32)
}

/// Converts an LSP range to a text-crate `TextRange`.
pub fn text_range_from_lsp(range: lsp_types::Range) -> TextRange {
    TextRange {
        start: text_position_from_lsp(range.start),
        end: text_position_from_lsp(range.end),
    }
}

/// Converts a text-crate `TextRange` to an LSP range.
pub fn lsp_range_from_text(range: TextRange) -> lsp_types::Range {
    lsp_types::Range::new(
        lsp_position_from_text(range.start),
        lsp_position_from_text(range.end),
    )
}

/// Converts an LSP position encoding kind to a text-crate `TextEncoding`.
pub fn text_encoding_from_lsp(encoding: PositionEncodingKind) -> TextEncoding {
    if encoding == PositionEncodingKind::UTF8 {
        TextEncoding::Utf8
    } else {
        TextEncoding::Utf16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_round_trip() {
        let line = "hé😀";
        assert_eq!(
            position_character_to_byte_index(line, 0, PositionEncodingKind::UTF8),
            Some(0)
        );
        assert_eq!(
            position_character_to_byte_index(line, 3, PositionEncodingKind::UTF8),
            Some(3)
        );
        assert_eq!(
            byte_index_to_position_character(line, 3, PositionEncodingKind::UTF8),
            Some(3)
        );
    }

    #[test]
    fn utf16_round_trip() {
        let line = "a😀b";
        assert_eq!(
            position_character_to_byte_index(line, 0, PositionEncodingKind::UTF16),
            Some(0)
        );
        assert_eq!(
            position_character_to_byte_index(line, 1, PositionEncodingKind::UTF16),
            Some(1)
        );
        assert_eq!(
            position_character_to_byte_index(line, 3, PositionEncodingKind::UTF16),
            Some(5)
        );
        assert_eq!(
            byte_index_to_position_character(line, 5, PositionEncodingKind::UTF16),
            Some(3)
        );
    }

    #[test]
    fn utf32_round_trip() {
        let line = "a😀b";
        assert_eq!(
            position_character_to_byte_index(line, 0, PositionEncodingKind::UTF32),
            Some(0)
        );
        assert_eq!(
            position_character_to_byte_index(line, 2, PositionEncodingKind::UTF32),
            Some(5)
        );
        assert_eq!(
            byte_index_to_position_character(line, 5, PositionEncodingKind::UTF32),
            Some(2)
        );
    }

    #[test]
    fn position_to_byte_offset_spans_lines() {
        let text = "a😀\nbc";
        let position = Position::new(1, 1);
        assert_eq!(
            position_to_byte_offset(text, position, PositionEncodingKind::UTF8),
            Some(7)
        );
    }

    #[test]
    fn text_position_round_trips() {
        let pos = lsp_types::Position::new(5, 12);
        let tp = text_position_from_lsp(pos);
        assert_eq!(tp.line, 5);
        assert_eq!(tp.character, 12);
        let back = lsp_position_from_text(tp);
        assert_eq!(back.line, 5);
        assert_eq!(back.character, 12);
    }

    #[test]
    fn text_range_round_trips() {
        let range = lsp_types::Range::new(
            lsp_types::Position::new(1, 2),
            lsp_types::Position::new(10, 20),
        );
        let tr = text_range_from_lsp(range);
        assert_eq!(tr.start.line, 1);
        assert_eq!(tr.start.character, 2);
        assert_eq!(tr.end.line, 10);
        assert_eq!(tr.end.character, 20);
        let back = lsp_range_from_text(tr);
        assert_eq!(back.start.line, 1);
        assert_eq!(back.start.character, 2);
        assert_eq!(back.end.line, 10);
        assert_eq!(back.end.character, 20);
    }

    #[test]
    fn text_encoding_from_lsp_utf8() {
        assert_eq!(
            text_encoding_from_lsp(PositionEncodingKind::UTF8),
            TextEncoding::Utf8
        );
    }

    #[test]
    fn text_encoding_from_lsp_utf16() {
        assert_eq!(
            text_encoding_from_lsp(PositionEncodingKind::UTF16),
            TextEncoding::Utf16
        );
    }

    #[test]
    fn text_encoding_from_lsp_utf32_falls_back_to_utf16() {
        assert_eq!(
            text_encoding_from_lsp(PositionEncodingKind::UTF32),
            TextEncoding::Utf16
        );
    }
}
