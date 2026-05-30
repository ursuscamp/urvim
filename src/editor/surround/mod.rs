//! Shared surround keybinding metadata.

use super::DelimiterFamily;

/// Returns canonical key strings for every supported surround delimiter selector.
pub fn delimiter_selectors() -> &'static [(&'static str, DelimiterFamily)] {
    &[
        ("(", DelimiterFamily::Paren),
        (")", DelimiterFamily::Paren),
        ("[", DelimiterFamily::Square),
        ("]", DelimiterFamily::Square),
        ("{", DelimiterFamily::Curly),
        ("}", DelimiterFamily::Curly),
        ("<LessThan>", DelimiterFamily::Angle),
        ("<GreaterThan>", DelimiterFamily::Angle),
        ("\"", DelimiterFamily::DoubleQuote),
        ("'", DelimiterFamily::SingleQuote),
        ("`", DelimiterFamily::Backtick),
    ]
}
