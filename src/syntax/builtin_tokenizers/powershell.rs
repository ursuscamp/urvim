//! Builtin handwritten scanner for PowerShell syntax.

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
tag_static!(KW, "keyword");
tag_static!(S, "string");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(VAR, "variable");
tag_static!(OP, "operator");

const PS_STRING: ContextId = ContextId::new("powershell", "ps_string");
const PS_SINGLE_STRING: ContextId = ContextId::new("powershell", "ps_single_string");
const PS_INLINE_STRING: ContextId = ContextId::new("powershell", "ps_inline_string");
const PS_INLINE_SINGLE: ContextId = ContextId::new("powershell", "ps_inline_single");
const PS_BLOCK_COMMENT: ContextId = ContextId::new("powershell", "ps_block_comment");

/// Tokenize one line of PowerShell using the builtin scanner.
pub(crate) fn tokenize_powershell_line(
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    let (mut ctx, inj, parent_style) = match state {
        SyntaxState::Code(CodeState::RuleList {
            contexts,
            injection,
            parent_style,
        }) => (contexts, injection, parent_style),
        SyntaxState::Code(CodeState::Normal { contexts }) => (contexts, None, None),
        SyntaxState::Plain => (ContextStack::default(), None, None),
    };

    let mut spans: Vec<SyntaxSpan> = Vec::new();
    let mut index = 0usize;
    let bytes = line.as_bytes();
    let line_len = bytes.len();

    while index < line_len {
        let tail = &line[index..];
        let tail_bytes = &bytes[index..];

        // ── Inside double-quoted here-string ─────────────────────────
        if ctx.top_is(PS_STRING) {
            // Rule 2: "@ close
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'"' && tail_bytes[1] == b'@' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PS_STRING);
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
            // Rule 4: Content [^"\\]+ (no \n exclusion, multi-line)
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

        // ── Inside single-quoted here-string ─────────────────────────
        if ctx.top_is(PS_SINGLE_STRING) {
            // Rule 6: '@ close
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'\'' && tail_bytes[1] == b'@' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PS_SINGLE_STRING);
                index = end;
                continue;
            }
            // Rule 8: Content [^'\\n]+
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

        // ── Inside inline double-quoted string ───────────────────────
        if ctx.top_is(PS_INLINE_STRING) {
            // Rule 9: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PS_INLINE_STRING);
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
            // Rule 11: Content [^"\\\n]+
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

        // ── Inside inline single-quoted string ───────────────────────
        if ctx.top_is(PS_INLINE_SINGLE) {
            // Rule 13: Closing '
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PS_INLINE_SINGLE);
                index = end;
                continue;
            }
            // Rule 15: Content [^'\\n]+
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

        // ── Inside block comment ─────────────────────────────────────
        if ctx.top_is(PS_BLOCK_COMMENT) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'#' && tail_bytes[1] == b'>' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
                ctx.pop(PS_BLOCK_COMMENT);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '#');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*COMMENT).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else {
                break;
            };
            let end = index + ch.len_utf8();
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // ── Top-level ────────────────────────────────────────────────

        // Rule: Block comment open <#
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'<' && tail_bytes[1] == b'#' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            ctx.push(PS_BLOCK_COMMENT);
            index = end;
            continue;
        }

        // Rule 1: Comment
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Rule 3: Double-quoted here-string open @"
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'@' && tail_bytes[1] == b'"' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PS_STRING);
            index = end;
            continue;
        }

        // Rule 7: Single-quoted here-string open @'
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'@' && tail_bytes[1] == b'\'' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PS_SINGLE_STRING);
            index = end;
            continue;
        }

        // Rule 10: Inline double-quoted string open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PS_INLINE_STRING);
            index = end;
            continue;
        }

        // Rule 14: Inline single-quoted string open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PS_INLINE_SINGLE);
            index = end;
            continue;
        }

        // Rule 16: Automatic constant $true / $false / $null
        // (checked before variable; no \b at end per regex)
        if let Some(cnst_len) = match_ps_constant(tail) {
            spans.push(SyntaxSpan::new(index, index + cnst_len, (*CNST).clone()));
            index += cnst_len;
            continue;
        }

        // Rule 17: Variable $...
        if let Some(var_len) = match_variable(tail) {
            spans.push(SyntaxSpan::new(index, index + var_len, (*VAR).clone()));
            index += var_len;
            continue;
        }

        // Rule 18: Keyword with word boundaries
        if let Some(kw_len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + kw_len, (*KW).clone()));
            index += kw_len;
            continue;
        }

        // Rule 19: Number with word boundaries
        if let Some(num_len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // Rule 20: Type annotation [...]
        if let Some(ta_len) = match_type_annotation(tail) {
            spans.push(SyntaxSpan::new(index, index + ta_len, (*KW).clone()));
            index += ta_len;
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
        SyntaxState::Code(CodeState::RuleList {
            contexts: ctx,
            injection: inj,
            parent_style,
        }),
    )
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn match_ps_constant(tail: &str) -> Option<usize> {
    for word in &["$true", "$false", "$null"] {
        if tail.starts_with(word) {
            let after = word.len();
            if after < tail.len() && is_word_byte(tail.as_bytes()[after]) {
                continue;
            }
            return Some(word.len());
        }
    }
    None
}

fn match_variable(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.is_empty() || bytes[0] != b'$' {
        return None;
    }
    if bytes.len() < 2 {
        return None;
    }
    let second = bytes[1];
    if !second.is_ascii_alphabetic() && second != b'_' {
        return None;
    }
    let mut i = 2;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    Some(i)
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for kw in &[
        "class", "enum", "filter", "function", "if", "elseif", "else", "foreach", "for", "while",
        "do", "until", "switch", "return", "param", "begin", "process", "end", "try", "catch",
        "finally", "throw", "break", "continue", "default", "in", "not", "and", "or", "where",
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
            _ => {}
        }
    }

    let mut i = consume_digits_and_underscores(bytes, 0, u8::is_ascii_digit)?;

    if i < len && bytes[i] == b'.' {
        let dot_pos = i;
        if i + 1 < len && bytes[i + 1].is_ascii_digit() {
            i = consume_digits_and_underscores(bytes, i + 1, u8::is_ascii_digit)?;
        } else {
            i = dot_pos;
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

fn match_type_annotation(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    if len < 3 || bytes[0] != b'[' {
        return None;
    }
    let first = bytes[1];
    if !first.is_ascii_alphabetic() && first != b'_' {
        return None;
    }
    let mut i = 2;
    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.') {
        i += 1;
    }
    if i < len && bytes[i] == b']' {
        Some(i + 1)
    } else {
        None
    }
}

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &["===", "==", "!=", "<=", ">=", "++", "--"],
        b"+-*/%=&|!<>^~?",
    )
}
