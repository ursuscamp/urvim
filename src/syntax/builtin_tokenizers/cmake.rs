//! Builtin handwritten scanner for CMake syntax.

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
tag_static!(CNST, "constant");
tag_static!(VAR, "variable");

const CMAKE_BRACKET_STRING: ContextId = ContextId::new("cmake", "cmake_bracket_string");
const CMAKE_STRING: ContextId = ContextId::new("cmake", "cmake_string");

/// Tokenize one line of CMake using the builtin scanner.
pub(crate) fn tokenize_cmake_line(
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

        // ── Inside bracket string ────────────────────────────────────
        if ctx.top_is(CMAKE_BRACKET_STRING) {
            // Rule 2: Closing ]]
            if tail_bytes.len() >= 2 && tail_bytes[0] == b']' && tail_bytes[1] == b']' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(CMAKE_BRACKET_STRING);
                index = end;
                continue;
            }
            // Rule 4: Content [^\]\n]+
            let run = run_while(tail, |c| c != ']');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside double-quoted string ──────────────────────────────
        if ctx.top_is(CMAKE_STRING) {
            // Rule 5: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(CMAKE_STRING);
                index = end;
                continue;
            }
            // Rule 9: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 7: Variable ${...}
            if let Some(var_len) = match_cmake_variable(tail) {
                spans.push(SyntaxSpan::new(index, index + var_len, (*VAR).clone()));
                index += var_len;
                continue;
            }
            // Rule 8: Content [^"\\\n$]+
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

        // ── Top-level ────────────────────────────────────────────────

        // Rule 1: Comment
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Rule 3: Bracket string opening [[
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'[' && tail_bytes[1] == b'[' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(CMAKE_BRACKET_STRING);
            index = end;
            continue;
        }

        // Rule 6: Double-quoted string opening "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(CMAKE_STRING);
            index = end;
            continue;
        }

        // Rule 10: Keyword with word boundaries (checked before
        //          constant/number because keywords may start with digits).
        if let Some(kw_len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + kw_len, (*KW).clone()));
            index += kw_len;
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

        // Rule 13: Punctuation
        if matches!(tail_bytes[0], b'(' | b')' | b'[' | b']' | b',') {
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
        SyntaxState::Code(CodeState::RuleList {
            contexts: ctx,
            injection: inj,
            parent_style,
        }),
    )
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for kw in &[
        "add_custom_command",
        "add_custom_target",
        "add_definitions",
        "add_executable",
        "add_library",
        "cmake_minimum_required",
        "endif",
        "else",
        "elseif",
        "endforeach",
        "function",
        "foreach",
        "if",
        "include",
        "macro",
        "message",
        "project",
        "return",
        "set",
        "target_include_directories",
        "target_link_libraries",
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

    for word in &["OFF", "TRUE", "FALSE", "YES", "NO", "IGNORE", "NOTFOUND"] {
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

fn match_cmake_variable(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();

    if len < 4 || bytes[0] != b'$' || bytes[1] != b'{' {
        return None;
    }

    let mut i = 2;
    if i >= len || !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
        return None;
    }
    i += 1;
    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }

    if i < len && bytes[i] == b'}' {
        Some(i + 1)
    } else {
        None
    }
}
