//! Builtin handwritten scanner for Markdown syntax.

use std::sync::LazyLock;

use super::scanner::{is_word_byte, run_while};
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

tag_static!(HEADING, "markup.heading");
tag_static!(THEMATIC, "markup.thematic_break");
tag_static!(QUOTE, "markup.quote");
tag_static!(LIST, "markup.list");
tag_static!(CODE, "markup.code");
tag_static!(LINK, "markup.link");
tag_static!(STRONG, "markup.strong");
tag_static!(STRONG_TEXT, "markup.strong.text");
tag_static!(EMPH, "markup.emphasis");
tag_static!(EMPH_TEXT, "markup.emphasis.text");
tag_static!(CODE_INLINE, "markup.code.inline");
tag_static!(CODE_INLINE_TEXT, "markup.code.inline.text");

const MARKDOWN_CODE_FENCE: ContextId = ContextId::new("markdown", "markdown_code_fence");
const MARKDOWN_CODE_FENCE_BODY: ContextId = ContextId::new("markdown", "markdown_code_fence_body");
const MARKDOWN_STRONG: ContextId = ContextId::new("markdown", "markdown_strong");
const MARKDOWN_EMPHASIS: ContextId = ContextId::new("markdown", "markdown_emphasis");
const MARKDOWN_INLINE_CODE: ContextId = ContextId::new("markdown", "markdown_inline_code");

const MARKDOWN_FOLD_H1: SyntaxFoldKind = 1;
const MARKDOWN_FOLD_H2: SyntaxFoldKind = 2;
const MARKDOWN_FOLD_H3: SyntaxFoldKind = 3;
const MARKDOWN_FOLD_H4: SyntaxFoldKind = 4;
const MARKDOWN_FOLD_H5: SyntaxFoldKind = 5;
const MARKDOWN_FOLD_H6: SyntaxFoldKind = 6;

