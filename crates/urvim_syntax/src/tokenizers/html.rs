//! Builtin handwritten scanner for HTML syntax.

use std::sync::LazyLock;

use super::scanner::run_while;
use crate::state::{
    CodeState, ContextId, ContextStack, InjectedSyntaxFallback, NestedState, SyntaxFoldEvent,
    SyntaxFoldEventKind, SyntaxFoldKind, SyntaxLineResult, SyntaxSpan, SyntaxState,
    TokenizerInjectionState, syntax_definition_by_name, tokenize_injected_body,
};
use urvim_theme::Tag;

macro_rules! tag_static {
    ($name:ident, $s:expr) => {
        static $name: LazyLock<Tag> = LazyLock::new(|| Tag::parse($s).unwrap());
    };
}

tag_static!(C, "comment");
tag_static!(K, "keyword");
tag_static!(CNST, "constant");
tag_static!(P, "punctuation");
tag_static!(MT, "markup.tag");
tag_static!(VP, "variable.property");
tag_static!(S, "string");

const HTML_COMMENT: ContextId = ContextId::new("html", "html_comment");
const INSIDE_TAG: ContextId = ContextId::new("html", "inside_tag");
const TAG_NAME_EXPECTED: ContextId = ContextId::new("html", "tag_name_expected");
const HTML_DOUBLE_STRING: ContextId = ContextId::new("html", "html_double_string");
const HTML_SINGLE_STRING: ContextId = ContextId::new("html", "html_single_string");
const SCRIPT_HOST: ContextId = ContextId::new("html", "script_host");
const STYLE_HOST: ContextId = ContextId::new("html", "style_host");
const HTML_PENDING_TAG_FOLD_KIND: &str = "html.pending_tag_fold_kind";
const HTML_TAG_FOLD_KIND_BASE: SyntaxFoldKind = 1024;

