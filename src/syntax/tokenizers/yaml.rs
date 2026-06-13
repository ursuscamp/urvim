//! Builtin handwritten scanner for YAML syntax.

use std::sync::LazyLock;

use super::scanner::{is_word_byte, match_word_from_list, run_while};
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
tag_static!(PROP, "variable.property");
tag_static!(VAR, "variable");

const YAML_DOUBLE_STRING: ContextId = ContextId::new("yaml", "yaml_double_string");
const YAML_SINGLE_STRING: ContextId = ContextId::new("yaml", "yaml_single_string");

/// Tokenize one line of YAML using the builtin scanner.
pub(crate) fn tokenize_yaml_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Inside double-quoted string ──────────────────────────────
        if ctx.top_is(YAML_DOUBLE_STRING) {
            let t = tail.as_bytes();
            // Rule 7: Closing "
            if t[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(YAML_DOUBLE_STRING);
                index = end;
                continue;
            }
            // Rule 8: Escape \.
            if t[0] == b'\\' && t.len() >= 2 {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 9: Content [^"\\]+
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

        // ── Inside single-quoted string ──────────────────────────────
        if ctx.top_is(YAML_SINGLE_STRING) {
            let t = tail.as_bytes();
            // Rule 10: Closing '
            if t[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(YAML_SINGLE_STRING);
                index = end;
                continue;
            }
            // Rule 11: Content [^']+
            let run = run_while(tail, |c| c != '\'');
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
        if !tail.is_empty() && tail.as_bytes()[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Rule 2: Directive %... (^ anchor via tail)
        if let Some(len) = match_directive(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        // Rule 3: Block scalar header (^ anchor via tail, $ end)
        if let Some(len) = match_block_scalar(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*S).clone()));
            index += len;
            continue;
        }

        // Rule 4: Indented block content (^ anchor via tail, $ end)
        if let Some(len) = match_indented_block(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*S).clone()));
            index += len;
            continue;
        }

        // Rule 5: YAML key (^ anchor via tail, lookahead :)
        if let Some(len) = match_key(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*PROP).clone()));
            index += len;
            continue;
        }

        // Rule 6: Anchor / alias / tag
        // Note: \s* prefix means it can consume leading whitespace; checked
        //       before bare string so &/*/! win over catch-all string
        if let Some(len) = match_anchor_alias_tag(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*VAR).clone()));
            index += len;
            continue;
        }

        // Rule 12: Double-quoted string open "
        if !tail.is_empty() && tail.as_bytes()[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(YAML_DOUBLE_STRING);
            index = end;
            continue;
        }

        // Rule 12': Single-quoted string open '
        if !tail.is_empty() && tail.as_bytes()[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(YAML_SINGLE_STRING);
            index = end;
            continue;
        }

        // Rule 13: Constant with word boundaries
        if let Some(len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        // Rule 14: Number with word boundaries
        if let Some(len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // Rule 15: Bare string (catch-all, $ end)
        if let Some(len) = match_bare_string(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*S).clone()));
            index += len;
            continue;
        }

        // Rule 16: Punctuation
        if !tail.is_empty() {
            let b = tail.as_bytes()[0];
            if matches!(b, b':' | b'[' | b']' | b'{' | b'}' | b',' | b'?' | b'-') {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
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

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn match_directive(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    // ^%[A-Za-z]+.*$
    if len == 0 || bytes[0] != b'%' {
        return None;
    }
    let mut i = 1;
    if i >= len || !bytes[i].is_ascii_alphabetic() {
        return None;
    }
    while i < len && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    // .*$ — consume rest of line
    Some(len)
}

fn match_block_scalar(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // ^\s*
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // [A-Za-z0-9_-]+
    if i >= len || !is_name_byte(bytes[i]) {
        return None;
    }
    i += 1;
    while i < len && is_name_byte(bytes[i]) {
        i += 1;
    }

    // :
    if i >= len || bytes[i] != b':' {
        return None;
    }
    i += 1;

    // \s*
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // [>|]
    if i >= len || (bytes[i] != b'>' && bytes[i] != b'|') {
        return None;
    }
    i += 1;

    // [+-]?
    if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }

    // [0-9]*
    while i < len && bytes[i].is_ascii_digit() {
        i += 1;
    }

    // \s*$
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i == len { Some(len) } else { None }
}

fn match_indented_block(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    // ^\s{2,}.+$
    if len == 0 {
        return None;
    }
    let mut i = 0;
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i < 2 {
        return None;
    }
    // .+$ — need at least one non-whitespace char before end
    if i >= len {
        return None;
    }
    Some(len)
}

fn match_key(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // ^\s*
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // [A-Za-z0-9_-]+
    if i >= len || !is_name_byte(bytes[i]) {
        return None;
    }
    while i < len && is_name_byte(bytes[i]) {
        i += 1;
    }

    // Lookahead: :
    if i < len && bytes[i] == b':' {
        Some(i)
    } else {
        None
    }
}

fn match_anchor_alias_tag(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return None;
    }
    let mut i = 0;

    // \s*
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // (?:&...|*...|!...)
    if i >= len {
        return None;
    }
    let marker = bytes[i];
    if marker != b'&' && marker != b'*' && marker != b'!' {
        return None;
    }
    i += 1;
    if i >= len || !is_name_byte(bytes[i]) {
        return None;
    }
    while i < len && is_name_byte(bytes[i]) {
        i += 1;
    }

    Some(i)
}

fn match_constant(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }
    match_word_from_list(
        tail,
        &["true", "false", "null", "yes", "no", "on", "off"],
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

    // \b\d+
    if i >= len || !bytes[i].is_ascii_digit() {
        return None;
    }
    i += 1;
    while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
        // (?:_?\d)* — underscores are allowed between digits
        if bytes[i] == b'_' {
            i += 1;
            if i >= len || !bytes[i].is_ascii_digit() {
                return None;
            }
            i += 1;
        } else {
            i += 1;
        }
    }

    // \b
    if i < len && is_word_byte(bytes[i]) {
        return None;
    }

    Some(i)
}

fn match_bare_string(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return None;
    }

    // The regex \s*[^#\n"'|>{&*!:][^#\n]*$ backtracks \s* if the
    // greedy whitespace consumption leaves an excluded character.
    // We try all possible whitespace prefixes (longest first, then
    // backtracking) to find one where the next char is allowed.
    let ws_count = bytes
        .iter()
        .take_while(|b| **b == b' ' || **b == b'\t')
        .count();

    for ws in (0..=ws_count).rev() {
        let first_idx = ws;
        if first_idx >= len {
            continue;
        }

        let first = bytes[first_idx];
        let excluded = matches!(
            first,
            b'#' | b'"' | b'\'' | b'|' | b'>' | b'{' | b'&' | b'*' | b'!' | b':'
        );
        if excluded {
            continue;
        }

        let mut i = first_idx + 1;
        while i < len && bytes[i] != b'#' {
            i += 1;
        }

        if i == len {
            return Some(len);
        }
    }

    None
}