/// Tokenize one line of Markdown using the builtin scanner.
pub(crate) fn tokenize_markdown_line(line: &str, state: SyntaxState) -> SyntaxLineResult {
    let (mut ctx, mut inj, parent_style, tokenizer_state) = match state {
        SyntaxState::Code(CodeState::Scanner {
            contexts,
            injection,
            parent_style,
            tokenizer_state,
        }) => (contexts, injection, parent_style, tokenizer_state),
        SyntaxState::Plain => (ContextStack::default(), None, None, Default::default()),
    };

    let mut spans: Vec<SyntaxSpan> = Vec::new();
    let mut fold_events: Vec<SyntaxFoldEvent> = Vec::new();
    let mut index = 0usize;
    let bytes = line.as_bytes();
    let line_len = bytes.len();

    while index < line_len {
        let tail = &line[index..];
        let tail_bytes = &bytes[index..];

        // ── Handle injection (code fence body) ──────────────────────
        if ctx.top_is(MARKDOWN_CODE_FENCE_BODY) {
            // Check fence close first
            if index == 0 {
                let delim = ctx
                    .payload_for(MARKDOWN_CODE_FENCE)
                    .unwrap_or("")
                    .to_string();
                if !delim.is_empty() {
                    // Close fence is just the fence marker at line start
                    let mut i = 0;
                    while i < line_len && (bytes[i] == b' ' || bytes[i] == b'\t') && i < 3 {
                        i += 1;
                    }
                    if line[i..].starts_with(&delim) {
                        // Check rest of line is whitespace only
                        let mut j = i + delim.len();
                        while j < line_len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                            j += 1;
                        }
                        if j == line_len {
                            spans.push(SyntaxSpan::new(index, j, (*CODE).clone()));
                            ctx.pop_top(MARKDOWN_CODE_FENCE_BODY);
                            ctx.pop_top(MARKDOWN_CODE_FENCE);
                            index = j;
                            continue;
                        }
                    }
                }
            }

            // Process code fence body via injection
            if let Some(ref mut inj_state) = inj {
                // Tokenize the rest of the line as injected content
                let body = &line[index..];
                let body_spans = tokenize_injected_body(inj_state, body, index);
                spans.extend(body_spans);
                index = line_len;
                continue;
            }

            // No injection — fall through (skip/plain content)
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Strong **...** ───────────────────────────────────────────
        if ctx.top_is(MARKDOWN_STRONG) {
            if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'*' {
                let end = index + 2;
                spans.push(SyntaxSpan::new(index, end, (*STRONG).clone()));
                ctx.pop_top(MARKDOWN_STRONG);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '*');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*STRONG_TEXT).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Emphasis *...* ───────────────────────────────────────────
        if ctx.top_is(MARKDOWN_EMPHASIS) {
            if tail_bytes[0] == b'*' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*EMPH).clone()));
                ctx.pop_top(MARKDOWN_EMPHASIS);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '*');
            if run > 0 {
                spans.push(SyntaxSpan::new(index, index + run, (*EMPH_TEXT).clone()));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Inline code `...` ────────────────────────────────────────
        if ctx.top_is(MARKDOWN_INLINE_CODE) {
            if tail_bytes[0] == b'`' {
                let end = index + 1;
                spans.push(SyntaxSpan::new(index, end, (*CODE_INLINE).clone()));
                ctx.pop_top(MARKDOWN_INLINE_CODE);
                index = end;
                continue;
            }
            let run = run_while(tail, |c| c != '`' && c != '\n');
            if run > 0 {
                spans.push(SyntaxSpan::new(
                    index,
                    index + run,
                    (*CODE_INLINE_TEXT).clone(),
                ));
                index += run;
                continue;
            }
            let Some(ch) = tail.chars().next() else { break };
            index += ch.len_utf8();
            continue;
        }

        // ── Top-level (^)-anchored rules ─────────────────────────────

        // ATX heading
        if let Some(heading) = match_heading(line, index) {
            push_markdown_heading_fold_events(&mut fold_events, heading.level);
            let len = heading.end;
            if len == line_len {
                spans.push(SyntaxSpan::new(index, len, (*HEADING).clone()));
                index = len;
                continue;
            }
        }

        // Setext heading underline
        if index == 0 {
            let trimmed = line.trim();
            if trimmed.len() >= 1 && trimmed.chars().all(|c| c == '=') {
                let trimmed2 = line.trim();
                if trimmed2.chars().all(|c| c == '=') {
                    spans.push(SyntaxSpan::new(index, line_len, (*HEADING).clone()));
                    index = line_len;
                    continue;
                }
            }
            // Thematic break
            if is_thematic_break(line) {
                spans.push(SyntaxSpan::new(index, line_len, (*THEMATIC).clone()));
                index = line_len;
                continue;
            }
        }

        // Blockquote
        if let Some(len) = match_blockquote(line, index) {
            spans.push(SyntaxSpan::new(index, len, (*QUOTE).clone()));
            index = len;
            continue;
        }

        // List item
        if let Some(len) = match_list(line, index) {
            spans.push(SyntaxSpan::new(index, len, (*LIST).clone()));
            index = len;
            continue;
        }

        // Code fence open
        if index == 0 {
            if let Some(fence) = match_fence_open(line) {
                let marker_end = fence.marker_len;
                spans.push(SyntaxSpan::new(index, marker_end, (*CODE).clone()));
                ctx.push_with_payload(MARKDOWN_CODE_FENCE, &fence.marker);
                ctx.push(MARKDOWN_CODE_FENCE_BODY);
                // Set up injection for the fence body (and rest of this line)
                if let Some(def) = syntax_definition_by_name(&fence.lang) {
                    let nested = NestedState::new_syntax(def);
                    inj = Some(TokenizerInjectionState {
                        nested: Some(nested),
                        fallback: InjectedSyntaxFallback::Unstyled,
                        parent_style: parent_style.clone(),
                    });
                }
                // Process rest of this line via injection
                if marker_end < line_len {
                    if let Some(ref mut inj_state) = inj {
                        let body = &line[marker_end..];
                        let body_spans = tokenize_injected_body(inj_state, body, marker_end);
                        spans.extend(body_spans);
                    }
                }
                index = line_len;
                continue;
            }
            // Indented code block
            if line_len >= 5
                && bytes[0] == b' '
                && bytes[1] == b' '
                && bytes[2] == b' '
                && bytes[3] == b' '
            {
                spans.push(SyntaxSpan::new(index, line_len, (*CODE).clone()));
                index = line_len;
                continue;
            }
        }

        // Inline link [...](...)
        if tail_bytes[0] == b'!' && tail_bytes.len() >= 2 && tail_bytes[1] == b'[' {
            if let Some(len) = match_image_link(tail) {
                spans.push(SyntaxSpan::new(index, index + len, (*LINK).clone()));
                index += len;
                continue;
            }
        }
        if tail_bytes[0] == b'[' {
            if let Some(len) = match_link(tail) {
                spans.push(SyntaxSpan::new(index, index + len, (*LINK).clone()));
                index += len;
                continue;
            }
        }

        // Reference definition ^\[[^\]]+\]:[ \t].*$
        if index == 0 && tail_bytes[0] == b'[' {
            if let Some(len) = match_ref_definition(line) {
                spans.push(SyntaxSpan::new(index, len, (*LINK).clone()));
                index = len;
                continue;
            }
        }

        // Autolink <...>
        if tail_bytes[0] == b'<'
            && let Some(len) = match_autolink(tail)
        {
            spans.push(SyntaxSpan::new(index, index + len, (*LINK).clone()));
            index += len;
            continue;
        }

        // ** strong open
        if tail_bytes.len() >= 2 && tail_bytes[0] == b'*' && tail_bytes[1] == b'*' {
            let end = index + 2;
            spans.push(SyntaxSpan::new(index, end, (*STRONG).clone()));
            ctx.push(MARKDOWN_STRONG);
            index = end;
            continue;
        }

        // __...__ underscore strong (single match)
        if let Some(len) = match_underscore_strong(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*STRONG).clone()));
            index += len;
            continue;
        }

        // * emphasis open
        if tail_bytes[0] == b'*' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*EMPH).clone()));
            ctx.push(MARKDOWN_EMPHASIS);
            index = end;
            continue;
        }

        // _..._ underscore emphasis (single match)
        if let Some(len) = match_underscore_emph(tail, index, bytes) {
            spans.push(SyntaxSpan::new(index, index + len, (*EMPH).clone()));
            index += len;
            continue;
        }

        // ` inline code open
        if tail_bytes[0] == b'`' {
            let end = index + 1;
            spans.push(SyntaxSpan::new(index, end, (*CODE_INLINE).clone()));
            ctx.push(MARKDOWN_INLINE_CODE);
            index = end;
            continue;
        }

        // Skip one char (prose/whitespace not otherwise highlighted)
        let Some(ch) = tail.chars().next() else { break };
        index += ch.len_utf8();
    }

    // Clear stale injection when fence context is gone
    if inj.is_some() && !ctx.contains_anywhere(MARKDOWN_CODE_FENCE_BODY) {
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

struct FenceInfo {
    marker_len: usize,
    marker: String,
    lang: String,
}

struct HeadingInfo {
    level: u8,
    end: usize,
}

fn match_heading(line: &str, _index: usize) -> Option<HeadingInfo> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len && i < 3 && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let mut count = 0;
    while i < len && bytes[i] == b'#' && count < 6 {
        count += 1;
        i += 1;
    }
    if count == 0 {
        return None;
    }
    if i >= len || (bytes[i] != b' ' && bytes[i] != b'\t') {
        return None;
    }
    // Consume rest of line
    Some(HeadingInfo {
        level: count,
        end: len,
    })
}

