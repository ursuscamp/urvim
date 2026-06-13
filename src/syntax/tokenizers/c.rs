//! Builtin handwritten scanner for C syntax.

use std::sync::LazyLock;

use super::scanner::{
    is_word_byte, match_function_call_with, match_operator_from_sets, match_two_byte_escape,
    run_while,
};
use crate::buffer::syntax::{
    CodeState, ContextId, ContextStack, SyntaxLineResult, SyntaxSpan, SyntaxState,
};
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
tag_static!(S_ESCAPE, "string.escape");
tag_static!(S_INTERP, "string.interpolation");

const C_COMMENT: ContextId = ContextId::new("c", "c_comment");
const PRINTF_STRING: ContextId = ContextId::new("c", "printf_string");
const PRINTF_CALL: ContextId = ContextId::new("c", "printf_call");
const PRINTF_CALL_HEAD: ContextId = ContextId::new("c", "printf_call_head");
const C_STRING: ContextId = ContextId::new("c", "c_string");
const C_CHAR: ContextId = ContextId::new("c", "c_char");

/// Tokenize one line of C using the builtin scanner.
pub(crate) fn tokenize_c_line(line: &str, state: SyntaxState) -> SyntaxLineResult {
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
    let mut fold_events = Vec::new();
    let mut index = 0usize;
    let bytes = line.as_bytes();
    let line_len = bytes.len();

    while index < line_len {
        let tail = &line[index..];
        let tail_bytes = &bytes[index..];

        // ── Inside block comment ─────────────────────────────────────
        if ctx.top_is(C_COMMENT) {
            // Rule 3: */ close
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(C_COMMENT);
                index = end;
                continue;
            }
            // Rule 2: /* nested open
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.push(C_COMMENT);
                index = end;
                continue;
            }
            // Rule 4: Content [^*\n]+
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
            // Rule 5: Single *
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

        // ── Inside printf format string ──────────────────────────────
        if ctx.top_is(PRINTF_STRING) {
            // Rule 13: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(PRINTF_STRING);
                index = end;
                continue;
            }
            // Rule 15: %% (literal percent)
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'%' && tail_bytes[1] == b'%' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                index = end;
                continue;
            }
            // Rule 16: \. escape
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                index = end;
                continue;
            }
            // Rule 17: % format specifier
            if let Some(fmt_len) = match_format_spec(tail) {
                spans.push(SyntaxSpan::new(index, index + fmt_len, (*S_INTERP).clone()));
                index += fmt_len;
                continue;
            }
            // Rule 14: Content [^%\"\\\n]+
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

        // ── Inside printf call (argument list) ───────────────────────
        if ctx.top_is(PRINTF_CALL) {
            // Rule 10: ) close
            if tail_bytes[0] == b')' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop_top(PRINTF_CALL);
                index = end;
                continue;
            }
            // Rule 11: " open format string → switches to printf_string
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(PRINTF_CALL);
                ctx.push(PRINTF_STRING);
                index = end;
                continue;
            }
            // Fall through to top-level (numbers, identifiers, etc.)
        }

        // ── Inside printf call head (after printf keyword, before () ─
        if ctx.top_is(PRINTF_CALL_HEAD) {
            // Rule 9: ( → pop head, push call
            if tail_bytes[0] == b'(' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop_top(PRINTF_CALL_HEAD);
                ctx.push(PRINTF_CALL);
                index = end;
                continue;
            }
            // Fall through to top-level
        }

        // ── Inside regular string ────────────────────────────────────
        if ctx.top_is(C_STRING) {
            // Rule 19: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(C_STRING);
                index = end;
                continue;
            }
            // Rule 21: Escape \. (string.escape)
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                index = end;
                continue;
            }
            // Rule 20: Content [^"\\\n]+
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
        if ctx.top_is(C_CHAR) {
            // Rule 23: Closing '
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
                ctx.pop_top(C_CHAR);
                index = end;
                continue;
            }
            // Rule 25: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
                index = end;
                continue;
            }
            // Rule 24: Content [^'\\\n]+
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

        // Rule 2: Block comment /* (outside comment)
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(C_COMMENT);
            index = end;
            continue;
        }

        // Rule 6: Preprocessor # (at line start via tail)
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*KW).clone()));
            index = end;
            continue;
        }

        // Rule 7: printf-family functions (checks before generic
        //          function call so printf gets priority)
        if let Some(fn_len) = match_printf_function(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + fn_len, (*FN).clone()));
            ctx.push(PRINTF_CALL_HEAD);
            index += fn_len;
            continue;
        }

        // Rule 8: Generic function call (lookahead \s*\()
        if let Some(fn_len) = match_function_call(tail) {
            spans.push(SyntaxSpan::new(index, index + fn_len, (*FN).clone()));
            index += fn_len;
            continue;
        }

        // Rule 18: String open " (push c_string)
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(C_STRING);
            index = end;
            continue;
        }

        // Rule 22: Char open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
            ctx.push(C_CHAR);
            index = end;
            continue;
        }

        // Rule 26: Number with word boundaries
        if let Some(len) = match_c_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // Rule 27: Type with word boundaries
        if let Some(len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        // Rule 28: Keyword with word boundaries
        if let Some(len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        // Rule 29: Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            super::bracket_folds::push_delimiter_fold_event(&mut fold_events, tail_bytes[0]);
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        // Rule 30: Operator
        if let Some(len) = match_operator(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*OP).clone()));
            index += len;
            continue;
        }

        // ── No match – skip one char ─────────────────────────────────
        let Some(ch) = tail.chars().next() else { break };
        index += ch.len_utf8();
    }

    SyntaxLineResult {
        spans,
        fold_events,
        state: SyntaxState::Code(CodeState::Scanner {
            contexts: ctx,
            injection: inj,
            parent_style,
            tokenizer_state,
        }),
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn match_format_spec(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'%' {
        return None;
    }
    let len = bytes.len();
    let mut i = 1;

    // (?:\d+\$)? — positional argument
    let pos_start = i;
    while i < len && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > pos_start && i < len && bytes[i] == b'$' {
        i += 1;
    } else {
        i = pos_start;
    }

    // [-+ #0']* — flags
    while i < len && matches!(bytes[i], b'-' | b'+' | b' ' | b'#' | b'0' | b'\'') {
        i += 1;
    }

    // (?:\*|\d+)? — width
    if i < len && bytes[i] == b'*' {
        i += 1;
    } else {
        while i < len && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }

    // (?:\.(?:\*|\d+))? — precision
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

    // (?:hh|h|ll|l|j|z|t|L)? — length modifier
    if i < len {
        let two = if i + 1 < len { &bytes[i..i + 2] } else { b"" };
        if two == b"hh" || two == b"ll" {
            i += 2;
        } else if matches!(bytes[i], b'h' | b'l' | b'j' | b'z' | b't' | b'L') {
            i += 1;
        }
    }

    // [diuoxXfFeEgGaAcspn] — conversion specifier
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

fn match_function_call(tail: &str) -> Option<usize> {
    match_function_call_with(tail, is_ascii_ident_start, is_ascii_ident_continue)
}

fn is_ascii_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ascii_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn match_c_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    let bytes = tail.as_bytes();
    let len = bytes.len();

    // Hex: 0[xX][0-9A-Fa-f](?:_?[0-9A-Fa-f])* suffix?
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
        // optional integer suffix
        i = skip_int_suffix(bytes, i, len);
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Binary: 0[bB][01](?:_?[01])* suffix?
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

    // Octal: 0[0-7](?:_?[0-7])* suffix?  Only matches if at least
    // one octal digit follows 0; otherwise 0 alone falls through to
    // the decimal path.
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

    // Decimal/float: \d... or .\d...
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

    // Optional float suffix [fFlLuU] or integer suffix
    if i < len {
        match bytes[i] {
            b'f' | b'F' | b'l' | b'L' => {
                i += 1;
                // [lL]{0,2}
                while i < len && (bytes[i] == b'l' || bytes[i] == b'L') && i < len {
                    i += 1;
                }
            }
            b'u' | b'U' => {
                i += 1;
                // [lL]{0,2}
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
                // [lL]{0,2} max 2
                let mut count = 1;
                while i < len && (bytes[i] == b'l' || bytes[i] == b'L') && count < 2 {
                    i += 1;
                    count += 1;
                }
                // optional u/U
                if i < len && (bytes[i] == b'u' || bytes[i] == b'U') {
                    i += 1;
                }
            }
            _ => {}
        }
    }
    i
}

fn match_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for typ in &[
        "_Bool", "bool", "char", "double", "float", "int", "long", "short", "signed", "size_t",
        "unsigned", "void", "wchar_t",
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

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for kw in &[
        "auto", "break", "case", "continue", "default", "do", "else", "enum", "for", "goto", "if",
        "return", "sizeof", "static", "struct", "switch", "typedef", "union", "volatile", "while",
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

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &["==", "!=", "<=", ">=", "->", "++", "--"],
        b"+-*/%=&|!<>^~?",
    )
}
