//! Builtin handwritten scanner for Ruby syntax.

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
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(VAR, "variable");
tag_static!(OP, "operator");

const RUBY_HEREDOC: ContextId = ContextId::new("ruby", "ruby_heredoc");
const RUBY_HEREDOC_BODY: ContextId = ContextId::new("ruby", "ruby_heredoc_body");
const RUBY_TRIPLE_DOUBLE: ContextId = ContextId::new("ruby", "ruby_triple_double");
const RUBY_DOUBLE_STRING: ContextId = ContextId::new("ruby", "ruby_double_string");
const RUBY_SINGLE_STRING: ContextId = ContextId::new("ruby", "ruby_single_string");
const RUBY_BLOCK_COMMENT: ContextId = ContextId::new("ruby", "ruby_block_comment");

/// Tokenize one line of Ruby using the builtin scanner.
pub(crate) fn tokenize_ruby_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Inside heredoc body ──────────────────────────────────────
        if ctx.top_is(RUBY_HEREDOC_BODY) {
            let delim = ctx.payload_for(RUBY_HEREDOC).unwrap_or("").to_string();

            if index == 0 && !delim.is_empty() {
                let mut i = 0;
                while i < line_len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                    i += 1;
                }
                if line[i..].starts_with(&delim) {
                    let end = i + delim.len();
                    let mut j = end;
                    while j < line_len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                        j += 1;
                    }
                    if j == line_len {
                        spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                        ctx.pop_top(RUBY_HEREDOC_BODY);
                        ctx.pop_top(RUBY_HEREDOC);
                        index = end;
                        continue;
                    }
                }
            }

            if index < line_len {
                spans.push(SyntaxSpan::new(index, line_len, (*S_HEREDOC).clone()));
                index = line_len;
                continue;
            }
            break;
        }

        // ── Inside triple double string ──────────────────────────────
        if ctx.top_is(RUBY_TRIPLE_DOUBLE) {
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(RUBY_TRIPLE_DOUBLE);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                index = end;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside double string ─────────────────────────────────────
        if ctx.top_is(RUBY_DOUBLE_STRING) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(RUBY_DOUBLE_STRING);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside single string ─────────────────────────────────────
        if ctx.top_is(RUBY_SINGLE_STRING) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(RUBY_SINGLE_STRING);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
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

        // ── Inside multiline comment ─────────────────────────────────
        if ctx.top_is(RUBY_BLOCK_COMMENT) {
            spans.push(SyntaxSpan::new(index, line_len, (*COMMENT).clone()));
            if is_ruby_block_comment_end(line) {
                ctx.pop_top(RUBY_BLOCK_COMMENT);
            }
            index = line_len;
            continue;
        }

        // ── Top-level ────────────────────────────────────────────────

        if index == 0 && is_ruby_block_comment_start(line) {
            spans.push(SyntaxSpan::new(index, line_len, (*COMMENT).clone()));
            ctx.push(RUBY_BLOCK_COMMENT);
            index = line_len;
            continue;
        }

        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Heredoc open
        if let Some(hd_len) = match_heredoc_open(tail) {
            spans.push(SyntaxSpan::new(index, index + hd_len, (*S_ESCAPE).clone()));
            let content = &tail[2..];
            let mut delim = String::new();
            let mut started = false;
            for ch in content.chars() {
                if ch.is_alphanumeric() || ch == '_' {
                    started = true;
                    delim.push(ch);
                } else if started {
                    break;
                }
            }
            ctx.push_with_payload(RUBY_HEREDOC, &delim);
            ctx.push(RUBY_HEREDOC_BODY);
            index += hd_len;
            continue;
        }

        // Triple double open """
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'"'
            && tail_bytes[1] == b'"'
            && tail_bytes[2] == b'"'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(RUBY_TRIPLE_DOUBLE);
            index = end;
            continue;
        }

        // Double string open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(RUBY_DOUBLE_STRING);
            index = end;
            continue;
        }

        // Single string open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(RUBY_SINGLE_STRING);
            index = end;
            continue;
        }

        // Symbol :name
        if let Some(sym_len) = match_symbol(tail) {
            spans.push(SyntaxSpan::new(index, index + sym_len, (*CNST).clone()));
            index += sym_len;
            continue;
        }

        // Variable $ @ @@
        if let Some(var_len) = match_variable(tail) {
            spans.push(SyntaxSpan::new(index, index + var_len, (*VAR).clone()));
            index += var_len;
            continue;
        }

        if let Some(len) = match_keyword(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*KW).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_constant(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_char_literal(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*S).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        if let Some(len) = match_capitalized_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
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

fn is_ruby_block_comment_start(line: &str) -> bool {
    line == "=begin" || line.starts_with("=begin ") || line.starts_with("=begin\t")
}

fn is_ruby_block_comment_end(line: &str) -> bool {
    line == "=end" || line.starts_with("=end ") || line.starts_with("=end\t")
}

fn match_heredoc_open(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 4 || bytes[0] != b'<' || bytes[1] != b'<' {
        return None;
    }
    let mut i = 2;
    // Optional - or ~
    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'~') {
        i += 1;
    }
    // \s*
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    // Optional quote
    let mut has_quote = false;
    if i < bytes.len() && (bytes[i] == b'\'' || bytes[i] == b'"' || bytes[i] == b'`') {
        has_quote = true;
        i += 1;
    }
    if i >= bytes.len() || !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
        return None;
    }
    while i < bytes.len() && is_word_byte(bytes[i]) {
        i += 1;
    }
    if has_quote && i < bytes.len() && (bytes[i] == b'\'' || bytes[i] == b'"' || bytes[i] == b'`') {
        i += 1;
    }
    // Optional ; (not in the TOML pattern but support it)
    if i < bytes.len() && bytes[i] == b';' {
        i += 1;
    }
    Some(i)
}

