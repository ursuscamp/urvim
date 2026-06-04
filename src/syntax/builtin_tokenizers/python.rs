//! Builtin handwritten scanner for Python syntax.

use std::sync::LazyLock;

use super::scanner::{
    is_word_byte, match_function_call_with, match_line_prefixed_identifier_with,
    match_operator_from_sets, match_two_byte_escape, run_while,
};
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
tag_static!(TYP, "type");
tag_static!(VAR, "variable");
tag_static!(FN, "function");
tag_static!(OP, "operator");

const PY_FEXPR_DOUBLE: ContextId = ContextId::new("python", "py_fexpr_double");
const PY_FEXPR_TRIPLE_DOUBLE: ContextId = ContextId::new("python", "py_fexpr_triple_double");
const PY_RAW_FEXPR_DOUBLE: ContextId = ContextId::new("python", "py_raw_fexpr_double");
const PY_RAW_FEXPR_TRIPLE_DOUBLE: ContextId =
    ContextId::new("python", "py_raw_fexpr_triple_double");
const PY_FTRIPLE_DOUBLE: ContextId = ContextId::new("python", "py_ftriple_double");
const PY_RAW_FTRIPLE_DOUBLE: ContextId = ContextId::new("python", "py_raw_ftriple_double");
const PY_TRIPLE_DOUBLE: ContextId = ContextId::new("python", "py_triple_double");
const PY_RAW_TRIPLE_DOUBLE: ContextId = ContextId::new("python", "py_raw_triple_double");
const PY_FDOUBLE: ContextId = ContextId::new("python", "py_fdouble");
const PY_RAW_FDOUBLE: ContextId = ContextId::new("python", "py_raw_fdouble");
const PY_DOUBLE: ContextId = ContextId::new("python", "py_double");
const PY_RAW_DOUBLE: ContextId = ContextId::new("python", "py_raw_double");

