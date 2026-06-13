//! Builtin handwritten scanner for Justfile syntax.

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
tag_static!(OP, "operator");
tag_static!(VAR, "variable");

const JUST_DOUBLE_STRING: ContextId = ContextId::new("justfile", "just_double_string");
const JUST_SINGLE_STRING: ContextId = ContextId::new("justfile", "just_single_string");

/// Tokenize one line of Justfile using the builtin scanner.
pub(crate) fn tokenize_justfile_line(
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
        if ctx.top_is(JUST_DOUBLE_STRING) {
            // Rule 1: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(JUST_DOUBLE_STRING);
                index = end;
                continue;
            }
            // Rule 4: Variable {{...}}
            if tail_bytes.len() >= 4 && tail_bytes[0] == b'{' && tail_bytes[1] == b'{' {
                let mut i = 2;
                let start = i;
                while i < tail_bytes.len() && tail_bytes[i] != b'}' && tail_bytes[i] != b'\n' {
                    i += 1;
                }
                if i > start && i < tail_bytes.len() && tail_bytes[i] == b'}' {
                    if i + 1 < tail_bytes.len() && tail_bytes[i + 1] == b'}' {
                        let end = index + i + 2;
                        spans.push(SyntaxSpan::new(index, end, (*VAR).clone()));
                        index = end;
                        continue;
                    }
                }
            }
            // Rule 5: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 3: Content [^"\\n{}]+ (excludes { } so they
            //          fall through to punctuation / variable rules)
            let run = run_while(tail, |c| {
                c != '"' && c != '\\' && c != '\n' && c != '{' && c != '}'
            });
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Fall through to top-level rules ({, } hit punctuation)
        }

        // ── Inside single-quoted string ──────────────────────────────
        if ctx.top_is(JUST_SINGLE_STRING) {
            // Rule 6: Closing '
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(JUST_SINGLE_STRING);
                index = end;
                continue;
            }
            // Rule 8: Content [^'\\n]+
            let run = run_while(tail, |c| c != '\'' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Fall through to top-level rules
        }

        // ── Top-level ────────────────────────────────────────────────
        // (also reached as fallthrough from string contexts)

        // Rule 9: Comment
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Rule 2: Opening " (push)
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(JUST_DOUBLE_STRING);
            index = end;
            continue;
        }

        // Rule 7: Opening ' (push)
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(JUST_SINGLE_STRING);
            index = end;
            continue;
        }

        // Anchor-at-line-start rules (only at position 0)
        if index == 0 {
            // Rule 10: Recipe target (checked before assignment)
            if let Some(end) = match_recipe(line) {
                spans.push(SyntaxSpan::new(0, end, (*KW).clone()));
                index = end;
                continue;
            }
            // Rule 11: Assignment operator
            if let Some(end) = match_assignment(line) {
                spans.push(SyntaxSpan::new(0, end, (*OP).clone()));
                index = end;
                continue;
            }
        }

        // Rule 12: Number with word boundaries
        if let Some(num_len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // Rule 13: Keyword with word boundaries
        if let Some(kw_len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + kw_len, (*KW).clone()));
            index += kw_len;
            continue;
        }

        // Rule 14: Punctuation
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

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn match_recipe(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i >= len || !is_name_byte(bytes[i]) {
        return None;
    }
    i += 1;
    while i < len && is_name_byte(bytes[i]) {
        i += 1;
    }

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i >= len || bytes[i] != b':' {
        return None;
    }
    i += 1;

    if i >= len {
        return Some(i);
    }

    if bytes[i] == b' ' || bytes[i] == b'\t' {
        let colon_end = i;
        i += 1;
        if i < len && bytes[i] != b'=' {
            return Some(colon_end);
        }
    }

    None
}

fn match_assignment(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i >= len || !is_name_byte(bytes[i]) {
        return None;
    }
    i += 1;
    while i < len && is_name_byte(bytes[i]) {
        i += 1;
    }

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i < len {
        if i + 1 < len {
            let two = [bytes[i], bytes[i + 1]];
            if matches!(&two, b":=" | b"+=" | b"?=") {
                return Some(i + 2);
            }
        }
        if bytes[i] == b'=' {
            return Some(i + 1);
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

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    for kw in &["default", "export", "import", "set"] {
        if tail.starts_with(kw) {
            let after = kw.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}
