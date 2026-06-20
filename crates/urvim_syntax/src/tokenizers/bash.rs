//! Builtin handwritten scanner for Bash syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{is_word_byte, match_operator_from_sets, run_while};
use crate::state::{CodeState, ContextId, ContextStack, SyntaxSpan, SyntaxState};
use urvim_theme::Tag;

macro_rules! tag_static {
    ($name:ident, $s:expr) => {
        static $name: LazyLock<Tag> = LazyLock::new(|| Tag::parse($s).unwrap());
    };
}

tag_static!(COMMENT, "comment");
tag_static!(KW, "keyword");
tag_static!(S, "string");
tag_static!(S_ESCAPE, "string.escape");
tag_static!(S_HEREDOC, "string.heredoc");
tag_static!(S_INTERP, "string.interpolation");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(VAR, "variable");
tag_static!(OP, "operator");

const BASH_HEREDOC: ContextId = ContextId::new("bash", "bash_heredoc");
const BASH_HEREDOC_BODY: ContextId = ContextId::new("bash", "bash_heredoc_body");
const BASH_PRINTF_STRING: ContextId = ContextId::new("bash", "bash_printf_string");
const BASH_PRINTF_CALL: ContextId = ContextId::new("bash", "bash_printf_call");
const BASH_ANSI_SINGLE_QUOTED: ContextId = ContextId::new("bash", "bash_ansi_single_quoted");
const BASH_SINGLE_QUOTED: ContextId = ContextId::new("bash", "bash_single_quoted");
const BASH_DOUBLE_QUOTED: ContextId = ContextId::new("bash", "bash_double_quoted");
const BASH_BACKTICK: ContextId = ContextId::new("bash", "bash_backtick");
const BASH_COMMAND_SUB: ContextId = ContextId::new("bash", "bash_command_sub");
const BASH_ARITHMETIC_SUB: ContextId = ContextId::new("bash", "bash_arithmetic_sub");