fn push_markdown_heading_fold_events(fold_events: &mut Vec<SyntaxFoldEvent>, level: u8) {
    for fold_kind in markdown_heading_fold_kinds_from_deepest(level) {
        fold_events.push(SyntaxFoldEvent::close_before_current_line(fold_kind));
    }
    fold_events.push(SyntaxFoldEvent::new(
        SyntaxFoldEventKind::Open,
        markdown_heading_fold_kind(level),
    ));
}

fn markdown_heading_fold_kinds_from_deepest(level: u8) -> impl Iterator<Item = SyntaxFoldKind> {
    (level..=6).rev().map(markdown_heading_fold_kind)
}

fn markdown_heading_fold_kind(level: u8) -> SyntaxFoldKind {
    match level {
        1 => MARKDOWN_FOLD_H1,
        2 => MARKDOWN_FOLD_H2,
        3 => MARKDOWN_FOLD_H3,
        4 => MARKDOWN_FOLD_H4,
        5 => MARKDOWN_FOLD_H5,
        _ => MARKDOWN_FOLD_H6,
    }
}

fn is_thematic_break(line: &str) -> bool {
    let s = line.trim();
    if s.len() < 3 {
        return false;
    }
    let first = s.as_bytes()[0];
    if first != b'-' && first != b'*' && first != b'_' {
        return false;
    }
    let mut i = 0;
    while i < s.len() {
        if s.as_bytes()[i] == first || s.as_bytes()[i] == b' ' || s.as_bytes()[i] == b'\t' {
            i += 1;
        } else {
            return false;
        }
    }
    true
}

fn match_blockquote(line: &str, _index: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= len || bytes[i] != b'>' {
        return None;
    }
    // Consume entire line
    Some(len)
}

fn match_list(line: &str, _index: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i >= len {
        return None;
    }
    // Check for - + * or digit+.) at line start
    if matches!(bytes[i], b'-' | b'+' | b'*') {
        i += 1;
    } else if bytes[i].is_ascii_digit() {
        i += 1;
        while i < len && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i >= len || !matches!(bytes[i], b'.' | b')') {
            return None;
        }
        i += 1;
    } else {
        return None;
    }
    if i >= len || (bytes[i] != b' ' && bytes[i] != b'\t') {
        return None;
    }
    Some(len)
}

