//! Shared surround keybinding metadata.

use super::{BracketKind, DelimiterFamily, TextObject};

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

/// Returns canonical key strings for every supported text object sequence.
pub fn text_object_sequences() -> &'static [(&'static str, TextObject)] {
    &[
        ("iw", TextObject::InnerWord),
        ("aw", TextObject::AroundWord),
        ("iW", TextObject::InnerBigWord),
        ("aW", TextObject::AroundBigWord),
        ("i(", TextObject::InnerBracket(BracketKind::Paren)),
        ("i)", TextObject::InnerBracket(BracketKind::Paren)),
        ("a(", TextObject::AroundBracket(BracketKind::Paren)),
        ("a)", TextObject::AroundBracket(BracketKind::Paren)),
        ("i[", TextObject::InnerBracket(BracketKind::Square)),
        ("i]", TextObject::InnerBracket(BracketKind::Square)),
        ("a[", TextObject::AroundBracket(BracketKind::Square)),
        ("a]", TextObject::AroundBracket(BracketKind::Square)),
        ("i{", TextObject::InnerBracket(BracketKind::Curly)),
        ("i}", TextObject::InnerBracket(BracketKind::Curly)),
        ("a{", TextObject::AroundBracket(BracketKind::Curly)),
        ("a}", TextObject::AroundBracket(BracketKind::Curly)),
        ("i<LessThan>", TextObject::InnerBracket(BracketKind::Angle)),
        (
            "i<GreaterThan>",
            TextObject::InnerBracket(BracketKind::Angle),
        ),
        ("a<LessThan>", TextObject::AroundBracket(BracketKind::Angle)),
        (
            "a<GreaterThan>",
            TextObject::AroundBracket(BracketKind::Angle),
        ),
        ("i'", TextObject::InnerQuote(super::QuoteKind::Single)),
        ("a'", TextObject::AroundQuote(super::QuoteKind::Single)),
        ("i\"", TextObject::InnerQuote(super::QuoteKind::Double)),
        ("a\"", TextObject::AroundQuote(super::QuoteKind::Double)),
        ("i`", TextObject::InnerQuote(super::QuoteKind::Backtick)),
        ("a`", TextObject::AroundQuote(super::QuoteKind::Backtick)),
    ]
}
