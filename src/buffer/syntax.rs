//! Syntax highlighting tokenizers and indent-scope cache primitives.

use crate::buffer::Buffer;
use crate::buffer::BufferId;
use crate::globals;
use crate::job::{Job, JobContext, JobKind, JobPriority, JobToken};
use crate::syntax::{
    ContextControl, InjectedSyntaxFallback, InjectedSyntaxSelector, SyntaxDefinition, SyntaxRule,
    builtin_syntax_registry,
};
use crate::theme::Tag;
use imbl::Vector;
use regex::Regex;
use smol_str::SmolStr;
use std::sync::Arc;

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
    lines: Vector<SyntaxLine>,
}

impl SyntaxCache {
    /// Creates an empty syntax cache for a syntax name.
    pub fn new(syntax_name: impl Into<SmolStr>) -> Self {
        Self {
            syntax_name: syntax_name.into(),
            lines: Vector::new(),
        }
    }

    /// Updates the cached syntax name, clearing cached data when it changes.
    pub fn set_syntax_name(&mut self, syntax_name: impl Into<SmolStr>) {
        let syntax_name = syntax_name.into();
        if self.syntax_name != syntax_name {
            self.syntax_name = syntax_name;
            self.lines = Vector::new();
        }
    }

    /// Returns the canonical syntax name tracked by this cache.
    pub fn syntax_name(&self) -> &str {
        &self.syntax_name
    }

    /// Invalidates cached syntax data from the provided line onward.
    pub fn invalidate_from(&mut self, line: usize) {
        if line >= self.lines.len() {
            return;
        }
        self.lines.truncate(line.min(self.lines.len()));
    }

    /// Returns cached spans for a line without computing any missing prefix.
    pub fn cached_spans_for_line(&self, line: usize) -> Option<Vec<SyntaxSpan>> {
        self.lines.get(line).map(|entry| entry.spans.clone())
    }

    /// Returns how many leading lines currently have cached syntax data.
    pub fn cached_line_count(&self) -> usize {
        self.lines.len()
    }

    /// Returns true when every line in the buffer has a cached syntax result.
    pub fn is_complete_for_line_count(&self, line_count: usize) -> bool {
        self.lines.len() >= line_count
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

        if target_line >= self.lines.len() {
            let mut state = self
                .lines
                .last()
                .map(|line| line.state.clone())
                .unwrap_or_default();
            let start_line = self.lines.len();

            for line_text in line_texts.iter().take(target_line + 1).skip(start_line) {
                let (spans, next_state) = tokenize_line(syntax_name, line_text, state);
                self.lines
                    .push_back(SyntaxLine::new(spans, next_state.clone()));
                state = next_state;
            }
        }
    }
}

/// Result produced by the background buffer cache refresh job.
#[derive(Debug, Clone)]
pub struct BufferCacheRefreshResult {
    /// Buffer the result applies to.
    pub buffer_id: BufferId,
    /// Cache generation that produced the result.
    pub generation: u64,
    /// Completed buffer cache snapshot.
    pub cache: BufferCache,
}

/// Backward-compatible alias for the buffer cache refresh result.
pub type SyntaxCatchUpResult = BufferCacheRefreshResult;

/// A stable identifier for one indent scope inside a cache generation.
pub type IndentScopeId = usize;

/// A line-based indentation range derived from visual indentation width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentScope {
    /// Stable scope id for this cache generation.
    pub id: IndentScopeId,
    /// Inclusive starting line index.
    pub start_line: usize,
    /// Inclusive ending line index, or `None` while the scope is open.
    pub end_line: Option<usize>,
    /// Normalized visual indentation width for boundary lines.
    pub indent_width: usize,
    has_non_empty_inside: bool,
}

impl IndentScope {
    fn open(id: IndentScopeId, start_line: usize, indent_width: usize) -> Self {
        Self {
            id,
            start_line,
            end_line: None,
            indent_width,
            has_non_empty_inside: false,
        }
    }

    fn finalize(&mut self, end_line: usize) {
        self.end_line = Some(end_line);
    }

    fn invalidate(&mut self) {
        self.end_line = None;
        self.has_non_empty_inside = false;
    }

    fn mark_non_empty_inside(&mut self) {
        self.has_non_empty_inside = true;
    }

    /// Returns true when the scope is still tracked by the cache.
    pub fn is_active(&self) -> bool {
        self.end_line.is_some() || self.is_open()
    }

