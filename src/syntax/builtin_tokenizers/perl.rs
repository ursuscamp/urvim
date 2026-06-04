//! Builtin handwritten scanner for Perl syntax.

use std::sync::LazyLock;

use super::scanner::match_two_byte_escape;

use super::scanner::{is_word_byte, match_operator_from_sets, run_while};
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
tag_static!(S_ESCAPE, "string.escape");
tag_static!(S_HEREDOC, "string.heredoc");
tag_static!(P, "punctuation");
tag_static!(NUM, "number");
tag_static!(CNST, "constant");
tag_static!(TYP, "type");
tag_static!(VAR, "variable");
tag_static!(OP, "operator");

const PERL_HEREDOC: ContextId = ContextId::new("perl", "perl_heredoc");
const PERL_HEREDOC_BODY: ContextId = ContextId::new("perl", "perl_heredoc_body");
const PERL_POD: ContextId = ContextId::new("perl", "perl_pod");
const PERL_DOUBLE_STRING: ContextId = ContextId::new("perl", "perl_double_string");
const PERL_SINGLE_STRING: ContextId = ContextId::new("perl", "perl_single_string");

/// Tokenize one line of Perl using the builtin scanner.
pub(crate) fn tokenize_perl_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
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

        // ── Inside heredoc body ──────────────────────────────────────
        if ctx.top_is(PERL_HEREDOC_BODY) {
            let delim = ctx.payload_for(PERL_HEREDOC).unwrap_or("").to_string();

            // Close: ^[ \t]*delimiter[ \t]*$
            if index == 0 && !delim.is_empty() {
                let mut i = 0;
                while i < line_len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                    i += 1;
                }
                if line[i..].starts_with(&delim) {
                    let end = i + delim.len();
                    // Check trailing whitespace to end
                    let mut j = end;
                    while j < line_len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                        j += 1;
                    }
                    if j == line_len {
                        spans.push(SyntaxSpan::new(index, end, (*S_ESCAPE).clone()));
                        ctx.pop_top(PERL_HEREDOC_BODY);
                        ctx.pop_top(PERL_HEREDOC);
                        index = end;
                        continue;
                    }
                }
            }

            // Content: consume entire line
            if index < line_len {
                spans.push(SyntaxSpan::new(index, line_len, (*S_HEREDOC).clone()));
                index = line_len;
                continue;
            }
            break;
        }

        // ── Inside POD comment ───────────────────────────────────────
        if ctx.top_is(PERL_POD) {
            spans.push(SyntaxSpan::new(index, line_len, (*COMMENT).clone()));
            if is_pod_cut(line) {
                ctx.pop_top(PERL_POD);
            }
            index = line_len;
            continue;
        }

        // ── Inside double string ────────────────────────────────────
        if ctx.top_is(PERL_DOUBLE_STRING) {
            if tail_bytes[0] == b'"' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(PERL_DOUBLE_STRING);
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

        // ── Inside single string ────────────────────────────────────
        if ctx.top_is(PERL_SINGLE_STRING) {
            if tail_bytes[0] == b'\'' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*S).clone()));
                ctx.pop_top(PERL_SINGLE_STRING);
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

        // ── Top-level ────────────────────────────────────────────────

        if index == 0 && is_pod_start(line) {
            spans.push(SyntaxSpan::new(index, line_len, (*COMMENT).clone()));
            if !is_pod_cut(line) {
                ctx.push(PERL_POD);
            }
            index = line_len;
            continue;
        }

        if tail_bytes[0] == b'#' {
            let end = line_len;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT).clone()));
            index = end;
            continue;
        }

        // Heredoc open <<IDENT or <<'IDENT' or <<-IDENT
        if let Some(hd_len) = match_heredoc_open(tail) {
            spans.push(SyntaxSpan::new(index, index + hd_len, (*S_ESCAPE).clone()));
            // Extract just the identifier for the payload by scanning the
            // heredoc marker portion of tail for the first word-char sequence.
            let content = &tail[2..]; // skip <<
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
            ctx.push_with_payload(PERL_HEREDOC, &delim);
            ctx.push(PERL_HEREDOC_BODY);
            index += hd_len;
            continue;
        }

        // Double string open "
        if tail_bytes[0] == b'"' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PERL_DOUBLE_STRING);
            index = end;
            continue;
        }

        // Single string open '
        if tail_bytes[0] == b'\'' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(PERL_SINGLE_STRING);
            index = end;
            continue;
        }

        // Quote-like operators q{...} qw(...) qr[...] qx<...>
        if let Some(ql_len) = match_quote_like(tail) {
            spans.push(SyntaxSpan::new(index, index + ql_len, (*S).clone()));
            index += ql_len;
            continue;
        }

        // Regex operators and bare regexes after =~ or !~.
        if let Some(ro_len) = match_regex_op(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + ro_len, (*S).clone()));
            index += ro_len;
            continue;
        }

        // Variable $... @... %...
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
        SyntaxState::Code(CodeState::RuleList {
            contexts: ctx,
            injection: inj,
            parent_style,
        }),
    )
}

