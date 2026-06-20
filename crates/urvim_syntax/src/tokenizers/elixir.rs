//! Builtin handwritten scanner for Elixir syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{
    is_word_byte, match_operator_from_sets, match_prefixed_identifier_with, run_while,
};
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
tag_static!(TYP, "type");
tag_static!(OP, "operator");

const ELIXIR_TRIPLE_STRING: ContextId = ContextId::new("elixir", "elixir_triple_string");
const ELIXIR_SINGLE_TRIPLE: ContextId = ContextId::new("elixir", "elixir_single_triple");
const ELIXIR_STRING: ContextId = ContextId::new("elixir", "elixir_string");
const ELIXIR_SINGLE_STRING: ContextId = ContextId::new("elixir", "elixir_single_string");

/// Tokenize one line of Elixir using the builtin scanner.
pub(crate) fn tokenize_elixir_line(
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

        // ── Inside triple double string ──────────────────────────────
        if ctx.top_is(ELIXIR_TRIPLE_STRING) {
            // Rule 2: Closing """
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(ELIXIR_TRIPLE_STRING);
                index = end;
                continue;
            }
            // Rule 4: Content [^"\\]+ (no \n exclusion)
            let run = run_while(tail, |c| c != '"' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Fall through: lone " opens a regular string
        }

        // ── Inside triple single string ──────────────────────────────
        if ctx.top_is(ELIXIR_SINGLE_TRIPLE) {
            // Rule 5: Closing '''
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'\''
                && tail_bytes[1] == b'\''
                && tail_bytes[2] == b'\''
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(ELIXIR_SINGLE_TRIPLE);
                index = end;
                continue;
            }
            // Rule 7: Content [^'\\]+ (no \n exclusion)
            let run = run_while(tail, |c| c != '\'' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Fall through: lone ' opens a regular string
        }

        // ── Inside double string ─────────────────────────────────────
        if ctx.top_is(ELIXIR_STRING) {
            // Rule 8: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(ELIXIR_STRING);
                index = end;
                continue;
            }
            // Rule 11: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 10: Content [^"\\\n]+
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

        // ── Inside single string ─────────────────────────────────────
        if ctx.top_is(ELIXIR_SINGLE_STRING) {
            // Rule 12: Closing '
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(ELIXIR_SINGLE_STRING);
                index = end;
                continue;
            }
            // Rule 14: Content [^'\n]+
            let run = run_while(tail, |c| c != '\'' && c != '\n');
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

        // Rule 3: Triple double string open """
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'"'
            && tail_bytes[1] == b'"'
            && tail_bytes[2] == b'"'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(ELIXIR_TRIPLE_STRING);
            index = end;
            continue;
        }

        // Rule 6: Triple single string open '''
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'\''
            && tail_bytes[1] == b'\''
            && tail_bytes[2] == b'\''
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(ELIXIR_SINGLE_TRIPLE);
            index = end;
            continue;
        }

        // Rule 9: Double string open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(ELIXIR_STRING);
            index = end;
            continue;
        }

        // Rule 13: Single string open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(ELIXIR_SINGLE_STRING);
            index = end;
            continue;
        }

        // Rule 15: Atom :...
        if let Some(atom_len) = match_atom(tail) {
            spans.push(SyntaxSpan::new(index, index + atom_len, (*CNST).clone()));
            index += atom_len;
            continue;
        }

        // Rule 16: Module attribute @...
        if let Some(attr_len) = match_attribute(tail) {
            spans.push(SyntaxSpan::new(index, index + attr_len, (*KW).clone()));
            index += attr_len;
            continue;
        }

        // Rule 17: Keyword with word boundaries
        if let Some(kw_len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + kw_len, (*KW).clone()));
            index += kw_len;
            continue;
        }

        // Rule 18: Constant with word boundaries
        if let Some(cnst_len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + cnst_len, (*CNST).clone()));
            index += cnst_len;
            continue;
        }

        // Rule 19: Number with word boundaries
        if let Some(num_len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // Rule 20: Capitalized type with word boundaries
        if let Some(typ_len) = match_capitalized_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + typ_len, (*TYP).clone()));
            index += typ_len;
            continue;
        }

        // Rule 21: Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        // Rule 22: Operator
        if let Some(op_len) = match_operator(tail) {
            spans.push(SyntaxSpan::new(index, index + op_len, (*OP).clone()));
            index += op_len;
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

fn is_extended_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'!' || b == b'?' || b == b'='
}

fn match_atom(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 2 || bytes[0] != b':' {
        return None;
    }
    let second = bytes[1];
    if !second.is_ascii_alphabetic() && second != b'_' {
        return None;
    }
    let mut i = 2;
    while i < bytes.len() && is_extended_word_byte(bytes[i]) {
        i += 1;
    }
    Some(i)
}

fn match_attribute(tail: &str) -> Option<usize> {
    match_prefixed_identifier_with(tail, b'@', is_ascii_ident_start, is_extended_word_byte)
}

fn is_ascii_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for kw in &[
        "def",
        "defp",
        "defmacro",
        "defmodule",
        "do",
        "end",
        "fn",
        "if",
        "unless",
        "case",
        "cond",
        "with",
        "receive",
        "after",
        "try",
        "catch",
        "rescue",
        "raise",
        "quote",
        "unquote",
        "import",
        "alias",
        "require",
        "use",
        "when",
        "for",
        "in",
        "and",
        "or",
        "not",
        "xor",
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

fn match_constant(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for word in &["false", "nil", "true"] {
        if tail.starts_with(word) {
            let after = word.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }

    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    let bytes = tail.as_bytes();
    let len = bytes.len();

    // 0x...
    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'x' {
        let mut i = 2;
        if i >= len || !(bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
            return None;
        }
        i += 1;
        while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // 0o...
    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'o' {
        let mut i = 2;
        if i >= len || !(matches!(bytes[i], b'0'..=b'7') || bytes[i] == b'_') {
            return None;
        }
        i += 1;
        while i < len && (matches!(bytes[i], b'0'..=b'7') || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // 0b...
    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'b' {
        let mut i = 2;
        if i >= len || !((bytes[i] == b'0' || bytes[i] == b'1') || bytes[i] == b'_') {
            return None;
        }
        i += 1;
        while i < len && ((bytes[i] == b'0' || bytes[i] == b'1') || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Decimal: \d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d[\d_]*)?
    if !bytes[0].is_ascii_digit() {
        return None;
    }
    let mut i = 1;
    while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
        i += 1;
    }

    if i < len && bytes[i] == b'.' {
        let dot_pos = i;
        i += 1;
        if i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
        } else {
            i = dot_pos;
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

    if i < len && is_word_byte(bytes[i]) {
        return None;
    }

    Some(i)
}

fn match_capitalized_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }

    if index > 0 && is_word_byte(full_bytes[index - 1]) {
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
        &["===", "==", "!=", "<=", ">=", "->", "<-", "|>", "++", "--"],
        b"+-*/%=&|!<>^~?",
    )
}
