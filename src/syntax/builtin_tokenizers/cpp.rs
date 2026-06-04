//! Builtin handwritten scanner for C++ syntax.

use std::sync::LazyLock;

use super::scanner::{
    is_word_byte, match_function_call_with, match_operator_from_sets, match_two_byte_escape,
    run_while,
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
tag_static!(NS, "namespace");
tag_static!(OP, "operator");
tag_static!(S_ESCAPE, "string.escape");
tag_static!(S_INTERP, "string.interpolation");

const CPP_COMMENT: ContextId = ContextId::new("cpp", "cpp_comment");
const CPP_RAW_STRING: ContextId = ContextId::new("cpp", "cpp_raw_string");
const CPP_RAW_STRING_BODY: ContextId = ContextId::new("cpp", "cpp_raw_string_body");
const PRINTF_STRING: ContextId = ContextId::new("cpp", "printf_string");
const PRINTF_CALL: ContextId = ContextId::new("cpp", "printf_call");
const PRINTF_CALL_HEAD: ContextId = ContextId::new("cpp", "printf_call_head");
const CPP_STRING: ContextId = ContextId::new("cpp", "cpp_string");
const CPP_CHAR: ContextId = ContextId::new("cpp", "cpp_char");

/// Tokenize one line of C++ using the builtin scanner.
pub(crate) fn tokenize_cpp_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Inside block comment ─────────────────────────────────────
        if ctx.top_is(CPP_COMMENT) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(CPP_COMMENT);
                index = end;
                continue;
            }
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.push(CPP_COMMENT);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '*' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(
                    index,
                    index + run,
                    (*COMMENT_BLOCK).clone(),
                ));
                index += run;
                continue;
            }
            if tail_bytes[0] == b'*' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                index = end;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside raw string body ───────────────────────────────────
        if ctx.top_is(CPP_RAW_STRING_BODY) {
            // Check closing: ) + delimiter + "
            if tail_bytes[0] == b')' {
                let delim = ctx.payload_for(CPP_RAW_STRING).unwrap_or("");
                let expected = format!("){}\"", delim);
                if tail.starts_with(&expected) {
                    let end = index + expected.len();
                    spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                    ctx.pop_top(CPP_RAW_STRING_BODY);
                    ctx.pop_top(CPP_RAW_STRING);
                    index = end;
                    continue;
                }
            }
            // Content [^)]+ (anything but ))
            let run = run_while(tail, |c| c != ')');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Single ) that didn't match closing — emit as string
            if tail_bytes[0] == b')' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                index = end;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside printf format string ──────────────────────────────
        if ctx.top_is(PRINTF_STRING) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(PRINTF_STRING);
                index = end;
                continue;
            }
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'%' && tail_bytes[1] == b'%' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                index = end;
                continue;
            }
            if let Some(fmt_len) = match_format_spec(tail) {
                spans.push(SyntaxSpan::new(index, index + fmt_len, (*S_INTERP).clone()));
                index += fmt_len;
                continue;
            }
            let run = run_while(tail, |c| c != '%' && c != '"' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside printf call ───────────────────────────────────────
        if ctx.top_is(PRINTF_CALL) {
            if tail_bytes[0] == b')' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop_top(PRINTF_CALL);
                index = end;
                continue;
            }
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(PRINTF_CALL);
                ctx.push(PRINTF_STRING);
                index = end;
                continue;
            }
        }

        // ── Inside printf call head ──────────────────────────────────
        if ctx.top_is(PRINTF_CALL_HEAD) {
            if tail_bytes[0] == b'(' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop_top(PRINTF_CALL_HEAD);
                ctx.push(PRINTF_CALL);
                index = end;
                continue;
            }
        }

        // ── Inside regular string ────────────────────────────────────
        if ctx.top_is(CPP_STRING) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(CPP_STRING);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
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

        // ── Inside char literal ──────────────────────────────────────
        if ctx.top_is(CPP_CHAR) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
                ctx.pop_top(CPP_CHAR);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
                index = end;
                continue;
            }
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

        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'/' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(CPP_COMMENT);
            index = end;
            continue;
        }

        // Preprocessor # (at line start via tail)
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*KW).clone()));
            index = end;
            continue;
        }

        // Raw string R"delim("
        if let Some((rs_len, delim)) = match_raw_string_open(tail) {
            spans.push(SyntaxSpan::new(index, index + rs_len, (*S).clone()));
            ctx.push_with_payload(CPP_RAW_STRING, &delim);
            ctx.push(CPP_RAW_STRING_BODY);
            index += rs_len;
            continue;
        }

        // printf-family functions
        if let Some(fn_len) = match_printf_function(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + fn_len, (*FN).clone()));
            ctx.push(PRINTF_CALL_HEAD);
            index += fn_len;
            continue;
        }

        // Namespace (lookahead ::)
        if let Some(ns_len) = match_namespace(tail) {
            spans.push(SyntaxSpan::new(index, index + ns_len, (*NS).clone()));
            index += ns_len;
            continue;
        }

        // Function call (lookahead \s*\()
        if let Some(fn_len) = match_function_call(tail) {
            spans.push(SyntaxSpan::new(index, index + fn_len, (*FN).clone()));
            index += fn_len;
            continue;
        }

        // String open " (push cpp_string)
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(CPP_STRING);
            index = end;
            continue;
        }

        // Char open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
            ctx.push(CPP_CHAR);
            index = end;
            continue;
        }

        // Number
        if let Some(len) = match_cpp_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // Keyword
        if let Some(len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        // Type
        if let Some(len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        // Constant
        if let Some(len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        // Capitalized type
        if let Some(len) = match_capitalized_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        // Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        // Operator
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
        SyntaxState::Code(CodeState::RuleList {
            contexts: ctx,
            injection: inj,
            parent_style,
        }),
    )
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn match_raw_string_open(tail: &str) -> Option<(usize, String)> {
    let bytes = tail.as_bytes();
    // R"delim(
    if bytes.len() < 4 || bytes[0] != b'R' || bytes[1] != b'"' {
        return None;
    }
    let mut i = 2;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    let delim_end = i;
    if i < bytes.len() && bytes[i] == b'(' {
        let delim = tail[2..delim_end].to_string();
        Some((i + 1, delim))
    } else {
        None
    }
}

fn match_format_spec(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'%' {
        return None;
    }
    let len = bytes.len();
    let mut i = 1;
    let pos_start = i;
    while i < len && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > pos_start && i < len && bytes[i] == b'$' {
        i += 1;
    } else {
        i = pos_start;
    }
    while i < len && matches!(bytes[i], b'-' | b'+' | b' ' | b'#' | b'0' | b'\'') {
        i += 1;
    }
    if i < len && bytes[i] == b'*' {
        i += 1;
    } else {
        while i < len && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i < len && bytes[i] == b'.' {
        i += 1;
        if i < len && bytes[i] == b'*' {
            i += 1;
        } else {
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    if i < len {
        let two = if i + 1 < len { &bytes[i..i + 2] } else { b"" };
        if two == b"hh" || two == b"ll" {
            i += 2;
        } else if matches!(bytes[i], b'h' | b'l' | b'j' | b'z' | b't' | b'L') {
            i += 1;
        }
    }
    if i < len
        && matches!(
            bytes[i],
            b'd' | b'i'
                | b'u'
                | b'o'
                | b'x'
                | b'X'
                | b'f'
                | b'F'
                | b'e'
                | b'E'
                | b'g'
                | b'G'
                | b'a'
                | b'A'
                | b'c'
                | b's'
                | b'p'
                | b'n'
        )
    {
        Some(i + 1)
    } else {
        None
    }
}

fn match_printf_function(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    for fname in &[
        "printf",
        "fprintf",
        "sprintf",
        "snprintf",
        "vprintf",
        "vfprintf",
        "vsprintf",
        "vsnprintf",
    ] {
        if tail.starts_with(fname) {
            let after = fname.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_namespace(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let first = bytes[0];
    if !first.is_ascii_alphabetic() && first != b'_' {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    let after = &tail[i..];
    let after_bytes = after.as_bytes();
    let mut j = 0;
    while j < after_bytes.len() && (after_bytes[j] == b' ' || after_bytes[j] == b'\t') {
        j += 1;
    }
    if j + 1 < after_bytes.len() && after_bytes[j] == b':' && after_bytes[j + 1] == b':' {
        return Some(i);
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

fn match_cpp_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_word_byte(full_bytes[index - 1])) {
        return None;
    }
    let bytes = tail.as_bytes();
    let len = bytes.len();

    // Hex
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
        i = skip_int_suffix(bytes, i, len);
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Binary
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
        i = skip_int_suffix(bytes, i, len);
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Octal
    if len >= 2
        && bytes[0] == b'0'
        && matches!(bytes[1], b'0'..=b'7')
        && !(bytes[1] == b'x' || bytes[1] == b'X' || bytes[1] == b'b' || bytes[1] == b'B')
    {
        let mut i = 2;
        while i < len {
            if matches!(bytes[i], b'0'..=b'7') {
                i += 1;
            } else if bytes[i] == b'_' && i + 1 < len && matches!(bytes[i + 1], b'0'..=b'7') {
                i += 2;
            } else {
                break;
            }
        }
        i = skip_int_suffix(bytes, i, len);
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Decimal/float
    let mut i = 0;
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
        if i < len && bytes[i] == b'.' {
            let dot = i;
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
                i = dot;
            }
        }
    } else if i < len && bytes[i] == b'.' {
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

    if i < len {
        match bytes[i] {
            b'f' | b'F' | b'l' | b'L' => {
                i += 1;
                while i < len && (bytes[i] == b'l' || bytes[i] == b'L') {
                    i += 1;
                }
            }
            b'u' | b'U' => {
                i += 1;
                while i < len && (bytes[i] == b'l' || bytes[i] == b'L') {
                    i += 1;
                }
            }
            _ => {}
        }
    }

    if i < len && is_word_byte(bytes[i]) {
        return None;
    }
    Some(i)
}

fn skip_int_suffix(bytes: &[u8], mut i: usize, len: usize) -> usize {
    if i < len {
        match bytes[i] {
            b'u' | b'U' => {
                i += 1;
                while i < len && (bytes[i] == b'l' || bytes[i] == b'L') {
                    i += 1;
                }
            }
            b'l' | b'L' => {
                i += 1;
                let mut count = 1;
                while i < len && (bytes[i] == b'l' || bytes[i] == b'L') && count < 2 {
                    i += 1;
                    count += 1;
                }
                if i < len && (bytes[i] == b'u' || bytes[i] == b'U') {
                    i += 1;
                }
            }
            _ => {}
        }
    }
    i
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    for kw in &[
        "alignas",
        "alignof",
        "and",
        "and_eq",
        "asm",
        "auto",
        "bitand",
        "bitor",
        "bool",
        "break",
        "case",
        "catch",
        "char",
        "class",
        "concept",
        "const",
        "consteval",
        "constexpr",
        "constinit",
        "const_cast",
        "continue",
        "co_await",
        "co_return",
        "co_yield",
        "decltype",
        "default",
        "delete",
        "do",
        "double",
        "dynamic_cast",
        "else",
        "enum",
        "explicit",
        "export",
        "extern",
        "float",
        "for",
        "friend",
        "goto",
        "if",
        "inline",
        "int",
        "long",
        "mutable",
        "namespace",
        "new",
        "noexcept",
        "not",
        "not_eq",
        "nullptr",
        "operator",
        "or",
        "or_eq",
        "private",
        "protected",
        "public",
        "register",
        "reinterpret_cast",
        "return",
        "short",
        "signed",
        "sizeof",
        "static",
        "static_assert",
        "static_cast",
        "struct",
        "switch",
        "template",
        "this",
        "thread_local",
        "throw",
        "try",
        "typedef",
        "typeid",
        "typename",
        "union",
        "unsigned",
        "using",
        "virtual",
        "void",
        "volatile",
        "wchar_t",
        "while",
        "xor",
        "xor_eq",
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
        "bool",
        "char",
        "char16_t",
        "char32_t",
        "double",
        "float",
        "int",
        "long",
        "short",
        "signed",
        "size_t",
        "string",
        "unsigned",
        "void",
        "wchar_t",
        "auto",
        "nullptr_t",
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
    for word in &["true", "false", "nullptr"] {
        if tail.starts_with(word) {
            let after = word.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
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
        &["==", "!=", "<=", ">=", "->", "++", "--", "::", "<<", ">>"],
        b"+-*/%=&|!<>^~?",
    )
}
