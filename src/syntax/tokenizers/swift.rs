//! Builtin handwritten scanner for Swift syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{
    is_word_byte, match_function_call_with, match_line_prefixed_identifier_with,
    match_operator_from_sets, run_while,
};
use crate::buffer::syntax::{CodeState, ContextId, ContextStack, SyntaxSpan, SyntaxState};
use crate::theme::Tag;

macro_rules! tag_static {
    ($name:ident, $s:expr) => {
        static $name: LazyLock<Tag> = LazyLock::new(|| Tag::parse($s).unwrap());
    };
}

tag_static!(COMMENT_LINE, "comment.line");
tag_static!(COMMENT_BLOCK, "comment.block");
tag_static!(KW, "keyword");
tag_static!(S, "string");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(FN, "function");
tag_static!(OP, "operator");

const SWIFT_BLOCK_COMMENT: ContextId = ContextId::new("swift", "swift_block_comment");
const SWIFT_TRIPLE_STRING: ContextId = ContextId::new("swift", "swift_triple_string");
const SWIFT_STRING: ContextId = ContextId::new("swift", "swift_string");

/// Tokenize one line of Swift using the builtin scanner.
pub(crate) fn tokenize_swift_line(
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    let (mut ctx, inj, parent_style, tokenizer_state) = match state {
        SyntaxState::Code(CodeState::Scanner {
            contexts,
            injection,
            parent_style,
            tokenizer_state,
        }) => (contexts, injection, parent_style, tokenizer_state),
        SyntaxState::Plain => (ContextStack::default(), None, None, Default::default()),
    };

    let mut spans: Vec<SyntaxSpan> = Vec::new();
    let mut index = 0usize;
    let bytes = line.as_bytes();
    let line_len = bytes.len();

    while index < line_len {
        let tail = &line[index..];
        let tail_bytes = &bytes[index..];

        if ctx.top_is(SWIFT_BLOCK_COMMENT) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(SWIFT_BLOCK_COMMENT);
                index = end;
                continue;
            }
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.push(SWIFT_BLOCK_COMMENT);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '*' && c != '/');
            if run > 0 {
                spans.push(SyntaxSpan::new(
                    index,
                    index + run,
                    (*COMMENT_BLOCK).clone(),
                ));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else {
                break;
            };
            let end = index + ch.len_utf8();
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            index = end;
            continue;
        }

        if ctx.top_is(SWIFT_TRIPLE_STRING) {
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(SWIFT_TRIPLE_STRING);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        if ctx.top_is(SWIFT_STRING) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(SWIFT_STRING);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'/' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(SWIFT_BLOCK_COMMENT);
            index = end;
            continue;
        }

        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'"'
            && tail_bytes[1] == b'"'
            && tail_bytes[2] == b'"'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(SWIFT_TRIPLE_STRING);
            index = end;
            continue;
        }

        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(SWIFT_STRING);
            index = end;
            continue;
        }

        if let Some(len) = match_annotation(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_function_call(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*FN).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_capitalized_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        if let Some(len) = match_operator(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*OP).clone()));
            index += len;
            continue;
        }

        let Some(ch) = tail.chars().next() else { break };
        index += ch.len_utf8();
    }

    (
        spans,
        SyntaxState::Code(CodeState::Scanner {
            contexts: ctx,
            injection: inj,
            parent_style,
            tokenizer_state,
        }),
    )
}

