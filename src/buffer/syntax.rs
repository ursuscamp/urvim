//! Buffer-owned syntax highlighting cache and tokenizers.

use crate::buffer::Buffer;
use crate::syntax::{
    ContextControl, InjectedSyntaxFallback, InjectedSyntaxSelector, SyntaxDefinition, SyntaxRule,
    builtin_syntax_registry,
};
use crate::theme::Tag;
use regex::Regex;
use smol_str::SmolStr;

/// A highlighted span within one buffer line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxSpan {
    /// Start byte offset within the line.
    pub start_byte: usize,
    /// End byte offset within the line.
    pub end_byte: usize,
    /// The syntax tag that should style this span.
    pub style: Tag,
}

impl SyntaxSpan {
    /// Creates a new syntax span.
    pub fn new(start_byte: usize, end_byte: usize, style: Tag) -> Self {
        Self {
            start_byte,
            end_byte,
            style,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxLine {
    pub spans: Vec<SyntaxSpan>,
    state: SyntaxState,
}

impl SyntaxLine {
    fn new(spans: Vec<SyntaxSpan>, state: SyntaxState) -> Self {
        Self { spans, state }
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxCache {
    syntax_name: SmolStr,
    lines: Vec<SyntaxLine>,
}

impl SyntaxCache {
    /// Creates an empty syntax cache for a syntax name.
    pub fn new(syntax_name: impl Into<SmolStr>) -> Self {
        Self {
            syntax_name: syntax_name.into(),
            lines: Vec::new(),
        }
    }

    /// Updates the cached syntax name, clearing cached data when it changes.
    pub fn set_syntax_name(&mut self, syntax_name: impl Into<SmolStr>) {
        let syntax_name = syntax_name.into();
        if self.syntax_name != syntax_name {
            self.syntax_name = syntax_name;
            self.lines.clear();
        }
    }

    /// Invalidates cached syntax data from the provided line onward.
    pub fn invalidate_from(&mut self, line: usize) {
        self.lines.truncate(line.min(self.lines.len()));
    }

    /// Returns the cached spans for a line, computing any missing prefix first.
    pub fn spans_for_line(
        &mut self,
        syntax_name: &str,
        line_texts: &[&str],
        line: usize,
    ) -> Option<Vec<SyntaxSpan>> {
        self.set_syntax_name(syntax_name);
        if line >= line_texts.len() {
            return None;
        }

        self.ensure_through(syntax_name, line_texts, line);
        self.lines.get(line).map(|entry| entry.spans.clone())
    }

    /// Ensures syntax data exists through the requested line.
    pub fn ensure_through(&mut self, syntax_name: &str, line_texts: &[&str], line: usize) {
        self.set_syntax_name(syntax_name);

        let target_line = line.min(line_texts.len().saturating_sub(1));
        if self.lines.len() > line_texts.len() {
            self.lines.truncate(line_texts.len());
        }
        if target_line < self.lines.len() {
            return;
        }

        let mut state = self
            .lines
            .last()
            .map(|line| line.state.clone())
            .unwrap_or_default();
        let start_line = self.lines.len();

        for line_text in line_texts.iter().take(target_line + 1).skip(start_line) {
            let (spans, next_state) = tokenize_line(syntax_name, line_text, state);
            self.lines.push(SyntaxLine::new(spans, next_state.clone()));
            state = next_state;
        }
    }
}

#[derive(Debug, Clone, Default)]
enum SyntaxState {
    #[default]
    Plain,
    Code(CodeState),
}

#[derive(Debug, Clone)]
enum CodeState {
    Normal {
        contexts: ContextStack,
    },
    RuleList {
        contexts: ContextStack,
        injection: Option<RuleListInjectionState>,
        parent_style: Option<Tag>,
    },
}

#[derive(Debug, Clone)]
struct RuleListInjectionState {
    nested: Option<NestedState>,
    fallback: InjectedSyntaxFallback,
    parent_style: Option<Tag>,
}

#[derive(Debug, Clone)]
enum NestedState {
    Syntax {
        syntax_name: SmolStr,
        state: Box<SyntaxState>,
    },
}

#[derive(Debug, Clone, Default)]
struct ContextStack {
    entries: Vec<crate::syntax::ContextEntry>,
}

impl ContextStack {
    fn contains(&self, marker: &str) -> bool {
        self.entries.iter().any(|active| active.name == marker)
    }

    fn contains_all(&self, required: &[smol_str::SmolStr]) -> bool {
        required
            .iter()
            .all(|marker| self.entries.iter().any(|active| active.name == *marker))
    }

    fn payload_for(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.name == name)
            .and_then(|entry| entry.payload.as_deref())
    }

    fn push_all(
        &mut self,
        markers: &[crate::syntax::ContextPush],
        captures: Option<&regex::Captures<'_>>,
    ) {
        for marker in markers {
            let payload = marker.capture.and_then(|capture| {
                captures
                    .and_then(|captures| captures.get(capture))
                    .map(|capture| capture.as_str().to_string())
            });
            self.entries.push(crate::syntax::ContextEntry {
                name: marker.name.clone(),
                payload,
            });
        }
    }

    fn pop_all(&mut self, markers: &[smol_str::SmolStr]) {
        for marker in markers {
            if let Some(index) = self
                .entries
                .iter()
                .rposition(|active| active.name == *marker)
            {
                self.entries.remove(index);
            }
        }
    }
}

fn apply_context_control(
    contexts: &mut ContextStack,
    control: Option<&ContextControl>,
    captures: Option<&regex::Captures<'_>>,
) {
    let Some(control) = control else {
        return;
    };

    contexts.pop_all(&control.pop);
    contexts.push_all(&control.push, captures);
}

fn context_payload_matches(
    contexts: &ContextStack,
    control: &ContextControl,
    matched_text: &str,
    captures: Option<&regex::Captures<'_>>,
) -> bool {
    let Some(payload_match) = control.payload_match.as_ref() else {
        return true;
    };

    let Some(payload) = contexts.payload_for(&payload_match.name) else {
        return false;
    };

    let candidate = if let Some(capture) = payload_match.capture {
        captures
            .and_then(|captures| captures.get(capture))
            .map(|capture| capture.as_str())
            .unwrap_or(matched_text)
    } else {
        matched_text
    };

    candidate.starts_with(payload)
}

fn syntax_definition(syntax_name: &str) -> Option<std::sync::Arc<SyntaxDefinition>> {
    builtin_syntax_registry().ok()?.promote(syntax_name).ok()
}

fn tokenize_line(
    syntax_name: &str,
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    syntax_definition(syntax_name)
        .map(|definition| {
            let use_rule_list = !definition.rules().is_empty();

            if use_rule_list {
                tokenize_rule_list_line(definition.as_ref(), line, state)
            } else {
                tokenize_code_line(definition.as_ref(), line, state)
            }
        })
        .unwrap_or((Vec::new(), SyntaxState::Plain))
}

fn tokenize_rule_list_line(
    definition: &SyntaxDefinition,
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    let mut spans = Vec::new();
    let mut index = 0;
    let mut state = match state {
        SyntaxState::Code(CodeState::RuleList {
            contexts,
            injection,
            parent_style,
        }) => (contexts, injection, parent_style),
        SyntaxState::Code(CodeState::Normal { contexts }) => (contexts, None, None),
        _ => (ContextStack::default(), None, None),
    };

    while index < line.len() {
        let (contexts, injection, parent_style) = &mut state;

        if injection.is_some() && !has_active_injection(definition.rules(), contexts) {
            *injection = None;
        }

        if let Some(injection_state) = injection.as_mut() {
            if let Some(regex_match) =
                find_next_rule_list_regex_match(definition.rules(), line, index, contexts)
            {
                if regex_match.0 > index {
                    let body = &line[index..regex_match.0];
                    let mut body_spans = tokenize_injection_body(injection_state, body, index);
                    spans.append(&mut body_spans);
                    index = regex_match.0;
                    continue;
                }
            } else {
                let body = &line[index..];
                let mut body_spans = tokenize_injection_body(injection_state, body, index);
                spans.append(&mut body_spans);
                index = line.len();
                continue;
            }
        }

        let mut chosen = None;
        for rule in definition.rules() {
            if let SyntaxRule::Regex {
                regex,
                tag,
                context,
            } = rule
            {
                if let Some(context) = context.as_ref()
                    && !contexts.contains_all(&context.requires)
                {
                    continue;
                }
                if let Some((start, end, captures)) =
                    regex_match_at_with_captures(regex, line, index)
                {
                    let matched_text = line.get(start..end).unwrap_or("");
                    if let Some(context) = context.as_ref()
                        && !context_payload_matches(
                            contexts,
                            context,
                            matched_text,
                            Some(&captures),
                        )
                    {
                        continue;
                    }

                    let is_closing = context
                        .as_ref()
                        .is_some_and(|context| !context.pop.is_empty() && context.push.is_empty());
                    let should_replace = chosen.as_ref().map_or(
                        true,
                        |candidate: &(
                            usize,
                            usize,
                            regex::Captures<'_>,
                            bool,
                            Tag,
                            Option<ContextControl>,
                        )| !candidate.3 && is_closing,
                    );
                    if should_replace {
                        chosen = Some((
                            start,
                            end,
                            captures,
                            is_closing,
                            tag.clone(),
                            context.clone(),
                        ));
                        if is_closing {
                            break;
                        }
                    }
                }
            }
        }

        if let Some((start, end, captures, _, tag, context)) = chosen {
            let mut chosen_spans = split_trailing_punctuation_span(start, end, &tag, line);
            spans.append(&mut chosen_spans);
            apply_context_control(contexts, context.as_ref(), Some(&captures));
            *parent_style = Some(tag);
            if contexts.entries.is_empty() {
                *parent_style = None;
            }
            index = end;
            continue;
        }

        let mut matched = false;
        for rule in definition.rules() {
            if let SyntaxRule::Injection {
                selector,
                fallback,
                context,
            } = rule
            {
                if injection.is_some() {
                    continue;
                }
                if let Some(context) = context.as_ref()
                    && !contexts.contains_all(&context.requires)
                {
                    continue;
                }

                let nested = resolve_rule_list_injection(selector, line.get(index..).unwrap_or(""));
                *injection = Some(RuleListInjectionState {
                    nested,
                    fallback: *fallback,
                    parent_style: parent_style.clone(),
                });
                matched = true;
                break;
            }
        }

        if matched {
            continue;
        }

        let Some((byte_len, _)) = next_char(line, index) else {
            break;
        };
        index += byte_len;
    }

    (
        spans,
        SyntaxState::Code(CodeState::RuleList {
            contexts: state.0,
            injection: state.1,
            parent_style: state.2,
        }),
    )
}

fn has_active_injection(rules: &[SyntaxRule], contexts: &ContextStack) -> bool {
    rules.iter().any(|rule| match rule {
        SyntaxRule::Injection { context, .. } => context
            .as_ref()
            .is_some_and(|context| contexts.contains_all(&context.requires)),
        SyntaxRule::Regex { .. } => false,
    })
}

fn find_next_rule_list_regex_match(
    rules: &[SyntaxRule],
    line: &str,
    start: usize,
    contexts: &ContextStack,
) -> Option<(usize, usize)> {
    let mut index = start;
    while index < line.len() {
        for rule in rules {
            if let SyntaxRule::Regex { regex, context, .. } = rule {
                if let Some(context) = context.as_ref()
                    && !contexts.contains_all(&context.requires)
                {
                    continue;
                }
                if contexts.contains("markdown_code_fence_body") && context.is_none() {
                    continue;
                }
                if let Some((start, end)) = regex_match_at(regex, line, index) {
                    return Some((start, end));
                }
            }
        }

        let Some((byte_len, _)) = next_char(line, index) else {
            break;
        };
        index += byte_len;
    }

    None
}

fn resolve_rule_list_injection(
    selector: &InjectedSyntaxSelector,
    opener_tail: &str,
) -> Option<NestedState> {
    let label = match selector {
        InjectedSyntaxSelector::Static { name } => name.as_str(),
        InjectedSyntaxSelector::Capture { pattern } => {
            let captures = pattern.captures(opener_tail)?;
            captures.get(1).or_else(|| captures.get(0))?.as_str()
        }
    };

    let registry = builtin_syntax_registry().ok()?;
    let canonical = registry.resolve_label(label)?;
    let definition = registry.promote(canonical.as_str()).ok()?;
    Some(NestedState::Syntax {
        syntax_name: SmolStr::new(definition.name()),
        state: Box::new(initial_state_for_definition(definition.as_ref())),
    })
}

fn tokenize_code_line(
    definition: &SyntaxDefinition,
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    let mut spans = Vec::new();
    let mut index = 0;
    let mut state = match state {
        SyntaxState::Code(code_state) => code_state,
        _ => CodeState::Normal {
            contexts: ContextStack::default(),
        },
    };

    while index < line.len() {
        match state.clone() {
            CodeState::RuleList {
                contexts,
                injection,
                parent_style,
            } => {
                let (rule_list_spans, next_state) = tokenize_rule_list_line(
                    definition,
                    line.get(index..).unwrap_or(""),
                    SyntaxState::Code(CodeState::RuleList {
                        contexts,
                        injection,
                        parent_style,
                    }),
                );
                spans.extend(offset_spans(rule_list_spans, index));
                state = match next_state {
                    SyntaxState::Code(CodeState::RuleList {
                        contexts,
                        injection,
                        parent_style,
                    }) => CodeState::RuleList {
                        contexts,
                        injection,
                        parent_style,
                    },
                    SyntaxState::Code(CodeState::Normal { contexts }) => {
                        CodeState::Normal { contexts }
                    }
                    SyntaxState::Plain => CodeState::Normal {
                        contexts: ContextStack::default(),
                    },
                };
                break;
            }
            CodeState::Normal { .. } => {
                let Some((byte_len, _ch)) = next_char(line, index) else {
                    break;
                };
                index += byte_len;
            }
        }
    }

    (spans, SyntaxState::Code(state))
}

fn split_trailing_punctuation_span(
    start: usize,
    end: usize,
    tag: &Tag,
    line: &str,
) -> Vec<SyntaxSpan> {
    let style = tag.as_str();
    if style != "type" && style != "variable.property" {
        return vec![SyntaxSpan::new(start, end, tag.clone())];
    }

    let Some(text) = line.get(start..end) else {
        return vec![SyntaxSpan::new(start, end, tag.clone())];
    };

    let suffix_len = text
        .as_bytes()
        .iter()
        .rev()
        .take_while(|&&byte| matches!(byte, b'{' | b'}' | b':' | b';' | b','))
        .count();

    if suffix_len == 0 || suffix_len >= text.len() {
        return vec![SyntaxSpan::new(start, end, tag.clone())];
    }

    let split = end - suffix_len;
    vec![
        SyntaxSpan::new(start, split, tag.clone()),
        SyntaxSpan::new(
            split,
            end,
            Tag::parse("punctuation").expect("valid punctuation tag"),
        ),
    ]
}

fn tokenize_nested_body(nested: &mut NestedState, body: &str, offset: usize) -> Vec<SyntaxSpan> {
    match nested {
        NestedState::Syntax { syntax_name, state } => {
            let (spans, next_state) = tokenize_line(syntax_name, body, state.as_ref().clone());
            **state = next_state;
            offset_spans(spans, offset)
        }
    }
}

fn tokenize_injection_body(injection_state: &mut RuleListInjectionState, body: &str, offset: usize) -> Vec<SyntaxSpan> {
    if let Some(nested) = injection_state.nested.as_mut() {
        return tokenize_nested_body(nested, body, offset);
    }

    if matches!(injection_state.fallback, InjectedSyntaxFallback::ParentStyle) {
        let Some(style) = injection_state.parent_style.clone() else {
            return Vec::new();
        };

        return vec![SyntaxSpan::new(offset, offset + body.len(), style)];
    }

    Vec::new()
}

fn offset_spans(spans: Vec<SyntaxSpan>, offset: usize) -> Vec<SyntaxSpan> {
    spans
        .into_iter()
        .map(|span| SyntaxSpan::new(span.start_byte + offset, span.end_byte + offset, span.style))
        .collect()
}

fn initial_state_for_definition(definition: &SyntaxDefinition) -> SyntaxState {
    if !definition.rules().is_empty() {
        SyntaxState::Code(CodeState::RuleList {
            contexts: ContextStack::default(),
            injection: None,
            parent_style: None,
        })
    } else {
        SyntaxState::Plain
    }
}

fn next_char(line: &str, index: usize) -> Option<(usize, char)> {
    let tail = line.get(index..)?;
    let ch = tail.chars().next()?;
    Some((ch.len_utf8(), ch))
}

fn regex_match_at(regex: &Regex, line: &str, index: usize) -> Option<(usize, usize)> {
    let pattern = regex.as_str();
    if pattern.starts_with('^') || pattern.starts_with("\\A") {
        let tail = line.get(index..)?;
        let matched = regex.find(tail)?;
        if matched.start() == 0 {
            return Some((index, index + matched.end()));
        }
        return None;
    }

    let matched = regex.find_at(line, index)?;
    if matched.start() == index {
        Some((matched.start(), matched.end()))
    } else {
        None
    }
}

fn regex_match_at_with_captures<'a>(
    regex: &'a Regex,
    line: &'a str,
    index: usize,
) -> Option<(usize, usize, regex::Captures<'a>)> {
    let tail = line.get(index..)?;
    let captures = regex.captures(tail)?;
    let matched = captures.get(0)?;
    if matched.start() == 0 {
        Some((index, index + matched.end(), captures))
    } else {
        None
    }
}

impl Buffer {
    /// Invalidates buffer-owned syntax data from the given line onward.
    pub fn invalidate_syntax_from(&mut self, line: usize) {
        self.syntax_cache.invalidate_from(line);
        if line == 0 {
            self.refresh_syntax();
        }
    }

    /// Returns the highlighted spans for a line, computing them on demand.
    pub fn syntax_spans_for_line(&mut self, line: usize) -> Option<Vec<SyntaxSpan>> {
        let line_texts: Vec<&str> = self.lines.iter().map(|line| line.as_ref()).collect();
        let syntax_name = self.syntax_name.clone();
        self.syntax_cache
            .spans_for_line(&syntax_name, &line_texts, line)
    }

    /// Ensures syntax data exists through a line without returning it.
    pub fn ensure_syntax_through(&mut self, line: usize) {
        let line_texts: Vec<&str> = self.lines.iter().map(|line| line.as_ref()).collect();
        let syntax_name = self.syntax_name.clone();
        self.syntax_cache
            .ensure_through(&syntax_name, &line_texts, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(value: &str) -> Tag {
        Tag::parse(value).expect("valid tag")
    }

    #[test]
    fn parent_style_fallback_uses_the_injection_opener_tag() {
        let definition = SyntaxDefinition {
            metadata: crate::syntax::SyntaxMetadata {
                name: SmolStr::new("example"),
                display_name: SmolStr::new("Example"),
                alias: Vec::new(),
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            rules: vec![
                SyntaxRule::Regex {
                    regex: Regex::new(r"^```[A-Za-z]+$").expect("valid opener regex"),
                    tag: tag("markup.code"),
                    context: Some(ContextControl {
                        requires: Vec::new(),
                        push: vec![crate::syntax::ContextPush {
                            name: SmolStr::new("fence"),
                            capture: None,
                        }],
                        pop: Vec::new(),
                        payload_match: None,
                    }),
                },
                SyntaxRule::Injection {
                    selector: InjectedSyntaxSelector::Static {
                        name: SmolStr::new("missing"),
                    },
                    fallback: InjectedSyntaxFallback::ParentStyle,
                    context: Some(ContextControl {
                        requires: vec![SmolStr::new("fence")],
                        push: Vec::new(),
                        pop: Vec::new(),
                        payload_match: None,
                    }),
                },
                SyntaxRule::Regex {
                    regex: Regex::new(r"^```$").expect("valid closer regex"),
                    tag: tag("markup.code"),
                    context: Some(ContextControl {
                        requires: vec![SmolStr::new("fence")],
                        push: Vec::new(),
                        pop: vec![SmolStr::new("fence")],
                        payload_match: None,
                    }),
                },
            ],
        };

        let (opener_spans, state) =
            tokenize_rule_list_line(&definition, "```example", SyntaxState::default());
        assert!(opener_spans.iter().any(|span| span.style == tag("markup.code")));

        let (body_spans, state) = tokenize_rule_list_line(&definition, "body text", state);
        assert_eq!(body_spans.len(), 1);
        assert_eq!(body_spans[0].style, tag("markup.code"));

        let (closing_spans, _) = tokenize_rule_list_line(&definition, "```", state);
        assert_eq!(closing_spans.len(), 1);
        assert_eq!(closing_spans[0].style, tag("markup.code"));
    }
}