/// Tokenize one line of Python using the builtin scanner.
pub(crate) fn tokenize_python_line(
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

        // ── F-expr contexts (fall through to top-level for content) ──
        if ctx.top_is(PY_FEXPR_DOUBLE) {
            if tail_bytes[0] == b'}' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(PY_FEXPR_DOUBLE);
                ctx.push(PY_FDOUBLE);
                index = end;
                continue;
            }
        }
        if ctx.top_is(PY_FEXPR_TRIPLE_DOUBLE) {
            if tail_bytes[0] == b'}' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(PY_FEXPR_TRIPLE_DOUBLE);
                ctx.push(PY_FTRIPLE_DOUBLE);
                index = end;
                continue;
            }
        }
        if ctx.top_is(PY_RAW_FEXPR_DOUBLE) {
            if tail_bytes[0] == b'}' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(PY_RAW_FEXPR_DOUBLE);
                ctx.push(PY_RAW_FDOUBLE);
                index = end;
                continue;
            }
        }
        if ctx.top_is(PY_RAW_FEXPR_TRIPLE_DOUBLE) {
            if tail_bytes[0] == b'}' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(PY_RAW_FEXPR_TRIPLE_DOUBLE);
                ctx.push(PY_RAW_FTRIPLE_DOUBLE);
                index = end;
                continue;
            }
        }

        // ── Inside triple double f-string ────────────────────────────
        if ctx.top_is(PY_FTRIPLE_DOUBLE) {
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PY_FTRIPLE_DOUBLE);
                index = end;
                continue;
            }
            if tail_bytes[0] == b'{' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(PY_FTRIPLE_DOUBLE);
                ctx.push(PY_FEXPR_TRIPLE_DOUBLE);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '{');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
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

        // ── Inside triple double raw f-string ────────────────────────
        if ctx.top_is(PY_RAW_FTRIPLE_DOUBLE) {
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PY_RAW_FTRIPLE_DOUBLE);
                index = end;
                continue;
            }
            if tail_bytes[0] == b'{' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(PY_RAW_FTRIPLE_DOUBLE);
                ctx.push(PY_RAW_FEXPR_TRIPLE_DOUBLE);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '{');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
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

        // ── Inside triple double string ──────────────────────────────
        if ctx.top_is(PY_TRIPLE_DOUBLE) {
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PY_TRIPLE_DOUBLE);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
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

        // ── Inside raw triple double string ──────────────────────────
        if ctx.top_is(PY_RAW_TRIPLE_DOUBLE) {
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PY_RAW_TRIPLE_DOUBLE);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
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

        // ── Inside double f-string ───────────────────────────────────
        // Note: No content rule — string body falls through to top-level
        // highlighting like the regex engine.
        if ctx.top_is(PY_FDOUBLE) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PY_FDOUBLE);
                index = end;
                continue;
            }
            if tail_bytes[0] == b'{' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(PY_FDOUBLE);
                ctx.push(PY_FEXPR_DOUBLE);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
        }

        // ── Inside raw double f-string ───────────────────────────────
        // No content rule — falls through to top-level highlighting.
        if ctx.top_is(PY_RAW_FDOUBLE) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PY_RAW_FDOUBLE);
                index = end;
                continue;
            }
            if tail_bytes[0] == b'{' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(PY_RAW_FDOUBLE);
                ctx.push(PY_RAW_FEXPR_DOUBLE);
                index = end;
                continue;
            }
        }

        // ── Inside double string (regular / bytes / unicode) ─────────
        if ctx.top_is(PY_DOUBLE) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PY_DOUBLE);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
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

        // ── Inside raw double string ─────────────────────────────────
        if ctx.top_is(PY_RAW_DOUBLE) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(PY_RAW_DOUBLE);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"');
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

        // Line comment
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Decorator @...
        if let Some(len) = match_decorator(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        // String open with prefix — match in TOML rule order
        let prefix_checks: &[(&str, usize, ContextId)] = &[
            ("fr\"\"\"", 5, PY_RAW_TRIPLE_DOUBLE),
            ("rf\"\"\"", 5, PY_RAW_TRIPLE_DOUBLE),
            ("f\"\"\"", 4, PY_FTRIPLE_DOUBLE),
            ("rb\"\"\"", 5, PY_RAW_TRIPLE_DOUBLE),
            ("br\"\"\"", 5, PY_RAW_TRIPLE_DOUBLE),
            ("r\"\"\"", 4, PY_RAW_TRIPLE_DOUBLE),
            ("b\"\"\"", 4, PY_TRIPLE_DOUBLE),
            ("u\"\"\"", 4, PY_TRIPLE_DOUBLE),
            ("\"\"\"", 3, PY_TRIPLE_DOUBLE),
            ("fr\"", 3, PY_RAW_DOUBLE),
            ("rf\"", 3, PY_RAW_DOUBLE),
            ("f\"", 2, PY_FDOUBLE),
            ("rb\"", 3, PY_RAW_DOUBLE),
            ("br\"", 3, PY_RAW_DOUBLE),
            ("r\"", 2, PY_RAW_DOUBLE),
            ("b\"", 2, PY_DOUBLE),
            ("u\"", 2, PY_DOUBLE),
        ];
        let mut matched = false;
        for (prefix, open_len, ctx_name) in prefix_checks {
            if tail.starts_with(prefix) {
                let end = index + open_len;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.push(*ctx_name);
                index = end;
                matched = true;
                break;
            }
        }
        if matched {
            continue;
        }
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PY_DOUBLE);
            index = end;
            continue;
        }

        // Function call (lookahead)
        if let Some(len) = match_function_call(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*FN).clone()));
            index += len;
            continue;
        }

        // Number
        if let Some(len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // Type annotation #...
        if let Some(len) = match_type_comment(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*VAR).clone()));
            index += len;
            continue;
        }

        // Keyword
        if let Some(len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        // Constant
        if let Some(len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        // Type
        if let Some(len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        // Variable
        if let Some(len) = match_variable_ident(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*VAR).clone()));
            index += len;
            continue;
        }

        // Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b':'
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

fn is_python_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