fn match_annotation(tail: &str) -> Option<usize> {
    match_line_prefixed_identifier_with(tail, b'@', is_ascii_ident_start, is_dotted_ident_continue)
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    for kw in &[
        "as",
        "associatedtype",
        "break",
        "case",
        "catch",
        "class",
        "continue",
        "default",
        "defer",
        "deinit",
        "do",
        "else",
        "enum",
        "extension",
        "fallthrough",
        "for",
        "func",
        "guard",
        "if",
        "in",
        "import",
        "init",
        "inout",
        "internal",
        "is",
        "let",
        "operator",
        "private",
        "protocol",
        "public",
        "repeat",
        "return",
        "self",
        "static",
        "struct",
        "subscript",
        "super",
        "switch",
        "throw",
        "try",
        "typealias",
        "var",
        "where",
        "while",
    ] {
        if tail.starts_with(kw) {
            let after = kw.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    for typ in &[
        "Bool",
        "Int",
        "Int8",
        "Int16",
        "Int32",
        "Int64",
        "UInt",
        "UInt8",
        "UInt16",
        "UInt32",
        "UInt64",
        "Float",
        "Double",
        "String",
        "Character",
        "Array",
        "Dictionary",
        "Set",
        "Optional",
        "Result",
        "Any",
        "Never",
    ] {
        if tail.starts_with(typ) {
            let after = typ.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_constant(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    for word in &["false", "null", "true", "nil"] {
        if tail.starts_with(word) {
            let after = word.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_function_call(tail: &str) -> Option<usize> {
    match_function_call_with(tail, is_ascii_ident_start, is_ascii_ident_continue)
}

fn is_ascii_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ascii_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_dotted_ident_continue(byte: u8) -> bool {
    is_ascii_ident_continue(byte) || byte == b'.'
}

fn match_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_word_byte(full_bytes[index - 1])) {
        return None;
    }
    let bytes = tail.as_bytes();
    let len = bytes.len();
    if !bytes[0].is_ascii_digit() {
        return None;
    }

    if len >= 2 && bytes[0] == b'0' {
        match bytes[1] {
            b'x' | b'X' => return match_based_number(bytes, 2, is_hex_number_byte),
            b'b' | b'B' => return match_based_number(bytes, 2, is_binary_number_byte),
            b'o' | b'O' => return match_based_number(bytes, 2, is_octal_number_byte),
            _ => {}
        }
    }

    let mut i = consume_digits_and_underscores(bytes, 0, u8::is_ascii_digit)?;
    if i < len && bytes[i] == b'.' {
        let dot = i;
        if i + 1 < len && bytes[i + 1].is_ascii_digit() {
            i = consume_digits_and_underscores(bytes, i + 1, u8::is_ascii_digit)?;
        } else {
            i = dot;
        }
    }

    if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
        let exponent_start = i;
        i += 1;
        if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        if let Some(exponent_end) = consume_digits_and_underscores(bytes, i, u8::is_ascii_digit) {
            i = exponent_end;
        } else {
            i = exponent_start;
        }
    }

    if i < len && is_word_byte(bytes[i]) {
        return None;
    }
    Some(i)
}

fn match_based_number(
    bytes: &[u8],
    start: usize,
    is_valid_digit: fn(&u8) -> bool,
) -> Option<usize> {
    let end = consume_digits_and_underscores(bytes, start, is_valid_digit)?;
    if end < bytes.len() && is_word_byte(bytes[end]) {
        return None;
    }
    Some(end)
}

fn consume_digits_and_underscores(
    bytes: &[u8],
    start: usize,
    is_valid_digit: fn(&u8) -> bool,
) -> Option<usize> {
    let mut i = start;
    let mut saw_digit = false;
    let mut previous_was_underscore = false;
    while i < bytes.len() && (is_valid_digit(&bytes[i]) || bytes[i] == b'_') {
        if bytes[i] == b'_' {
            if !saw_digit || previous_was_underscore {
                return None;
            }
            previous_was_underscore = true;
        } else {
            saw_digit = true;
            previous_was_underscore = false;
        }
        i += 1;
    }
    if !saw_digit || previous_was_underscore {
        return None;
    }
    Some(i)
}

fn is_hex_number_byte(byte: &u8) -> bool {
    byte.is_ascii_hexdigit()
}

fn is_binary_number_byte(byte: &u8) -> bool {
    *byte == b'0' || *byte == b'1'
}

fn is_octal_number_byte(byte: &u8) -> bool {
    matches!(*byte, b'0'..=b'7')
}

fn match_capitalized_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_word_byte(full_bytes[index - 1])) {
        return None;
    }
    let bytes = tail.as_bytes();
    if !bytes[0].is_ascii_uppercase() {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    if i < bytes.len() && is_word_byte(bytes[i]) {
        return None;
    }
    Some(i)
}

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &[
            "===", "!==", "==", "!=", "<=", ">=", "=>", "??", "?.", "++", "--",
        ],
        b"+-*/%=&|!<>^~?",
    )
}