fn match_fence_open(line: &str) -> Option<FenceInfo> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len && i < 3 && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    if i + 2 >= len {
        return None;
    }
    let marker = if line[i..].starts_with("```") {
        "```"
    } else if line[i..].starts_with("~~~") {
        "~~~"
    } else {
        return None;
    };
    i += marker.len();
    let marker_end = i;

    // Skip whitespace
    while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // Extract language identifier
    let lang_start = i;
    while i < len
        && (bytes[i].is_ascii_alphanumeric()
            || bytes[i] == b'_'
            || bytes[i] == b'+'
            || bytes[i] == b'-')
    {
        i += 1;
    }
    let lang = line[lang_start..i].to_string();

    Some(FenceInfo {
        marker_len: marker_end,
        marker: marker.to_string(),
        lang,
    })
}

fn match_image_link(tail: &str) -> Option<usize> {
    // ![text](url) or ![text][ref]
    let bytes = tail.as_bytes();
    if bytes.len() < 6 || bytes[0] != b'!' || bytes[1] != b'[' {
        return None;
    }
    let mut i = 2;
    while i < bytes.len() && bytes[i] != b']' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    i += 1; // ]
    if i >= bytes.len() {
        return None;
    }
    if bytes[i] == b'(' {
        i += 1;
        while i < bytes.len() && bytes[i] != b')' {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        Some(i + 1)
    } else if bytes[i] == b'[' {
        i += 1;
        while i < bytes.len() && bytes[i] != b']' {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        Some(i + 1)
    } else {
        None
    }
}

fn match_link(tail: &str) -> Option<usize> {
    // [text](url) or [text][ref]
    let bytes = tail.as_bytes();
    if bytes.len() < 4 || bytes[0] != b'[' {
        return None;
    }
    let mut i = 1;
    while i < bytes.len() && bytes[i] != b']' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    i += 1; // ]
    if i >= bytes.len() {
        return None;
    }
    if bytes[i] == b'(' {
        i += 1;
        while i < bytes.len() && bytes[i] != b')' {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        Some(i + 1)
    } else if bytes[i] == b'[' {
        i += 1;
        while i < bytes.len() && bytes[i] != b']' {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        Some(i + 1)
    } else {
        None
    }
}

fn match_ref_definition(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    if len < 5 || bytes[0] != b'[' {
        return None;
    }
    let mut i = 1;
    while i < len && bytes[i] != b']' {
        i += 1;
    }
    if i >= len {
        return None;
    }
    i += 1; // ]
    if i >= len || bytes[i] != b':' {
        return None;
    }
    i += 1; // :
    if i >= len || (bytes[i] != b' ' && bytes[i] != b'\t') {
        return None;
    }
    Some(len)
}

fn match_autolink(tail: &str) -> Option<usize> {
    let bytes = tail.as_bytes();
    if bytes.len() < 10 || bytes[0] != b'<' {
        return None;
    }
    let rest = &tail[1..];
    if rest.starts_with("https://") || rest.starts_with("http://") || rest.starts_with("mailto:") {
        let mut i = 1;
        while i < bytes.len() && bytes[i] != b'>' {
            i += 1;
        }
        if i < bytes.len() { Some(i + 1) } else { None }
    } else {
        None
    }
}

fn match_underscore_strong(tail: &str, _index: usize, _full_bytes: &[u8]) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    let start = if bytes[0] == b'_' && bytes[1] == b'_' {
        0
    } else if !is_word_byte(bytes[0]) && bytes[0] != b'\n' {
        1
    } else {
        return None;
    };
    if start + 2 >= len || bytes[start] != b'_' || bytes[start + 1] != b'_' {
        return None;
    }
    let mut i = start + 2;
    while i < len && bytes[i] != b'_' && bytes[i] != b'\n' {
        i += 1;
    }
    if i + 1 >= len || bytes[i] != b'_' || bytes[i + 1] != b'_' {
        return None;
    }
    Some(i + 2)
}

fn match_underscore_emph(tail: &str, index: usize, full_bytes: &[u8]) -> Option<usize> {
    let bytes = tail.as_bytes();
    let len = bytes.len();
    // (?:^|[^A-Za-z0-9_])_[^_\n]+_
    if index > 0 && is_word_byte(full_bytes[index - 1]) {
        return None;
    }
    if len < 3 || bytes[0] != b'_' {
        return None;
    }
    if bytes[1] == b'_' {
        return None;
    } // don't match __
    let mut i = 1;
    while i < len && bytes[i] != b'_' && bytes[i] != b'\n' {
        i += 1;
    }
    if i >= len || bytes[i] != b'_' {
        return None;
    }
    if i + 1 < len && bytes[i + 1] == b'_' {
        return None;
    } // don't match __
    Some(i + 1)
}
