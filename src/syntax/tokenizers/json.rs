//! Builtin handwritten scanner for JSON syntax.

use std::sync::LazyLock;

use super::scanner::run_while;
use crate::buffer::syntax::{
    CodeState, ContextId, ContextStack, SyntaxLineResult, SyntaxSpan, SyntaxState,
};
use crate::theme::Tag;

macro_rules! tag_static {
    ($name:ident, $s:expr) => {
        static $name: LazyLock<Tag> = LazyLock::new(|| Tag::parse($s).unwrap());
    };
}

tag_static!(S, "string");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");

const JSON_STRING: ContextId = ContextId::new("json", "json_string");

/// Tokenize one line of JSON using the builtin scanner.
pub(crate) fn tokenize_json_line(line: &str, state: SyntaxState) -> SyntaxLineResult {
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
    let mut fold_events = Vec::new();
    let mut index = 0usize;

    while index < line.len() {
        let tail = &line[index..];
        let bytes = tail.as_bytes();

        // ── Inside string ────────────────────────────────────────────
        if ctx.top_is(JSON_STRING) {
            // 1. Closing quote (requires json_string, pops)
            if bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(JSON_STRING);
                index = end;
                continue;
            }

            // 2. Escape sequences (requires json_string)
            if bytes[0] == b'\\' && bytes.len() >= 2 {
                let second = bytes[1];
                if matches!(
                    second,
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't'
                ) {
                    let end = index + 2;
                    spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                    index = end;
                    continue;
                }
                if second == b'u' && bytes.len() >= 6 {
                    let hex = &bytes[2..6];
                    if hex.iter().all(|b| b.is_ascii_hexdigit()) {
                        let end = index + 6;
                        spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                        index = end;
                        continue;
                    }
                }
            }

            // 3. String body content (requires json_string)
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
        }

        // ── Outside string ───────────────────────────────────────────

        // 4. Opening quote (push json_string)
        if bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(JSON_STRING);
            index = end;
            continue;
        }

        // 5. Punctuation: {}[]:,
        if matches!(bytes[0], b'{' | b'}' | b'[' | b']' | b',' | b':') {
            let end = index + 1;
            super::bracket_folds::push_delimiter_fold_event(&mut fold_events, bytes[0]);
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        // 6. Number
        if let Some(len) = match_number(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // 7. Constant: true / false / null
        if let Some(len) = match_constant(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        // ── No match – skip one char ─────────────────────────────────
        let Some(ch) = tail.chars().next() else {
            break;
        };
        index += ch.len_utf8();
    }

    SyntaxLineResult {
        spans,
        fold_events,
        state: SyntaxState::Code(CodeState::Scanner {
            contexts: ctx,
            injection: inj,
            parent_style,
            tokenizer_state,
        }),
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn match_number(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let mut i = 0;

    if bytes[i] == b'-' {
        i += 1;
        if i >= bytes.len() {
            return None;
        }
    }

    if bytes[i] == b'0' {
        i += 1;
    } else if bytes[i].is_ascii_digit() {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    } else {
        return None;
    }

    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        let mut has_frac = false;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            has_frac = true;
        }
        if !has_frac {
            return None;
        }
    }

    if i < bytes.len() && matches!(bytes[i], b'e' | b'E') {
        i += 1;
        if i < bytes.len() && matches!(bytes[i], b'+' | b'-') {
            i += 1;
        }
        let mut has_exp = false;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
            has_exp = true;
        }
        if !has_exp {
            return None;
        }
    }

    Some(i)
}

fn match_constant(tail: &str) -> Option<usize> {
    for word in &["true", "false", "null"] {
        if tail.starts_with(word) {
            let after = &tail[word.len()..];
            if after
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_')
            {
                return Some(word.len());
            }
        }
    }
    None
}
