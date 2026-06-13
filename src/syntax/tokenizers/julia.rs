//! Builtin handwritten scanner for Julia syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{is_word_byte, match_operator_from_sets, run_while};
use crate::buffer::syntax::{CodeState, ContextId, ContextStack, SyntaxSpan, SyntaxState};
use crate::theme::Tag;

macro_rules! tag_static {
    ($name:ident, $s:expr) => {
        static $name: LazyLock<Tag> = LazyLock::new(|| Tag::parse($s).unwrap());
    };
}

tag_static!(COMMENT, "comment");
tag_static!(COMMENT_BLOCK, "comment.block");
tag_static!(KW, "keyword");
tag_static!(S, "string");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(FN_MACRO, "function.macro");
tag_static!(OP, "operator");

const JULIA_BLOCK_COMMENT: ContextId = ContextId::new("julia", "julia_block_comment");
const JULIA_TRIPLE_STRING: ContextId = ContextId::new("julia", "julia_triple_string");
const JULIA_STRING: ContextId = ContextId::new("julia", "julia_string");

/// Tokenize one line of Julia using the builtin scanner.
pub(crate) fn tokenize_julia_line(
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

        // ── Inside block comment ─────────────────────────────────────
        if ctx.top_is(JULIA_BLOCK_COMMENT) {
            // Rule 2: =# close
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'=' && tail_bytes[1] == b'#' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(JULIA_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Rule 3: #= nested open
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'#' && tail_bytes[1] == b'=' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.push(JULIA_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Fall through to top-level (no content rule)
        }

        // ── Inside triple string ─────────────────────────────────────
        if ctx.top_is(JULIA_TRIPLE_STRING) {
            // Rule 4: Closing """
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(JULIA_TRIPLE_STRING);
                index = end;
                continue;
            }
            // Rule 6: Content [^"\\]+ (allows multi-line)
            let run = run_while(tail, |c| c != '"' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Fall through: " by itself opens a regular string; \ is skipped
        }

        // ── Inside regular string ────────────────────────────────────
        if ctx.top_is(JULIA_STRING) {
            // Rule 8: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(JULIA_STRING);
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

        // ── Top-level ────────────────────────────────────────────────

        // Rule 1: Line comment #
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Rule 3: Block comment #= (after # check per TOML order)
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'#' && tail_bytes[1] == b'=' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(JULIA_BLOCK_COMMENT);
            index = end;
            continue;
        }

        // Rule 5: Triple string open """
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'"'
            && tail_bytes[1] == b'"'
            && tail_bytes[2] == b'"'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(JULIA_TRIPLE_STRING);
            index = end;
            continue;
        }

        // Rule 7: Raw string raw"..."
        if let Some(rs_len) = match_raw_string(tail) {
            spans.push(SyntaxSpan::new(index, index + rs_len, (*S).clone()));
            index += rs_len;
            continue;
        }

        // Rule 9: String open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(JULIA_STRING);
            index = end;
            continue;
        }

        // Rule 12: Char literal '...'
        if let Some(ch_len) = match_char_literal(tail) {
            spans.push(SyntaxSpan::new(index, index + ch_len, (*CNST).clone()));
            index += ch_len;
            continue;
        }

        // Rule 13: Keyword with word boundaries
        if let Some(kw_len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + kw_len, (*KW).clone()));
            index += kw_len;
            continue;
        }

        // Rule 14: Macro @...
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'@' {
            let second = tail_bytes[1];
            if second.is_ascii_alphabetic() || second == b'_' {
                let mut i = 2;
                while i < tail_bytes.len()
                    && (tail_bytes[i].is_ascii_alphanumeric() || tail_bytes[i] == b'_')
                {
                    i += 1;
                }
                spans.push(SyntaxSpan::new(index, index + i, (*FN_MACRO).clone()));
                index += i;
                continue;
            }
        }

        // Rule 15: Constant with word boundaries
        if let Some(cnst_len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + cnst_len, (*CNST).clone()));
            index += cnst_len;
            continue;
        }

        // Rule 16: Number with word boundaries
        if let Some(num_len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // Rule 17: Capitalized type with word boundaries
        if let Some(typ_len) = match_capitalized_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + typ_len, (*TYP).clone()));
            index += typ_len;
            continue;
        }

        // Rule 18: Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        // Rule 19: Operator
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

fn match_raw_string(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 5 || !tail.starts_with("raw\"") {
        return None;
    }
    let mut i = 4;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            return Some(i + 1);
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'\n' {
            return None;
        }
        i += 1;
    }
    None
}

fn match_char_literal(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'\'' {
        return None;
    }
    if bytes[1] == b'\\' {
        if bytes.len() >= 4 && bytes[3] == b'\'' {
            return Some(4);
        }
        None
    } else if bytes[1] != b'\'' && bytes[1] != b'\n' {
        if bytes[2] == b'\'' {
            return Some(3);
        }
        None
    } else {
        None
    }
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for kw in &[
        "begin", "break", "catch", "const", "continue", "do", "else", "elseif", "end", "export",
        "false", "finally", "for", "function", "global", "if", "import", "let", "local", "macro",
        "module", "quote", "return", "struct", "true", "try", "typeof", "using", "where", "while",
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

    for word in &["false", "missing", "nothing", "true"] {
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
        &["===", "==", "!=", "<=", ">=", "=>", "::", "++", "--"],
        b"+-*/%=&|!<>^~?",
    )
}