/// Tokenize one line of HTML using the builtin scanner.
pub(crate) fn tokenize_html_line(line: &str, state: SyntaxState) -> SyntaxLineResult {
    let (mut ctx, mut inj, parent_style, mut tokenizer_state) = match state {
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

        // ── Injection mode ───────────────────────────────────────────
        if let Some(ref mut inj_state) = inj {
            let boundary = closing_tag_pos(line, index, &ctx);
            match boundary {
                Some(b) if b > index => {
                    spans.extend(tokenize_injected_body(inj_state, &line[index..b], index));
                    index = b;
                    continue;
                }
                Some(_) => {} // boundary right here – fall through
                None => {
                    spans.extend(tokenize_injected_body(inj_state, tail, index));
                    index = line.len();
                    continue;
                }
            }
        }

        // ── HTML patterns, in TOML rule order ────────────────────────

        // 1. <!--  (comment open)
        if tail.starts_with("<!--") {
            let end = index + 4;
            spans.push(SyntaxSpan::new(index, end, (*C).clone()));
            ctx.push(HTML_COMMENT);
            index = end;
            continue;
        }

        if ctx.top_is(HTML_COMMENT) {
            if let Some(close) = tail.find("-->") {
                if close > 0 {
                    let end = index + close;
                    spans.push(SyntaxSpan::new(index, end, (*C).clone()));
                    index = end;
                    continue;
                }
            } else {
                spans.push(SyntaxSpan::new(index, line.len(), (*C).clone()));
                index = line.len();
                continue;
            }
        }

        // 2. -->  (comment close, requires html_comment)
        if ctx.top_is(HTML_COMMENT) && tail.starts_with("-->") {
            let end = index + 3;
            spans.push(SyntaxSpan::new(index, end, (*C).clone()));
            ctx.pop_top(HTML_COMMENT);
            index = end;
            continue;
        }

        // 3. <!DOCTYPE ... >
        if tail.starts_with("<!DOCTYPE") {
            if let Some(gt) = tail.find('>') {
                let end = index + gt + 1;
                spans.push(SyntaxSpan::new(index, end, (*K).clone()));
                index = end;
                continue;
            }
        }

        // 4. &...;  entity (always constant, no context requirement)
        if let Some(len) = match_entity(tail) {
            spans.push(SyntaxSpan::new(index, index + len, (*CNST).clone()));
            index += len;
            continue;
        }

        // ── Closing tags (preferred over < when context is active) ───

        // 19. </script> (closing, requires script_host)
        if ctx.contains_anywhere(SCRIPT_HOST) && tail.starts_with("</script>") {
            let end = index + 9;
            push_closing_tag_spans(&mut spans, index, "script");
            push_html_close_fold_event(&mut fold_events, "script");
            ctx.pop(SCRIPT_HOST);
            inj = None; // injection host is gone
            index = end;
            continue;
        }

        // 20. </style> (closing, requires style_host)
        if ctx.contains_anywhere(STYLE_HOST) && tail.starts_with("</style>") {
            let end = index + 8;
            push_closing_tag_spans(&mut spans, index, "style");
            push_html_close_fold_event(&mut fold_events, "style");
            ctx.pop(STYLE_HOST);
            inj = None;
            index = end;
            continue;
        }

        // Generic closing tag.
        if tail.starts_with("</") {
            let name_tail = &tail[2..];
            if let Some(name_len) = match_tag_name(name_tail) {
                let name = &name_tail[..name_len];
                push_closing_tag_spans(&mut spans, index, name);
                push_html_close_fold_event(&mut fold_events, name);
                index += 3 + name_len;
                continue;
            }
        }

        // 5. <  (tag open) – only if we aren't inside script/style body
        if tail.starts_with('<') {
            let end = index + 1;
            tokenizer_state.set_u32(HTML_PENDING_TAG_FOLD_KIND, None);
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            ctx.push(INSIDE_TAG);
            ctx.push(TAG_NAME_EXPECTED);
            index = end;
            continue;
        }

        // 6. script (inside tag, tag_name_expected)
        if ctx.contains_anywhere(INSIDE_TAG)
            && ctx.top_is(TAG_NAME_EXPECTED)
            && is_word(tail, "script")
        {
            spans.push(SyntaxSpan::new(index, index + 6, (*MT).clone()));
            tokenizer_state.set_u32(
                HTML_PENDING_TAG_FOLD_KIND,
                Some(html_tag_fold_kind("script")),
            );
            ctx.push(SCRIPT_HOST);
            index += 6;
            continue;
        }

        // 7. style (inside tag, tag_name_expected)
        if ctx.contains_anywhere(INSIDE_TAG)
            && ctx.top_is(TAG_NAME_EXPECTED)
            && is_word(tail, "style")
        {
            spans.push(SyntaxSpan::new(index, index + 5, (*MT).clone()));
            tokenizer_state.set_u32(
                HTML_PENDING_TAG_FOLD_KIND,
                Some(html_tag_fold_kind("style")),
            );
            ctx.push(STYLE_HOST);
            index += 5;
            continue;
        }

        // 8. Attribute name (word + =)
        if ctx.contains_anywhere(INSIDE_TAG)
            && !ctx.contains_anywhere(HTML_DOUBLE_STRING)
            && !ctx.contains_anywhere(HTML_SINGLE_STRING)
        {
            if let Some((name_len, is_attr)) = match_attr_name(tail) {
                if is_attr {
                    spans.push(SyntaxSpan::new(index, index + name_len, (*VP).clone()));
                    index += name_len;
                    continue;
                }
            }
        }

        // 9. " open (double-string)
        if ctx.contains_anywhere(INSIDE_TAG)
            && !ctx.contains_anywhere(HTML_DOUBLE_STRING)
            && !ctx.contains_anywhere(HTML_SINGLE_STRING)
            && tail.starts_with('"')
        {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(HTML_DOUBLE_STRING);
            index = end;
            continue;
        }

        // 10. " close double-string
        if ctx.contains_anywhere(INSIDE_TAG)
            && ctx.top_is(HTML_DOUBLE_STRING)
            && tail.starts_with('"')
        {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.pop_top(HTML_DOUBLE_STRING);
            index = end;
            continue;
        }

        // 11. & inside double-string
        if ctx.contains_anywhere(INSIDE_TAG) && ctx.top_is(HTML_DOUBLE_STRING) {
            if let Some(len) = match_entity(tail) {
                spans.push(SyntaxSpan::new(index, index + len, (*S).clone()));
                index += len;
                continue;
            }
        }

        // 12. text in double-string
        if ctx.contains_anywhere(INSIDE_TAG) && ctx.top_is(HTML_DOUBLE_STRING) {
            let run = run_while(tail, |c| c != '"' && c != '&');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
        }

        // 13. ' open (single-string)
        if ctx.contains_anywhere(INSIDE_TAG)
            && !ctx.contains_anywhere(HTML_DOUBLE_STRING)
            && !ctx.contains_anywhere(HTML_SINGLE_STRING)
            && tail.starts_with('\'')
        {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.push(HTML_SINGLE_STRING);
            index = end;
            continue;
        }

        // 14. ' close (single-string)
        if ctx.contains_anywhere(INSIDE_TAG)
            && ctx.top_is(HTML_SINGLE_STRING)
            && tail.starts_with('\'')
        {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*S).clone()));
            ctx.pop_top(HTML_SINGLE_STRING);
            index = end;
            continue;
        }

        // 15. & inside single-string
        if ctx.contains_anywhere(INSIDE_TAG) && ctx.top_is(HTML_SINGLE_STRING) {
            if let Some(len) = match_entity(tail) {
                spans.push(SyntaxSpan::new(index, index + len, (*S).clone()));
                index += len;
                continue;
            }
        }

        // 16. text in single-string
        if ctx.contains_anywhere(INSIDE_TAG) && ctx.top_is(HTML_SINGLE_STRING) {
            let run = run_while(tail, |c| c != '\'' && c != '&');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*S).clone()));
                index += run;
                continue;
            }
        }

        // 17. Tag name (requires inside_tag + tag_name_expected)
        if ctx.contains_anywhere(INSIDE_TAG) && ctx.top_is(TAG_NAME_EXPECTED) {
            if let Some(name_len) = match_tag_name(tail) {
                let name = &tail[..name_len];
                let fold_kind = if is_void_tag(name) {
                    None
                } else {
                    Some(html_tag_fold_kind(name))
                };
                tokenizer_state.set_u32(HTML_PENDING_TAG_FOLD_KIND, fold_kind);
                spans.push(SyntaxSpan::new(index, index + name_len, (*MT).clone()));
                index += name_len;
                continue;
            }
        }

        // Self-closing tag close.
        if ctx.contains_anywhere(INSIDE_TAG) && tail.starts_with("/>") {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, index + 1, (*P).clone()));
            spans.push(SyntaxSpan::new(index + 1, end, (*P).clone()));
            tokenizer_state.set_u32(HTML_PENDING_TAG_FOLD_KIND, None);
            ctx.pop(TAG_NAME_EXPECTED);
            ctx.pop(HTML_DOUBLE_STRING);
            ctx.pop(HTML_SINGLE_STRING);
            ctx.pop(INSIDE_TAG);
            index = end;
            continue;
        }

        // 18. >  (tag close)
        if ctx.contains_anywhere(INSIDE_TAG) && tail.starts_with('>') {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            if let Some(fold_kind) = tokenizer_state.get_u32(HTML_PENDING_TAG_FOLD_KIND) {
                fold_events.push(SyntaxFoldEvent::new(SyntaxFoldEventKind::Open, fold_kind));
                tokenizer_state.set_u32(HTML_PENDING_TAG_FOLD_KIND, None);
            }
            ctx.pop(TAG_NAME_EXPECTED);
            ctx.pop(HTML_DOUBLE_STRING);
            ctx.pop(HTML_SINGLE_STRING);
            ctx.pop(INSIDE_TAG);
            index = end;
            continue;
        }

        // ── Set up injection for script/style body content ────────────
        if inj.is_none() {
            if ctx.contains_anywhere(SCRIPT_HOST) {
                if let Some(def) = syntax_definition_by_name("javascript") {
                    let nested = NestedState::new_syntax(def);
                    inj = Some(TokenizerInjectionState {
                        nested: Some(nested),
                        fallback: InjectedSyntaxFallback::Unstyled,
                        parent_style: parent_style.clone(),
                    });
                }
                continue;
            }
            if ctx.contains_anywhere(STYLE_HOST) {
                if let Some(def) = syntax_definition_by_name("css") {
                    let nested = NestedState::new_syntax(def);
                    inj = Some(TokenizerInjectionState {
                        nested: Some(nested),
                        fallback: InjectedSyntaxFallback::Unstyled,
                        parent_style: parent_style.clone(),
                    });
                }
                continue;
            }
        }

        // 23. Catch-all punctuation: < > / =
        if tail.starts_with('<')
            || tail.starts_with('>')
            || tail.starts_with('/')
            || tail.starts_with('=')
        {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*P).clone()));
            index = end;
            continue;
        }

        // ── No match – skip one char ─────────────────────────────────
        let Some(ch) = tail.chars().next() else { break };
        index += ch.len_utf8();
    }

    // Clear stale injection when its host context is gone
    if inj.is_some() && !ctx.contains_anywhere(SCRIPT_HOST) && !ctx.contains_anywhere(STYLE_HOST) {
        inj = None;
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

fn match_entity(tail: &str) -> Option<usize> {
    if !tail.starts_with('&') {
        return None;
    }
    let semi = tail[1..].find(';')?;
    let name = &tail[1..=semi];
    if name.is_empty() {
        return None;
    }
    if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '#') {
        Some(1 + semi + 1)
    } else {
        None
    }
}