fn match_symbol(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 2 || bytes[0] != b':' {
        return None;
    }
    let second = bytes[1];
    if !second.is_ascii_alphabetic() && second != b'_' {
        return None;
    }
    let mut i = 2;
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric()
            || bytes[i] == b'_'
            || bytes[i] == b'!'
            || bytes[i] == b'?'
            || bytes[i] == b'=')
    {
        i += 1;
    }
    Some(i)
}

fn match_variable(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut i = 0;
    // @@class_var
    if i + 1 < bytes.len() && bytes[0] == b'@' && bytes[1] == b'@' {
        i = 2;
    // @instance_var
    } else if bytes[0] == b'@' {
        i = 1;
    // $global
    } else if bytes[0] == b'$' {
        i = 1;
    } else {
        return None;
    }
    if i >= bytes.len() || !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
        return None;
    }
    i += 1;
    while i < bytes.len() && is_word_byte(bytes[i]) {
        i += 1;
    }
    Some(i)
}

fn match_keyword(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    for kw in &[
        "BEGIN", "END", "alias", "and", "begin", "break", "case", "class", "def", "defined?", "do",
        "else", "elsif", "end", "ensure", "false", "for", "if", "in", "module", "next", "nil",
        "not", "or", "redo", "rescue", "retry", "return", "self", "super", "then", "true", "undef",
        "unless", "until", "when", "while", "yield",
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
    for word in &["false", "null", "true", "nil", "self"] {
        if tail.starts_with(word) {
            let after = word.len();
            if after >= tail.len() || !is_word_byte(tail.as_bytes()[after]) {
                return Some(after);
            }
        }
    }
    None
}

fn match_char_literal(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'?' {
        return None;
    }
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }

    if bytes[1] == b'\\' {
        if bytes.len() < 3 || is_line_or_space(bytes[2]) {
            return None;
        }
        return Some(3);
    }

    let next = bytes[1];
    if is_line_or_space(next) || is_char_literal_delimiter(next) {
        return None;
    }
    let ch = tail[1..].chars().next()?;
    Some(1 + ch.len_utf8())
}

