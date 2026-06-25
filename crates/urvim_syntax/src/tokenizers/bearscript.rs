//! Builtin handwritten scanner for BearScript syntax.

use std::sync::LazyLock;

use super::scanner::{is_word_byte, match_function_call_with, match_word_from_list, run_while};
use crate::state::{CodeState, ContextId, ContextStack, SyntaxSpan, SyntaxState};
use urvim_theme::Tag;

macro_rules! tag_static {
    ($name:ident, $value:expr) => {
        static $name: LazyLock<Tag> = LazyLock::new(|| Tag::parse($value).unwrap());
    };
}

tag_static!(COMMENT, "comment");
tag_static!(KEYWORD, "keyword");
tag_static!(STRING, "string");
tag_static!(PUNCTUATION, "punctuation");
tag_static!(NUMBER, "number");
tag_static!(CONSTANT, "constant");
tag_static!(FUNCTION, "function");
tag_static!(PROPERTY, "variable.property");
tag_static!(OPERATOR, "operator");

const STRING_CONTEXT: ContextId = ContextId::new("bearscript", "string");
const INTERPOLATION_CONTEXT: ContextId = ContextId::new("bearscript", "interpolation");

/// Tokenizes one line of BearScript using its lexer grammar.
pub(crate) fn tokenize_bearscript_line(
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    let (mut contexts, injection, parent_style, tokenizer_state) = match state {
        SyntaxState::Code(CodeState::Scanner {
            contexts,
            injection,
            parent_style,
            tokenizer_state,
        }) => (contexts, injection, parent_style, tokenizer_state),
        SyntaxState::Plain => (ContextStack::default(), None, None, Default::default()),
    };
    let mut spans = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let tail = &line[index..];
        let tail_bytes = &bytes[index..];

        if contexts.top_is(STRING_CONTEXT) {
            if tail_bytes[0] == b'"' {
                spans.push(SyntaxSpan::new(index, index + 1, (*STRING).clone()));
                contexts.pop_top(STRING_CONTEXT);
                index += 1;
                continue;
            }
            if tail_bytes[0] == b'{' {
                spans.push(SyntaxSpan::new(index, index + 1, (*PUNCTUATION).clone()));
                contexts.push(INTERPOLATION_CONTEXT);
                index += 1;
                continue;
            }
            if tail_bytes[0] == b'\\' && tail_bytes.len() >= 2 {
                spans.push(SyntaxSpan::new(index, index + 2, (*STRING).clone()));
                index += 2;
                continue;
            }
            let run = run_while(tail, |ch| ch != '"' && ch != '{' && ch != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*STRING).clone()));
                index += run;
                continue;
            }
        }

        if contexts.top_is(INTERPOLATION_CONTEXT) {
            if tail_bytes[0] == b'}' {
                spans.push(SyntaxSpan::new(index, index + 1, (*PUNCTUATION).clone()));
                contexts.pop_top(INTERPOLATION_CONTEXT);
                index += 1;
                continue;
            }
            if tail_bytes[0] == b'{' {
                spans.push(SyntaxSpan::new(index, index + 1, (*PUNCTUATION).clone()));
                contexts.push(INTERPOLATION_CONTEXT);
                index += 1;
                continue;
            }
        }

        if contexts.top_is(STRING_CONTEXT) {
            let char_len = tail.chars().next().map_or(1, char::len_utf8);
            spans.push(SyntaxSpan::new(index, index + char_len, (*STRING).clone()));
            index += char_len;
            continue;
        }

        if tail_bytes[0] == b'#' {
            spans.push(SyntaxSpan::new(index, bytes.len(), (*COMMENT).clone()));
            break;
        }
        if tail_bytes[0] == b'"' {
            spans.push(SyntaxSpan::new(index, index + 1, (*STRING).clone()));
            contexts.push(STRING_CONTEXT);
            index += 1;
            continue;
        }
        if let Some(length) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + length, (*KEYWORD).clone()));
            index += length;
            continue;
        }
        if let Some(length) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + length, (*CONSTANT).clone()));
            index += length;
            continue;
        }
        if let Some(length) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + length, (*NUMBER).clone()));
            index += length;
            continue;
        }
        if let Some(length) = match_property(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + length, (*PROPERTY).clone()));
            index += length;
            continue;
        }
        if let Some(length) = match_function_call_with(tail, is_identifier_start, is_word_byte) {
            spans.push(SyntaxSpan::new(index, index + length, (*FUNCTION).clone()));
            index += length;
            continue;
        }
        if tail.starts_with("..") {
            spans.push(SyntaxSpan::new(index, index + 2, (*PUNCTUATION).clone()));
            index += 2;
            continue;
        }
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b';' | b':' | b'.'
        ) {
            spans.push(SyntaxSpan::new(index, index + 1, (*PUNCTUATION).clone()));
            index += 1;
            continue;
        }
        if let Some(length) = match_operator(tail) {
            spans.push(SyntaxSpan::new(index, index + length, (*OPERATOR).clone()));
            index += length;
            continue;
        }
        index += tail.chars().next().map_or(1, char::len_utf8);
    }

    (
        spans,
        SyntaxState::Code(CodeState::Scanner {
            contexts,
            injection,
            parent_style,
            tokenizer_state,
        }),
    )
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn match_keyword(tail: &str, index: usize, bytes: &[u8]) -> Option<usize> {
    match_word_from_list(
        tail,
        &[
            "continue", "return", "import", "export", "while", "break", "else", "for", "and",
            "not", "gen", "yield", "let", "else", "if", "in", "fn", "or", "as",
        ],
        index,
        bytes,
        is_word_byte,
    )
}