fn push_closing_tag_spans(spans: &mut Vec<SyntaxSpan>, start: usize, name: &str) {
    spans.push(SyntaxSpan::new(start, start + 1, (*P).clone()));
    spans.push(SyntaxSpan::new(start + 1, start + 2, (*P).clone()));
    spans.push(SyntaxSpan::new(
        start + 2,
        start + 2 + name.len(),
        (*MT).clone(),
    ));
    spans.push(SyntaxSpan::new(
        start + 2 + name.len(),
        start + 3 + name.len(),
        (*P).clone(),
    ));
}

fn push_html_close_fold_event(fold_events: &mut Vec<SyntaxFoldEvent>, name: &str) {
    if !is_void_tag(name) {
        fold_events.push(SyntaxFoldEvent::new(
            SyntaxFoldEventKind::Close,
            html_tag_fold_kind(name),
        ));
    }
}

fn html_tag_fold_kind(name: &str) -> SyntaxFoldKind {
    let mut hash = 0x811c9dc5u32;
    for byte in name.bytes() {
        hash ^= u32::from(byte.to_ascii_lowercase());
        hash = hash.wrapping_mul(0x01000193);
    }
    HTML_TAG_FOLD_KIND_BASE.wrapping_add(hash)
}

fn is_void_tag(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn match_tag_name(tail: &str) -> Option<usize> {
    let first = tail.chars().next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let len = run_while(tail, |c| {
        c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '.'
    });
    if len > 0 { Some(len) } else { None }
}

fn match_attr_name(tail: &str) -> Option<(usize, bool)> {
    let first = tail.chars().next()?;
    if !first.is_ascii_alphabetic() && first != '_' && first != ':' {
        return None;
    }
    let name_len = run_while(tail, |c| {
        c.is_ascii_alphanumeric() || c == ':' || c == '.' || c == '_' || c == '-'
    });
    if name_len == 0 {
        return None;
    }
    Some((name_len, tail[name_len..].trim_start().starts_with('=')))
}

fn is_word(tail: &str, word: &str) -> bool {
    if !tail.starts_with(word) {
        return false;
    }
    let after = &tail[word.len()..];
    after
        .chars()
        .next()
        .is_none_or(|c| !c.is_alphanumeric() && c != '_' && c != '-')
}

/// Scan forward for the next HTML pattern that should end an injection.
///
/// Mirrors `find_next_rule_list_regex_match` in the regex engine:
/// checks each byte position for any HTML pattern that could act as an
/// injection boundary. Returns the byte position of the match, or None.
fn closing_tag_pos(line: &str, start: usize, ctx: &ContextStack) -> Option<usize> {
    let mut pos = start;
    while pos < line.len() {
        let tail = &line[pos..];

        // Closing tags have highest priority
        if ctx.contains_anywhere(SCRIPT_HOST) && tail.starts_with("</script>") {
            return Some(pos);
        }
        if ctx.contains_anywhere(STYLE_HOST) && tail.starts_with("</style>") {
            return Some(pos);
        }

        // < tag open (no context required) – ends any injection body
        if tail.starts_with('<') {
            return Some(pos);
        }

        // DOCTYPE
        if tail.starts_with("<!DOCTYPE") {
            return Some(pos);
        }

        // Entity &...;
        if match_entity(tail).is_some() {
            return Some(pos);
        }

        // > inside a tag context
        if ctx.contains_anywhere(INSIDE_TAG) && tail.starts_with('>') {
            return Some(pos);
        }

        // Catch-all single-char punctuation: > / =
        if tail.starts_with('>') || tail.starts_with('/') || tail.starts_with('=') {
            return Some(pos);
        }

        let Some(ch) = tail.chars().next() else { break };
        pos += ch.len_utf8();
    }
    None
}
