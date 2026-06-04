//! Builtin handwritten scanner for TypeScript syntax.

use std::sync::LazyLock;

use super::scanner::{
    match_function_call_with, match_line_prefixed_identifier_with, match_operator_from_sets,
    match_prefixed_identifier_with, match_two_byte_escape, match_word_from_list, run_while,
};
use crate::buffer::syntax::{CodeState, ContextId, ContextStack, SyntaxSpan, SyntaxState};
use crate::syntax::builtin_tokenizers::jsx;
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
tag_static!(S_ESCAPE, "string.escape");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(VAR, "variable");
tag_static!(PROP, "variable.property");
tag_static!(FN, "function");
tag_static!(MTAG, "markup.tag");
tag_static!(OP, "operator");

const TS_TEMPLATE_EXPR: ContextId = ContextId::new("typescript", "ts_template_expr");
const TS_TEMPLATE: ContextId = ContextId::new("typescript", "ts_template");
const TS_DOUBLE: ContextId = ContextId::new("typescript", "ts_double");
const TS_SINGLE: ContextId = ContextId::new("typescript", "ts_single");
const TS_COMMENT: ContextId = ContextId::new("typescript", "ts_comment");
const JSX_TAG: ContextId = ContextId::new("typescript", "jsx_tag");

/// Tokenize one line of TypeScript using the builtin scanner.
pub(crate) fn tokenize_typescript_line(
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

        if ctx.top_is(TS_TEMPLATE_EXPR) {
            if tail_bytes[0] == b'}' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(TS_TEMPLATE_EXPR);
                ctx.push(TS_TEMPLATE);
                index = end;
                continue;
            }
        }

        if ctx.top_is(TS_TEMPLATE) {
            if tail_bytes[0] == b'`' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(TS_TEMPLATE);
                index = end;
                continue;
            }
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'$' && tail_bytes[1] == b'{' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(TS_TEMPLATE);
                ctx.push(TS_TEMPLATE_EXPR);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '`' && c != '\\' && c != '$');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            if tail_bytes[0] == b'$' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                index = end;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        if ctx.top_is(TS_DOUBLE) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(TS_DOUBLE);
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

        if ctx.top_is(TS_SINGLE) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(TS_SINGLE);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '\'' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        if ctx.top_is(TS_COMMENT) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop(TS_COMMENT);
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

        if ctx.top_is(JSX_TAG) {
            if tail_bytes[0] == b'>' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(JSX_TAG);
                index = end;
                continue;
            }
            if tail_bytes[0] == b'/' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
        }

        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'/' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(TS_COMMENT);
            index = end;
            continue;
        }

        if tail_bytes[0] == b'`' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(TS_TEMPLATE);
            index = end;
            continue;
        }

        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(TS_DOUBLE);
            index = end;
            continue;
        }

        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(TS_SINGLE);
            index = end;
            continue;
        }

        if let Some(tag_match) = jsx::match_jsx_tag(tail) {
            push_jsx_tag_spans(&mut spans, index, tag_match);
            if tag_match.name.is_some() {
                ctx.push(JSX_TAG);
            }
            index += tag_match.len;
            continue;
        }

        if let Some(rx_len) = match_regex(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + rx_len, (*S).clone()));
            index += rx_len;
            continue;
        }

        if let Some(len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_decorator(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_private_field(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*VAR).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_function_call(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*FN).clone()));
            index += len;
            continue;
        }

        if ctx.top_is(JSX_TAG)
            && let Some(len) = jsx::match_jsx_attribute(tail)
        {
            spans.push(SyntaxSpan::new(index, index + len, (*PROP).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_variable(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*VAR).clone()));
            index += len;
            continue;
        }

        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        if matches!(tail_bytes[0], b'<' | b'>' | b'/') {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

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

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

fn push_jsx_tag_spans(spans: &mut Vec<SyntaxSpan>, index: usize, tag_match: jsx::JsxTagMatch) {
    spans.push(SyntaxSpan::new(index, index + 1, (*P).clone()));
    if tag_match.has_slash {
        spans.push(SyntaxSpan::new(index + 1, index + 2, (*P).clone()));
    }
    if let Some((start, end)) = tag_match.name {
        spans.push(SyntaxSpan::new(index + start, index + end, (*MTAG).clone()));
    }
    if tag_match.name.is_none() && tag_match.len > 1 {
        spans.push(SyntaxSpan::new(
            index + tag_match.len - 1,
            index + tag_match.len,
            (*P).clone(),
        ));
    }
}

fn match_regex(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'/' || !can_start_regex(index, full_bytes) {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && bytes[i] != b'\n' {
        if bytes[i] == b'/' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_lowercase() {
                i += 1;
            }
            return Some(i);
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        i += 1;
    }
    None
}

fn can_start_regex(index: usize, full_bytes: &[u8]) -> bool {
    if index == 0 {
        return true;
    }
    let mut i = index;
    while i > 0 {
        i -= 1;
        let byte = full_bytes[i];
        if byte == b' ' || byte == b'\t' {
            continue;
        }
        return matches!(
            byte,
            b'(' | b'[' | b'{' | b'=' | b':' | b',' | b';' | b'!' | b'?'
        );
    }
    true
}

fn match_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_word_byte(full_bytes[index - 1])) {
        return None;
    }
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut int_end = 0;

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

    let mut i = 0;
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

    if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
        let saved = i;
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
            i = saved;
        }
    }

    if i < len && bytes[i] == b'n' {
        i += 1;
    }

    if i < len && is_word_byte(bytes[i]) {
        if int_end > 0 && int_end < i {
            if int_end >= len || !is_word_byte(bytes[int_end]) {
                return Some(int_end);
            }
        }
        return None;
    }
    Some(i)
}

