//! Shared conversions between LSP positions and byte offsets.

use lsp_types::{Position, PositionEncodingKind};

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
}