/// Tokenize one line of Bash using the builtin scanner.
pub(crate) fn tokenize_bash_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Heredoc body ─────────────────────────────────────────────
        if ctx.top_is(BASH_HEREDOC_BODY) {
            let delim = ctx.payload_for(BASH_HEREDOC).unwrap_or("").to_string();
            if index == 0 && !delim.is_empty() && line.starts_with(&delim) {
                if delim.len() == line_len || line[delim.len()..].trim().is_empty() {
                    spans.push(SyntaxSpan::new(index, line_len, (*S_ESCAPE).clone()));
                    ctx.pop(BASH_HEREDOC_BODY);
                    ctx.pop(BASH_HEREDOC);
                    index = line_len;
                    continue;
                }
            }
            if index < line_len {
                spans.push(SyntaxSpan::new(index, line_len, (*S_HEREDOC).clone()));
                index = line_len;
                continue;
            }
            break;
        }

        // ── printf format string ─────────────────────────────────────
        if ctx.top_is(BASH_PRINTF_STRING) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(BASH_PRINTF_STRING);
                index = end;
                continue;
            }
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'%' && tail_bytes[1] == b'%' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                index = end;
                continue;
            }
            if let Some(len) = match_printf_format(tail) {
                spans.push(SyntaxSpan::new(index, index + len, (*S_INTERP).clone()));
                index += len;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '%' && c != '\'' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── printf call head ─────────────────────────────────────────
        if ctx.top_is(BASH_PRINTF_CALL) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(BASH_PRINTF_CALL);
                ctx.push(BASH_PRINTF_STRING);
                index = end;
                continue;
            }
        }

        // ── ANSI-C $'...' ───────────────────────────────────────────
        if ctx.top_is(BASH_ANSI_SINGLE_QUOTED) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(BASH_ANSI_SINGLE_QUOTED);
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
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Single-quoted string ─────────────────────────────────────
        if ctx.top_is(BASH_SINGLE_QUOTED) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(BASH_SINGLE_QUOTED);
                index = end;
                continue;
            }
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

        // ── Double-quoted string ─────────────────────────────────────
        if ctx.top_is(BASH_DOUBLE_QUOTED) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(BASH_DOUBLE_QUOTED);
                index = end;
                continue;
            }
            // ${...}
            if tail_bytes[0] == b'$' && tail_bytes.len() >= 2 && tail_bytes[1] == b'{' {
                let mut i = 2;
                while i < tail_bytes.len() && tail_bytes[i] != b'}' && tail_bytes[i] != b'\n' {
                    i += 1;
                }
                if i < tail_bytes.len() && tail_bytes[i] == b'}' {
                    spans.push(SyntaxSpan::new(index, index + i + 1, (*VAR).clone()));
                    index += i + 1;
                    continue;
                }
            }
            // $((...))
            if tail_bytes.len() >= 4
                && tail_bytes[0] == b'$'
                && tail_bytes[1] == b'('
                && tail_bytes[2] == b'('
            {
                let mut i = 3;
                while i < tail_bytes.len() && tail_bytes[i] != b')' {
                    i += 1;
                }
                if i + 1 < tail_bytes.len() && tail_bytes[i] == b')' && tail_bytes[i + 1] == b')' {
                    spans.push(SyntaxSpan::new(index, index + i + 2, (*P).clone()));
                    index += i + 2;
                    continue;
                }
            }
            // $(...)
            if tail_bytes.len() >= 3 && tail_bytes[0] == b'$' && tail_bytes[1] == b'(' {
                let mut i = 2;
                while i < tail_bytes.len() && tail_bytes[i] != b')' {
                    i += 1;
                }
                if i < tail_bytes.len() && tail_bytes[i] == b')' {
                    spans.push(SyntaxSpan::new(index, index + i + 1, (*P).clone()));
                    index += i + 1;
                    continue;
                }
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
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '$');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Backtick ────────────────────────────────────────────────
        if ctx.top_is(BASH_BACKTICK) {
            if tail_bytes[0] == b'`' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(BASH_BACKTICK);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '`' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Command substitution $() ────────────────────────────────
        if ctx.top_is(BASH_COMMAND_SUB) {
            if tail_bytes[0] == b')' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(BASH_COMMAND_SUB);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != ')' && c != '\\');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Arithmetic substitution $(()) ────────────────────────────
        if ctx.top_is(BASH_ARITHMETIC_SUB) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b')' && tail_bytes[1] == b')' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(BASH_ARITHMETIC_SUB);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != ')' && c != '\\');
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

        // Heredoc open
        if let Some(len) = match_heredoc_open(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*S_ESCAPE).clone()));
            let content = &tail[2..];
            let mut delim = String::new();
            let mut started = false;
            for c in content.chars() {
                if c.is_alphanumeric() || c == '_' {
                    started = true;
                    delim.push(c);
                } else if started {
                    break;
                }
            }
            ctx.push_with_payload(BASH_HEREDOC, &delim);
            ctx.push(BASH_HEREDOC_BODY);
            index += len;
            continue;
        }

        // printf keyword (before string opens)
        if let Some(len) = match_printf_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            ctx.push(BASH_PRINTF_CALL);
            index += len;
            continue;
        }

        // $'...'
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'$' && tail_bytes[1] == b'\'' {
            spans.push(SyntaxSpan::new(index, index + 2, (*S).clone()));
            ctx.push(BASH_ANSI_SINGLE_QUOTED);
            index += 2;
            continue;
        }

        // '...'
        if tail_bytes[0] == b'\'' {
            spans.push(SyntaxSpan::new(index, index + 1, (*S).clone()));
            ctx.push(BASH_SINGLE_QUOTED);
            index += 1;
            continue;
        }

        // "..."
        if tail_bytes[0] == b'"' {
            spans.push(SyntaxSpan::new(index, index + 1, (*S).clone()));
            ctx.push(BASH_DOUBLE_QUOTED);
            index += 1;
            continue;
        }

        // ${...}
        if tail_bytes.len() >= 3 && tail_bytes[0] == b'$' && tail_bytes[1] == b'{' {
            let mut i = 2;
            while i < tail_bytes.len() && tail_bytes[i] != b'}' && tail_bytes[i] != b'\n' {
                i += 1;
            }
            if i < tail_bytes.len() && tail_bytes[i] == b'}' {
                spans.push(SyntaxSpan::new(index, index + i + 1, (*VAR).clone()));
                index += i + 1;
                continue;
            }
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

        // $(()) arithmetic sub
        if tail_bytes.len() >= 4
            && tail_bytes[0] == b'$'
            && tail_bytes[1] == b'('
            && tail_bytes[2] == b'('
        {
            spans.push(SyntaxSpan::new(index, index + 3, (*P).clone()));
            ctx.push(BASH_ARITHMETIC_SUB);
            index += 3;
            continue;
        }

        // $() command sub
        if tail_bytes.len() >= 3 && tail_bytes[0] == b'$' && tail_bytes[1] == b'(' {
            spans.push(SyntaxSpan::new(index, index + 2, (*P).clone()));
            ctx.push(BASH_COMMAND_SUB);
            index += 2;
            continue;
        }

        // ` backtick
        if tail_bytes[0] == b'`' {
            spans.push(SyntaxSpan::new(index, index + 1, (*P).clone()));
            ctx.push(BASH_BACKTICK);
            index += 1;
            continue;
        }

        // Number
        if let Some(len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // [[ ]] (( ))
        if tail_bytes.len() >= 2 {
            let two = &tail[..2];
            if two == "[[" || two == "]]" || two == "((" || two == "))" {
                spans.push(SyntaxSpan::new(index, index + 2, (*KW).clone()));
                index += 2;
                continue;
            }
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

fn match_heredoc_open(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 4 || bytes[0] != b'<' || bytes[1] != b'<' {
        return None;
    }
    let mut i = 2;
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
    }
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let mut has_quote = false;
    if i < bytes.len() && (bytes[i] == b'\'' || bytes[i] == b'"') {
        has_quote = true;
        i += 1;
    }
    if i >= bytes.len() || !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
        return None;
    }
    while i < bytes.len() && is_word_byte(bytes[i]) {
        i += 1;
    }
    if has_quote && i < bytes.len() && (bytes[i] == b'\'' || bytes[i] == b'"') {
        i += 1;
    }
    Some(i)
}

fn match_printf_format(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'%' {
        return None;
    }
    let len = bytes.len();
    let mut i = 1;
    let pos_start = i;
    while i < len && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > pos_start && i < len && bytes[i] == b'$' {
        i += 1;
    } else {
        i = pos_start;
    }
    while i < len && matches!(bytes[i], b'-' | b'+' | b' ' | b'#' | b'0' | b'\'') {
        i += 1;
    }
    if i < len && bytes[i] == b'*' {
        i += 1;
    } else {
        while i < len && bytes[i].is_ascii_digit() {
            i += 1;
        }
    }
    if i < len && bytes[i] == b'.' {
        i += 1;
        if i < len && bytes[i] == b'*' {
            i += 1;
        } else {
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    if i < len {
        let two = if i + 1 < len { &bytes[i..i + 2] } else { b"" };
        if two == b"hh" || two == b"ll" {
            i += 2;
        } else if matches!(bytes[i], b'h' | b'l' | b'j' | b'z' | b't' | b'L') {
            i += 1;
        }
    }
    if i < len
        && matches!(
            bytes[i],
            b'd' | b'i'
                | b'u'
                | b'o'
                | b'x'
                | b'X'
                | b'f'
                | b'F'
                | b'e'
                | b'E'
                | b'g'
                | b'G'
                | b'a'
                | b'A'
                | b'c'
                | b's'
                | b'p'
                | b'n'
        )
    {
        Some(i + 1)
    } else {
        None
    }
}

fn match_printf_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    if tail.starts_with("printf") {
        let after = 6;
        if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
            Some(6)
        } else {
            None
        }
    } else {
        None
    }
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
        "case", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in",
        "select", "then", "until", "while",
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
        "alias", "bind", "builtin", "declare", "echo", "export", "local", "readonly", "source",
        "typeset", "unset",
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
        &["===", "==", "!=", "<=", ">=", "&&", "||", "="],
        b"+-*/%&|!<>^~?",
    )
}
