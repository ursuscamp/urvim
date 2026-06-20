//! Builtin handwritten scanner for PHP syntax.

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

tag_static!(COMMENT_LINE, "comment.line");
tag_static!(COMMENT_BLOCK, "comment.block");
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

const PHP_COMMENT: ContextId = ContextId::new("php", "php_comment");
const PHP_HEREDOC: ContextId = ContextId::new("php", "php_heredoc");
const PHP_HEREDOC_BODY: ContextId = ContextId::new("php", "php_heredoc_body");
const PHP_TRIPLE_DOUBLE: ContextId = ContextId::new("php", "php_triple_double");
const PHP_DOUBLE: ContextId = ContextId::new("php", "php_double");
const PHP_SINGLE: ContextId = ContextId::new("php", "php_single");

/// Tokenize one line of PHP using the builtin scanner.
pub(crate) fn tokenize_php_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Inside block comment ─────────────────────────────────────
        if ctx.top_is(PHP_COMMENT) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'/' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(PHP_COMMENT);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '*' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(
                    index,
                    index + run,
                    (*COMMENT_BLOCK).clone(),
                ));
                index += run;
                continue;
            }
            if tail_bytes[0] == b'*' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                index = end;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inside heredoc body ──────────────────────────────────────
        if ctx.top_is(PHP_HEREDOC_BODY) {
            let delim = ctx.payload_for(PHP_HEREDOC).unwrap_or("").to_string();

            // Check heredoc close at position 0:
            // ^[ \t]*delimiter (word-bounded)
            if index == 0 && !delim.is_empty() {
                let mut i = 0;
                while i < line_len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                    i += 1;
                }
                if line[i..].starts_with(&delim) {
                    let end = i + delim.len();
                    if end >= line_len || !is_word_byte(bytes[end]) {
                        spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                        ctx.pop_top(PHP_HEREDOC_BODY);
                        ctx.pop_top(PHP_HEREDOC);
                        index = end;
                        continue;
                    }
                }
            }

            // Content: consume remaining line (rule 9: .+)
            if index < line_len {
                spans.push(SyntaxSpan::new(index, line_len, (*S_HEREDOC).clone()));
                index = line_len;
                continue;
            }
            break;
        }

        // ── Inside triple double string ──────────────────────────────
        if ctx.top_is(PHP_TRIPLE_DOUBLE) {
            // Closing """
            if tail_bytes.len() >= 3
                && tail_bytes[0] == b'"'
                && tail_bytes[1] == b'"'
                && tail_bytes[2] == b'"'
            {
                let end = index + 3;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(PHP_TRIPLE_DOUBLE);
                index = end;
                continue;
            }
            // Content [^"\n]+
            let run = run_while(tail, |c| c != '"' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
            // Single " inside triple
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
        if ctx.top_is(PHP_DOUBLE) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(PHP_DOUBLE);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
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
        if ctx.top_is(PHP_SINGLE) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(PHP_SINGLE);
                index = end;
                continue;
            }
            if let Some(escape_len) = match_two_byte_escape(tail) {
                let end = index + escape_len;
                spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
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

        // ── Top-level ────────────────────────────────────────────────

        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'/' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        if tail_bytes.len() >= 2 && tail_bytes[0] == b'/' && tail_bytes[1] == b'*' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(PHP_COMMENT);
            index = end;
            continue;
        }

        // Heredoc open <<<IDENT
        if tail_bytes.len() >= 4
            && tail_bytes[0] == b'<'
            && tail_bytes[1] == b'<'
            && tail_bytes[2] == b'<'
        {
            let after = &tail[3..];
            let ws_count = after
                .bytes()
                .take_while(|b| *b == b' ' || *b == b'\t')
                .count();
            let after_ws = &after[ws_count..];
            if !after_ws.is_empty() {
                let mut delim_end = 0;
                let ab = after_ws.as_bytes();
                let mut has_quote = false;
                // Optional opening quote
                if ab[0] == b'\'' || ab[0] == b'"' {
                    has_quote = true;
                    delim_end = 1;
                }
                if delim_end < ab.len()
                    && (ab[delim_end].is_ascii_alphabetic() || ab[delim_end] == b'_')
                {
                    let id_start = delim_end;
                    delim_end += 1;
                    while delim_end < ab.len() && is_word_byte(ab[delim_end]) {
                        delim_end += 1;
                    }
                    if has_quote
                        && delim_end < ab.len()
                        && (ab[delim_end] == b'\'' || ab[delim_end] == b'"')
                    {
                        delim_end += 1;
                    }
                    let hd_len = 3 + ws_count + delim_end;
                    let hd_delim = &after_ws[id_start..delim_end - if has_quote { 1 } else { 0 }];
                    let hd_delim = hd_delim.trim_matches(|c| c == '\'' || c == '"').to_string();
                    spans.push(SyntaxSpan::new(index, index + hd_len, (*S_ESCAPE).clone()));
                    ctx.push_with_payload(PHP_HEREDOC, &hd_delim);
                    ctx.push(PHP_HEREDOC_BODY);
                    index += hd_len;
                    continue;
                }
            }
        }

        // Triple double open """
        if tail_bytes.len() >= 3
            && tail_bytes[0] == b'"'
            && tail_bytes[1] == b'"'
            && tail_bytes[2] == b'"'
        {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PHP_TRIPLE_DOUBLE);
            index = end;
            continue;
        }

        // Double string open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PHP_DOUBLE);
            index = end;
            continue;
        }

        // Single string open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PHP_SINGLE);
            index = end;
            continue;
        }

        // Variable $...
        if let Some(var_len) = match_variable(tail) {
            spans.push(SyntaxSpan::new(index, index + var_len, (*VAR).clone()));
            index += var_len;
            continue;
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

        // Number
        if let Some(len) = match_number(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*NUM).clone()));
            index += len;
            continue;
        }

        // Capitalized type
        if let Some(len) = match_capitalized_type(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*TYP).clone()));
            index += len;
            continue;
        }

        // Punctuation
        if matches!(
            tail_bytes[0],
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b',' | b'.' | b';' | b':'
        ) {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
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

fn is_word_start_byte(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn match_variable(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.is_empty() || bytes[0] != b'$' {
        return None;
    }
    if bytes.len() < 2 || !is_word_start_byte(bytes[1]) {
        return None;
    }
    let mut i = 2;
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
        "abstract",
        "and",
        "array",
        "as",
        "break",
        "callable",
        "case",
        "catch",
        "class",
        "clone",
        "const",
        "continue",
        "declare",
        "default",
        "die",
        "do",
        "echo",
        "else",
        "elseif",
        "empty",
        "enddeclare",
        "endfor",
        "endforeach",
        "endif",
        "endswitch",
        "endwhile",
        "eval",
        "exit",
        "extends",
        "final",
        "finally",
        "fn",
        "for",
        "foreach",
        "function",
        "global",
        "goto",
        "if",
        "implements",
        "include",
        "include_once",
        "instanceof",
        "insteadof",
        "interface",
        "match",
        "namespace",
        "new",
        "or",
        "print",
        "private",
        "protected",
        "public",
        "require",
        "require_once",
        "return",
        "static",
        "switch",
        "throw",
        "trait",
        "try",
        "use",
        "var",
        "while",
        "xor",
        "yield",
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
        "bool",
        "callable",
        "class-string",
        "false",
        "float",
        "int",
        "iterable",
        "mixed",
        "never",
        "null",
        "object",
        "parent",
        "self",
        "string",
        "true",
        "void",
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
    for word in &["false", "null", "true"] {
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
        &["===", "!==", "==", "!=", "<=", ">=", "=>", "++", "--"],
        b"+-*/%=&|!<>^~?.",
    )
}