    /// Returns true when the scope is open at the current cache frontier.
    pub fn is_open(&self) -> bool {
        self.end_line.is_none()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct IndentScopeCache {
    scopes: Vector<IndentScope>,
    line_to_scopes: Vector<Vector<IndentScopeId>>,
    open_scope_stack: Vector<IndentScopeId>,
    scanned_through_line: usize,
}

impl IndentScopeCache {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn invalidate_from(&mut self, line: usize) {
        let boundary = line.min(self.line_to_scopes.len());
        let sync_point = self.previous_sync_point(boundary);

        self.line_to_scopes.truncate(sync_point);
        self.scanned_through_line = sync_point;
        self.open_scope_stack = Vector::new();

        let prefix_len = self
            .scopes
            .iter()
            .collect::<Vec<_>>()
            .partition_point(|scope| scope.start_line < sync_point);
        self.scopes.truncate(prefix_len);
    }

    fn previous_sync_point(&self, boundary: usize) -> usize {
        if boundary == 0 {
            return 0;
        }

        for line_idx in (0..boundary).rev() {
            if !self.line_has_open_scopes(line_idx) {
                return line_idx + 1;
            }
        }

        0
    }

    fn line_has_open_scopes(&self, line_idx: usize) -> bool {
        self.line_to_scopes.get(line_idx).is_some_and(|scope_ids| {
            scope_ids.iter().any(|scope_id| {
                self.scopes.get(*scope_id).is_some_and(|scope| {
                    scope.end_line.map_or(true, |end_line| end_line > line_idx)
                })
            })
        })
    }

    pub(crate) fn ensure_through(
        &mut self,
        line_texts: &[&str],
        target_line: usize,
        tab_width: usize,
    ) -> bool {
        if line_texts.is_empty() {
            self.scanned_through_line = 0;
            self.open_scope_stack = Vector::new();
            return true;
        }

        let normalized_tab_width = tab_width.max(1);
        let target_line = target_line.min(line_texts.len() - 1);
        if target_line < self.scanned_through_line {
            return self.scanned_through_line >= line_texts.len();
        }

        if self.line_to_scopes.len() > line_texts.len() {
            self.line_to_scopes.truncate(line_texts.len());
        }

        for (line_idx, line_text) in line_texts
            .iter()
            .enumerate()
            .take(target_line + 1)
            .skip(self.scanned_through_line)
        {
            self.ensure_row_for_line(line_idx);
            // Empty lines should not open or close scopes; they inherit the current stack.
            if line_text.is_empty() {
                self.commit_row(line_idx, self.open_scope_stack.clone());
                continue;
            }
            let indent_width = leading_indent_width(line_text, normalized_tab_width);
            let mut invalid_branch_start = None;
            let mut closed_scope_on_line = None;

            while let Some(&scope_id) = self.open_scope_stack.last() {
                let scope_indent = self.scopes[scope_id].indent_width;
                if scope_indent > indent_width {
                    invalid_branch_start = Some(self.scopes[scope_id].start_line);
                    self.close_scope_invalid(scope_id, line_idx.saturating_sub(1));
                    continue;
                }
                break;
            }

            if let Some(&scope_id) = self.open_scope_stack.last()
                && self.scopes[scope_id].indent_width == indent_width
            {
                if self.close_scope_line(scope_id, line_idx) {
                    closed_scope_on_line = Some(scope_id);
                } else {
                    invalid_branch_start = Some(self.scopes[scope_id].start_line);
                }
                self.open_scope_stack.pop_back();
            }

            if let Some(branch_start) = invalid_branch_start {
                self.trim_invalid_branch(branch_start, line_idx);
                if closed_scope_on_line.is_some_and(|scope_id| scope_id >= self.scopes.len()) {
                    closed_scope_on_line = None;
                }
            }

            let mut row_members = self.open_scope_stack.clone();
            if let Some(scope_id) = closed_scope_on_line {
                row_members.push_back(scope_id);
            }

            if !line_text.is_empty() {
                for scope_id in &row_members {
                    if let Some(scope) = self.scopes.get_mut(*scope_id)
                        && line_idx > scope.start_line
                    {
                        scope.mark_non_empty_inside();
                    }
                }
            }

            let new_scope_id = self.scopes.len();
            self.scopes
                .push_back(IndentScope::open(new_scope_id, line_idx, indent_width));
            row_members.push_back(new_scope_id);
            self.open_scope_stack.push_back(new_scope_id);

            self.commit_row(line_idx, row_members);
        }

        self.scanned_through_line = target_line + 1;
        if target_line == line_texts.len() - 1 {
            self.finalize_to_eof(line_texts);
            return true;
        }

        false
    }

    fn ensure_row_for_line(&mut self, line_idx: usize) {
        if self.line_to_scopes.len() <= line_idx {
            while self.line_to_scopes.len() <= line_idx {
                self.line_to_scopes.push_back(Vector::new());
            }
        } else {
            if let Some(row) = self.line_to_scopes.get_mut(line_idx) {
                *row = Vector::new();
            }
        }
    }

    fn commit_row(&mut self, line_idx: usize, scope_ids: Vector<IndentScopeId>) {
        let mut scope_ids: Vec<IndentScopeId> = scope_ids.into_iter().collect();
        scope_ids.sort_by_key(|scope_id| {
            let scope = &self.scopes[*scope_id];
            (
                scope.start_line,
                std::cmp::Reverse(scope.end_line.unwrap_or(usize::MAX)),
            )
        });
        if let Some(row) = self.line_to_scopes.get_mut(line_idx) {
            *row = scope_ids.into_iter().collect();
        }
    }

    fn close_scope_line(&mut self, scope_id: IndentScopeId, end_line: usize) -> bool {
        if let Some(scope) = self.scopes.get_mut(scope_id) {
            if scope.has_non_empty_inside && end_line > scope.start_line + 1 {
                scope.finalize(end_line);
                return true;
            }
            scope.invalidate();
        }
        false
    }

    fn close_scope_invalid(&mut self, scope_id: IndentScopeId, end_line: usize) {
        if let Some(scope) = self.scopes.get(scope_id) {
            self.remove_scope_memberships(scope.start_line, end_line, scope_id);
        }
        self.open_scope_stack.pop_back();
    }

    fn trim_invalid_branch(&mut self, branch_start: usize, end_line: usize) {
        let prefix_len = self
            .scopes
            .iter()
            .collect::<Vec<_>>()
            .partition_point(|scope| scope.start_line < branch_start);
        self.scopes.truncate(prefix_len);
        self.open_scope_stack = self
            .open_scope_stack
            .iter()
            .copied()
            .filter(|scope_id| *scope_id < prefix_len)
            .collect();
        for row in self
            .line_to_scopes
            .iter_mut()
            .take(end_line + 1)
            .skip(branch_start)
        {
            *row = row
                .iter()
                .copied()
                .filter(|scope_id| *scope_id < prefix_len)
                .collect();
        }
    }

    fn finalize_to_eof(&mut self, line_texts: &[&str]) {
        if line_texts.is_empty() {
            return;
        }

        let last_line = line_texts.len() - 1;
        while let Some(&scope_id) = self.open_scope_stack.last() {
            if self.close_scope_line(scope_id, last_line) {
                self.open_scope_stack.pop_back();
                continue;
            }

            let branch_start = self.scopes[scope_id].start_line;
            self.close_scope_invalid(scope_id, last_line);
            self.trim_invalid_branch(branch_start, last_line);
        }
    }

    fn remove_scope_memberships(
        &mut self,
        start_line: usize,
        end_line: usize,
        scope_id: IndentScopeId,
    ) {
        for row in self
            .line_to_scopes
            .iter_mut()
            .take(end_line + 1)
            .skip(start_line)
        {
            *row = row
                .iter()
                .copied()
                .filter(|existing| *existing != scope_id)
                .collect();
        }
    }

    pub(crate) fn indent_scopes(&self) -> &Vector<IndentScope> {
        &self.scopes
    }

    pub(crate) fn line_indent_scope_ids(&self, line: usize) -> Option<&Vector<IndentScopeId>> {
        self.line_to_scopes.get(line)
    }
}

/// Buffer-owned cache state derived from the current text.
#[derive(Debug, Clone)]
pub struct BufferCache {
    syntax_cache: SyntaxCache,
    indent_scope_cache: IndentScopeCache,
    indent_scope_cache_stale: bool,
}

impl BufferCache {
    /// Creates an empty buffer cache for a syntax name.
    pub fn new(syntax_name: impl Into<SmolStr>) -> Self {
        Self {
            syntax_cache: SyntaxCache::new(syntax_name),
            indent_scope_cache: IndentScopeCache::empty(),
            indent_scope_cache_stale: true,
        }
    }

    /// Returns the canonical syntax name tracked by this buffer cache.
    pub fn syntax_name(&self) -> &str {
        self.syntax_cache.syntax_name()
    }

    /// Updates the cached syntax name, clearing cached data when it changes.
    pub fn set_syntax_name(&mut self, syntax_name: impl Into<SmolStr>) {
        let syntax_name = syntax_name.into();
        if self.syntax_cache.syntax_name() != syntax_name {
            self.syntax_cache.set_syntax_name(syntax_name);
            self.indent_scope_cache = IndentScopeCache::empty();
            self.indent_scope_cache_stale = true;
        }
    }

    /// Replaces the current cache with a newer snapshot.
    pub fn replace_with(&mut self, other: BufferCache) {
        *self = other;
    }

    /// Invalidates cached data from the provided line onward.
    pub fn invalidate_from(&mut self, line: usize) {
        if line >= self.syntax_cache.cached_line_count() {
            return;
        }
        self.syntax_cache.invalidate_from(line);
        self.indent_scope_cache.invalidate_from(line);
        self.indent_scope_cache_stale = true;
    }

    /// Returns cached spans for a line without computing any missing prefix.
    pub fn cached_spans_for_line(&self, line: usize) -> Option<Vec<SyntaxSpan>> {
        self.syntax_cache.cached_spans_for_line(line)
    }

    /// Returns how many leading lines currently have cached syntax data.
    pub fn cached_line_count(&self) -> usize {
        self.syntax_cache.cached_line_count()
    }

    /// Returns true when every line in the buffer has a cached syntax result.
    pub fn is_complete_for_line_count(&self, line_count: usize) -> bool {
        self.syntax_cache.is_complete_for_line_count(line_count)
    }

    /// Returns the cached spans for a line, computing any missing prefix first.
    pub fn spans_for_line(
        &mut self,
        syntax_name: &str,
        line_texts: &[&str],
        line: usize,
    ) -> Option<Vec<SyntaxSpan>> {
        self.syntax_cache
            .spans_for_line(syntax_name, line_texts, line)
    }

    /// Ensures syntax and indent cache data exists through the requested line.
    pub fn ensure_through(&mut self, syntax_name: &str, line_texts: &[&str], line: usize) {
        self.syntax_cache
            .ensure_through(syntax_name, line_texts, line);

        if line_texts.is_empty() {
            self.indent_scope_cache_stale = false;
            return;
        }

        let target_line = line.min(line_texts.len().saturating_sub(1));
        let tab_width = globals::with_config(|config| config.tab_width)
            .unwrap_or(4)
            .max(1);
        self.indent_scope_cache_stale =
            !self
                .indent_scope_cache
                .ensure_through(line_texts, target_line, tab_width);
    }

    /// Returns true when the indent-scope cache needs rebuilding.
    pub fn indent_scope_cache_stale(&self) -> bool {
        self.indent_scope_cache_stale
    }

    /// Returns all cached indent scopes for this buffer snapshot.
    pub fn indent_scopes(&self) -> &Vector<IndentScope> {
        self.indent_scope_cache.indent_scopes()
    }

    /// Returns cached containing indent scope ids for a line.
    pub fn line_indent_scope_ids(&self, line: usize) -> Option<&Vector<IndentScopeId>> {
        self.indent_scope_cache.line_indent_scope_ids(line)
    }
}

fn leading_indent_width(line: &str, tab_width: usize) -> usize {
    line.chars()
        .take_while(|ch| matches!(*ch, ' ' | '\t'))
        .fold(0, |acc, ch| acc + if ch == '\t' { tab_width } else { 1 })
}

struct BufferCacheRefreshJob {
    buffer_id: BufferId,
    generation: u64,
    syntax_name: SmolStr,
    cache: BufferCache,
    line_texts: Vector<Arc<str>>,
}

impl BufferCacheRefreshJob {
    fn new(
        buffer_id: BufferId,
        generation: u64,
        syntax_name: SmolStr,
        cache: BufferCache,
        line_texts: Vector<Arc<str>>,
    ) -> Self {
        Self {
            buffer_id,
            generation,
            syntax_name,
            cache,
            line_texts,
        }
    }
}

impl Job for BufferCacheRefreshJob {
    type Output = BufferCacheRefreshResult;

    fn run(mut self, context: &JobContext) -> Self::Output {
        let line_refs: Vec<&str> = self.line_texts.iter().map(|line| line.as_ref()).collect();
        if !line_refs.is_empty() {
            if self.cache.syntax_cache.lines.len() > line_refs.len() {
                self.cache.syntax_cache.lines.truncate(line_refs.len());
            }

            let target_line = line_refs.len() - 1;
            let mut state = self
                .cache
                .syntax_cache
                .lines
                .last()
                .map(|line| line.state.clone())
                .unwrap_or_default();
            let start_line = self.cache.syntax_cache.lines.len();

            for line_text in line_refs.iter().take(target_line + 1).skip(start_line) {
                if !context.is_current() {
                    tracing::debug!(
                        kind = %context.kind(),
                        generation = context.token().generation(),
                        "stopping stale buffer cache refresh job"
                    );
                    break;
                }

                let (spans, next_state) = tokenize_line(&self.syntax_name, line_text, state);
                self.cache
                    .syntax_cache
                    .lines
                    .push_back(SyntaxLine::new(spans, next_state.clone()));
                state = next_state;
            }

            if context.is_current() {
                let tab_width = globals::with_config(|config| config.tab_width)
                    .unwrap_or(4)
                    .max(1);
                self.cache.indent_scope_cache_stale = !self
                    .cache
                    .indent_scope_cache
                    .ensure_through(&line_refs, target_line, tab_width);
            }
        } else {
            self.cache.set_syntax_name(self.syntax_name.as_str());
        }

        BufferCacheRefreshResult {
            buffer_id: self.buffer_id,
            generation: self.generation,
            cache: self.cache,
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
                    let should_replace = chosen.as_ref().is_none_or(
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

fn tokenize_injection_body(
    injection_state: &mut RuleListInjectionState,
    body: &str,
    offset: usize,
) -> Vec<SyntaxSpan> {
    if let Some(nested) = injection_state.nested.as_mut() {
        return tokenize_nested_body(nested, body, offset);
    }

    if matches!(
        injection_state.fallback,
        InjectedSyntaxFallback::ParentStyle
    ) {
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
    fn sync_undo_snapshot_cache_if_current(&mut self) {
        if self.current_text_matches_undo_head() {
            self.undo_state
                .update_buffer_cache(self.buffer_cache.clone());
        }
    }

    /// Invalidates buffer-owned cache data from the given line onward.
    pub fn invalidate_syntax_from(&mut self, line: usize) {
        self.buffer_cache.invalidate_from(line);
        self.syntax_generation = self.syntax_generation.wrapping_add(1);
        self.syntax_background_generation = None;
        if line == 0 {
            self.refresh_syntax();
        }
    }

    /// Returns cached spans for a line without computing missing prefix data.
    pub fn cached_syntax_spans_for_line(&self, line: usize) -> Option<Vec<SyntaxSpan>> {
        self.buffer_cache.cached_spans_for_line(line)
    }

    /// Returns true when the indent-scope cache needs rebuilding.
    pub fn indent_scope_cache_stale(&self) -> bool {
        self.buffer_cache.indent_scope_cache_stale()
    }

    /// Returns all cached indent scopes for the current buffer snapshot.
    pub fn cached_indent_scopes(&self) -> &Vector<IndentScope> {
        self.buffer_cache.indent_scopes()
    }

    /// Returns cached containing indent scope ids for the requested line.
    pub fn cached_line_indent_scope_ids(&self, line: usize) -> Option<&Vector<IndentScopeId>> {
        self.buffer_cache.line_indent_scope_ids(line)
    }

    /// Returns true when all buffer lines currently have cached syntax data.
    pub fn syntax_cache_complete(&self) -> bool {
        self.buffer_cache
            .is_complete_for_line_count(self.line_count())
    }

    /// Returns how many leading lines currently have cached syntax spans.
    pub fn cached_syntax_line_count(&self) -> usize {
        self.buffer_cache.cached_line_count()
    }

    /// Returns the current syntax generation used to reject stale background results.
    pub fn syntax_generation(&self) -> u64 {
        self.syntax_generation
    }

    /// Returns true when a syntax catch-up job has been queued for the current generation.
    pub fn syntax_background_pending(&self) -> bool {
        self.syntax_background_generation.is_some()
    }

    /// Returns the highlighted spans for a line, computing them on demand.
    pub fn syntax_spans_for_line(&mut self, line: usize) -> Option<Vec<SyntaxSpan>> {
        let line_texts: Vec<&str> = self.lines.iter().map(|line| line.as_ref()).collect();
        let syntax_name = self.syntax_name().to_owned();
        let spans = self
            .buffer_cache
            .spans_for_line(&syntax_name, &line_texts, line);
        if spans.is_some() {
            self.sync_undo_snapshot_cache_if_current();
        }
        spans
    }

    /// Applies a background buffer cache refresh result when it still matches this buffer.
    pub fn apply_buffer_cache_refresh_result(&mut self, result: BufferCacheRefreshResult) -> bool {
        if result.generation != self.syntax_generation {
            return false;
        }

        self.buffer_cache.replace_with(result.cache);
        self.sync_undo_snapshot_cache_if_current();
        if self.syntax_background_generation == Some(result.generation) {
            self.syntax_background_generation = None;
        }
        true
    }

    /// Applies a background syntax catch-up result when it still matches this buffer.
    pub fn apply_syntax_catch_up_result(&mut self, result: SyntaxCatchUpResult) -> bool {
        self.apply_buffer_cache_refresh_result(result)
    }

    /// Requests background buffer cache refresh when the cache is incomplete.
    pub fn request_buffer_cache_refresh(&mut self, buffer_id: BufferId) {
        self.request_buffer_cache_refresh_with_priority(buffer_id, JobPriority::Background);
    }

    /// Requests buffer cache refresh with the given job priority when the cache is incomplete.
    ///
    /// Buffer cache refresh uses latest-only job submission so stale queued work for the same buffer
    /// can be pruned before it consumes worker time.
    pub fn request_buffer_cache_refresh_with_priority(
        &mut self,
        buffer_id: BufferId,
        priority: JobPriority,
    ) {
        if self.buffer_cache_complete() {
            return;
        }

        if self.syntax_background_generation == Some(self.syntax_generation) {
            return;
        }

        let job = BufferCacheRefreshJob::new(
            buffer_id,
            self.syntax_generation,
            self.syntax_name().to_owned().into(),
            self.buffer_cache.clone(),
            self.lines.clone(),
        );
        let kind = JobKind::new(format!("buffer-cache:{}", buffer_id.get()));
        let token = JobToken::new(self.syntax_generation);

        let submitted = globals::with_job_manager(|job_manager| {
            job_manager
                .map(|job_manager| {
                    job_manager
                        .submit_latest_only(kind.clone(), priority, token, job)
                        .is_ok()
                })
                .unwrap_or(false)
        });

        if submitted {
            self.syntax_background_generation = Some(self.syntax_generation);
        }
    }

    /// Requests background syntax catch-up when the cache is incomplete.
    pub fn request_syntax_catch_up(&mut self, buffer_id: BufferId) {
        self.request_buffer_cache_refresh(buffer_id);
    }

    /// Requests syntax catch-up with the given job priority when the cache is incomplete.
    ///
    /// Syntax catch-up uses latest-only job submission so stale queued work for the same buffer
    /// can be pruned before it consumes worker time.
    pub fn request_syntax_catch_up_with_priority(
        &mut self,
        buffer_id: BufferId,
        priority: JobPriority,
    ) {
        self.request_buffer_cache_refresh_with_priority(buffer_id, priority);
    }

    /// Ensures syntax data exists through a line without returning it.
    pub fn ensure_syntax_through(&mut self, line: usize) {
        let line_texts: Vec<&str> = self.lines.iter().map(|line| line.as_ref()).collect();
        let syntax_name = self.syntax_name().to_owned();
        self.buffer_cache
            .ensure_through(&syntax_name, &line_texts, line);
        self.sync_undo_snapshot_cache_if_current();
    }

    /// Returns true when the complete buffer cache is available for the current text.
    pub fn buffer_cache_complete(&self) -> bool {
        self.buffer_cache
            .is_complete_for_line_count(self.line_count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{BufferId, Cursor};
    use crate::config::Config;
    use crate::globals;
    use crate::job::{JobEvent, JobHandle, JobKind, JobPriority, JobToken};
    use crate::path::AbsolutePath;
    use std::sync::Mutex;
    use std::thread;
    use std::time::{Duration, Instant};

    fn tag(value: &str) -> Tag {
        Tag::parse(value).expect("valid tag")
    }

    fn temp_path_with_ext(name: &str, ext: &str) -> AbsolutePath {
        let path = std::env::temp_dir().join(format!(
            "urvim-syntax-tests-{}-{}.{}",
            name,
            std::process::id(),
            ext
        ));
        AbsolutePath::from_path(path.as_path()).expect("temp path should be absolute")
    }

    fn wait_for_event(handle: &JobHandle) -> JobEvent {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(event) = handle.poll_completion() {
                return event;
            }
            assert!(Instant::now() < deadline, "timed out waiting for job event");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn scope_tuples(buffer: &Buffer) -> Vec<(usize, Option<usize>, usize)> {
        buffer
            .cached_indent_scopes()
            .iter()
            .filter(|scope| scope.is_active())
            .map(|scope| (scope.start_line, scope.end_line, scope.indent_width))
            .collect()
    }

    #[test]
    fn parent_style_fallback_uses_the_injection_opener_tag() {
        let definition = SyntaxDefinition {
            metadata: crate::syntax::SyntaxMetadata {
                name: SmolStr::new("example"),
                display_name: SmolStr::new("Example"),
                alias: Vec::new(),
                comment_prefix: None,
                glyph: None,
                glyph_color: None,
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
        assert!(
            opener_spans
                .iter()
                .any(|span| span.style == tag("markup.code"))
        );

        let (body_spans, state) = tokenize_rule_list_line(&definition, "body text", state);
        assert_eq!(body_spans.len(), 1);
        assert_eq!(body_spans[0].style, tag("markup.code"));

        let (closing_spans, _) = tokenize_rule_list_line(&definition, "```", state);
        assert_eq!(closing_spans.len(), 1);
        assert_eq!(closing_spans[0].style, tag("markup.code"));
    }

    #[test]
    fn cached_spans_can_be_replaced_and_read_without_recomputing() {
        let mut cache = SyntaxCache::new("plain");
        assert_eq!(cache.cached_spans_for_line(0), None);

        cache.lines.push_back(SyntaxLine::new(
            vec![SyntaxSpan::new(0, 3, tag("text.plain"))],
            SyntaxState::default(),
        ));

        let cached = cache
            .cached_spans_for_line(0)
            .expect("cached spans should be available");
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].style, tag("text.plain"));

        let mut replacement = SyntaxCache::new("plain");
        replacement.lines.push_back(SyntaxLine::new(
            vec![SyntaxSpan::new(0, 4, tag("text.replaced"))],
            SyntaxState::default(),
        ));
        replacement.lines.push_back(SyntaxLine::new(
            vec![SyntaxSpan::new(0, 5, tag("text.replaced"))],
            SyntaxState::default(),
        ));

        cache = replacement;

        let cached = cache
            .cached_spans_for_line(1)
            .expect("replacement cache should have line 1");
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].style, tag("text.replaced"));
    }

    #[test]
    fn undo_and_redo_restore_syntax_cache_snapshots() {
        let mut buffer = Buffer::from_str("root\n  child\nroot-close");

        buffer.ensure_syntax_through(2);
        let initial_result = SyntaxCatchUpResult {
            buffer_id: BufferId::new(1),
            generation: buffer.syntax_generation(),
            cache: buffer.buffer_cache.clone(),
        };
        assert!(buffer.apply_syntax_catch_up_result(initial_result));
        let initial_scopes = scope_tuples(&buffer);
        assert!(buffer.syntax_cache_complete());
        assert!(!buffer.indent_scope_cache_stale());

        buffer.insert_text(Cursor::new(2, 0), "  nested\n");
        buffer.push_snapshot(Cursor::new(3, 0));
        buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
        let edited_result = SyntaxCatchUpResult {
            buffer_id: BufferId::new(1),
            generation: buffer.syntax_generation(),
            cache: buffer.buffer_cache.clone(),
        };
        assert!(buffer.apply_syntax_catch_up_result(edited_result));
        let edited_scopes = scope_tuples(&buffer);
        assert!(buffer.syntax_cache_complete());
        assert!(!buffer.indent_scope_cache_stale());

        assert!(buffer.undo().is_some());
        assert_eq!(scope_tuples(&buffer), initial_scopes);
        assert!(buffer.syntax_cache_complete());
        assert!(!buffer.indent_scope_cache_stale());

        assert!(buffer.redo().is_some());
        assert_eq!(scope_tuples(&buffer), edited_scopes);
        assert!(buffer.syntax_cache_complete());
        assert!(!buffer.indent_scope_cache_stale());
    }

    #[test]
    fn undo_recomputes_highlighting_after_line_insert_before_assert_macro() {
        let path = temp_path_with_ext("undo-rust-assert", "rs");
        let original = "fn main() {\n    assert!(true);\n}\n";
        let mut buffer = Buffer::from_str_with_path(original, path.clone());

        buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
        let expected = buffer
            .syntax_spans_for_line(1)
            .expect("assert line should exist")
            .to_vec();

        buffer.insert_lines_before(1, 1);
        buffer.push_snapshot(Cursor::new(2, 0));
        let result = BufferCacheRefreshResult {
            buffer_id: BufferId::new(1),
            generation: buffer.syntax_generation(),
            cache: buffer.buffer_cache.clone(),
        };
        assert!(buffer.apply_buffer_cache_refresh_result(result));

        assert!(buffer.undo().is_some());
        let undo_spans = buffer
            .syntax_spans_for_line(1)
            .expect("assert line should exist after undo");

        assert_eq!(undo_spans, expected);
    }

    #[test]
    fn indent_scope_cache_is_invalidated_with_syntax_invalidation() {
        let mut buffer = Buffer::from_str("a\n  b\na");
        buffer.ensure_syntax_through(2);
        assert!(!buffer.indent_scope_cache_stale());
        assert_eq!(scope_tuples(&buffer), vec![(0, Some(2), 0)]);

        buffer.invalidate_syntax_from(1);

        assert!(buffer.indent_scope_cache_stale());
        assert!(scope_tuples(&buffer).is_empty());
        assert!(buffer.cached_line_indent_scope_ids(0).is_none());
        assert!(buffer.cached_line_indent_scope_ids(1).is_none());
    }

    #[test]
    fn indent_scope_builder_matches_visual_width_for_tabs_and_spaces() {
        let _config_guard = globals::set_test_config(Config {
            tab_width: 4,
            ..Config::default()
        });
        let mut buffer = Buffer::from_str("\tstart\n\t\tbody\n    close");

        buffer.ensure_syntax_through(2);

        assert_eq!(scope_tuples(&buffer), vec![(0, Some(2), 4)]);
    }

    #[test]
    fn indent_scope_builder_supports_nested_scopes() {
        let mut buffer =
            Buffer::from_str("root\n  inner-open\n    inner-body\n  inner-close\nroot-close");

        buffer.ensure_syntax_through(4);

        assert_eq!(
            scope_tuples(&buffer),
            vec![(0, Some(4), 0), (1, Some(3), 2)]
        );
        let containing = buffer
            .cached_line_indent_scope_ids(2)
            .expect("line 2 should have containing scopes");
        assert_eq!(containing.len(), 2);
        let first = &buffer.cached_indent_scopes()[containing[0]];
        let second = &buffer.cached_indent_scopes()[containing[1]];
        assert_eq!((first.start_line, first.end_line), (0, Some(4)));
        assert_eq!((second.start_line, second.end_line), (1, Some(3)));
    }

    #[test]
    fn indent_scope_builder_closes_unmatched_scopes_at_eof() {
        let mut buffer = Buffer::from_str("outer\n  body\n    tail");

        buffer.ensure_syntax_through(2);

        assert_eq!(scope_tuples(&buffer), vec![(0, Some(2), 0)]);
    }

    #[test]
    fn indent_scope_builder_treats_whitespace_only_lines_as_non_empty() {
        let mut with_whitespace = Buffer::from_str("start\n   \nstart");
        with_whitespace.ensure_syntax_through(2);
        assert_eq!(scope_tuples(&with_whitespace), vec![(0, Some(2), 0)]);

        let mut with_empty = Buffer::from_str("start\n\nstart");
        with_empty.ensure_syntax_through(2);
        assert!(scope_tuples(&with_empty).is_empty());
    }

    #[test]
    fn viewport_scope_ensure_establishes_only_the_visible_prefix() {
        let mut buffer = Buffer::from_str("root\n  child\n    leaf\n  child-close\nroot-close");

        buffer.ensure_syntax_through(2);

        assert!(buffer.indent_scope_cache_stale());
        assert!(buffer.cached_line_indent_scope_ids(2).is_some());
        assert!(buffer.cached_line_indent_scope_ids(3).is_none());
        assert!(buffer.cached_line_indent_scope_ids(4).is_none());
        assert_eq!(
            scope_tuples(&buffer),
            vec![(0, None, 0), (1, None, 2), (2, None, 4)]
        );
    }

    #[test]
    fn invalidated_suffix_rescans_from_the_saved_frontier() {
        let mut buffer = Buffer::from_str("root\n  child\n    leaf\n  child-close\nroot-close");

        buffer.ensure_syntax_through(2);
        buffer.invalidate_syntax_from(1);

        assert!(buffer.indent_scope_cache_stale());
        assert!(scope_tuples(&buffer).is_empty());
        assert!(buffer.cached_line_indent_scope_ids(0).is_none());
        assert!(buffer.cached_line_indent_scope_ids(1).is_none());

        buffer.ensure_syntax_through(4);

        assert!(!buffer.indent_scope_cache_stale());
        assert_eq!(
            scope_tuples(&buffer),
            vec![(0, Some(4), 0), (1, Some(3), 2)]
        );
    }

    #[test]
    fn closing_line_scope_ids_do_not_duplicate() {
        let mut buffer = Buffer::from_str("outer\n  body\nouter");
        buffer.ensure_syntax_through(2);

        let scope_ids = buffer
            .cached_line_indent_scope_ids(2)
            .expect("closing line should have containing scopes");
        assert_eq!(scope_ids.iter().copied().collect::<Vec<_>>(), vec![0]);
    }

    #[test]
    fn nested_scope_survives_dedent_to_sibling_line() {
        let mut buffer = Buffer::from_str("root\n  a\n    body\n  b\nroot-close");
        buffer.ensure_syntax_through(4);

        assert_eq!(
            scope_tuples(&buffer),
            vec![(0, Some(4), 0), (1, Some(3), 2)]
        );
    }

    #[test]
    fn empty_lines_do_not_close_outer_scope_early() {
        let mut buffer = Buffer::from_str("fn main() {\n    let x = 1;\n\n    let y = 2;\n}");
        buffer.ensure_syntax_through(4);

        assert_eq!(scope_tuples(&buffer), vec![(0, Some(4), 0)]);
    }

    #[test]
    fn empty_line_inherits_containing_scopes() {
        let mut buffer =
            Buffer::from_str("root\n  inner-open\n    body\n\n  inner-close\nroot-close");
        buffer.ensure_syntax_through(5);

        let empty_line_scope_ids = buffer
            .cached_line_indent_scope_ids(3)
            .expect("empty line should have containing scopes");
        assert_eq!(empty_line_scope_ids.len(), 2);
        let outer = &buffer.cached_indent_scopes()[empty_line_scope_ids[0]];
        let inner = &buffer.cached_indent_scopes()[empty_line_scope_ids[1]];
        assert_eq!((outer.start_line, outer.end_line), (0, Some(5)));
        assert_eq!((inner.start_line, inner.end_line), (1, Some(4)));
    }

    #[test]
    fn dedent_popping_multiple_levels_preserves_outer_and_sibling_scopes() {
        let mut buffer = Buffer::from_str(
            "root\n  lvl1-open\n    lvl2-open\n      deep\n  sibling-open\n    sibling-body\n  sibling-close\nroot-close",
        );
        buffer.ensure_syntax_through(7);

        assert_eq!(
            scope_tuples(&buffer),
            vec![(0, Some(7), 0), (1, Some(4), 2), (4, Some(6), 2)]
        );
    }

    #[test]
    fn partial_then_complete_ensure_preserves_function_scope_end() {
        let mut buffer = Buffer::from_str(
            "fn helper() {}\n\nfn main() {\n    let x = 1;\n\n    if x > 0 {\n        println!(\"x\");\n    }\n\n    let y = 2;\n}\n\nfn after() {}\n",
        );

        buffer.ensure_syntax_through(6);
        assert!(buffer.indent_scope_cache_stale());

        buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
        assert!(!buffer.indent_scope_cache_stale());

        let main_scope = buffer
            .cached_indent_scopes()
            .iter()
            .find(|scope| scope.start_line == 2)
            .expect("main scope should exist after complete ensure");
        assert_eq!(main_scope.end_line, Some(10));
    }

    #[test]
    fn invalidate_from_eof_keeps_cache_complete() {
        let mut buffer = Buffer::from_str("one\ntwo\nthree");
        buffer.ensure_syntax_through(2);
        assert!(!buffer.indent_scope_cache_stale());

        buffer.invalidate_syntax_from(buffer.line_count());

        assert!(buffer.syntax_cache_complete());
        assert!(!buffer.indent_scope_cache_stale());
    }

    #[test]
    fn inserting_line_keeps_parent_scope_active_for_following_lines() {
        let mut buffer = Buffer::from_str("root\n  first\nroot-close");
        buffer.ensure_syntax_through(2);
        assert_eq!(scope_tuples(&buffer), vec![(0, Some(2), 0)]);

        buffer.insert_text(Cursor::new(1, 0), "  inserted\n");
        buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));

        assert_eq!(scope_tuples(&buffer), vec![(0, Some(3), 0)]);
        let line_scope_ids = buffer
            .cached_line_indent_scope_ids(2)
            .expect("line after insertion should have scope ids");
        assert_eq!(line_scope_ids.iter().copied().collect::<Vec<_>>(), vec![0]);
    }

    #[test]
    fn background_generation_is_cleared_when_syntax_is_invalidated() {
        let mut buffer = Buffer::from_str("line 1\nline 2");
        buffer.syntax_generation = 7;
        buffer.syntax_background_generation = Some(7);
        buffer.ensure_syntax_through(1);

        buffer.invalidate_syntax_from(1);

        assert_eq!(buffer.syntax_generation(), 8);
        assert_eq!(buffer.syntax_background_generation, None);
        assert!(buffer.cached_syntax_spans_for_line(0).is_some());
        assert_eq!(buffer.cached_syntax_spans_for_line(1), None);
    }

    #[test]
    fn complete_cache_requests_no_background_catch_up() {
        let mut buffer = Buffer::from_str("line 1");
        buffer.ensure_syntax_through(0);
        buffer.syntax_background_generation = None;

        buffer.request_syntax_catch_up(BufferId::new(1));

        assert_eq!(buffer.syntax_background_generation, None);
    }

    #[test]
    fn background_catch_up_job_populates_offscreen_spans() {
        let path = temp_path_with_ext("background-catch-up", "rs");
        let text = std::iter::repeat_n(
            "fn main() { let value: Option<String> = Some(\"hi\"); } // note",
            64,
        )
        .collect::<Vec<_>>()
        .join("\n");
        let buffer = Buffer::from_str_with_path(&text, path);
        let handle = JobHandle::new();
        let token = JobToken::new(buffer.syntax_generation());
        let job = BufferCacheRefreshJob::new(
            BufferId::new(1),
            buffer.syntax_generation(),
            buffer.syntax_name().to_owned().into(),
            buffer.buffer_cache.clone(),
            buffer.lines.clone(),
        );

        handle
            .submit(
                JobKind::new("syntax:background"),
                JobPriority::Background,
                token,
                job,
            )
            .expect("syntax catch-up job should submit");

        let event = wait_for_event(&handle);
        let (_kind, _token, result) = event
            .into_completed_output::<SyntaxCatchUpResult>()
            .expect("syntax catch-up output should downcast");

        assert!(result.cache.cached_spans_for_line(50).is_some());
        assert!(result.cache.is_complete_for_line_count(64));

        handle.shutdown();
    }

    #[test]
    fn latest_only_syntax_catch_up_skips_stale_queue_entries() {
        let handle = JobHandle::new();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_blocker = Arc::clone(&gate);

        struct GateJob {
            gate: Arc<(Mutex<bool>, std::sync::Condvar)>,
        }

        impl Job for GateJob {
            type Output = ();

            fn run(self, _context: &JobContext) -> Self::Output {
                let (lock, cvar) = &*self.gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    open = cvar.wait(open).unwrap();
                }
            }
        }

        handle
            .submit(
                JobKind::new("blocker"),
                JobPriority::Foreground,
                JobToken::new(1),
                GateJob {
                    gate: gate_for_blocker,
                },
            )
            .expect("blocker job should submit");

        thread::sleep(Duration::from_millis(25));

        let old_path = temp_path_with_ext("latest-only-old", "rs");
        let old_buffer = Buffer::from_str_with_path("fn main() { let old = 1; }", old_path);
        let new_path = temp_path_with_ext("latest-only-new", "rs");
        let new_text = std::iter::repeat_n(
            "fn main() { let value: Option<String> = Some(\"hi\"); } // note",
            32,
        )
        .collect::<Vec<_>>()
        .join("\n");
        let new_buffer = Buffer::from_str_with_path(&new_text, new_path);

        handle
            .submit_latest_only(
                JobKind::new("syntax:1"),
                JobPriority::Background,
                JobToken::new(1),
                BufferCacheRefreshJob::new(
                    BufferId::new(1),
                    old_buffer.syntax_generation(),
                    old_buffer.syntax_name().to_owned().into(),
                    old_buffer.buffer_cache.clone(),
                    old_buffer.lines.clone(),
                ),
            )
            .expect("old syntax job should submit");

        handle
            .submit_latest_only(
                JobKind::new("syntax:1"),
                JobPriority::Background,
                JobToken::new(2),
                BufferCacheRefreshJob::new(
                    BufferId::new(1),
                    new_buffer.syntax_generation(),
                    new_buffer.syntax_name().to_owned().into(),
                    new_buffer.buffer_cache.clone(),
                    new_buffer.lines.clone(),
                ),
            )
            .expect("new syntax job should submit");

        {
            let (lock, cvar) = &*gate;
            let mut open = lock.lock().unwrap();
            *open = true;
            cvar.notify_all();
        }

        let blocker_event = wait_for_event(&handle);
        assert_eq!(blocker_event.kind().as_str(), "blocker");

        let syntax_event = wait_for_event(&handle);
        let (_kind, token, result) = syntax_event
            .into_completed_output::<SyntaxCatchUpResult>()
            .expect("latest syntax job should complete");
        assert_eq!(token.generation(), 2);
        assert!(result.cache.is_complete_for_line_count(32));

        handle.shutdown();
    }

    #[test]
    fn stale_background_result_is_rejected_after_invalidation() {
        let path = temp_path_with_ext("stale-result", "rs");
        let mut buffer = Buffer::from_str_with_path("fn main() {}", path);
        let result = SyntaxCatchUpResult {
            buffer_id: BufferId::new(1),
            generation: buffer.syntax_generation(),
            cache: buffer.buffer_cache.clone(),
        };

        buffer.invalidate_syntax_from(0);

        assert!(!buffer.apply_syntax_catch_up_result(result));
    }
}
