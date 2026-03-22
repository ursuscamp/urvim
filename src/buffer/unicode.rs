use super::*;

pub fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

pub fn str_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn grapheme_width(grapheme: &str) -> usize {
    UnicodeWidthStr::width(grapheme)
}

pub fn to_byte_index(char_idx: usize, text: &str) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}
