//! Builtin handwritten scanner for Rust syntax.

use std::sync::LazyLock;

use super::scanner::run_while;
use crate::buffer::syntax::{
    CodeState, ContextId, ContextStack, SyntaxSpan, SyntaxState, tokenize_injected_body,
};
use crate::theme::Tag;

macro_rules! tag_static {
    ($name:ident, $s:expr) => {
        static $name: LazyLock<Tag> = LazyLock::new(|| Tag::parse($s).unwrap());
    };
}

tag_static!(KW, "keyword");
tag_static!(STR, "string");
tag_static!(STR_ESCAPE, "string.escape");
tag_static!(VAR, "variable");
tag_static!(VAR_GLOBAL, "variable.global");
tag_static!(TYP, "type");
tag_static!(NUM, "number");
tag_static!(OP, "operator");
tag_static!(PUNCT, "punctuation");
tag_static!(FN, "function");
tag_static!(FN_MACRO, "function.macro");
tag_static!(NS, "namespace");
tag_static!(CONST, "constant");
tag_static!(COMMENT_BLOCK, "comment.block");
tag_static!(COMMENT_DOC, "comment.documentation");
tag_static!(COMMENT_LINE, "comment.line");

const RUST_DOC_COMMENT: ContextId = ContextId::new("rust", "rust_doc_comment");
const RUST_BLOCK_COMMENT: ContextId = ContextId::new("rust", "rust_block_comment");
const FORMAT_CALL: ContextId = ContextId::new("rust", "format_call");
const RUST_RAW_STRING: ContextId = ContextId::new("rust", "rust_raw_string");
const RUST_RAW_STRING_BODY: ContextId = ContextId::new("rust", "rust_raw_string_body");
const RUST_STRING: ContextId = ContextId::new("rust", "rust_string");
const RUST_BYTE_CHAR: ContextId = ContextId::new("rust", "rust_byte_char");
const FORMAT_STRING: ContextId = ContextId::new("rust", "format_string");
const FORMAT_EXPR: ContextId = ContextId::new("rust", "format_expr");

