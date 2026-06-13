//! Builtin handwritten scanner for Makefile syntax.

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
tag_static!(OP, "operator");
tag_static!(VAR, "variable");

const MAKE_DOUBLE_STRING: ContextId = ContextId::new("makefile", "make_double_string");
const MAKE_SINGLE_STRING: ContextId = ContextId::new("makefile", "make_single_string");

/// Tokenize one line of Makefile using the builtin scanner.
pub(crate) fn tokenize_makefile_line(
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
        if ctx.top_is(MAKE_DOUBLE_STRING) {
            // Rule 1: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(MAKE_DOUBLE_STRING);
                index = end;
                continue;
            }
            // Rule 4: Escape \.
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            // Rule 3: Content [^"\\n{}]+
            let run = run_while(tail, |c| {
                c != '"' && c != '\\' && c != '\n' && c != '{' && c != '}'
            });
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Fall through to top-level rules (so {, }, $(...) etc.
            // inside strings are handled by variable/punctuation rules)
        }

        // ── Inside single-quoted string ──────────────────────────────
        if ctx.top_is(MAKE_SINGLE_STRING) {
            // Rule 5: Closing '
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(MAKE_SINGLE_STRING);
                index = end;
                continue;
            }
            // Rule 7: Content [^'\\n]+
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

        // Rule 8: Comment
        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Rule 9: Variable $(...) or ${...}
        if let Some(var_len) = match_make_variable(tail) {
            spans.push(SyntaxSpan::new(index, index + var_len, (*VAR).clone()));
            index += var_len;
            continue;
        }

        // Anchor-at-line-start rules (only at position 0)
        if index == 0 {
            // Rule 10: Control keyword at line start
            if let Some(end) = match_control_keyword(line) {
                spans.push(SyntaxSpan::new(0, end, (*KW).clone()));
                index = end;
                continue;
            }
            // Rule 11: Assignment operator at line start
            if let Some(end) = match_assignment_operator(line) {
                spans.push(SyntaxSpan::new(0, end, (*OP).clone()));
                index = end;
                continue;
            }
            // Rule 12: Target rule at line start
            if let Some(end) = match_target_rule(line) {
                spans.push(SyntaxSpan::new(0, end, (*KW).clone()));
                index = end;
                continue;
            }
        }

        // Rule 13: Punctuation
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

fn match_make_variable(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();

    // $(...)
    if len >= 4 && bytes[0] == b'$' && bytes[1] == b'(' {
        let start = 2;
        let mut i = start;
        while i < len && bytes[i] != b')' {
            i += 1;
        }
        if i > start && i < len && bytes[i] == b')' {
            return Some(i + 1);
        }
    }

    // ${...}
    if len >= 4 && bytes[0] == b'$' && bytes[1] == b'{' {
        let start = 2;
        let mut i = start;
        while i < len && bytes[i] != b'}' {
            i += 1;
        }
        if i > start && i < len && bytes[i] == b'}' {
            return Some(i + 1);
        }
    }

    None
}

fn match_control_keyword(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    for kw in &[
        "include", "define", "endef", "ifeq", "ifneq", "ifdef", "ifndef", "else", "endif",
        "override", "export", "unexport", "private", "vpath",
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

fn match_assignment_operator(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i >= len || !match_name_start_byte(bytes[i]) {
        return None;
    }
    while i < len && match_name_byte(bytes[i]) {
        i += 1;
    }

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // Check assignment operators
    if i < len {
        if i + 1 < len {
            let two = [bytes[i], bytes[i + 1]];
            if matches!(&two, b":=" | b"+=" | b"?=" | b"!=") {
                return Some(i + 2);
            }
        }
        if bytes[i] == b'=' {
            return Some(i + 1);
        }
    }

    None
}

fn match_target_rule(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i >= len || !match_name_start_byte(bytes[i]) {
        return None;
    }
    while i < len && match_name_byte(bytes[i]) {
        i += 1;
    }

    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i < len && bytes[i] == b':' {
        Some(i + 1)
    } else {
        None
    }
}

fn match_name_start_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'/' || b == b'%'
}

fn match_name_byte(b: u8) -> bool {
    match_name_start_byte(b) || b == b'-'
}