fn match_decorator(tail: &str) -> Option<usize> {
    match_line_prefixed_identifier_with(tail, b'@', is_ascii_ident_start, is_dotted_ident_continue)
}

fn match_private_field(tail: &str) -> Option<usize> {
    match_prefixed_identifier_with(tail, b'#', is_ascii_ident_start, is_word_byte)
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    match_word_from_list(
        tail,
        &[
            "as",
            "async",
            "await",
            "abstract",
            "break",
            "case",
            "catch",
            "class",
            "const",
            "continue",
            "declare",
            "default",
            "delete",
            "do",
            "else",
            "enum",
            "export",
            "extends",
            "false",
            "finally",
            "for",
            "from",
            "function",
            "if",
            "implements",
            "import",
            "in",
            "infer",
            "interface",
            "is",
            "keyof",
            "let",
            "module",
            "namespace",
            "new",
            "null",
            "of",
            "readonly",
            "return",
            "satisfies",
            "static",
            "super",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "type",
            "typeof",
            "undefined",
            "unknown",
            "var",
            "void",
            "while",
            "with",
            "yield",
            "public",
            "private",
            "protected",
            "override",
            "get",
            "set",
        ],
        index,
        full_bytes,
        is_word_byte,
    )
}

fn match_constant(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    match_word_from_list(
        tail,
        &["true", "false", "null", "undefined"],
        index,
        full_bytes,
        is_word_byte,
    )
}

fn match_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    for typ in &[
        "Array", "Boolean", "Date", "Error", "Map", "Number", "Object", "Promise", "Record", "Set",
        "String", "Symbol", "WeakMap", "WeakSet", "any", "bigint", "boolean", "never", "number",
        "object", "string", "symbol", "unknown", "void",
    ] {
        if tail.starts_with(typ) {
            let after = typ.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    if tail.as_bytes()[0].is_ascii_uppercase() {
        let mut i = 1;
        while i < tail.len() && is_word_byte(tail.as_bytes()[i]) {
            i += 1;
        }
        if i < tail.len() && is_word_byte(tail.as_bytes()[i]) {
            return None;
        }
        return Some(i);
    }
    None
}

fn match_function_call(tail: &str) -> Option<usize> {
    match_function_call_with(tail, is_javascript_ident_start, is_word_byte)
}

fn is_javascript_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$'
}

fn is_ascii_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_dotted_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.'
}

fn match_variable(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_word_byte(full_bytes[index - 1])) {
        return None;
    }
    let bytes = tail.as_bytes();
    if !bytes[0].is_ascii_alphabetic() && bytes[0] != b'_' && bytes[0] != b'$' {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && is_word_byte(bytes[i]) {
        i += 1;
    }
    Some(i)
}

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &[
            "===", "!==", "==", "!=", "<=", ">=", "=>", "??", "&&", "||", "++", "--",
        ],
        b"+-*/%=&|!<>^~?",
    )
}
