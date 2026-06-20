//! Builtin handwritten scanner for TOML syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{is_word_byte, run_while};
use crate::state::{CodeState, ContextId, ContextStack, SyntaxSpan, SyntaxState};
use urvim_theme::Tag;

macro_rules! tag_static {
    ($name:ident, $s:expr) => {
        static $name: LazyLock<Tag> = LazyLock::new(|| Tag::parse($s).unwrap());
    };
}

tag_static!(COMMENT, "comment");
tag_static!(KW, "keyword");
tag_static!(S, "string");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(VAR, "variable");
tag_static!(OP, "operator");

const TOML_TRIPLE_DOUBLE_STRING: ContextId = ContextId::new("toml", "toml_triple_double_string");
const TOML_SINGLE_STRING: ContextId = ContextId::new("toml", "toml_single_string");
const TOML_DOUBLE_STRING: ContextId = ContextId::new("toml", "toml_double_string");

/// Tokenize one line of TOML using the builtin scanner.
pub(crate) fn tokenize_toml_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Inside triple-double string ──────────────────────────────
        if ctx.top_is(TOML_TRIPLE_DOUBLE_STRING) {
            // Rule 4: Closing """
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(TOML_TRIPLE_DOUBLE_STRING);
                index = end;
                continue;
            }
            // Rule 5: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 6: Non-quote content [^"]+
            let run = run_while(tail, |c| c != '"' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Rule 7: Single quote "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                index = end;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside single string ─────────────────────────────────────
        if ctx.top_is(TOML_SINGLE_STRING) {
            // Rule 9: Closing '
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(TOML_SINGLE_STRING);
                index = end;
                continue;
            }
            // Rule 10: Content [^']+
            let run = run_while(tail, |c| c != '\'');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside double string ─────────────────────────────────────
        if ctx.top_is(TOML_DOUBLE_STRING) {
            // Rule 11: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(TOML_DOUBLE_STRING);
                index = end;
                continue;
            }
            // Rule 12: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 13: Content [^"\\]+
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

        // ── Top-level ────────────────────────────────────────────────

        // Rule 1: Comment
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Rules 2-3: Table headers (only at line start)
        if index == 0 {
            if let Some(end) = match_table_header(line, true) {
                spans.push(SyntaxSpan::new(0, end, (*KW).clone()));
                index = end;
                continue;
            }
            if let Some(end) = match_table_header(line, false) {
                spans.push(SyntaxSpan::new(0, end, (*KW).clone()));
                index = end;
                continue;
            }
        }

        // Rule 8: Opening """
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'"'
            && tail_bytes[1] == b'"'
            && tail_bytes[2] == b'"'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(TOML_TRIPLE_DOUBLE_STRING);
            index = end;
            continue;
        }

        // Rule 14: Opening '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(TOML_SINGLE_STRING);
            index = end;
            continue;
        }

        // Rule 15: Opening "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(TOML_DOUBLE_STRING);
            index = end;
            continue;
        }

        // Rules 16-20: Numbers (in order of regex priority)
        if let Some(len) = match_datetime(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }
        if let Some(len) = match_hex(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }
        if let Some(len) = match_octal(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }
        if let Some(len) = match_binary(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }
        if let Some(len) = match_regular_number(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // Rule 21: Constants with word boundaries
        if let Some(len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        // Rule 22: Variable (identifier)
        if let Some(len) = match_identifier(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*VAR).clone()));
            index += len;
            continue;
        }

        // Rule 23: = operator
        if tail_bytes[0] == b'=' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*OP).clone()));
            index = end;
            continue;
        }

        // Rule 24: Punctuation [] , .
        if matches!(tail_bytes[0], b'[' | b']' | b',' | b'.') {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
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

fn match_table_header(line: &str, is_array: bool) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if is_array {
        if i + 1 >= len || bytes[i] != b'[' || bytes[i + 1] != b'[' {
            return None;
        }
        i += 2;
    } else {
        if i >= len || bytes[i] != b'[' {
            return None;
        }
        i += 1;
    }

    let close = if is_array { "]]" } else { "]" };

    if i >= len || !(bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        return None;
    }
    i += 1;
    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-') {
        i += 1;
    }

    loop {
        if i + close.len() > len {
            return None;
        }
        if line[i..].starts_with(close) {
            i += close.len();
            break;
        }
        if bytes[i] == b'.' {
            i += 1;
            if i >= len || !(bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                return None;
            }
            i += 1;
            while i < len
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-')
            {
                i += 1;
            }
        } else {
            return None;
        }
    }

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i == len { Some(len) } else { None }
}