pub(crate) fn tokenize_rust_line(line: &str, state: SyntaxState) -> (Vec<SyntaxSpan>, SyntaxState) {
    let (mut ctx, mut inj, parent_style) = match state {
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

    while index < line.len() {
        let tail = &line[index..];

        // ── Injection mode (format_expr inside format!("...")) ──────
        if let Some(ref mut inj_state) = inj {
            let boundary = scan_rust_expr_boundary(line, index, &ctx, &spans, tail);
            match boundary {
                Some(b) if b > index => {
                    spans.extend(tokenize_injected_body(inj_state, &line[index..b], index));
                    index = b;
                    continue;
                }
                Some(_) => {}
                None => {
                    spans.extend(tokenize_injected_body(inj_state, tail, index));
                    index = line.len();
                    continue;
                }
            }
        }

        // ── 1-3. Single-line comments ────────────────────────────────
        if tail.starts_with("///") {
            let end = line.len();
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_DOC).clone()));
            index = end;
            continue;
        }
        if tail.starts_with("//!") {
            let end = line.len();
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_DOC).clone()));
            index = end;
            continue;
        }
        if tail.starts_with("//") {
            let end = line.len();
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_LINE).clone()));
            index = end;
            continue;
        }

        // ── 4-7. Doc comments /** ... */ ─────────────────────────────
        if ctx.top_is(RUST_DOC_COMMENT) {
            if tail.starts_with("*/") {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_DOC).clone()));
                ctx.pop_top(RUST_DOC_COMMENT);
                index = end;
                continue;
            }
            // [^*\n]+ content
            let run = run_while(tail, |c| c != '*' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*COMMENT_DOC).clone()));
                index += run;
                continue;
            }
            // single *
            if tail.starts_with('*') {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_DOC).clone()));
                index = end;
                continue;
            }
        }
        if tail.starts_with("/**") {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_DOC).clone()));
            ctx.push(RUST_DOC_COMMENT);
            index = end;
            continue;
        }

        // ── 8-11. Block comments / * ... */ ──────────────────────────
        if ctx.top_is(RUST_BLOCK_COMMENT) {
            if tail.starts_with("*/") {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.pop_top(RUST_BLOCK_COMMENT);
                index = end;
                continue;
            }
            if tail.starts_with("/*") {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                ctx.push(RUST_BLOCK_COMMENT);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '*' && c != '/' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(
                    index,
                    index + run,
                    (*COMMENT_BLOCK).clone(),
                ));
                index += run;
                continue;
            }
            if tail.starts_with('*') {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                index = end;
                continue;
            }
            if tail.starts_with('/') {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
                index = end;
                continue;
            }
        }
        if tail.starts_with("/*") {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*COMMENT_BLOCK).clone()));
            ctx.push(RUST_BLOCK_COMMENT);
            index = end;
            continue;
        }

        // ── 12-14. Macro and format_call context ────────────────────
        // \b[A-Za-z_][A-Za-z0-9_]*!  → function.macro + format_call
        if !ctx.top_is(RUST_STRING)
            && !ctx.top_is(RUST_RAW_STRING_BODY)
            && let Some((name, after)) = match_ident(tail)
            && after.starts_with('!')
        {
            let end = index + name.len() + 1;
            spans.push(SyntaxSpan::new(index, end, (*FN_MACRO).clone()));
            ctx.push(FORMAT_CALL);
            index = end;
            continue;
        }
        // ( inside format_call
        if ctx.top_is(FORMAT_CALL) && tail.starts_with('(') {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*PUNCT).clone()));
            index = end;
            continue;
        }
        // ) inside format_call
        if ctx.top_is(FORMAT_CALL) && tail.starts_with(')') {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*PUNCT).clone()));
            ctx.pop_top(FORMAT_CALL);
            index = end;
            continue;
        }

        // ── 15-18. br#"..."# raw byte strings ───────────────────────
        if let Some(rest) = tail.strip_prefix("br") {
            let hash_len = rest.bytes().take_while(|&b| b == b'#').count();
            let hashes = &rest[..hash_len];
            if rest[hash_len..].starts_with('"') {
                let total = 3 + hash_len;
                spans.push(SyntaxSpan::new(index, index + total, (*STR).clone()));
                ctx.push_with_payload(RUST_RAW_STRING, hashes);
                ctx.push(RUST_RAW_STRING_BODY);
                index += total;
                continue;
            }
        }

        // 16. " with payload match (close raw string)
        if ctx.top_is(RUST_RAW_STRING_BODY) {
            let payload = ctx.payload_for(RUST_RAW_STRING).unwrap_or("");
            if tail.starts_with('"') && tail[1..].starts_with(payload) {
                let end = index + 1 + payload.len();
                spans.push(SyntaxSpan::new(index, end, (*STR).clone()));
                ctx.pop_top(RUST_RAW_STRING_BODY);
                ctx.pop_top(RUST_RAW_STRING);
                index = end;
                continue;
            }
        }

        // 17-18. Raw string body content
        if ctx.top_is(RUST_RAW_STRING_BODY) {
            let run = run_while(tail, |c| c != '"');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*STR).clone()));
                index += run;
                continue;
            }
            if tail.starts_with('"') {
                // lone " inside raw string body (not closing)
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*STR).clone()));
                index = end;
                continue;
            }
        }

        // ── 19-22. r#"..."# raw strings ─────────────────────────────
        if let Some(rest) = tail.strip_prefix('r') {
            let hash_len = rest.bytes().take_while(|&b| b == b'#').count();
            let hashes = &rest[..hash_len];
            if rest[hash_len..].starts_with('"') {
                let total = 2 + hash_len;
                spans.push(SyntaxSpan::new(index, index + total, (*STR).clone()));
                ctx.push_with_payload(RUST_RAW_STRING, hashes);
                ctx.push(RUST_RAW_STRING_BODY);
                index += total;
                continue;
            }
        }

        // ── 23-26. b"..." byte strings ───────────────────────────────
        if ctx.top_is(RUST_STRING) {
            if tail.starts_with('"') {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*STR).clone()));
                ctx.pop_top(RUST_STRING);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*STR).clone()));
                index += run;
                continue;
            }
            if tail.starts_with('\\') {
                let end = (index + 2).min(line.len());
                spans.push(SyntaxSpan::new(index, end, (*STR_ESCAPE).clone()));
                index = end;
                continue;
            }
        }
        if tail.starts_with("b\"") {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*STR).clone()));
            ctx.push(RUST_STRING);
            index = end;
            continue;
        }

        // ── 27-28. Complete char/byte literals ──────────────────────
        if let Some(rest) = tail.strip_prefix("b'") {
            // b'(?:[^'\\\n]|\\.)'
            if let Some(end) = try_parse_char_lit(rest) {
                let total = 2 + end;
                spans.push(SyntaxSpan::new(index, index + total, (*CONST).clone()));
                index += total;
                continue;
            }
        }
        if tail.starts_with('\'')
            && !ctx.top_is(RUST_STRING)
            && !ctx.top_is(RUST_RAW_STRING_BODY)
            && !ctx.top_is(FORMAT_STRING)
        {
            // '(?:[^'\\\n]|\\.)'
            let rest = &tail[1..];
            if let Some(end) = try_parse_char_lit(rest) {
                let total = 1 + end;
                spans.push(SyntaxSpan::new(index, index + total, (*CONST).clone()));
                index += total;
                continue;
            }
        }

        // ── 29-32. b'...' byte char (multi-line, context) ───────────
        if ctx.top_is(RUST_BYTE_CHAR) {
            if tail.starts_with('\'') {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*CONST).clone()));
                ctx.pop_top(RUST_BYTE_CHAR);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '\'' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*CONST).clone()));
                index += run;
                continue;
            }
            if tail.starts_with('\\') {
                let end = (index + 2).min(line.len());
                spans.push(SyntaxSpan::new(index, end, (*CONST).clone()));
                index = end;
                continue;
            }
        }
        if tail.starts_with("b'") {
            // Opening byte char – push context for multi-line
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*CONST).clone()));
            ctx.push(RUST_BYTE_CHAR);
            index = end;
            continue;
        }

        // ── 33. \' escaped single quote ─────────────────────────────
        if tail.starts_with("\\'") {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*CONST).clone()));
            index = end;
            continue;
        }

        // ── 34-35. Lifetimes – emit "'" as constant, let ident fall through ──
        if tail.starts_with('\'')
            && !ctx.top_is(RUST_STRING)
            && !ctx.top_is(RUST_RAW_STRING_BODY)
            && !ctx.top_is(FORMAT_STRING)
        {
            let rest = &tail[1..];
            if let Some((name_word, after)) = match_ident(rest)
                && !name_word.is_empty()
            {
                let after_char = after.chars().next();
                let is_lifetime = after_char.is_none_or(|c| {
                    c == ':' || c.is_whitespace() || c == ',' || c == ')' || c == '>' || c == ';'
                });
                if is_lifetime {
                    // Regex engine matches ' as constant, then ident as variable
                    spans.push(SyntaxSpan::new(index, index + 1, (*CONST).clone()));
                    index += 1;
                    continue;
                }
            }
        }

        // ── 36. Attributes #![ or #[ ────────────────────────────────
        if tail.starts_with("#![") || tail.starts_with("#[") {
            let len = if tail.starts_with("#![") { 3 } else { 2 };
            let end = index + len;
            spans.push(SyntaxSpan::new(index, end, (*PUNCT).clone()));
            index = end;
            continue;
        }

        // ── 37-40. Numbers ──────────────────────────────────────────
        // Must check before identifiers so 0xff doesn't match as ident
        if let Some(num_len) = match_number(tail) {
            spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
            index += num_len;
            continue;
        }

        // ── 41. Keywords ─────────────────────────────────────────────
        if let Some((word, _)) = match_ident(tail)
            && is_keyword(word)
        {
            let end = index + word.len();
            spans.push(SyntaxSpan::new(index, end, (*KW).clone()));
            index = end;
            continue;
        }

        // ── 42. None/Some/Ok/Err constants ──────────────────────────
        if let Some((word, _)) = match_ident(tail)
            && matches!(word, "None" | "Some" | "Ok" | "Err")
        {
            let end = index + word.len();
            spans.push(SyntaxSpan::new(index, end, (*CONST).clone()));
            index = end;
            continue;
        }

        // ── 43. Format string body [^"\\{}\n]+ ──────────────────────
        if ctx.top_is(FORMAT_STRING) {
            let run = run_while(tail, |c| {
                c != '"' && c != '\\' && c != '{' && c != '}' && c != '\n'
            });
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*STR).clone()));
                index += run;
                continue;
            }
        }

        // ── 44. UPPER_CASE globals ──────────────────────────────────
        if let Some((word, _)) = match_upper_ident(tail)
            && word
                .chars()
                .all(|c| c.is_uppercase() || c == '_' || c.is_ascii_digit())
        {
            let end = index + word.len();
            spans.push(SyntaxSpan::new(index, end, (*VAR_GLOBAL).clone()));
            index = end;
            continue;
        }

        // ── 45. Type names (capitalized, known types) ────────────────
        if let Some((word, _)) = match_ident(tail)
            && is_upper_start(word)
        {
            let end = index + word.len();
            spans.push(SyntaxSpan::new(index, end, (*TYP).clone()));
            index = end;
            continue;
        }

        // ── 46. Namespace (ident ::) ────────────────────────────────
        if !ctx.top_is(RUST_STRING)
            && !ctx.top_is(RUST_RAW_STRING_BODY)
            && let Some((word, after)) = match_ident(tail)
            && after.trim_start().starts_with("::")
        {
            let end = index + word.len();
            spans.push(SyntaxSpan::new(index, end, (*NS).clone()));
            index = end;
            continue;
        }

        // ── 47. Function calls (ident ( ────────────────────────────
        if !ctx.top_is(RUST_STRING)
            && !ctx.top_is(RUST_RAW_STRING_BODY)
            && !ctx.top_is(FORMAT_STRING)
            && !ctx.top_is(FORMAT_CALL)
            && let Some((word, after)) = match_ident(tail)
            && after.trim_start().starts_with('(')
            && !matches!(word, "if" | "while" | "for" | "match" | "unsafe")
        {
            let end = index + word.len();
            spans.push(SyntaxSpan::new(index, end, (*FN).clone()));
            index = end;
            continue;
        }

        // ── 48. Variable.global with : lookahead ────────────────────
        if !ctx.top_is(RUST_STRING)
            && !ctx.top_is(RUST_RAW_STRING_BODY)
            && let Some((word, after)) = match_ident(tail)
            && after.trim_start().starts_with(':')
            && !after.trim_start().starts_with("::")
        {
            let end = index + word.len();
            spans.push(SyntaxSpan::new(index, end, (*VAR_GLOBAL).clone()));
            index = end;
            continue;
        }

        // ── 49-52. Format string {{ }}, { } ─────────────────────────
        if ctx.top_is(FORMAT_STRING) {
            if tail.starts_with("{{") || tail.starts_with("}}") {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*STR_ESCAPE).clone()));
                index = end;
                continue;
            }
            if tail.starts_with('{') {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*PUNCT).clone()));
                ctx.pop_top(FORMAT_STRING);
                ctx.push(FORMAT_EXPR);
                index = end;
                continue;
            }
        }

        // 53. } inside format_expr (back to format_string)
        if ctx.top_is(FORMAT_EXPR) && tail.starts_with('}') {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*PUNCT).clone()));
            ctx.pop_top(FORMAT_EXPR);
            ctx.push(FORMAT_STRING);
            index = end;
            continue;
        }

        // ── 54. Variable names (lowercase or r# prefix) ─────────────
        if !ctx.top_is(FORMAT_STRING)
            && let Some((word, _)) = match_ident(tail)
            && (word.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
                || word.starts_with("r#"))
            && !is_keyword(word)
        {
            let end = index + word.len();
            spans.push(SyntaxSpan::new(index, end, (*VAR).clone()));
            index = end;
            continue;
        }

        // ── 55. Punctuation ()[] , . ; : (each char individually) ────
        if tail.starts_with('(')
            || tail.starts_with(')')
            || tail.starts_with('[')
            || tail.starts_with(']')
            || tail.starts_with(',')
            || tail.starts_with('.')
            || tail.starts_with(';')
            || tail.starts_with(':')
        {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*PUNCT).clone()));
            index = end;
            continue;
        }

        // ── 56. Braces {} ────────────────────────────────────────────
        if (tail.starts_with('{') || tail.starts_with('}'))
            && !ctx.top_is(FORMAT_STRING)
            && !ctx.top_is(FORMAT_EXPR)
        {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*PUNCT).clone()));
            index = end;
            continue;
        }

        // ── 57. Operators ───────────────────────────────────────────
        if let Some(op_len) = match_operator(tail) {
            let end = index + op_len;
            spans.push(SyntaxSpan::new(index, end, (*OP).clone()));
            index = end;
            continue;
        }

        // ── 58. " inside format_call → switch to format_string ──────
        if ctx.top_is(FORMAT_CALL) && tail.starts_with('"') {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*STR).clone()));
            ctx.pop_top(FORMAT_CALL);
            ctx.push(FORMAT_STRING);
            index = end;
            continue;
        }

        // ── 59. " inside format_string → close ─────────────────────
        if ctx.top_is(FORMAT_STRING) && tail.starts_with('"') {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*STR).clone()));
            ctx.pop_top(FORMAT_STRING);
            index = end;
            continue;
        }

        // ── 60-61. Numbers in format strings ────────────────────────
        if ctx.top_is(FORMAT_STRING) {
            if let Some(num_len) = match_number(tail) {
                spans.push(SyntaxSpan::new(index, index + num_len, (*NUM).clone()));
                index += num_len;
                continue;
            }
            // :NN width
            if let Some(rest) = tail.strip_prefix(':') {
                let digits = run_while(rest, |c| c.is_ascii_digit());
                if digits > 0 {
                    let end = index + 1 + digits;
                    spans.push(SyntaxSpan::new(index, end, (*NUM).clone()));
                    index = end;
                    continue;
                }
            }
        }

        // ── 62. General \. escapes ──────────────────────────────────
        if tail.starts_with('\\')
            && !ctx.top_is(RUST_STRING)
            && !ctx.top_is(RUST_RAW_STRING_BODY)
            && !ctx.top_is(FORMAT_STRING)
        {
            let end = (index + 2).min(line.len());
            spans.push(SyntaxSpan::new(index, end, (*STR_ESCAPE).clone()));
            index = end;
            continue;
        }

        // ── 63-66. Standard "..." strings ───────────────────────────
        if ctx.top_is(RUST_STRING) {
            // Already handled above in b" section. Catch " close here for regular strings too.
            if tail.starts_with('"') {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*STR).clone()));
                ctx.pop_top(RUST_STRING);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '"' && c != '\\' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*STR).clone()));
                index += run;
                continue;
            }
            if tail.starts_with('\\') {
                let end = (index + 2).min(line.len());
                spans.push(SyntaxSpan::new(index, end, (*STR_ESCAPE).clone()));
                index = end;
                continue;
            }
        }
        if tail.starts_with('"')
            && !ctx.top_is(RUST_STRING)
            && !ctx.top_is(RUST_RAW_STRING_BODY)
            && !ctx.top_is(FORMAT_STRING)
        {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*STR).clone()));
            ctx.push(RUST_STRING);
            index = end;
            continue;
        }

        // ── 54b. Variable (also hit non-keyword idents) ─────────────
        if let Some((word, _)) = match_ident(tail)
            && !is_keyword(word)
        {
            let end = index + word.len();
            spans.push(SyntaxSpan::new(index, end, (*VAR).clone()));
            index = end;
            continue;
        }

        // ── Fallback: skip one char ─────────────────────────────────
        let Some(ch) = tail.chars().next() else { break };
        index += ch.len_utf8();
    }

    // Clear stale injection
    if inj.is_some() && !ctx.top_is(FORMAT_EXPR) {
        inj = None;
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

fn match_ident(tail: &str) -> Option<(&str, &str)> {
    let first = tail.chars().next()?;
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }
    let len = run_while(tail, |c| c.is_ascii_alphanumeric() || c == '_');
    if len == 0 {
        return None;
    }
    Some((&tail[..len], &tail[len..]))
}