fn is_line_or_space(byte: u8) -> bool {
    byte == b'\n' || byte == b' ' || byte == b'\t'
}

fn is_char_literal_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'?' | b':'
            | b','
            | b';'
            | b')'
            | b']'
            | b'}'
            | b'('
            | b'['
            | b'{'
            | b'|'
            | b'&'
            | b'='
            | b'<'
            | b'>'
            | b'+'
            | b'-'
            | b'*'
            | b'/'
            | b'%'
    )
}

fn match_number(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_word_byte(full_bytes[index - 1])) {
        return None;
    }
    let bytes = tail.as_bytes();
    let len = bytes.len();
    if !bytes[0].is_ascii_digit() {
        return None;
    }

    if len >= 2 && bytes[0] == b'0' {
        match bytes[1] {
            b'x' | b'X' => return match_based_number(bytes, 2, is_hex_number_byte),
            b'b' | b'B' => return match_based_number(bytes, 2, is_binary_number_byte),
            b'o' | b'O' => return match_based_number(bytes, 2, is_octal_number_byte),
            _ => {}
        }
    }

    let mut i = consume_digits_and_underscores(bytes, 0, u8::is_ascii_digit)?;
    if i < len && bytes[i] == b'.' {
        let dot = i;
        if i + 1 < len && bytes[i + 1].is_ascii_digit() {
            i = consume_digits_and_underscores(bytes, i + 1, u8::is_ascii_digit)?;
        } else {
            i = dot;
        }
    }

    if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
        let exponent_start = i;
        i += 1;
        if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        if let Some(exponent_end) = consume_digits_and_underscores(bytes, i, u8::is_ascii_digit) {
            i = exponent_end;
        } else {
            i = exponent_start;
        }
    }

    if i < len && is_word_byte(bytes[i]) {
        return None;
    }
    Some(i)
}

fn match_based_number(
    bytes: &[u8],
    start: usize,
    is_valid_digit: fn(&u8) -> bool,
) -> Option<usize> {
    let end = consume_digits_and_underscores(bytes, start, is_valid_digit)?;
    if end < bytes.len() && is_word_byte(bytes[end]) {
        return None;
    }
    Some(end)
}

fn consume_digits_and_underscores(
    bytes: &[u8],
    start: usize,
    is_valid_digit: fn(&u8) -> bool,
) -> Option<usize> {
    let mut i = start;
    let mut saw_digit = false;
    let mut previous_was_underscore = false;
    while i < bytes.len() && (is_valid_digit(&bytes[i]) || bytes[i] == b'_') {
        if bytes[i] == b'_' {
            if !saw_digit || previous_was_underscore {
                return None;
            }
            previous_was_underscore = true;
        } else {
            saw_digit = true;
            previous_was_underscore = false;
        }
        i += 1;
    }
    if !saw_digit || previous_was_underscore {
        return None;
    }
    Some(i)
}

fn is_hex_number_byte(byte: &u8) -> bool {
    byte.is_ascii_hexdigit()
}

fn is_binary_number_byte(byte: &u8) -> bool {
    *byte == b'0' || *byte == b'1'
}

fn is_octal_number_byte(byte: &u8) -> bool {
    matches!(*byte, b'0'..=b'7')
}

fn match_capitalized_type(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    if tail.is_empty() || (index > 0 && is_word_byte(full_bytes[index - 1])) {
        return None;
    }
    let bytes = tail.as_bytes();
    if !bytes[0].is_ascii_uppercase() {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    if i < bytes.len() && is_word_byte(bytes[i]) {
        return None;
    }
    Some(i)
}

fn match_operator(tail: &str) -> Option<usize> {
    match_operator_from_sets(
        tail,
        &["===", "==", "!=", "<=", ">=", "=>", "||", "&&", "++", "--"],
        b"+-*/%=&|!<>^~?:.",
    )
}