fn is_pod_start(line: &str) -> bool {
    if !line.starts_with('=') {
        return false;
    }
    let directive = line[1..]
        .split(|ch: char| ch.is_whitespace())
        .next()
        .unwrap_or("");
    matches!(
        directive,
        "pod"
            | "head1"
            | "head2"
            | "head3"
            | "head4"
            | "over"
            | "item"
            | "back"
            | "begin"
            | "for"
            | "encoding"
            | "cut"
    )
}

fn is_pod_cut(line: &str) -> bool {
    line == "=cut" || line.starts_with("=cut ") || line.starts_with("=cut\t")
}

fn match_heredoc_open(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 4 || bytes[0] != b'<' || bytes[1] != b'<' {
        return None;
    }
    let mut i = 2;
    // Optional -
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
    }
    // \s*
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    // Optional quote
    let mut has_quote = false;
    if i < bytes.len() && (bytes[i] == b'\'' || bytes[i] == b'"') {
        has_quote = true;
        i += 1;
    }
    // Identifier
    if i >= bytes.len() || !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
        return None;
    }
    while i < bytes.len() && is_word_byte(bytes[i]) {
        i += 1;
    }
    // Optional closing quote
    if has_quote && i < bytes.len() && (bytes[i] == b'\'' || bytes[i] == b'"') {
        i += 1;
    }
    // Optional whitespace and ;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b';' {
        i += 1;
    }
    Some(i)
}

fn match_quote_like(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'q' {
        return None;
    }
    let mut i = 1;
    if i < bytes.len() && matches!(bytes[i], b'w' | b'x' | b'r') {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let close = match bytes[i] {
        b'{' => b'}',
        b'(' => b')',
        b'[' => b']',
        b'<' => b'>',
        _ => return None,
    };
    i += 1;
    while i < bytes.len() && bytes[i] != b'\n' {
        if bytes[i] == close {
            return Some(i + 1);
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        i += 1;
    }
    None
}

fn match_regex_op(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();

    if tail.starts_with("tr/") {
        return match_two_part_slash_regex(bytes, 3);
    }
    if tail.starts_with("qr/") {
        return match_one_part_slash_regex(bytes, 3);
    }
    if len >= 2 && bytes[0] == b'm' && bytes[1] == b'/' {
        return match_one_part_slash_regex(bytes, 2);
    }
    if len >= 2 && (bytes[0] == b's' || bytes[0] == b'y') && bytes[1] == b'/' {
        return match_two_part_slash_regex(bytes, 2);
    }
    if !bytes.is_empty() && bytes[0] == b'/' && can_start_bare_regex(index, full_bytes) {
        return match_one_part_slash_regex(bytes, 1);
    }
    None
}

fn match_one_part_slash_regex(bytes: &[u8], mut i: usize) -> Option<usize> {
    i = consume_until_unescaped_slash(bytes, i)? + 1;
    while i < bytes.len() && bytes[i].is_ascii_lowercase() {
        i += 1;
    }
    Some(i)
}

fn match_two_part_slash_regex(bytes: &[u8], mut i: usize) -> Option<usize> {
    i = consume_until_unescaped_slash(bytes, i)? + 1;
    i = consume_until_unescaped_slash(bytes, i)? + 1;
    while i < bytes.len() && bytes[i].is_ascii_lowercase() {
        i += 1;
    }
    Some(i)
}

fn consume_until_unescaped_slash(bytes: &[u8], mut i: usize) -> Option<usize> {
    while i < bytes.len() && bytes[i] != b'\n' {
        if bytes[i] == b'/' {
            return Some(i);
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
        } else {
            i += 1;
        }
    }
    None
}

fn can_start_bare_regex(index: usize, full_bytes: &[u8]) -> bool {
    if index < 2 {
        return false;
    }
    let mut i = index;
    while i > 0 {
        i -= 1;
        if full_bytes[i] == b' ' || full_bytes[i] == b'\t' {
            continue;
        }
        if full_bytes[i] != b'~' {
            return false;
        }
        if i == 0 {
            return false;
        }
        let mut j = i;
        while j > 0 {
            j -= 1;
            if full_bytes[j] == b' ' || full_bytes[j] == b'\t' {
                continue;
            }
            return full_bytes[j] == b'=' || full_bytes[j] == b'!';
        }
        return false;
    }
    false
}

fn match_variable(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    if !matches!(bytes[0], b'$' | b'@' | b'%') {
        return None;
    }
    if bytes.len() < 2 || !(bytes[1].is_ascii_alphabetic() || bytes[1] == b'_') {
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
        "sub", "my", "our", "local", "if", "elsif", "else", "unless", "while", "until", "for",
        "foreach", "continue", "last", "next", "redo", "goto", "return", "package", "use",
        "require", "do", "given", "when", "default", "state", "say",
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
    for word in &["undef", "__FILE__", "__LINE__", "__PACKAGE__"] {
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
        &["===", "==", "!=", "<=", ">=", "=>", "=~", "!~", "++", "--"],
        b"+-*/%=&|!<>^~?:",
    )
}
