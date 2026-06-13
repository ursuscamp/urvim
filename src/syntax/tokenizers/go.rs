//! Builtin handwritten scanner for Go syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{is_word_byte, match_function_call_with, match_operator_from_sets, run_while};
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

const GO_BLOCK_COMMENT: ContextId = ContextId::new("go", "go_block_comment");
const GO_RAW_STRING: ContextId = ContextId::new("go", "go_raw_string");
const GO_STRING: ContextId = ContextId::new("go", "go_string");
const GO_CHAR: ContextId = ContextId::new("go", "go_char");

/// Tokenize one line of Go using the builtin scanner.
pub(crate) fn tokenize_go_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Inside block comment ─────────────────────────────────────
        if ctx.top_is(GO_BLOCK_COMMENT) {
            // Rule 2: */ close
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(GO_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Rule 3: /* nested open
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.push(GO_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Fall through to top-level (no content rule)
        }

        // ── Inside raw string ────────────────────────────────────────
        if ctx.top_is(GO_RAW_STRING) {
            // Rule 4: Closing `
            if tail_bytes[0] == b'`' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(GO_RAW_STRING);
                index = end;
                continue;
            }
            // Rule 6: Content [^`]+ (multi-line, no \n exclusion)
            let run = run_while(tail, |c| c != '`');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside string ────────────────────────────────────────────
        if ctx.top_is(GO_STRING) {
            // Rule 7: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(GO_STRING);
                index = end;
                continue;
            }
            // Rule 10: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 9: Content [^"\\\n]+
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

        // ── Inside char literal ──────────────────────────────────────
        if ctx.top_is(GO_CHAR) {
            // Rule 11: Closing '
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
                ctx.pop_top(GO_CHAR);
                index = end;
                continue;
            }
            // Rule 14: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 13: Content [^'\\\n]+
            let run = run_while(tail, |c| c != '\'' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*CNST).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Top-level ────────────────────────────────────────────────

        // Rule 1: Line comment //
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'/' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        // Rule 3: Block comment /* (outside comment)
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(GO_BLOCK_COMMENT);
            index = end;
            continue;
        }

        // Rule 5: Raw string open `
        if tail_bytes[0] == b'`' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(GO_RAW_STRING);
            index = end;
            continue;
        }

        // Rule 8: String open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(GO_STRING);
            index = end;
            continue;
        }

        // Rule 12: Char open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
            ctx.push(GO_CHAR);
            index = end;
            continue;
        }

        // Rule 15: Number with word boundaries
        if let Some(len) = match_go_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // Rule 16: Keyword with word boundaries
        if let Some(len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        // Rule 17: Type with word boundaries
        if let Some(len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        // Rule 18: Function call (identifier + lookahead \s*\()
        if let Some(len) = match_function_call(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*FN).clone()));
            index += len;
            continue;
        }

        // Rule 19: Constant with word boundaries
        if let Some(len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        // Rule 20: Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        // Rule 21: Operator
        if let Some(len) = match_operator(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*OP).clone()));
            index += len;
            continue;
        }

        // ── No match – skip one char ─────────────────────────────────
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

// ── helpers ─────────────────────────────────────────────────────────────────

fn match_go_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    let bytes = tail.as_bytes();
    let len = bytes.len();

    // Hex: 0[xX][0-9A-Fa-f](?:_?[0-9A-Fa-f])*[iI]?
    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'x' {
        let mut i = 2;
        if i >= len || !bytes[i].is_ascii_hexdigit() {
            return None;
        }
        i += 1;
        while i < len {
            if bytes[i].is_ascii_hexdigit() {
                i += 1;
            } else if bytes[i] == b'_' && i + 1 < len && bytes[i + 1].is_ascii_hexdigit() {
                i += 2;
            } else {
                break;
            }
        }
        if i < len && (bytes[i] == b'i' || bytes[i] == b'I') {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Octal: 0[oO][0-7](?:_?[0-7])*[iI]?
    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'o' {
        let mut i = 2;
        if i >= len || !matches!(bytes[i], b'0'..=b'7') {
            return None;
        }
        i += 1;
        while i < len {
            if matches!(bytes[i], b'0'..=b'7') {
                i += 1;
            } else if bytes[i] == b'_' && i + 1 < len && matches!(bytes[i + 1], b'0'..=b'7') {
                i += 2;
            } else {
                break;
            }
        }
        if i < len && (bytes[i] == b'i' || bytes[i] == b'I') {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Binary: 0[bB][01](?:_?[01])*[iI]?
    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'b' {
        let mut i = 2;
        if i >= len || !(bytes[i] == b'0' || bytes[i] == b'1') {
            return None;
        }
        i += 1;
        while i < len {
            if bytes[i] == b'0' || bytes[i] == b'1' {
                i += 1;
            } else if bytes[i] == b'_'
                && i + 1 < len
                && (bytes[i + 1] == b'0' || bytes[i + 1] == b'1')
            {
                i += 2;
            } else {
                break;
            }
        }
        if i < len && (bytes[i] == b'i' || bytes[i] == b'I') {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Decimal / float / imaginary:
    // (?:\d(?:_?\d)*(?:\.\d(?:_?\d)*)?|\.\d(?:_?\d)*)
    // (?:[eE][+-]?\d(?:_?\d)*)?[iI]?
    let mut i = 0;

    // Try \d first
    if i < len && bytes[i].is_ascii_digit() {
        i += 1;
        while i < len {
            if bytes[i].is_ascii_digit() {
                i += 1;
            } else if bytes[i] == b'_' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
                i += 2;
            } else {
                break;
            }
        }

        // Optional fractional part
        if i < len && bytes[i] == b'.' {
            let dot_pos = i;
            i += 1;
            if i < len && bytes[i].is_ascii_digit() {
                i += 1;
                while i < len {
                    if bytes[i].is_ascii_digit() {
                        i += 1;
                    } else if bytes[i] == b'_' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
                        i += 2;
                    } else {
                        break;
                    }
                }
            } else {
                i = dot_pos;
            }
        }
    } else if i < len && bytes[i] == b'.' {
        // Try \.\d
        i += 1;
        if i >= len || !bytes[i].is_ascii_digit() {
            return None;
        }
        i += 1;
        while i < len {
            if bytes[i].is_ascii_digit() {
                i += 1;
            } else if bytes[i] == b'_' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
                i += 2;
            } else {
                break;
            }
        }
    } else {
        return None;
    }

    // Optional exponent
    if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        if i >= len || !bytes[i].is_ascii_digit() {
            return None;
        }
        i += 1;
        while i < len {
            if bytes[i].is_ascii_digit() {
                i += 1;
            } else if bytes[i] == b'_' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
                i += 2;
            } else {
                break;
            }
        }
    }

    // Optional imaginary suffix
    if i < len && (bytes[i] == b'i' || bytes[i] == b'I') {
        i += 1;
    }

    if i < len && is_word_byte(bytes[i]) {
        return None;
    }

    Some(i)
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for kw in &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
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
        "any",
        "bool",
        "byte",
        "comparable",
        "complex64",
        "complex128",
        "error",
        "float32",
        "float64",
        "int",
        "int8",
        "int16",
        "int32",
        "int64",
        "rune",
        "string",
        "uint",
        "uint8",
        "uint16",
        "uint32",
        "uint64",
        "uintptr",
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

fn match_function_call(tail: &str) -> Option<usize> {
    match_function_call_with(tail, is_ascii_ident_start, is_ascii_ident_continue)
}

fn is_ascii_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ascii_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn match_constant(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for word in &["false", "iota", "nil", "true"] {
        if tail.starts_with(word) {
            let after = word.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &["==", "!=", "<=", ">=", ":=", "++", "--"],
        b"+-*/%=&|!<>^~?",
    )
}