fn match_constant(tail: &str, index: usize, bytes: &[u8]) -> Option<usize> {
    match_word_from_list(tail, &["false", "true", "null"], index, bytes, is_word_byte)
}

fn match_number(tail: &str, index: usize, bytes: &[u8]) -> Option<usize> {
    if !tail_bytes_are_number_start(tail, index, bytes) {
        return None;
    }
    let tail_bytes = tail.as_bytes();
    let mut length = 1;
    while length < tail_bytes.len() && tail_bytes[length].is_ascii_digit() {
        length += 1;
    }
    if length + 1 < tail_bytes.len()
        && tail_bytes[length] == b'.'
        && tail_bytes[length + 1].is_ascii_digit()
    {
        length += 1;
        while length < tail_bytes.len() && tail_bytes[length].is_ascii_digit() {
            length += 1;
        }
    }
    Some(length)
}

fn tail_bytes_are_number_start(tail: &str, index: usize, bytes: &[u8]) -> bool {
    tail.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && (index == 0 || !is_word_byte(bytes[index - 1]))
}

fn match_property(tail: &str, index: usize, bytes: &[u8]) -> Option<usize> {
    if index == 0 || bytes[index - 1] != b'.' || !is_identifier_start(tail.as_bytes()[0]) {
        return None;
    }
    let length = tail
        .as_bytes()
        .iter()
        .take_while(|byte| is_word_byte(**byte))
        .count();
    Some(length)
}

fn match_operator(tail: &str) -> Option<usize> {
    ["==", "!=", "<=", ">="]
        .into_iter()
        .find(|operator| tail.starts_with(operator))
        .map(str::len)
        .or_else(|| {
            matches!(
                tail.as_bytes()[0],
                b'+' | b'-' | b'*' | b'/' | b'%' | b'=' | b'!' | b'<' | b'>'
            )
            .then_some(1)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearscript_interpolation_preserves_string_state() {
        let (first_spans, state) =
            tokenize_bearscript_line("let name = \"hello {user", SyntaxState::Plain);
        assert!(first_spans.iter().any(|span| span.style == *STRING));
        assert!(first_spans.iter().any(|span| span.style == *PUNCTUATION));

        let (second_spans, state) = tokenize_bearscript_line(".name}!\"", state);
        assert!(second_spans.iter().any(|span| span.style == *PROPERTY));
        assert!(second_spans.iter().any(|span| span.style == *PUNCTUATION));
        assert!(second_spans.iter().any(|span| span.style == *STRING));

        let (_, state) = tokenize_bearscript_line("let done = true", state);
        assert!(!matches!(state, SyntaxState::Plain));
    }
}