fn match_upper_ident(tail: &str) -> Option<(&str, &str)> {
    let first = tail.chars().next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    let len = run_while(tail, |c| c.is_ascii_alphanumeric() || c == '_');
    if len == 0 {
        return None;
    }
    Some((&tail[..len], &tail[len..]))
}

fn is_upper_start(word: &str) -> bool {
    word.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

fn match_number(tail: &str) -> Option<usize> {
    // Binary: 0b...
    if tail.starts_with("0b") || tail.starts_with("0B") {
        let body = run_while(&tail[2..], |c| c == '0' || c == '1' || c == '_');
        if body == 0 {
            return None;
        }
        let mut end = 2 + body;
        // Optional suffix
        let suffix = match_suffix(&tail[end..]);
        end += suffix;
        // Check word boundary
        if tail
            .as_bytes()
            .get(end)
            .is_some_and(|b| b.is_ascii_alphanumeric())
        {
            return None;
        }
        return Some(end);
    }
    // Octal: 0o...
    if tail.starts_with("0o") || tail.starts_with("0O") {
        let body = run_while(&tail[2..], |c| matches!(c, '0'..='7' | '_'));
        if body == 0 {
            return None;
        }
        let mut end = 2 + body;
        let suffix = match_suffix(&tail[end..]);
        end += suffix;
        if end < tail.len() && tail.as_bytes()[end].is_ascii_alphanumeric() {
            return None;
        }
        return Some(end);
    }
    // Hex: 0x...
    if let Some(rest) = tail.strip_prefix("0x").or_else(|| tail.strip_prefix("0X")) {
        let body = run_while(rest, |c| c.is_ascii_hexdigit() || c == '_');
        if body == 0 {
            return None;
        }
        let mut end = 2 + body;
        let suffix = match_suffix(&tail[end..]);
        end += suffix;
        if end < tail.len() && tail.as_bytes()[end].is_ascii_alphanumeric() {
            return None;
        }
        return Some(end);
    }
    // Decimal/integer
    // Integer part
    if tail.as_bytes().first().is_some_and(|b| b.is_ascii_digit()) {
        let mut end = run_while(tail, |c| c.is_ascii_digit() || c == '_');
        // Optional fractional part
        if tail[end..].starts_with('.')
            && tail
                .as_bytes()
                .get(end + 1)
                .is_some_and(|b| b.is_ascii_digit())
        {
            end += 1;
            end += run_while(&tail[end..], |c| c.is_ascii_digit() || c == '_');
        }
        // Optional exponent
        if tail[end..].starts_with('e') || tail[end..].starts_with('E') {
            end += 1;
            if tail[end..].starts_with('+') || tail[end..].starts_with('-') {
                end += 1;
            }
            let exp_digits = run_while(&tail[end..], |c| c.is_ascii_digit() || c == '_');
            if exp_digits == 0 {
                return None;
            }
            end += exp_digits;
        }
        // Required: integer must have at least one digit
        let has_integer = tail[..end].chars().any(|c| c.is_ascii_digit());
        if !has_integer {
            return None;
        }
        let suffix = match_suffix(&tail[end..]);
        end += suffix;
        if tail
            .as_bytes()
            .get(end)
            .is_some_and(|b| b.is_ascii_alphanumeric())
        {
            return None;
        }
        if end > 0 {
            return Some(end);
        }
    }
    // Leading dot like .5
    if tail.starts_with('.') && tail.len() > 1 && tail.as_bytes()[1].is_ascii_digit() {
        let mut end = 1 + run_while(&tail[1..], |c| c.is_ascii_digit() || c == '_');
        if tail[end..].starts_with('e') || tail[end..].starts_with('E') {
            end += 1;
            if tail[end..].starts_with('+') || tail[end..].starts_with('-') {
                end += 1;
            }
            let exp = run_while(&tail[end..], |c| c.is_ascii_digit() || c == '_');
            if exp == 0 {
                return None;
            }
            end += exp;
        }
        let suffix = match_suffix(&tail[end..]);
        end += suffix;
        if end < tail.len() && tail.as_bytes()[end].is_ascii_alphanumeric() {
            return None;
        }
        return Some(end);
    }
    None
}

fn match_suffix(tail: &str) -> usize {
    let suffixes = [
        "f32", "f64", "usize", "isize", "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32",
        "i64", "i128",
    ];
    for s in suffixes {
        if let Some(after) = tail.strip_prefix(s)
            && (after.is_empty()
                || !after
                    .as_bytes()
                    .first()
                    .is_some_and(|b| b.is_ascii_alphanumeric()))
        {
            return s.len();
        }
    }
    // _? prefix variation
    if let Some(rest) = tail.strip_prefix('_') {
        for s in suffixes {
            if let Some(after) = rest.strip_prefix(s)
                && (after.is_empty()
                    || !after
                        .as_bytes()
                        .first()
                        .is_some_and(|b| b.is_ascii_alphanumeric()))
            {
                return 1 + s.len();
            }
        }
    }
    0
}

fn match_operator(tail: &str) -> Option<usize> {
    let ops = ["=>", "->", "==", "!=", "<=", ">=", "&&", "||"];
    for &op in &ops {
        if tail.starts_with(op) {
            return Some(op.len());
        }
    }
    let single = [
        '+', '-', '*', '/', '%', '=', '&', '|', '!', '<', '>', '^', '?',
    ];
    let first = tail.chars().next()?;
    if single.contains(&first) {
        Some(first.len_utf8())
    } else {
        None
    }
}

fn try_parse_char_lit(rest: &str) -> Option<usize> {
    if rest.is_empty() {
        return None;
    }
    if rest.starts_with('\\') {
        // Escape sequence: must be followed by a char and then closing '
        if rest.len() >= 3 && rest.as_bytes()[2] == b'\'' {
            Some(3)
        } else {
            None
        }
    } else if rest.starts_with('\'') {
        None // empty char
    } else {
        let ch = rest.chars().next()?;
        if ch == '\n' {
            return None;
        }
        if rest.len() > ch.len_utf8() && rest.as_bytes()[ch.len_utf8()] == b'\'' {
            Some(ch.len_utf8() + 1)
        } else {
            None
        }
    }
}

/// Scan forward for the next Rust pattern that should end a format_expr injection.
fn scan_rust_expr_boundary(
    line: &str,
    start: usize,
    ctx: &ContextStack,
    _spans: &[SyntaxSpan],
    _tail: &str,
) -> Option<usize> {
    if !ctx.top_is(FORMAT_EXPR) {
        return None;
    }
    let tail = &line[start..];
    // Look for } that ends the format expression
    // Also check for any pattern that could act as boundary
    let mut depth = 0u32;
    for (i, ch) in tail.char_indices() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            if depth == 0 {
                return Some(start + i);
            }
            depth -= 1;
        }
    }
    None
}
