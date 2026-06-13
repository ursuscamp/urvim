//! Builtin handwritten scanner for Fish syntax.

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
tag_static!(TYP, "type");
tag_static!(VAR, "variable");
tag_static!(OP, "operator");

const FISH_SUBSHELL: ContextId = ContextId::new("fish", "fish_subshell");
const FISH_STRING: ContextId = ContextId::new("fish", "fish_string");
const FISH_SINGLE_STRING: ContextId = ContextId::new("fish", "fish_single_string");

pub(crate) fn tokenize_fish_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
    let (mut ctx, _inj, _parent_style, tokenizer_state) = match state {
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

        // ── Subshell (...) ───────────────────────────────────────────
        if ctx.top_is(FISH_SUBSHELL) {
            if tail_bytes[0] == b')' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(FISH_SUBSHELL);
                index = end;
                continue;
            }
        }

        // ── Double-quoted string ─────────────────────────────────────
        if ctx.top_is(FISH_STRING) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(FISH_STRING);
                index = end;
                continue;
            }
            if tail_bytes[0] == b'$' && tail_bytes.len() >= 2 {
                let second = tail_bytes[1];
                if second.is_ascii_alphabetic() || second == b'_' {
                    let mut i = 2;
                    while i < tail_bytes.len()
                        && (tail_bytes[i].is_ascii_alphanumeric() || tail_bytes[i] == b'_')
                    {
                        i += 1;
                    }
                    spans.push(SyntaxSpan::new(index, index + i, (*VAR).clone()));
                    index += i;
                    continue;
                }
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '\n' && c != '$');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Single-quoted string (Fish expands $var in single quotes!) ─
        if ctx.top_is(FISH_SINGLE_STRING) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(FISH_SINGLE_STRING);
                index = end;
                continue;
            }
            if tail_bytes[0] == b'$' && tail_bytes.len() >= 2 {
                let second = tail_bytes[1];
                if second.is_ascii_alphabetic() || second == b'_' {
                    let mut i = 2;
                    while i < tail_bytes.len()
                        && (tail_bytes[i].is_ascii_alphanumeric() || tail_bytes[i] == b'_')
                    {
                        i += 1;
                    }
                    spans.push(SyntaxSpan::new(index, index + i, (*VAR).clone()));
                    index += i;
                    continue;
                }
            }
            let run = run_while(tail, |c| c != '\'' && c != '\n' && c != '$');
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

        if tail_bytes[0] == b'#' {
            spans.push(SyntaxSpan::new(index, line_len, (*COMMENT).clone()));
            index = line_len;
            continue;
        }

        // ( subshell open
        if tail_bytes[0] == b'(' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            ctx.push(FISH_SUBSHELL);
            index = end;
            continue;
        }

        // " double string
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(FISH_STRING);
            index = end;
            continue;
        }

        // ' single string (Fish expands $var)
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(FISH_SINGLE_STRING);
            index = end;
            continue;
        }

        // $var
        if tail_bytes[0] == b'$' && tail_bytes.len() >= 2 {
            let second = tail_bytes[1];
            if second.is_ascii_alphabetic() || second == b'_' {
                let mut i = 2;
                while i < tail_bytes.len()
                    && (tail_bytes[i].is_ascii_alphanumeric() || tail_bytes[i] == b'_')
                {
                    i += 1;
                }
                spans.push(SyntaxSpan::new(index, index + i, (*VAR).clone()));
                index += i;
                continue;
            }
        }

        // Number
        if let Some(len) = match_number(tail, index, bytes) {
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

        // Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b';'
        ) {
            spans.push(SyntaxSpan::new(index, index + 1, (*P).clone()));
            index += 1;
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

    let parent_style = spans.last().map(|s| s.style.clone());
    (
        spans,
        SyntaxState::Code(CodeState::Scanner {
            contexts: ctx,
            injection: None,
            parent_style,
            tokenizer_state,
        }),
    )
}

fn match_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_word_byte(full_bytes[index - 1])) {
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
        let dot = i;
        i += 1;
        if i < len && bytes[i].is_ascii_digit() {
            i += 1;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
        } else {
            i = dot;
        }
    }
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
        "and", "begin", "break", "case", "continue", "else", "end", "for", "function", "if", "not",
        "or", "return", "switch", "while",
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
        "cd", "echo", "math", "set", "source", "status", "string", "test",
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
    for word in &["true", "false"] {
        if tail.starts_with(word) {
            let after = word.len();
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
        &["==", "!=", "<=", ">=", "&&", "||"],
        b"+-*/%=&|!<>^~?:",
    )
}
