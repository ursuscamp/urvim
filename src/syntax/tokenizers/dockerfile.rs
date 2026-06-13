//! Builtin handwritten scanner for Dockerfile syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{is_word_byte, run_while};
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
tag_static!(VAR, "variable");

const DOCKER_STRING: ContextId = ContextId::new("dockerfile", "docker_string");
const DOCKER_SINGLE_STRING: ContextId = ContextId::new("dockerfile", "docker_single_string");

/// Tokenize one line of Dockerfile using the builtin scanner.
pub(crate) fn tokenize_dockerfile_line(
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

        // ── Inside double-quoted string ──────────────────────────────
        if ctx.top_is(DOCKER_STRING) {
            // Rule 4: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(DOCKER_STRING);
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
            // Rule 3: Variable $... (checked before content)
            if let Some(var_len) = match_variable(line, index) {
                spans.push(SyntaxSpan::new(index, index + var_len, (*VAR).clone()));
                index += var_len;
                continue;
            }
            // Rule 6: Content [^"\\$\n]+
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '$' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside single-quoted string ──────────────────────────────
        if ctx.top_is(DOCKER_SINGLE_STRING) {
            // Rule 8: Closing '
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(DOCKER_SINGLE_STRING);
                index = end;
                continue;
            }
            // Rule 3: Variable $... (checked before content)
            if let Some(var_len) = match_variable(line, index) {
                spans.push(SyntaxSpan::new(index, index + var_len, (*VAR).clone()));
                index += var_len;
                continue;
            }
            // Rule 10: Content [^'$\n]+
            let run = run_while(tail, |c| c != '\'' && c != '$' && c != '\n');
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

        // Rule 1: Comment
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Rule 2: Instruction keyword at line start
        if index == 0 {
            if let Some(end) = match_instruction(line) {
                spans.push(SyntaxSpan::new(0, end, (*KW).clone()));
                index = end;
                continue;
            }
        }

        // Rule 3: Variable $...
        if let Some(var_len) = match_variable(line, index) {
            spans.push(SyntaxSpan::new(index, index + var_len, (*VAR).clone()));
            index += var_len;
            continue;
        }

        // Rule 5: Opening " (push)
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(DOCKER_STRING);
            index = end;
            continue;
        }

        // Rule 9: Opening ' (push)
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(DOCKER_SINGLE_STRING);
            index = end;
            continue;
        }

        // Rule 11: Number with word boundaries
        if let Some(num_len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // Rule 12: Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b':' | b'='
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
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

fn match_instruction(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    for kw in &[
        "FROM",
        "RUN",
        "CMD",
        "LABEL",
        "MAINTAINER",
        "EXPOSE",
        "ENV",
        "ARG",
        "ADD",
        "COPY",
        "ENTRYPOINT",
        "VOLUME",
        "USER",
        "WORKDIR",
        "HEALTHCHECK",
        "SHELL",
        "STOPSIGNAL",
        "ONBUILD",
    ] {
        if line[i..].starts_with(kw) {
            let after = i + kw.len();
            if after >= len || !is_word_byte(bytes[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_variable(line: &str, index: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    if index > 0 && bytes[index - 1] == b'\\' {
        return None;
    }

    let tail = &bytes[index..];
    if tail.is_empty() || tail[0] != b'$' {
        return None;
    }

    let len = tail.len();
    let mut i = 1;

    if i < len && tail[i] == b'{' {
        i += 1;
        if i >= len || !is_identifier_start(tail[i]) {
            return None;
        }

        i += 1;
        while i < len && is_identifier_continue(tail[i]) {
            i += 1;
        }

        while i < len && tail[i] != b'}' {
            i += 1;
        }
        if i >= len || tail[i] != b'}' {
            return None;
        }

        return Some(i + 1);
    }

    // Identifier: [A-Za-z_][A-Za-z0-9_]*
    if i >= len || !is_identifier_start(tail[i]) {
        return None;
    }
    i += 1;
    while i < len && is_identifier_continue(tail[i]) {
        i += 1;
    }

    Some(i)
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_identifier_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
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

    // \d+
    if i >= len || !bytes[i].is_ascii_digit() {
        return None;
    }
    i += 1;
    while i < len && bytes[i].is_ascii_digit() {
        i += 1;
    }

    // (?:\.\d+)?
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

    // Right word boundary
    if i < len && is_word_byte(bytes[i]) {
        return None;
    }

    Some(i)
}
