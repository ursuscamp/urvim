//! Key resolution helpers for text-object state machines.

use super::{BracketKind, QuoteKind, TextObject};

/// Whether a text-object key sequence targets the inner or around region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextObjectScope {
    /// Select the object contents, excluding delimiters or surrounding whitespace.
    Inner,
    /// Select the object plus delimiters or surrounding whitespace.
    Around,
}

impl TextObjectScope {
    /// Resolves `i` and `a` text-object scope keys.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "i" => Some(Self::Inner),
            "a" => Some(Self::Around),
            _ => None,
        }
    }
}

/// Resolves a text-object target key after an `i` or `a` scope key.
pub fn resolve(scope: TextObjectScope, key: &str) -> Option<TextObject> {
    match (scope, key) {
        (TextObjectScope::Inner, "w") => Some(TextObject::InnerWord),
        (TextObjectScope::Around, "w") => Some(TextObject::AroundWord),
        (TextObjectScope::Inner, "W") => Some(TextObject::InnerBigWord),
        (TextObjectScope::Around, "W") => Some(TextObject::AroundBigWord),
        (TextObjectScope::Inner, key) => resolve_bracket(key, TextObject::InnerBracket)
            .or_else(|| resolve_quote(key, TextObject::InnerQuote)),
        (TextObjectScope::Around, key) => resolve_bracket(key, TextObject::AroundBracket)
            .or_else(|| resolve_quote(key, TextObject::AroundQuote)),
    }
}

fn resolve_bracket(
    key: &str,
    make_text_object: impl FnOnce(BracketKind) -> TextObject,
) -> Option<TextObject> {
    let kind = match key {
        "(" | ")" => BracketKind::Paren,
        "[" | "]" => BracketKind::Square,
        "{" | "}" => BracketKind::Curly,
        "<LessThan>" | "<GreaterThan>" => BracketKind::Angle,
        _ => return None,
    };
    Some(make_text_object(kind))
}

fn resolve_quote(
    key: &str,
    make_text_object: impl FnOnce(QuoteKind) -> TextObject,
) -> Option<TextObject> {
    let kind = match key {
        "'" => QuoteKind::Single,
        "\"" => QuoteKind::Double,
        "`" => QuoteKind::Backtick,
        _ => return None,
    };
    Some(make_text_object(kind))
}