fn match_decorator(tail: &str) -> Option<usize> {
    match_line_prefixed_identifier_with(tail, b'@', is_ascii_ident_start, is_dotted_ident_continue)
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
    if tail.is_empty() || (index > 0 && is_python_word_byte(full_bytes[index - 1])) {
        return None;
    }
    let bytes = tail.as_bytes();
    let len = bytes.len();

    // Hex
    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'x' {
        let mut i = 2;
        if i >= len || !(bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
            return None;
        }
        i += 1;
        while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && bytes[i] == b'n' {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Octal
    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'o' {
        let mut i = 2;
        if i >= len || !(matches!(bytes[i], b'0'..=b'7') || bytes[i] == b'_') {
            return None;
        }
        i += 1;
        while i < len && (matches!(bytes[i], b'0'..=b'7') || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && bytes[i] == b'n' {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Binary
    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'b' {
        let mut i = 2;
        if i >= len || !((bytes[i] == b'0' || bytes[i] == b'1') || bytes[i] == b'_') {
            return None;
        }
        i += 1;
        while i < len && ((bytes[i] == b'0' || bytes[i] == b'1') || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && bytes[i] == b'n' {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Decimal/float
    let mut i = 0;
    // Position after integer part (before any optional fractional part)
    // Used for backtracking when fractional part fails word boundary.
    let mut int_end = 0;

    if i < len && bytes[i].is_ascii_digit() {
        i += 1;
        while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
        int_end = i;
        if i < len && bytes[i] == b'.' {
            i += 1;
            if i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                    i += 1;
                }
            } else {
                i = int_end;
            }
        }
    } else if i < len && bytes[i] == b'.' {
        i += 1;
        if i >= len || !(bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            return None;
        }
        i += 1;
        while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
    } else {
        return None;
    }

    // Try exponent
    if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
        let saved_i = i;
        i += 1;
        if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        if i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
        } else {
            i = saved_i;
        }
    }

    if i < len && bytes[i] == b'n' {
        i += 1;
    }

    // Check word boundary; backtrack to int part if fractional fails
    if i < len && is_word_byte(bytes[i]) {
        if int_end > 0 && int_end < i {
            // Retry with just the integer part (no fractional/exponent)
            if int_end >= len || !is_word_byte(bytes[int_end]) {
                return Some(int_end);
            }
        }
        return None;
    }
    Some(i)
}

fn match_type_comment(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'#' || !(bytes[1].is_ascii_alphabetic() || bytes[1] == b'_')
    {
        return None;
    }
    let mut i = 2;
    while i < bytes.len() && is_word_byte(bytes[i]) {
        i += 1;
    }
    Some(i)
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    for kw in &[
        "as", "async", "await", "break", "case", "class", "continue", "def", "del", "elif", "else",
        "except", "finally", "for", "from", "if", "import", "in", "is", "lambda", "match",
        "nonlocal", "pass", "raise", "return", "try", "type", "while", "with", "yield",
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
    for word in &["False", "None", "True"] {
        if tail.starts_with(word) {
            let after = word.len();
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
    // Check named types first
    for typ in &[
        "Exception",
        "TypeError",
        "ValueError",
        "int",
        "float",
        "str",
        "list",
        "dict",
        "set",
        "tuple",
        "object",
    ] {
        if tail.starts_with(typ) {
            let after = typ.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    // Capitalized identifier as type
    if tail.as_bytes()[0].is_ascii_uppercase() {
        let mut i = 1;
        while i < tail.len()
            && (tail.as_bytes()[i].is_ascii_alphanumeric() || tail.as_bytes()[i] == b'_')
        {
            i += 1;
        }
        if i < tail.len() && is_word_byte(tail.as_bytes()[i]) {
            return None;
        }
        // Only match if it's uppercase (the first char check ensures)
        return Some(i);
    }
    None
}

fn match_variable_ident(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_python_word_byte(full_bytes[index - 1])) {
        return None;
    }
    let bytes = tail.as_bytes();
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' && bytes[0] != b'$' {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && is_python_word_byte(bytes[i]) {
        i += 1;
    }
    Some(i)
}

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &["==", "!=", "<=", ">=", ":=", "->", "++", "--"],
        b"+-*/%=&|^~<>",
    )
}
