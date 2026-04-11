use super::*;
use crate::config::DEFAULT_TAB_WIDTH;
use crate::globals;
use unicode_segmentation::UnicodeSegmentation;

pub fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

pub fn str_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn grapheme_width(grapheme: &str) -> usize {
    UnicodeWidthStr::width(grapheme)
}

/// Returns the configured tab width, falling back to the built-in default.
pub fn configured_tab_width() -> usize {
    globals::with_config(|config| config.tab_width).unwrap_or(DEFAULT_TAB_WIDTH)
}

/// Returns the visual width of a character at the given column.
pub fn display_char_width(ch: char, _visual_col: usize, tab_width: usize) -> usize {
    if ch == '\t' {
        tab_width.max(1)
    } else {
        char_width(ch)
    }
}

/// Returns the visual width of a grapheme at the given column.
pub fn display_grapheme_width(grapheme: &str, visual_col: usize, tab_width: usize) -> usize {
    if grapheme == "\t" {
        display_char_width('\t', visual_col, tab_width)
    } else {
        grapheme_width(grapheme)
    }
}

/// Returns the visual width of text starting at the given column.
pub fn display_width_at(text: &str, start_visual_col: usize, tab_width: usize) -> usize {
    let mut visual_col = start_visual_col;
    let mut width = 0;

    for grapheme in text.graphemes(true) {
        let grapheme_width = display_grapheme_width(grapheme, visual_col, tab_width);
        visual_col += grapheme_width;
        width += grapheme_width;
    }

    width
}

/// Expands tab characters in text to spaces using the given starting column.
pub fn expand_tabs(text: &str, start_visual_col: usize, tab_width: usize) -> String {
    let mut visual_col = start_visual_col;
    let mut output = String::with_capacity(text.len());

    for grapheme in text.graphemes(true) {
        if grapheme == "\t" {
            let width = display_char_width('\t', visual_col, tab_width);
            output.push_str(&" ".repeat(width));
            visual_col += width;
        } else {
            output.push_str(grapheme);
            visual_col += grapheme_width(grapheme);
        }
    }

    output
}

pub fn to_byte_index(char_idx: usize, text: &str) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}