fn match_datetime(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }

    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    let bytes = tail.as_bytes();
    let len = bytes.len();

    // Try full date-time or date-only
    if len >= 10
        && bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(|b| b.is_ascii_digit())
    {
        let mut i = 10;

        // Optional time part T HH:MM:SS
        if i < len && (bytes[i] == b'T' || bytes[i] == b' ') {
            i += 1;
            if i + 8 <= len
                && bytes[i..i + 2].iter().all(|b| b.is_ascii_digit())
                && bytes[i + 2] == b':'
                && bytes[i + 3..i + 5].iter().all(|b| b.is_ascii_digit())
                && bytes[i + 5] == b':'
                && bytes[i + 6..i + 8].iter().all(|b| b.is_ascii_digit())
            {
                i += 8;
                if i < len && bytes[i] == b'.' {
                    i += 1;
                    let mut has_frac = false;
                    while i < len && bytes[i].is_ascii_digit() {
                        i += 1;
                        has_frac = true;
                    }
                    if !has_frac {
                        return None;
                    }
                }
                if i < len && bytes[i] == b'Z' {
                    i += 1;
                } else if i + 5 < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                    i += 1;
                    if bytes[i..i + 2].iter().all(|b| b.is_ascii_digit())
                        && bytes[i + 2] == b':'
                        && bytes[i + 3..i + 5].iter().all(|b| b.is_ascii_digit())
                    {
                        i += 5;
                    } else {
                        return None;
                    }
                }
            } else {
                return None;
            }
        }

        if i == len || !is_word_byte(bytes[i]) {
            return Some(i);
        }
        return None;
    }

    // Try time-only: HH:MM:SS[.frac]
    if len >= 8
        && bytes[0..2].iter().all(|b| b.is_ascii_digit())
        && bytes[2] == b':'
        && bytes[3..5].iter().all(|b| b.is_ascii_digit())
        && bytes[5] == b':'
        && bytes[6..8].iter().all(|b| b.is_ascii_digit())
    {
        let mut i = 8;
        if i < len && bytes[i] == b'.' {
            i += 1;
            let mut has_frac = false;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
                has_frac = true;
            }
            if !has_frac {
                return None;
            }
        }
        if i == len || !is_word_byte(bytes[i]) {
            return Some(i);
        }
    }

    None
}

fn match_hex(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    if i < len && bytes[i] == b'-' {
        i += 1;
    }
    if i + 2 < len && bytes[i] == b'0' && (bytes[i + 1] | 0x20) == b'x' {
        i += 2;
        let start = i;
        while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
            i += 1;
        }
        if i > start { Some(i) } else { None }
    } else {
        None
    }
}

fn match_octal(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    if i < len && bytes[i] == b'-' {
        i += 1;
    }
    if i + 2 < len && bytes[i] == b'0' && (bytes[i + 1] | 0x20) == b'o' {
        i += 2;
        let start = i;
        while i < len && (matches!(bytes[i], b'0'..=b'7') || bytes[i] == b'_') {
            i += 1;
        }
        if i > start { Some(i) } else { None }
    } else {
        None
    }
}

fn match_binary(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    if i < len && bytes[i] == b'-' {
        i += 1;
    }
    if i + 2 < len && bytes[i] == b'0' && (bytes[i + 1] | 0x20) == b'b' {
        i += 2;
        let start = i;
        while i < len && (bytes[i] == b'0' || bytes[i] == b'1' || bytes[i] == b'_') {
            i += 1;
        }
        if i > start { Some(i) } else { None }
    } else {
        None
    }
}

fn match_regular_number(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    if i < len && bytes[i] == b'-' {
        i += 1;
    }

    let start = i;
    // Try \d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d[\d_]*)?
    if i < len && bytes[i].is_ascii_digit() {
        i += 1;
        while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && bytes[i] == b'.' {
            i += 1;
            let mut has_frac = false;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
                has_frac = true;
            }
            if !has_frac {
                return None;
            }
        }
        if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
            i += 1;
            if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                i += 1;
            }
            let mut has_exp = false;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
                has_exp = true;
            }
            if !has_exp {
                return None;
            }
        }
        return Some(i - start + (if bytes[0] == b'-' { 1 } else { 0 }));
    }

    // Try \.\d[\d_]*(?:[eE][+-]?\d[\d_]*)?
    if i < len && bytes[i] == b'.' {
        i += 1;
        if i < len && bytes[i].is_ascii_digit() {
            i += 1;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
            if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
                i += 1;
                if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                    i += 1;
                }
                let mut has_exp = false;
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                    i += 1;
                    has_exp = true;
                }
                if !has_exp {
                    return None;
                }
            }
            return Some(i);
        }
    }

    None
}

fn match_constant(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for word in &["true", "false", "nan", "inf"] {
        if tail.starts_with(word) {
            let after_index = word.len();
            if after_index == tail.len() || !is_word_byte(tail.as_bytes()[after_index]) {
                return Some(after_index);
            }
        }
    }
    None
}

fn match_identifier(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let mut i = 0;

    if i >= bytes.len() {
        return None;
    }
    let first = bytes[i];
    if !first.is_ascii_alphabetic() && first != b'_' {
        return None;
    }
    i += 1;
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-')
    {
        i += 1;
    }
    Some(i)
}
