//! Builtin handwritten scanner for Nim syntax.

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

tag_static!(COMMENT_DOC, "comment.documentation");
tag_static!(COMMENT_LINE, "comment.line");
tag_static!(COMMENT_BLOCK, "comment.block");
tag_static!(KW, "keyword");
tag_static!(S, "string");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(OP, "operator");

const NIM_BLOCK_COMMENT: ContextId = ContextId::new("nim", "nim_block_comment");
const NIM_TRIPLE_STRING: ContextId = ContextId::new("nim", "nim_triple_string");
const NIM_STRING: ContextId = ContextId::new("nim", "nim_string");

/// Tokenize one line of Nim using the builtin scanner.
pub(crate) fn tokenize_nim_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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
        if ctx.top_is(NIM_BLOCK_COMMENT) {
            // Rule 3: ]# close
            if tail_bytes.len() >= 2 && tail_bytes[0] == b']' && tail_bytes[1] == b'#' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(NIM_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Rule 4: #[ nested open
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'#' && tail_bytes[1] == b'[' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.push(NIM_BLOCK_COMMENT);
                index = end;
                continue;
            }
            // Rule 4 content: consume everything else as comment text.
            let run = run_while(tail, |c| c != '#' && c != ']');
            if run > 0 {
                spans.push(SyntaxSpan::new(
                    index,
                    index + run,
                    (*COMMENT_BLOCK).clone(),
                ));
                index += run;
                continue;
            }
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            index = end;
            continue;
        }

        // ── Inside triple string ─────────────────────────────────────
        if ctx.top_is(NIM_TRIPLE_STRING) {
            // Rule 5: Closing """
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(NIM_TRIPLE_STRING);
                index = end;
                continue;
            }
            // Rule 7: Content [^"\\]+ (allows multi-line, no \n exclusion)
            let run = run_while(tail, |c| c != '"' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Fall through: " by itself opens a regular string; \ is skipped
        }

        // ── Inside regular string ────────────────────────────────────
        if ctx.top_is(NIM_STRING) {
            // Rule 9: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(NIM_STRING);
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

        // ── Top-level ────────────────────────────────────────────────

        // Rule 1: Doc comment ## (checked before #)
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'#' && tail_bytes[1] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_DOC).clone()));
            index = end;
            continue;
        }

        // Rule 4: Block comment #[ (outside comment)
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'#' && tail_bytes[1] == b'[' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(NIM_BLOCK_COMMENT);
            index = end;
            continue;
        }

        // Rule 2: Line comment #
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        // Rule 6: Triple string open """
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'"'
            && tail_bytes[1] == b'"'
            && tail_bytes[2] == b'"'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(NIM_TRIPLE_STRING);
            index = end;
            continue;
        }

        // Rule 8: Raw string r"..."
        if let Some(rs_len) = match_raw_string(tail) {
            spans.push(SyntaxSpan::new(index, index + rs_len, (*S).clone()));
            index += rs_len;
            continue;
        }

        // Rule 10: String open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(NIM_STRING);
            index = end;
            continue;
        }

        // Rule 13: Char literal '...'
        if let Some(ch_len) = match_char_literal(tail) {
            spans.push(SyntaxSpan::new(index, index + ch_len, (*CNST).clone()));
            index += ch_len;
            continue;
        }

        // Rule 14: Keyword with word boundaries
        if let Some(kw_len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + kw_len, (*KW).clone()));
            index += kw_len;
            continue;
        }

        // Rule 15: Type with word boundaries
        if let Some(typ_len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + typ_len, (*TYP).clone()));
            index += typ_len;
            continue;
        }

        // Rule 16: Constant with word boundaries
        if let Some(cnst_len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + cnst_len, (*CNST).clone()));
            index += cnst_len;
            continue;
        }

        // Rule 17: Number with word boundaries
        if let Some(num_len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // Rule 18: Pragma {....}
        if let Some(pragma_len) = match_pragma(tail) {
            spans.push(SyntaxSpan::new(index, index + pragma_len, (*KW).clone()));
            index += pragma_len;
            continue;
        }

        // Rule 19: Capitalized type with word boundaries
        if let Some(typ_len) = match_capitalized_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + typ_len, (*TYP).clone()));
            index += typ_len;
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
    if bytes.len() < 3 || bytes[0] != b'r' || bytes[1] != b'"' {
        return None;
    }
    let mut i = 2;
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
        "addr",
        "and",
        "as",
        "asm",
        "bind",
        "block",
        "break",
        "case",
        "cast",
        "concept",
        "const",
        "continue",
        "converter",
        "defer",
        "discard",
        "distinct",
        "div",
        "do",
        "elif",
        "else",
        "enum",
        "except",
        "export",
        "finally",
        "for",
        "from",
        "func",
        "if",
        "import",
        "in",
        "include",
        "interface",
        "is",
        "isnot",
        "iterator",
        "let",
        "macro",
        "method",
        "mixin",
        "mod",
        "nil",
        "not",
        "object",
        "of",
        "or",
        "out",
        "proc",
        "ptr",
        "raise",
        "ref",
        "return",
        "shl",
        "shr",
        "static",
        "template",
        "try",
        "tuple",
        "type",
        "using",
        "var",
        "when",
        "while",
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

fn match_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for typ in &[
        "bool", "char", "cstring", "float", "float32", "float64", "int", "int8", "int16", "int32",
        "int64", "uint", "uint8", "uint16", "uint32", "uint64", "string", "seq", "array", "set",
        "table", "Option", "Result",
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
    let mut i = 0;

    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'x' {
        i = 2;
        let start = i;
        while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
            i += 1;
        }
        if i == start || (i < len && is_word_byte(bytes[i])) {
            return None;
        }
        return Some(i);
    }

    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'b' {
        i = 2;
        let start = i;
        while i < len && (bytes[i] == b'0' || bytes[i] == b'1' || bytes[i] == b'_') {
            i += 1;
        }
        if i == start || (i < len && is_word_byte(bytes[i])) {
            return None;
        }
        return Some(i);
    }

    if len >= 3 && bytes[0] == b'0' && (bytes[1] | 0x20) == b'o' {
        i = 2;
        let start = i;
        while i < len && (matches!(bytes[i], b'0'..=b'7') || bytes[i] == b'_') {
            i += 1;
        }
        if i == start || (i < len && is_word_byte(bytes[i])) {
            return None;
        }
        return Some(i);
    }

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

fn match_pragma(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    if len < 4 || bytes[0] != b'{' || bytes[1] != b'.' {
        return None;
    }
    let start = 2;
    if start >= len || bytes[start] == b'\n' {
        return None;
    }
    let mut i = start;
    while i < len && bytes[i] != b'\n' {
        if bytes[i] == b'.' && i + 1 < len && bytes[i + 1] == b'}' {
            return Some(i + 2);
        }
        i += 1;
    }
    None
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
    match_operator_from_sets(tail, &["=>", "->", "++", "--"], b"+-*/%=&|!<>^~?")
}
