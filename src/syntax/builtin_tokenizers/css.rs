//! Builtin handwritten scanner for CSS syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::run_while;
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
tag_static!(PROP, "variable.property");

const CSS_COMMENT: ContextId = ContextId::new("css", "css_comment");
const CSS_DOUBLE_STRING: ContextId = ContextId::new("css", "css_double_string");
const CSS_SINGLE_STRING: ContextId = ContextId::new("css", "css_single_string");

/// Tokenize one line of CSS using the builtin scanner.
pub(crate) fn tokenize_css_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Inside block comment ─────────────────────────────────────
        if ctx.top_is(CSS_COMMENT) {
            // Rule 2: */ close
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
                ctx.pop_top(CSS_COMMENT);
                index = end;
                continue;
            }
            // Rule 1: /* nested open (no requires, matches inside)
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
                ctx.push(CSS_COMMENT);
                index = end;
                continue;
            }
            // Rule 3: Content [^*]+
            let run = run_while(tail, |c| c != '*');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*COMMENT).clone()));
                index += run;
                continue;
            }
            // Rule 4: Single *
            if tail_bytes[0] == b'*' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
                index = end;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside double string ─────────────────────────────────────
        if ctx.top_is(CSS_DOUBLE_STRING) {
            // Rule 5: Closing "
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(CSS_DOUBLE_STRING);
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
            // Rule 11: Content [^'"\\]+ (excludes ' " \)
            let run = run_while(tail, |c| c != '"' && c != '\'' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Fall through: ' opens a single string from top-level rules
        }

        // ── Inside single string ─────────────────────────────────────
        if ctx.top_is(CSS_SINGLE_STRING) {
            // Rule 6: Closing '
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(CSS_SINGLE_STRING);
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
            // Rule 12: Content [^'"\\]+ (excludes ' " \)
            let run = run_while(tail, |c| c != '"' && c != '\'' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Fall through: " opens a double string from top-level rules
        }

        // ── Top-level ────────────────────────────────────────────────
        // (also reached as fallthrough from string contexts)

        // Rule 1: Block comment /* (outside comment)
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            ctx.push(CSS_COMMENT);
            index = end;
            continue;
        }

        // Rule 8: Double string open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(CSS_DOUBLE_STRING);
            index = end;
            continue;
        }

        // Rule 10: Single string open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(CSS_SINGLE_STRING);
            index = end;
            continue;
        }

        // Rule 13: Hex color #...
        if let Some(hex_len) = match_hex_color(tail) {
            spans.push(SyntaxSpan::new(index, index + hex_len, (*CNST).clone()));
            index += hex_len;
            continue;
        }

        // Rules 14-16: At-rule / Selector / Property
        // (^ anchor matches at current index — regex engine applies
        // regex to tail = line[index..] so ^ matches at start of tail)
        if let Some(ar_len) = match_at_rule(tail) {
            spans.push(SyntaxSpan::new(index, index + ar_len, (*KW).clone()));
            index += ar_len;
            continue;
        }
        if let Some(sel_len) = match_selector(tail) {
            spans.push(SyntaxSpan::new(index, index + sel_len, (*TYP).clone()));
            index += sel_len;
            continue;
        }
        if let Some(prop_len) = match_property(tail) {
            spans.push(SyntaxSpan::new(index, index + prop_len, (*PROP).clone()));
            index += prop_len;
            continue;
        }

        // Rule 17: Number with optional unit
        if let Some(num_len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // Rule 18: Punctuation
        if matches!(
            tail_bytes[0],
            b'{' | b'}' | b':' | b';' | b'(' | b')' | b','
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
        SyntaxState::Code(CodeState::RuleList {
            contexts: ctx,
            injection: inj,
            parent_style,
        }),
    )
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn match_hex_color(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 4 || bytes[0] != b'#' {
        return None;
    }
    // Count hex digits
    let mut i = 1;
    while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
        i += 1;
    }
    let digit_count = i - 1;
    if (3..=8).contains(&digit_count) {
        // Check word boundary at end
        if i >= bytes.len() || !is_word_byte(bytes[i]) {
            return Some(i);
        }
    }
    None
}

fn match_at_rule(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // ^\s*
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // @
    if i >= len || bytes[i] != b'@' {
        return None;
    }
    i += 1;

    // [_A-Za-z-][A-Za-z0-9_-]*
    if i >= len || !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' || bytes[i] == b'-') {
        return None;
    }
    i += 1;
    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-') {
        i += 1;
    }

    Some(i)
}

fn match_selector(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return None;
    }

    // ^[^@{}] — first char not @ { }
    if bytes[0] == b'@' || bytes[0] == b'{' || bytes[0] == b'}' {
        return None;
    }

    // [^{}]*
    let mut i = 0;
    while i < len && bytes[i] != b'{' && bytes[i] != b'}' {
        i += 1;
    }

    // Lookahead: {
    if i < len && bytes[i] == b'{' {
        return Some(i);
    }

    None
}

fn match_property(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    // ^\s*
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // [A-Za-z-][A-Za-z0-9-]*
    if i >= len || !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'-') {
        return None;
    }
    i += 1;
    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
        i += 1;
    }

    // Lookahead: :
    if i < len && bytes[i] == b':' {
        return Some(i);
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

    // Optional unit suffix
    if i < len {
        let unit_start = i;
        while i < len && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }
        let unit = &tail[unit_start..i];
        let valid_units = [
            "px", "rem", "em", "vh", "vw", "vmin", "vmax", "%", "deg", "s", "ms",
        ];
        if !valid_units.contains(&unit) {
            i = unit_start;
        }
    }

    if i < len && is_word_byte(bytes[i]) {
        return None;
    }

    Some(i)
}
