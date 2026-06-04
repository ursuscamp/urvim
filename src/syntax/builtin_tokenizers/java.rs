//! Builtin handwritten scanner for Java syntax.

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

tag_static!(COMMENT_LINE, "comment.line");
tag_static!(COMMENT_DOC, "comment.documentation");
tag_static!(COMMENT_BLOCK, "comment.block");
tag_static!(KW, "keyword");
tag_static!(S, "string");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(FN, "function");
tag_static!(OP, "operator");

const JAVA_DOC_COMMENT: ContextId = ContextId::new("java", "java_doc_comment");
const JAVA_BLOCK_COMMENT: ContextId = ContextId::new("java", "java_block_comment");
const JAVA_TRIPLE_STRING: ContextId = ContextId::new("java", "java_triple_string");
const JAVA_STRING: ContextId = ContextId::new("java", "java_string");
const JAVA_CHAR: ContextId = ContextId::new("java", "java_char");

/// Tokenize one line of Java using the builtin scanner.
pub(crate) fn tokenize_java_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Inside doc comment ───────────────────────────────────────
        if ctx.top_is(JAVA_DOC_COMMENT) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_DOC).clone()));
                ctx.pop_top(JAVA_DOC_COMMENT);
                index = end;
                continue;
            }
            // Doc comments don't nest; /* inside stays doc content
            let run = run_while(tail, |c| c != '*' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*COMMENT_DOC).clone()));
                index += run;
                continue;
            }
            if tail_bytes[0] == b'*' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_DOC).clone()));
                index = end;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside block comment ─────────────────────────────────────
        if ctx.top_is(JAVA_BLOCK_COMMENT) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(JAVA_BLOCK_COMMENT);
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

        // ── Inside triple string ─────────────────────────────────────
        if ctx.top_is(JAVA_TRIPLE_STRING) {
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(JAVA_TRIPLE_STRING);
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

        // ── Inside string ────────────────────────────────────────────
        if ctx.top_is(JAVA_STRING) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(JAVA_STRING);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
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
        if ctx.top_is(JAVA_CHAR) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
                ctx.pop_top(JAVA_CHAR);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
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

        // Line comment //
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'/' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        // Doc comment /** (checked before /*)
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'/'
            && tail_bytes[1] == b'*'
            && tail_bytes[2] == b'*'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_DOC).clone()));
            ctx.push(JAVA_DOC_COMMENT);
            index = end;
            continue;
        }

        // Block comment /*
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(JAVA_BLOCK_COMMENT);
            index = end;
            continue;
        }

        // Triple string open """
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'"'
            && tail_bytes[1] == b'"'
            && tail_bytes[2] == b'"'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(JAVA_TRIPLE_STRING);
            index = end;
            continue;
        }

        // String open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(JAVA_STRING);
            index = end;
            continue;
        }

        // Char open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*CNST).clone()));
            ctx.push(JAVA_CHAR);
            index = end;
            continue;
        }

        // Number
        if let Some(len) = match_java_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // Annotation @... (at line start)
        if let Some(len) = match_annotation(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
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

        // Function call (lookahead \s*\()
        if let Some(len) = match_function_call(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*FN).clone()));
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

fn match_java_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_word_byte(full_bytes[index - 1])) {
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
        if i < len && (bytes[i] == b'l' || bytes[i] == b'L') {
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
        if i >= len || !(bytes[i] == b'0' || bytes[i] == b'1' || bytes[i] == b'_') {
            return None;
        }
        i += 1;
        while i < len && (bytes[i] == b'0' || bytes[i] == b'1' || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && (bytes[i] == b'l' || bytes[i] == b'L') {
            i += 1;
        }
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
        while i < len && (matches!(bytes[i], b'0'..=b'7') || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && (bytes[i] == b'l' || bytes[i] == b'L') {
            i += 1;
        }
        if i < len && is_word_byte(bytes[i]) {
            return None;
        }
        return Some(i);
    }

    // Decimal/float
    let mut i = 0;
    if i < len && bytes[i].is_ascii_digit() {
        i += 1;
        while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
        if i < len && bytes[i] == b'.' {
            let dot = i;
            i += 1;
            if i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                    i += 1;
                }
            } else {
                i = dot;
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

    if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
        i += 1;
        if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        if i >= len || !(bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            return None;
        }
        i += 1;
        while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
    }

    if i < len && matches!(bytes[i], b'f' | b'F' | b'd' | b'D' | b'l' | b'L') {
        i += 1;
    }

    if i < len && is_word_byte(bytes[i]) {
        return None;
    }
    Some(i)
}

fn match_annotation(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let i = match_line_prefixed_identifier_with(
        tail,
        b'@',
        is_ascii_ident_start,
        is_dotted_ident_continue,
    )?;
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
        "abstract",
        "assert",
        "boolean",
        "break",
        "byte",
        "case",
        "catch",
        "char",
        "class",
        "const",
        "continue",
        "default",
        "do",
        "double",
        "else",
        "enum",
        "extends",
        "final",
        "finally",
        "float",
        "for",
        "if",
        "implements",
        "import",
        "instanceof",
        "int",
        "interface",
        "long",
        "native",
        "new",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "short",
        "static",
        "strictfp",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "try",
        "void",
        "volatile",
        "while",
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
        "boolean",
        "byte",
        "char",
        "double",
        "float",
        "int",
        "long",
        "short",
        "String",
        "void",
        "BigDecimal",
        "BigInteger",
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
    for word in &["false", "null", "true"] {
        if tail.starts_with(word) {
            let after = word.len();
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

fn is_dotted_ident_continue(byte: u8) -> bool {
    is_ascii_ident_continue(byte) || byte == b'.'
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
        &["==", "!=", "<=", ">=", "->", "++", "--", "::"],
        b"+-*/%=&|!<>^~?",
    )
}
