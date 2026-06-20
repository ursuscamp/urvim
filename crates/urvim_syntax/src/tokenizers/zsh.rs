//! Builtin handwritten scanner for Zsh syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{is_word_byte, match_operator_from_sets, match_word_from_list, run_while};
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
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(VAR, "variable");
tag_static!(OP, "operator");

const ZSH_HEREDOC: ContextId = ContextId::new("zsh", "zsh_heredoc");
const ZSH_HEREDOC_BODY: ContextId = ContextId::new("zsh", "zsh_heredoc_body");
const ZSH_ANSI_SINGLE_QUOTED: ContextId = ContextId::new("zsh", "zsh_ansi_single_quoted");
const ZSH_SINGLE_QUOTED: ContextId = ContextId::new("zsh", "zsh_single_quoted");
const ZSH_DOUBLE_QUOTED: ContextId = ContextId::new("zsh", "zsh_double_quoted");
const ZSH_BACKTICK: ContextId = ContextId::new("zsh", "zsh_backtick");
const ZSH_COMMAND_SUB: ContextId = ContextId::new("zsh", "zsh_command_sub");
const ZSH_ARITHMETIC_SUB: ContextId = ContextId::new("zsh", "zsh_arithmetic_sub");

pub(crate) fn tokenize_zsh_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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
        if ctx.top_is(ZSH_HEREDOC_BODY) {
            let delim = ctx.payload_for(ZSH_HEREDOC).unwrap_or("").to_string();
            if index == 0 && !delim.is_empty() && line.starts_with(&delim) {
                if delim.len() == line_len || line[delim.len()..].trim().is_empty() {
                    spans.push(SyntaxSpan::new(index, line_len, (*S_ESCAPE).clone()));
                    ctx.pop(ZSH_HEREDOC_BODY);
                    ctx.pop(ZSH_HEREDOC);
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

        if ctx.top_is(ZSH_ANSI_SINGLE_QUOTED) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(ZSH_ANSI_SINGLE_QUOTED);
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

        if ctx.top_is(ZSH_SINGLE_QUOTED) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(ZSH_SINGLE_QUOTED);
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

        if ctx.top_is(ZSH_DOUBLE_QUOTED) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop(ZSH_DOUBLE_QUOTED);
                index = end;
                continue;
            }
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

        if ctx.top_is(ZSH_BACKTICK) {
            if tail_bytes[0] == b'`' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(ZSH_BACKTICK);
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

        if ctx.top_is(ZSH_COMMAND_SUB) {
            if tail_bytes[0] == b')' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(ZSH_COMMAND_SUB);
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

        if ctx.top_is(ZSH_ARITHMETIC_SUB) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b')' && tail_bytes[1] == b')' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*P).clone()));
                ctx.pop(ZSH_ARITHMETIC_SUB);
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
            ctx.push_with_payload(ZSH_HEREDOC, &delim);
            ctx.push(ZSH_HEREDOC_BODY);
            index += len;
            continue;
        }

        // $'...'
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'$' && tail_bytes[1] == b'\'' {
            spans.push(SyntaxSpan::new(index, index + 2, (*S).clone()));
            ctx.push(ZSH_ANSI_SINGLE_QUOTED);
            index += 2;
            continue;
        }

        if tail_bytes[0] == b'\'' {
            spans.push(SyntaxSpan::new(index, index + 1, (*S).clone()));
            ctx.push(ZSH_SINGLE_QUOTED);
            index += 1;
            continue;
        }

        if tail_bytes[0] == b'"' {
            spans.push(SyntaxSpan::new(index, index + 1, (*S).clone()));
            ctx.push(ZSH_DOUBLE_QUOTED);
            index += 1;
            continue;
        }

        // Glob qualifier *(...)
        if tail_bytes[0] == b'*' && tail_bytes.len() >= 2 && tail_bytes[1] == b'(' {
            let mut i = 2;
            while i < tail_bytes.len() && tail_bytes[i] != b'\n' && tail_bytes[i] != b')' {
                if tail_bytes[i] == b'\\' && i + 1 < tail_bytes.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i < tail_bytes.len() && tail_bytes[i] == b')' {
                spans.push(SyntaxSpan::new(index, index + i + 1, (*P).clone()));
                index += i + 1;
                continue;
            }
        }

        // ${...} with :modifier (checked before simple ${...})
        if tail_bytes.len() >= 4 && tail_bytes[0] == b'$' && tail_bytes[1] == b'{' {
            let mut i = 2;
            // Try to match ${identifier:...} pattern with modifier
            if i < tail_bytes.len()
                && (tail_bytes[i].is_ascii_alphabetic() || tail_bytes[i] == b'_')
            {
                i += 1;
                while i < tail_bytes.len()
                    && (tail_bytes[i].is_ascii_alphanumeric() || tail_bytes[i] == b'_')
                {
                    i += 1;
                }
                if i < tail_bytes.len() && tail_bytes[i] == b':' {
                    // Has modifier — consume to closing }
                    while i < tail_bytes.len() && tail_bytes[i] != b'}' && tail_bytes[i] != b'\n' {
                        i += 1;
                    }
                    if i < tail_bytes.len() && tail_bytes[i] == b'}' {
                        spans.push(SyntaxSpan::new(index, index + i + 1, (*VAR).clone()));
                        index += i + 1;
                        continue;
                    }
                } else {
                    // Simple ${var}
                    while i < tail_bytes.len() && tail_bytes[i] != b'}' && tail_bytes[i] != b'\n' {
                        i += 1;
                    }
                    if i < tail_bytes.len() && tail_bytes[i] == b'}' {
                        spans.push(SyntaxSpan::new(index, index + i + 1, (*VAR).clone()));
                        index += i + 1;
                        continue;
                    }
                }
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

        // $(())
        if tail_bytes.len() >= 4
            && tail_bytes[0] == b'$'
            && tail_bytes[1] == b'('
            && tail_bytes[2] == b'('
        {
            spans.push(SyntaxSpan::new(index, index + 3, (*P).clone()));
            ctx.push(ZSH_ARITHMETIC_SUB);
            index += 3;
            continue;
        }

        // $()
        if tail_bytes.len() >= 3 && tail_bytes[0] == b'$' && tail_bytes[1] == b'(' {
            spans.push(SyntaxSpan::new(index, index + 2, (*P).clone()));
            ctx.push(ZSH_COMMAND_SUB);
            index += 2;
            continue;
        }

        // `
        if tail_bytes[0] == b'`' {
            spans.push(SyntaxSpan::new(index, index + 1, (*P).clone()));
            ctx.push(ZSH_BACKTICK);
            index += 1;
            continue;
        }

        if let Some(len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        if tail_bytes.len() >= 2 {
            let two = &tail[..2];
            if two == "[[" || two == "]]" || two == "((" || two == "))" {
                spans.push(SyntaxSpan::new(index, index + 2, (*KW).clone()));
                index += 2;
                continue;
            }
        }

        if let Some(len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b';'
        ) {
            spans.push(SyntaxSpan::new(index, index + 1, (*P).clone()));
            index += 1;
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
    match_word_from_list(
        tail,
        &[
            "case", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in",
            "repeat", "select", "then", "until", "while",
        ],
        index,
        full_bytes,
        is_word_byte,
    )
}

fn match_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    match_word_from_list(
        tail,
        &[
            "autoload",
            "compdef",
            "declare",
            "emulate",
            "functions",
            "local",
            "setopt",
            "typeset",
            "unsetopt",
            "whence",
            "zmodload",
        ],
        index,
        full_bytes,
        is_word_byte,
    )
}

fn match_constant(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    match_word_from_list(tail, &["true", "false"], index, full_bytes, is_word_byte)
}

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &["===", "==", "!=", "<=", ">=", "&&", "||", "="],
        b"+-*/%&|!<>^~?",
    )
}
