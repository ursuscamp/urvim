//! Builtin handwritten scanner for Zig syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{
    is_word_byte, match_function_call_with, match_operator_from_sets, match_word_from_list,
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
tag_static!(FN_MACRO, "function.macro");
tag_static!(FN, "function");
tag_static!(OP, "operator");

const ZIG_BLOCK_COMMENT: ContextId = ContextId::new("zig", "zig_block_comment");
const ZIG_STRING: ContextId = ContextId::new("zig", "zig_string");

/// Tokenize one line of Zig using the builtin scanner.
pub(crate) fn tokenize_zig_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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
        if ctx.top_is(ZIG_BLOCK_COMMENT) {
            // Rule 2: */ close
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(ZIG_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Rule 3: /* nested open
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.push(ZIG_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Fall through to top-level (no content rule)
        }

        // ── Inside string ────────────────────────────────────────────
        if ctx.top_is(ZIG_STRING) {
            // Rule 4: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(ZIG_STRING);
                index = end;
                continue;
            }
            // Rule 7: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 6: Content [^"\\\n]+
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
            ctx.push(ZIG_BLOCK_COMMENT);
            index = end;
            continue;
        }

        // Rule 5: String open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(ZIG_STRING);
            index = end;
            continue;
        }

        // Rule 8: Char literal '...'
        if let Some(ch_len) = match_char_literal(tail) {
            spans.push(SyntaxSpan::new(index, index + ch_len, (*CNST).clone()));
            index += ch_len;
            continue;
        }

        // Rule 9: Keyword with word boundaries
        if let Some(kw_len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + kw_len, (*KW).clone()));
            index += kw_len;
            continue;
        }

        // Rule 10: Type with word boundaries
        if let Some(typ_len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + typ_len, (*TYP).clone()));
            index += typ_len;
            continue;
        }

        // Rule 11: Constant with word boundaries
        if let Some(cnst_len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + cnst_len, (*CNST).clone()));
            index += cnst_len;
            continue;
        }

        // Rule 12: Number with word boundaries
        if let Some(num_len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // Rule 13: Builtin macro @...
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

        // Rule 14: Function call (identifier + lookahead \s*\()
        if let Some(fn_len) = match_function_call(tail) {
            spans.push(SyntaxSpan::new(index, index + fn_len, (*FN).clone()));
            index += fn_len;
            continue;
        }

        // Rule 15: Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        // Rule 16: Operator
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
    match_word_from_list(
        tail,
        &[
            "align",
            "allowzero",
            "and",
            "asm",
            "async",
            "await",
            "break",
            "callconv",
            "catch",
            "comptime",
            "const",
            "continue",
            "defer",
            "else",
            "enum",
            "errdefer",
            "export",
            "extern",
            "fn",
            "for",
            "if",
            "inline",
            "noinline",
            "or",
            "orelse",
            "packed",
            "pub",
            "return",
            "switch",
            "test",
            "threadlocal",
            "try",
            "union",
            "unreachable",
            "usingnamespace",
            "var",
            "volatile",
            "while",
        ],
        index,
        full_bytes,
        is_word_byte,
    )
}

fn match_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    match_word_from_list(
        tail,
        &[
            "bool",
            "anyerror",
            "anyopaque",
            "comptime_float",
            "comptime_int",
            "f16",
            "f32",
            "f64",
            "i8",
            "i16",
            "i32",
            "i64",
            "i128",
            "isize",
            "u8",
            "u16",
            "u32",
            "u64",
            "u128",
            "usize",
            "void",
            "type",
            "noreturn",
        ],
        index,
        full_bytes,
        is_word_byte,
    )
}

fn match_constant(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    match_word_from_list(
        tail,
        &["false", "null", "true", "undefined"],
        index,
        full_bytes,
        is_word_byte,
    )
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
    let mut i = 0;

    if i >= len || !bytes[i].is_ascii_digit() {
        return None;
    }
    i += 1;
    while i < len && bytes[i].is_ascii_digit() {
        i += 1;
    }

    if i < len && bytes[i] == b'.' {
        let dot_pos = i;
        i += 1;
        if i < len && bytes[i].is_ascii_digit() {
            i += 1;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
        } else {
            i = dot_pos;
        }
    }

    if i < len && is_word_byte(bytes[i]) {
        return None;
    }

    Some(i)
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

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &["==", "!=", "<=", ">=", "++", "--"],
        b"+-*/%=&|!<>^~?:",
    )
}
