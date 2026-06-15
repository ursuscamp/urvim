//! Syntax highlighting tokenizers and indent-scope cache primitives.

use crate::background::{JobContext, JobEvent, JobKind, JobPayload, JobToken};
use crate::buffer::Buffer;
use crate::buffer::BufferId;
use crate::buffer::{BufferEditEffect, DiffRefreshJob, PieceTable, TextRef, TextSnapshot};
use crate::globals;
use crate::syntax::{SyntaxDefinition, SyntaxTokenizer, builtin_syntax_registry};
use crate::theme::Tag;
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::sync::Arc;

const SYNTAX_REFRESH_UPDATE_LINES: usize = 512;
const LINE_FINGERPRINT_OFFSET: u64 = 0xcbf29ce484222325;
const LINE_FINGERPRINT_PRIME: u64 = 0x100000001b3;
const CONTEXT_ID_OFFSET: u64 = 0xcbf29ce484222325;
const CONTEXT_ID_PRIME: u64 = 0x100000001b3;

/// Stable identity for one line's text used by syntax cache reconvergence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineFingerprint {
    hash: u64,
    len: usize,
}

impl LineFingerprint {
    /// Returns a fingerprint for contiguous text.
    pub fn new(text: &str) -> Self {
        let mut builder = LineFingerprintBuilder::new();
        builder.write(text.as_bytes());
        builder.finish()
    }

    /// Returns a fingerprint for non-contiguous text chunks.
    pub fn from_chunks<'a>(chunks: impl IntoIterator<Item = &'a str>) -> Self {
        let mut builder = LineFingerprintBuilder::new();
        for chunk in chunks {
            builder.write(chunk.as_bytes());
        }
        builder.finish()
    }

    fn from_text_ref(text: &(impl TextRef + ?Sized)) -> Self {
        Self::from_chunks(text.chunks())
    }
}

/// Stable identity for one syntax context stack entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub(crate) struct ContextId(u64);

impl ContextId {
    /// Creates a compile-time context id from a syntax namespace and local name.
    pub(crate) const fn new(namespace: &str, name: &str) -> Self {
        Self(hash_context_id(namespace, name))
    }
}

const fn hash_context_id(namespace: &str, name: &str) -> u64 {
    let mut hash = CONTEXT_ID_OFFSET;
    let namespace_bytes = namespace.as_bytes();
    let mut index = 0;
    while index < namespace_bytes.len() {
        hash ^= namespace_bytes[index] as u64;
        hash = hash.wrapping_mul(CONTEXT_ID_PRIME);
        index += 1;
    }

    hash ^= 0xff;
    hash = hash.wrapping_mul(CONTEXT_ID_PRIME);

    let name_bytes = name.as_bytes();
    index = 0;
    while index < name_bytes.len() {
        hash ^= name_bytes[index] as u64;
        hash = hash.wrapping_mul(CONTEXT_ID_PRIME);
        index += 1;
    }

    hash
}

#[derive(Debug, Clone, Copy)]
struct LineFingerprintBuilder {
    hash: u64,
    len: usize,
}

impl LineFingerprintBuilder {
    fn new() -> Self {
        Self {
            hash: LINE_FINGERPRINT_OFFSET,
            len: 0,
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.len += bytes.len();
        for byte in bytes {
            self.hash ^= u64::from(*byte);
            self.hash = self.hash.wrapping_mul(LINE_FINGERPRINT_PRIME);
        }
    }

    fn finish(self) -> LineFingerprint {
        LineFingerprint {
            hash: self.hash,
            len: self.len,
        }
    }
}

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

/// Tokenizer-scoped identifier used to match syntax fold open and close events.
pub type SyntaxFoldKind = u32;

/// Direction of a fold event produced by a tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxFoldEventKind {
    /// Opens a new fold region.
    Open,
    /// Closes the nearest matching open fold region.
    Close,
}

/// A line-based fold event emitted by a tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxFoldEvent {
    /// Whether this event opens or closes a fold region.
    pub kind: SyntaxFoldEventKind,
    /// Tokenizer-scoped kind used to match opens with closes.
    pub fold_kind: SyntaxFoldKind,
    /// Number of lines before the event line where a close should end.
    pub close_line_offset: usize,
}

impl SyntaxFoldEvent {
    /// Creates a new syntax fold event.
    pub fn new(kind: SyntaxFoldEventKind, fold_kind: SyntaxFoldKind) -> Self {
        Self {
            kind,
            fold_kind,
            close_line_offset: 0,
        }
    }

    /// Creates a close event whose region ends before the event line.
    pub fn close_before_current_line(fold_kind: SyntaxFoldKind) -> Self {
        Self {
            kind: SyntaxFoldEventKind::Close,
            fold_kind,
            close_line_offset: 1,
        }
    }
}

/// Result of tokenizing one line with a builtin scanner.
#[derive(Debug, Clone)]
pub struct SyntaxLineResult {
    /// Highlighted spans for the line.
    pub spans: Vec<SyntaxSpan>,
    /// Fold events emitted while scanning the line.
    pub fold_events: Vec<SyntaxFoldEvent>,
    /// State to pass to the next line.
    pub state: SyntaxState,
}

impl From<(Vec<SyntaxSpan>, SyntaxState)> for SyntaxLineResult {
    fn from((spans, state): (Vec<SyntaxSpan>, SyntaxState)) -> Self {
        Self {
            spans,
            fold_events: Vec::new(),
            state,
        }
    }
}

#[derive(Debug, Clone)]
struct SyntaxLine {
    fingerprint: LineFingerprint,
    spans: Vec<SyntaxSpan>,
    fold_events: Vec<SyntaxFoldEvent>,
    state: SyntaxState,
}

struct SyntaxLineView<'a> {
    fingerprint: LineFingerprint,
    spans: &'a [SyntaxSpan],
    state: &'a SyntaxState,
}

#[derive(Debug, Clone)]
pub struct SyntaxCache {
    syntax_name: SmolStr,
    lines: Vec<Option<SyntaxLine>>,
    fold_regions: Vec<SyntaxFoldRegion>,
}

/// A contiguous line range produced by matching syntax fold events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxFoldRegion {
    /// Inclusive starting line index.
    pub start_line: usize,
    /// Inclusive ending line index.
    pub end_line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingFoldOpen {
    line: usize,
    fold_kind: SyntaxFoldKind,
}

impl SyntaxCache {
    /// Creates an empty syntax cache for a syntax name.
    pub fn new(syntax_name: impl Into<SmolStr>) -> Self {
        Self {
            syntax_name: syntax_name.into(),
            lines: Vec::new(),
            fold_regions: Vec::new(),
        }
    }

    /// Updates the cached syntax name, clearing cached data when it changes.
    pub fn set_syntax_name(&mut self, syntax_name: impl Into<SmolStr>) {
        let syntax_name = syntax_name.into();
        if self.syntax_name != syntax_name {
            self.syntax_name = syntax_name;
            self.lines = Vec::new();
            self.fold_regions = Vec::new();
        }
    }

    /// Returns the canonical syntax name tracked by this cache.
    pub fn syntax_name(&self) -> &str {
        &self.syntax_name
    }

    /// Applies structural edit effects to cached syntax lines.
    pub fn apply_edits(&mut self, effects: &[BufferEditEffect]) {
        for effect in effects {
            self.apply_edit_effect(*effect);
        }
    }

    fn apply_edit_effect(&mut self, effect: BufferEditEffect) {
        self.apply_fold_region_edit_effect(effect);

        if self.lines.len() < effect.start_line {
            self.lines.resize_with(effect.start_line, || None);
        }

        let start = effect.start_line.min(self.lines.len());
        let end = start
            .saturating_add(effect.old_line_count)
            .min(self.lines.len());
        self.lines
            .splice(start..end, std::iter::repeat_n(None, effect.new_line_count));

        let after = start.saturating_add(effect.new_line_count);
        if let Some(slot) = self.lines.get_mut(after) {
            *slot = None;
        }
    }

    fn apply_fold_region_edit_effect(&mut self, effect: BufferEditEffect) {
        if self.fold_regions.is_empty() {
            return;
        }

        let start = effect.start_line;
        let delta = effect.new_line_count as isize - effect.old_line_count as isize;
        if delta == 0 && effect.old_line_count == 1 && effect.new_line_count == 1 {
            self.fold_regions.retain(|region| {
                let edit_touches_boundary = region.start_line == start || region.end_line == start;
                let edit_is_inside_region = region.start_line < start && region.end_line > start;
                !edit_touches_boundary || edit_is_inside_region
            });
            return;
        }

        if delta > 0 {
            let delta = delta as usize;
            for region in &mut self.fold_regions {
                if region.start_line >= start {
                    region.start_line = region.start_line.saturating_add(delta);
                    region.end_line = region.end_line.saturating_add(delta);
                } else if region.end_line >= start {
                    region.end_line = region.end_line.saturating_add(delta);
                }
            }
            return;
        }

        let old_end_exclusive = start.saturating_add(effect.old_line_count);
        self.fold_regions.retain_mut(|region| {
            if region.end_line < start {
                return true;
            }

            if region.start_line >= old_end_exclusive {
                region.start_line = region.start_line.saturating_add_signed(delta);
                region.end_line = region.end_line.saturating_add_signed(delta);
                return true;
            }

            false
        });
    }

    fn line_view(&self, line: usize) -> Option<SyntaxLineView<'_>> {
        self.lines
            .get(line)
            .and_then(Option::as_ref)
            .map(|line| SyntaxLineView {
                fingerprint: line.fingerprint,
                spans: &line.spans,
                state: &line.state,
            })
    }

    fn truncate_cached_lines(&mut self, line_count: usize) {
        self.lines.truncate(line_count);
        self.fold_regions
            .retain(|region| region.end_line < line_count);
    }

    #[cfg(test)]
    fn push_cached_line_for_test(
        &mut self,
        line: impl AsRef<str>,
        spans: Vec<SyntaxSpan>,
        state: SyntaxState,
    ) {
        self.lines.push(Some(SyntaxLine {
            fingerprint: LineFingerprint::new(line.as_ref()),
            spans,
            fold_events: Vec::new(),
            state,
        }));
    }

    /// Invalidates cached syntax data from the provided line onward.
    pub fn invalidate_from(&mut self, line: usize, _line_delta: isize) {
        if line >= self.lines.len() {
            return;
        }
        self.lines[line] = None;
        self.fold_regions.retain(|region| region.end_line < line);
    }

    /// Returns cached spans for a line without computing any missing prefix.
    pub fn cached_spans_for_line(&self, line: usize) -> Option<Vec<SyntaxSpan>> {
        self.cached_spans_for_line_ref(line)
            .map(|spans| spans.to_vec())
    }

    /// Returns cached spans for a line without cloning or computing missing prefix data.
    pub fn cached_spans_for_line_ref(&self, line: usize) -> Option<&[SyntaxSpan]> {
        self.line_view(line).map(|entry| entry.spans)
    }

    /// Returns all syntax fold regions derived from the current cache.
    pub fn fold_regions(&self) -> &[SyntaxFoldRegion] {
        &self.fold_regions
    }

    /// Returns the fold region starting at the requested line, if any.
    pub fn fold_region_starting_at(&self, line: usize) -> Option<SyntaxFoldRegion> {
        self.fold_regions
            .iter()
            .filter(|region| region.start_line == line)
            .max_by_key(|region| region.end_line)
            .copied()
    }

    /// Returns the innermost fold region containing the requested line.
    ///
    /// When multiple regions contain the line, the one with the largest start
    /// line (i.e. the innermost nested region) is returned.
    pub fn fold_region_containing(&self, line: usize) -> Option<SyntaxFoldRegion> {
        self.fold_regions
            .iter()
            .filter(|region| region.start_line < line && region.end_line >= line)
            .max_by_key(|region| region.start_line)
            .copied()
    }

    /// Returns cached spans for rendering, falling back to the last stale snapshot.
    pub fn render_spans_for_line_ref(
        &self,
        line: usize,
        current_fingerprint: LineFingerprint,
    ) -> Option<&[SyntaxSpan]> {
        if let Some(line_view) = self.line_view(line) {
            if line_view.fingerprint == current_fingerprint {
                return Some(line_view.spans);
            }
        }

        None
    }

    /// Returns how many leading lines currently have cached syntax data.
    pub fn cached_line_count(&self) -> usize {
        self.lines.iter().take_while(|line| line.is_some()).count()
    }

    fn first_missing_line(&self) -> Option<usize> {
        self.lines.iter().position(Option::is_none)
    }

    /// Returns true when every line in the buffer has a cached syntax result.
    pub fn is_complete_for_line_count(&self, line_count: usize) -> bool {
        self.lines.len() >= line_count && self.lines.iter().take(line_count).all(Option::is_some)
    }

    /// Returns the cached spans for a line, computing any missing prefix first.
    pub fn spans_for_line(
        &mut self,
        syntax_name: &str,
        line_texts: &PieceTable,
        line: usize,
    ) -> Option<Vec<SyntaxSpan>> {
        let syntax_definition = syntax_definition(syntax_name)?;
        self.spans_for_line_with_definition(syntax_definition.as_ref(), line_texts, line)
    }

    /// Ensures syntax data exists through the requested line.
    fn ensure_through(&mut self, syntax_name: &str, line_texts: &PieceTable, line: usize) {
        let Some(syntax_definition) = syntax_definition(syntax_name) else {
            return;
        };
        self.ensure_through_with_definition(syntax_definition.as_ref(), line_texts, line, || true);
    }

    #[cfg(test)]
    fn ensure_through_with<F>(
        &mut self,
        syntax_name: &str,
        line_texts: &PieceTable,
        line: usize,
        should_continue: F,
    ) where
        F: FnMut() -> bool,
    {
        let Some(syntax_definition) = syntax_definition(syntax_name) else {
            return;
        };

        self.ensure_through_with_definition(
            syntax_definition.as_ref(),
            line_texts,
            line,
            should_continue,
        );
    }

    fn spans_for_line_with_definition(
        &mut self,
        syntax_definition: &SyntaxDefinition,
        line_texts: &PieceTable,
        line: usize,
    ) -> Option<Vec<SyntaxSpan>> {
        self.set_syntax_name(syntax_definition.name());
        if line >= line_texts.line_count() {
            return None;
        }

        self.ensure_through_with_definition(syntax_definition, line_texts, line, || true);
        self.cached_spans_for_line(line)
    }

    fn ensure_through_with_definition<F>(
        &mut self,
        syntax_definition: &SyntaxDefinition,
        line_texts: &PieceTable,
        line: usize,
        should_continue: F,
    ) where
        F: FnMut() -> bool,
    {
        self.ensure_through_with_progress(
            syntax_definition,
            line_texts,
            line,
            should_continue,
            |_| {},
        );
    }

    fn ensure_through_with_progress<F, P>(
        &mut self,
        syntax_definition: &SyntaxDefinition,
        line_texts: &PieceTable,
        line: usize,
        mut should_continue: F,
        mut on_progress: P,
    ) where
        F: FnMut() -> bool,
        P: FnMut(&mut Self),
    {
        self.set_syntax_name(syntax_definition.name());

        if line_texts.line_count() == 0 {
            self.lines = Vec::new();
            self.fold_regions = Vec::new();
            return;
        }

        let target_line = line.min(line_texts.line_count().saturating_sub(1));
        self.truncate_cached_lines(line_texts.line_count());
        let mut scratch = String::new();

        loop {
            let scan_start = self.first_missing_line().unwrap_or(self.lines.len());
            if scan_start > target_line || scan_start >= line_texts.line_count() {
                if target_line >= line_texts.line_count().saturating_sub(1)
                    && self.lines.iter().take(target_line + 1).all(Option::is_some)
                {
                    self.rebuild_fold_regions(line_texts.line_count());
                }
                break;
            }

            let mut state = self
                .line_view(scan_start.saturating_sub(1))
                .map(|line| line.state.clone())
                .unwrap_or_default();
            let mut current_line = scan_start;

            while current_line < line_texts.line_count() {
                if !should_continue() {
                    return;
                }

                let old_line = self
                    .lines
                    .get(current_line)
                    .and_then(Option::as_ref)
                    .cloned();
                let line_ref = line_texts.line(current_line).expect("target line exists");
                let fingerprint = LineFingerprint::from_text_ref(&line_ref);
                let line_text = line_ref.contiguous_text_with_scratch(&mut scratch);
                let SyntaxLineResult {
                    spans,
                    fold_events,
                    state: next_state,
                } = tokenize_line_definition(syntax_definition, line_text, state);

                let fingerprint_changed = old_line
                    .as_ref()
                    .is_some_and(|old| old.fingerprint != fingerprint);
                let next_line = SyntaxLine {
                    fingerprint,
                    spans,
                    fold_events,
                    state: next_state.clone(),
                };
                let reconverged = old_line.as_ref().is_some_and(|old| {
                    old.fingerprint == next_line.fingerprint && old.state == next_line.state
                });

                if current_line >= self.lines.len() {
                    self.lines.resize_with(current_line, || None);
                    self.lines.push(Some(next_line));
                } else {
                    self.lines[current_line] = Some(next_line);
                }

                if (current_line + 1) % SYNTAX_REFRESH_UPDATE_LINES == 0 {
                    on_progress(self);
                }

                state = next_state;
                current_line += 1;

                if reconverged && !fingerprint_changed
                    || current_line > target_line
                        && self.first_missing_line().is_none()
                        && target_line < line_texts.line_count().saturating_sub(1)
                {
                    break;
                }
            }

            let reached_eof = current_line >= line_texts.line_count();
            if reached_eof {
                self.rebuild_fold_regions(line_texts.line_count());
            } else if self.lines.iter().take(target_line + 1).all(Option::is_some) {
                self.rebuild_fold_regions_through(target_line);
            } else {
                self.rebuild_fold_regions_through(current_line.saturating_sub(1));
            }
        }
    }

    fn rebuild_fold_regions(&mut self, line_count: usize) {
        if line_count == 0 {
            self.fold_regions = Vec::new();
            return;
        }

        self.rebuild_fold_regions_inner(line_count - 1, true);
    }

    fn rebuild_fold_regions_through(&mut self, last_line: usize) {
        self.rebuild_fold_regions_inner(last_line, false);
    }

    fn rebuild_fold_regions_inner(&mut self, last_line: usize, finalize_to_eof: bool) {
        let retained_suffix_regions = if finalize_to_eof {
            Vec::new()
        } else {
            self.fold_regions
                .iter()
                .copied()
                .filter(|region| region.end_line > last_line)
                .collect()
        };
        self.fold_regions = Vec::new();
        let mut stack = Vec::new();

        for line in 0..=last_line {
            let Some(entry) = self.lines.get(line).and_then(Option::as_ref) else {
                break;
            };
            apply_fold_events_for_line(
                &mut self.fold_regions,
                &mut stack,
                line,
                &entry.fold_events,
            );
        }

        if finalize_to_eof {
            finalize_fold_regions_to_eof(&mut self.fold_regions, &mut stack, last_line);
        }

        for region in retained_suffix_regions {
            if !self.fold_regions.contains(&region) {
                self.fold_regions.push(region);
            }
        }
        self.fold_regions.dedup();
    }
}

fn apply_fold_events_for_line(
    regions: &mut Vec<SyntaxFoldRegion>,
    stack: &mut Vec<PendingFoldOpen>,
    line: usize,
    events: &[SyntaxFoldEvent],
) {
    for event in events {
        match event.kind {
            SyntaxFoldEventKind::Open => stack.push(PendingFoldOpen {
                line,
                fold_kind: event.fold_kind,
            }),
            SyntaxFoldEventKind::Close => {
                let close_line = line.saturating_sub(event.close_line_offset);
                close_matching_fold(regions, stack, close_line, event.fold_kind);
            }
        }
    }
}

fn close_matching_fold(
    regions: &mut Vec<SyntaxFoldRegion>,
    stack: &mut Vec<PendingFoldOpen>,
    close_line: usize,
    fold_kind: SyntaxFoldKind,
) {
    let Some(position) = stack.iter().rposition(|open| open.fold_kind == fold_kind) else {
        return;
    };

    let start_line = stack[position].line;
    stack.remove(position);

    if start_line != close_line {
        regions.push(SyntaxFoldRegion {
            start_line,
            end_line: close_line,
        });
    }
}

fn finalize_fold_regions_to_eof(
    regions: &mut Vec<SyntaxFoldRegion>,
    stack: &mut Vec<PendingFoldOpen>,
    eof_line: usize,
) {
    while let Some(open) = stack.pop() {
        if open.line != eof_line {
            regions.push(SyntaxFoldRegion {
                start_line: open.line,
                end_line: eof_line,
            });
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

/// Result produced by the background syntax refresh job.
#[derive(Debug, Clone)]
pub struct SyntaxRefreshResult {
    /// Buffer the result applies to.
    pub buffer_id: BufferId,
    /// Cache generation that produced the result.
    pub generation: u64,
    /// Completed syntax cache snapshot.
    pub syntax_cache: SyntaxCache,
}

/// Result produced by the background indent scope refresh job.
#[derive(Debug, Clone)]
pub struct IndentScopeRefreshResult {
    /// Buffer the result applies to.
    pub buffer_id: BufferId,
    /// Cache generation that produced the result.
    pub generation: u64,
    /// Completed indent scope cache snapshot.
    pub indent_scope_cache: IndentScopeCache,
}

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

#[derive(Debug, Clone)]
pub struct IndentScopeCache {
    scopes: Vec<IndentScope>,
    line_scope_offsets: Vec<usize>,
    line_scope_ids: Vec<IndentScopeId>,
    open_scope_stack: Vec<IndentScopeId>,
    scanned_through_line: usize,
    stale: bool,
}

impl Default for IndentScopeCache {
    fn default() -> Self {
        Self {
            scopes: Vec::new(),
            line_scope_offsets: vec![0],
            line_scope_ids: Vec::new(),
            open_scope_stack: Vec::new(),
            scanned_through_line: 0,
            stale: true,
        }
    }
}

impl IndentScopeCache {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// Returns true when the cache data may be incomplete.
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    /// Applies one structural edit to the indent scope cache.
    pub fn apply_edit(&mut self, edit: BufferEditEffect) {
        self.apply_edits(&[edit]);
    }

    /// Applies structural edits to the indent scope cache.
    pub fn apply_edits(&mut self, edits: &[BufferEditEffect]) {
        if edits.is_empty() {
            return;
        }

        let line = edits.iter().map(|edit| edit.start_line).min().unwrap();
        self.invalidate_from(line);
    }

    pub(crate) fn invalidate_from(&mut self, line: usize) {
        let boundary = line.min(self.cached_line_count());
        let mut sync_point = self.previous_sync_point(boundary);

        // When all lines before the boundary have no open scopes (which happens
        // after all scopes are invalidated for flat content at indent 0),
        // previous_sync_point returns boundary, skipping the context line before
        // the edit. Back up by one to ensure that preceding line is reprocessed.
        if sync_point > 0 && sync_point >= boundary {
            sync_point = boundary - 1;
        }

        self.truncate_rows(sync_point);
        self.scanned_through_line = sync_point;
        self.open_scope_stack = Vec::new();
        self.stale = true;

        let prefix_len = self
            .scopes
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
        self.row_scope_ids(line_idx).is_some_and(|scope_ids| {
            scope_ids.iter().any(|scope_id| {
                self.scopes.get(*scope_id).is_some_and(|scope| {
                    scope.end_line.map_or(true, |end_line| end_line > line_idx)
                })
            })
        })
    }

    pub(crate) fn ensure_through(
        &mut self,
        line_texts: &PieceTable,
        target_line: usize,
        tab_width: usize,
    ) -> bool {
        self.ensure_through_with(line_texts, target_line, tab_width, || true)
    }

    fn ensure_through_with<F>(
        &mut self,
        line_texts: &PieceTable,
        target_line: usize,
        tab_width: usize,
        mut should_continue: F,
    ) -> bool
    where
        F: FnMut() -> bool,
    {
        if line_texts.line_count() == 0 {
            self.scanned_through_line = 0;
            self.open_scope_stack = Vec::new();
            self.truncate_rows(0);
            self.stale = false;
            return true;
        }

        let normalized_tab_width = tab_width.max(1);
        let target_line = target_line.min(line_texts.line_count() - 1);
        if target_line < self.scanned_through_line {
            return self.scanned_through_line >= line_texts.line_count();
        }

        if self.cached_line_count() > line_texts.line_count() {
            self.truncate_rows(line_texts.line_count());
        }

        for line_idx in self.scanned_through_line..=target_line {
            if !should_continue() {
                self.stale = true;
                return false;
            }

            let line_text = line_texts.line(line_idx).expect("target line exists");
            // Empty lines should not open or close scopes; they inherit the current stack.
            if line_text.is_empty() {
                self.commit_row(line_idx, self.open_scope_stack.clone());
                continue;
            }
            let indent_width = leading_indent_width(&line_text, normalized_tab_width);
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
                self.open_scope_stack.pop();
            }

            if let Some(branch_start) = invalid_branch_start {
                self.trim_invalid_branch(branch_start, line_idx);
                if closed_scope_on_line.is_some_and(|scope_id| scope_id >= self.scopes.len()) {
                    closed_scope_on_line = None;
                }
            }

            let mut row_members = self.open_scope_stack.clone();
            if let Some(scope_id) = closed_scope_on_line {
                row_members.push(scope_id);
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
                .push(IndentScope::open(new_scope_id, line_idx, indent_width));
            row_members.push(new_scope_id);
            self.open_scope_stack.push(new_scope_id);

            self.commit_row(line_idx, row_members);
        }

        self.scanned_through_line = target_line + 1;
        if target_line == line_texts.line_count() - 1 {
            self.finalize_to_eof(line_texts);
            self.stale = false;
            return true;
        }

        self.stale = true;
        false
    }

    fn commit_row(&mut self, line_idx: usize, mut scope_ids: Vec<IndentScopeId>) {
        scope_ids.sort_by_key(|scope_id| {
            let scope = &self.scopes[*scope_id];
            (
                scope.start_line,
                std::cmp::Reverse(scope.end_line.unwrap_or(usize::MAX)),
            )
        });
        self.truncate_rows(line_idx);
        self.line_scope_ids.extend(scope_ids);
        self.line_scope_offsets.push(self.line_scope_ids.len());
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
        self.open_scope_stack.pop();
    }

    fn trim_invalid_branch(&mut self, branch_start: usize, end_line: usize) {
        let prefix_len = self
            .scopes
            .partition_point(|scope| scope.start_line < branch_start);
        self.scopes.truncate(prefix_len);
        self.open_scope_stack
            .retain(|scope_id| *scope_id < prefix_len);
        self.filter_row_range(branch_start, end_line.saturating_add(1), |scope_id| {
            scope_id < prefix_len
        });
    }

    fn finalize_to_eof(&mut self, line_texts: &PieceTable) {
        if line_texts.line_count() == 0 {
            return;
        }

        let last_line = line_texts.line_count() - 1;
        while let Some(&scope_id) = self.open_scope_stack.last() {
            if self.close_scope_line(scope_id, last_line) {
                self.open_scope_stack.pop();
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
        self.filter_row_range(start_line, end_line.saturating_add(1), |existing| {
            existing != scope_id
        });
    }

    fn cached_line_count(&self) -> usize {
        self.line_scope_offsets.len().saturating_sub(1)
    }

    fn row_scope_ids(&self, line: usize) -> Option<&[IndentScopeId]> {
        let start = *self.line_scope_offsets.get(line)?;
        let end = *self.line_scope_offsets.get(line + 1)?;
        Some(&self.line_scope_ids[start..end])
    }

    fn truncate_rows(&mut self, line_count: usize) {
        let line_count = line_count.min(self.cached_line_count());
        self.line_scope_offsets.truncate(line_count + 1);
        let id_len = self.line_scope_offsets.last().copied().unwrap_or(0);
        self.line_scope_ids.truncate(id_len);
    }

    fn filter_row_range(
        &mut self,
        start_line: usize,
        end_line: usize,
        keep: impl Fn(usize) -> bool,
    ) {
        let cached_line_count = self.cached_line_count();
        let start_line = start_line.min(cached_line_count);
        let end_line = end_line.min(cached_line_count);
        if start_line >= end_line {
            return;
        }

        let prefix_id_len = self.line_scope_offsets[start_line];
        let suffix_id_start = self.line_scope_offsets[end_line];
        let mut ids = Vec::with_capacity(self.line_scope_ids.len());
        ids.extend_from_slice(&self.line_scope_ids[..prefix_id_len]);

        let mut offsets = self.line_scope_offsets[..=start_line].to_vec();
        for line_idx in start_line..end_line {
            let row_start = self.line_scope_offsets[line_idx];
            let row_end = self.line_scope_offsets[line_idx + 1];
            ids.extend(
                self.line_scope_ids[row_start..row_end]
                    .iter()
                    .copied()
                    .filter(|scope_id| keep(*scope_id)),
            );
            offsets.push(ids.len());
        }

        ids.extend_from_slice(&self.line_scope_ids[suffix_id_start..]);
        let id_delta = ids.len() as isize - self.line_scope_ids.len() as isize;
        for offset in self.line_scope_offsets.iter().skip(end_line + 1) {
            offsets.push(offset.saturating_add_signed(id_delta));
        }

        self.line_scope_ids = ids;
        self.line_scope_offsets = offsets;
    }

    pub(crate) fn indent_scopes(&self) -> &[IndentScope] {
        &self.scopes
    }

    pub(crate) fn line_indent_scope_ids(&self, line: usize) -> Option<&[IndentScopeId]> {
        self.row_scope_ids(line)
    }
}

/// Buffer-owned cache state derived from the current text.
#[derive(Debug, Clone)]
pub struct BufferCache {
    pub(crate) syntax_cache: SyntaxCache,
    pub(crate) indent_scope_cache: IndentScopeCache,
}

impl BufferCache {
    /// Creates an empty buffer cache for a syntax name.
    pub fn new(syntax_name: impl Into<SmolStr>) -> Self {
        Self {
            syntax_cache: SyntaxCache::new(syntax_name),
            indent_scope_cache: IndentScopeCache::empty(),
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
        }
    }

    /// Replaces the current cache with a newer snapshot.
    pub fn replace_with(&mut self, other: BufferCache) {
        *self = other;
    }

    /// Returns all syntax fold regions derived from the current cache.
    pub fn syntax_fold_regions(&self) -> &[SyntaxFoldRegion] {
        self.syntax_cache.fold_regions()
    }

    /// Returns the syntax fold region starting at the requested line, if any.
    pub fn syntax_fold_region_starting_at(&self, line: usize) -> Option<SyntaxFoldRegion> {
        self.syntax_cache.fold_region_starting_at(line)
    }

    /// Returns the innermost syntax fold region containing the requested line.
    pub fn syntax_fold_region_containing(&self, line: usize) -> Option<SyntaxFoldRegion> {
        self.syntax_cache.fold_region_containing(line)
    }

    /// Replaces only the syntax cache portion.
    pub fn replace_syntax_cache(&mut self, syntax_cache: SyntaxCache) {
        self.syntax_cache = syntax_cache;
    }

    /// Applies one structural edit to the buffer cache.
    pub fn apply_edit(&mut self, edit: BufferEditEffect) {
        self.apply_edits(&[edit]);
    }

    /// Applies structural edits to the buffer cache.
    pub fn apply_edits(&mut self, edits: &[BufferEditEffect]) {
        if edits.is_empty() {
            return;
        }

        self.syntax_cache.apply_edits(edits);
        self.indent_scope_cache.apply_edits(edits);
    }

    /// Replaces only the indent scope cache portion.
    pub fn replace_indent_scope_cache(&mut self, indent_scope_cache: IndentScopeCache) {
        self.indent_scope_cache = indent_scope_cache;
    }

    /// Invalidates cached data from the provided line onward.
    pub fn invalidate_from(&mut self, line: usize, line_delta: isize) {
        self.apply_edit(BufferEditEffect::from_line_delta(line, line_delta));
    }

    /// Returns cached spans for a line without computing any missing prefix.
    pub fn cached_spans_for_line(&self, line: usize) -> Option<Vec<SyntaxSpan>> {
        self.syntax_cache.cached_spans_for_line(line)
    }

    /// Returns cached syntax spans for a line without cloning span storage.
    pub fn cached_spans_for_line_ref(&self, line: usize) -> Option<&[SyntaxSpan]> {
        self.syntax_cache.cached_spans_for_line_ref(line)
    }

    /// Returns syntax spans for rendering, falling back to stale cached spans.
    pub fn render_spans_for_line_ref(
        &self,
        line: usize,
        current_fingerprint: LineFingerprint,
    ) -> Option<&[SyntaxSpan]> {
        self.syntax_cache
            .render_spans_for_line_ref(line, current_fingerprint)
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
        line_texts: &PieceTable,
        line: usize,
    ) -> Option<Vec<SyntaxSpan>> {
        self.syntax_cache
            .spans_for_line(syntax_name, line_texts, line)
    }

    /// Ensures syntax and indent cache data exists through the requested line.
    pub fn ensure_through(&mut self, syntax_name: &str, line_texts: &PieceTable, line: usize) {
        self.syntax_cache
            .ensure_through(syntax_name, line_texts, line);
        if line_texts.line_count() == 0 {
            return;
        }

        let target_line = line.min(line_texts.line_count().saturating_sub(1));
        let tab_width = globals::with_config(|config| config.tab_width)
            .unwrap_or(4)
            .max(1);
        self.indent_scope_cache
            .ensure_through(line_texts, target_line, tab_width);
    }

    fn ensure_syntax_through_with_budget(
        &mut self,
        syntax_name: &str,
        line_texts: &PieceTable,
        line: usize,
        budget: std::time::Duration,
    ) {
        let Some(syntax_definition) = syntax_definition(syntax_name) else {
            return;
        };
        let syntax_started_at = std::time::Instant::now();
        self.syntax_cache.ensure_through_with_definition(
            syntax_definition.as_ref(),
            line_texts,
            line,
            || syntax_started_at.elapsed() < budget,
        );
    }

    fn ensure_indent_through_with_budget(
        &mut self,
        line_texts: &PieceTable,
        line: usize,
        tab_width: usize,
        budget: std::time::Duration,
    ) {
        if line_texts.line_count() == 0 {
            return;
        }

        let target_line = line.min(line_texts.line_count().saturating_sub(1));
        let started_at = std::time::Instant::now();
        self.indent_scope_cache
            .ensure_through_with(line_texts, target_line, tab_width, || {
                started_at.elapsed() < budget
            });
    }

    /// Returns true when the indent-scope cache needs rebuilding.
    pub fn indent_scope_cache_stale(&self) -> bool {
        self.indent_scope_cache.is_stale()
    }

    /// Returns all cached indent scopes for this buffer snapshot.
    pub fn indent_scopes(&self) -> &[IndentScope] {
        self.indent_scope_cache.indent_scopes()
    }

    /// Returns cached containing indent scope ids for a line.
    pub fn line_indent_scope_ids(&self, line: usize) -> Option<&[IndentScopeId]> {
        self.indent_scope_cache.line_indent_scope_ids(line)
    }
}

fn leading_indent_width(line: &impl TextRef, tab_width: usize) -> usize {
    line.char_indices()
        .map(|(_, ch)| ch)
        .take_while(|ch| matches!(*ch, ' ' | '\t'))
        .fold(0, |acc, ch| acc + if ch == '\t' { tab_width } else { 1 })
}

/// Background job that refreshes the syntax cache for a buffer.
#[derive(Debug)]
pub struct SyntaxRefreshJob {
    buffer_id: BufferId,
    generation: u64,
    syntax_name: SmolStr,
    cache: SyntaxCache,
    line_texts: PieceTable,
}

impl SyntaxRefreshJob {
    pub fn new(
        buffer_id: BufferId,
        generation: u64,
        syntax_name: SmolStr,
        cache: SyntaxCache,
        line_texts: PieceTable,
    ) -> Self {
        Self {
            buffer_id,
            generation,
            syntax_name,
            cache,
            line_texts,
        }
    }

    /// Runs the syntax refresh job on the worker thread.
    pub fn run(mut self, context: &JobContext, event_tx: &std::sync::mpsc::Sender<JobEvent>) {
        if !self.line_texts.is_empty() {
            let target_line = self.line_texts.line_count() - 1;
            if let Some(syntax_definition) = syntax_definition(&self.syntax_name) {
                let kind = context.kind().clone();
                let token = context.token();
                let buffer_id = self.buffer_id;
                let generation = self.generation;
                self.cache.ensure_through_with_progress(
                    syntax_definition.as_ref(),
                    &self.line_texts,
                    target_line,
                    || context.is_current(),
                    |cache| {
                        event_tx
                            .send(JobEvent::Chunk {
                                kind: kind.clone(),
                                token,
                                payload: JobPayload::SyntaxRefresh(SyntaxRefreshResult {
                                    buffer_id,
                                    generation,
                                    syntax_cache: cache.clone(),
                                }),
                            })
                            .ok();
                    },
                );
            }
        }

        event_tx
            .send(JobEvent::Chunk {
                kind: context.kind().clone(),
                token: context.token(),
                payload: JobPayload::SyntaxRefresh(SyntaxRefreshResult {
                    buffer_id: self.buffer_id,
                    generation: self.generation,
                    syntax_cache: self.cache,
                }),
            })
            .ok();

        event_tx
            .send(JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: None,
            })
            .ok();
    }
}

/// Background job that refreshes the indent scope cache for a buffer.
#[derive(Debug)]
pub struct IndentScopeRefreshJob {
    buffer_id: BufferId,
    generation: u64,
    cache: IndentScopeCache,
    line_texts: PieceTable,
}

impl IndentScopeRefreshJob {
    pub fn new(
        buffer_id: BufferId,
        generation: u64,
        cache: IndentScopeCache,
        line_texts: PieceTable,
    ) -> Self {
        Self {
            buffer_id,
            generation,
            cache,
            line_texts,
        }
    }

    /// Runs the indent scope refresh job on the worker thread.
    pub fn run(mut self, context: &JobContext, event_tx: &std::sync::mpsc::Sender<JobEvent>) {
        if !self.line_texts.is_empty() {
            let target_line = self.line_texts.line_count() - 1;
            let tab_width = globals::with_config(|config| config.tab_width)
                .unwrap_or(4)
                .max(1);
            self.cache
                .ensure_through(&self.line_texts, target_line, tab_width);
        }

        event_tx
            .send(JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: Some(JobPayload::IndentScopeRefresh(IndentScopeRefreshResult {
                    buffer_id: self.buffer_id,
                    generation: self.generation,
                    indent_scope_cache: self.cache,
                })),
            })
            .ok();
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum SyntaxState {
    #[default]
    Plain,
    Code(CodeState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CodeState {
    Scanner {
        contexts: ContextStack,
        injection: Option<TokenizerInjectionState>,
        parent_style: Option<Tag>,
        tokenizer_state: SyntaxTokenizerState,
    },
}

/// Opaque tokenizer-owned state carried between tokenized lines.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyntaxTokenizerState {
    values: BTreeMap<String, SyntaxTokenizerStateValue>,
}

impl SyntaxTokenizerState {
    /// Returns a stored unsigned integer value.
    pub fn get_u32(&self, key: &str) -> Option<u32> {
        match self.values.get(key) {
            Some(SyntaxTokenizerStateValue::U32(value)) => Some(*value),
            _ => None,
        }
    }

    /// Sets or removes a stored unsigned integer value.
    pub fn set_u32(&mut self, key: impl Into<String>, value: Option<u32>) {
        let key = key.into();
        if let Some(value) = value {
            self.values
                .insert(key, SyntaxTokenizerStateValue::U32(value));
        } else {
            self.values.remove(&key);
        }
    }
}

/// IPC-friendly scalar value carried in tokenizer-owned state.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxTokenizerStateValue {
    /// Boolean state value.
    Bool(bool),
    /// Unsigned 32-bit integer state value.
    U32(u32),
    /// String state value.
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InjectedSyntaxFallback {
    Unstyled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenizerInjectionState {
    pub(crate) nested: Option<NestedState>,
    pub(crate) fallback: InjectedSyntaxFallback,
    pub(crate) parent_style: Option<Tag>,
}

#[derive(Debug, Clone)]
pub(crate) enum NestedState {
    Syntax {
        syntax_definition: Arc<SyntaxDefinition>,
        state: Box<SyntaxState>,
    },
}

impl NestedState {
    pub(crate) fn new_syntax(definition: Arc<SyntaxDefinition>) -> Self {
        let state = Box::new(initial_state_for_definition(definition.as_ref()));
        NestedState::Syntax {
            syntax_definition: definition,
            state,
        }
    }
}

impl PartialEq for NestedState {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Syntax {
                    syntax_definition: left_definition,
                    state: left_state,
                },
                Self::Syntax {
                    syntax_definition: right_definition,
                    state: right_state,
                },
            ) => left_definition.name() == right_definition.name() && left_state == right_state,
        }
    }
}

impl Eq for NestedState {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ContextStack {
    pub(crate) entries: Vec<ContextEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextEntry {
    pub(crate) name: ContextId,
    pub(crate) payload: Option<String>,
}

impl ContextStack {
    pub(crate) fn top_is(&self, marker: ContextId) -> bool {
        self.entries
            .last()
            .is_some_and(|active| active.name == marker)
    }

    pub(crate) fn contains_anywhere(&self, marker: ContextId) -> bool {
        self.depth(marker) > 0
    }

    pub(crate) fn depth(&self, marker: ContextId) -> usize {
        self.entries
            .iter()
            .filter(|active| active.name == marker)
            .count()
    }

    pub(crate) fn payload_for(&self, name: ContextId) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.name == name)
            .and_then(|entry| entry.payload.as_deref())
    }

    pub(crate) fn push(&mut self, marker: ContextId) {
        self.entries.push(ContextEntry {
            name: marker,
            payload: None,
        });
    }

    pub(crate) fn pop(&mut self, marker: ContextId) {
        if let Some(index) = self
            .entries
            .iter()
            .rposition(|active| active.name == marker)
        {
            self.entries.remove(index);
        }
    }

    pub(crate) fn pop_top(&mut self, marker: ContextId) -> bool {
        if self.top_is(marker) {
            self.entries.pop();
            true
        } else {
            false
        }
    }

    pub(crate) fn push_with_payload(&mut self, marker: ContextId, payload: &str) {
        self.entries.push(ContextEntry {
            name: marker,
            payload: Some(payload.to_string()),
        });
    }
}

fn syntax_definition(syntax_name: &str) -> Option<std::sync::Arc<SyntaxDefinition>> {
    builtin_syntax_registry().ok()?.get_by_name(syntax_name)
}

pub(crate) fn syntax_definition_by_name(name: &str) -> Option<std::sync::Arc<SyntaxDefinition>> {
    syntax_definition(name)
}

pub(crate) fn tokenize_line_definition(
    definition: &SyntaxDefinition,
    line: &str,
    state: SyntaxState,
) -> SyntaxLineResult {
    crate::syntax::tokenizers::dispatch_builtin(definition.tokenizer, definition, line, state)
}

pub(crate) fn tokenize_nested_body(
    nested: &mut NestedState,
    body: &str,
    offset: usize,
) -> Vec<SyntaxSpan> {
    match nested {
        NestedState::Syntax {
            syntax_definition,
            state,
        } => {
            let SyntaxLineResult {
                spans,
                fold_events: _,
                state: next_state,
            } = tokenize_line_definition(syntax_definition.as_ref(), body, state.as_ref().clone());
            **state = next_state;
            offset_spans(spans, offset)
        }
    }
}

/// Tokenize injected nested syntax and keep the host parent style in sync.
pub(crate) fn tokenize_injected_body(
    inj: &mut TokenizerInjectionState,
    body: &str,
    offset: usize,
) -> Vec<SyntaxSpan> {
    if let Some(ref mut nested) = inj.nested {
        let spans = tokenize_nested_body(nested, body, offset);
        let NestedState::Syntax { state, .. } = nested;
        if let SyntaxState::Code(CodeState::Scanner {
            parent_style: ps, ..
        }) = state.as_ref()
        {
            inj.parent_style = ps.clone();
        }
        spans
    } else {
        match inj.fallback {
            InjectedSyntaxFallback::Unstyled => Vec::new(),
        }
    }
}

fn offset_spans(spans: Vec<SyntaxSpan>, offset: usize) -> Vec<SyntaxSpan> {
    spans
        .into_iter()
        .map(|span| SyntaxSpan::new(span.start_byte + offset, span.end_byte + offset, span.style))
        .collect()
}

pub(crate) fn initial_state_for_definition(definition: &SyntaxDefinition) -> SyntaxState {
    match definition.tokenizer {
        SyntaxTokenizer::Plaintext => SyntaxState::Plain,
        _ => SyntaxState::Code(CodeState::Scanner {
            contexts: ContextStack::default(),
            injection: None,
            parent_style: None,
            tokenizer_state: SyntaxTokenizerState::default(),
        }),
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
        self.invalidate_syntax_from_with_line_delta(line, 0);
    }

    /// Invalidates buffer-owned cache data from the given line onward and records the line-count delta.
    pub fn invalidate_syntax_from_with_line_delta(&mut self, line: usize, line_delta: isize) {
        if line >= self.line_count() && line_delta == 0 {
            return;
        }
        self.apply_cache_edits(&[BufferEditEffect::from_line_delta(line, line_delta)]);
    }

    /// Applies structural cache edits and refreshes the visible cache state.
    pub fn apply_cache_edits(&mut self, edits: &[BufferEditEffect]) {
        if edits.is_empty() {
            return;
        }

        self.buffer_cache.apply_edits(edits);
        self.generations.syntax = self.generations.syntax.wrapping_add(1);
        self.generations.syntax_background = None;
        self.generations.indent_background = None;
        self.generations.diff = self.generations.diff.wrapping_add(1);
        self.generations.diff_background = None;

        self.sync_undo_snapshot_cache_if_current();
    }

    /// Returns cached spans for a line without computing missing prefix data.
    pub fn cached_syntax_spans_for_line(&self, line: usize) -> Option<Vec<SyntaxSpan>> {
        self.buffer_cache.cached_spans_for_line(line)
    }

    /// Returns cached spans for a line without cloning span storage.
    pub fn cached_syntax_spans_for_line_ref(&self, line: usize) -> Option<&[SyntaxSpan]> {
        self.buffer_cache.cached_spans_for_line_ref(line)
    }

    /// Returns syntax spans for rendering, falling back to stale cached spans.
    pub fn render_syntax_spans_for_line_ref(
        &self,
        line: usize,
        current_line_text: &(impl TextRef + ?Sized),
    ) -> Option<&[SyntaxSpan]> {
        self.render_syntax_spans_for_line(line, current_line_text)
    }

    /// Returns syntax spans for rendering when cached spans match the current line text.
    pub fn render_syntax_spans_for_line(
        &self,
        line: usize,
        current_line_text: &(impl TextRef + ?Sized),
    ) -> Option<&[SyntaxSpan]> {
        let current_fingerprint = LineFingerprint::from_text_ref(current_line_text);
        self.buffer_cache
            .render_spans_for_line_ref(line, current_fingerprint)
    }

    /// Returns true when the indent-scope cache needs rebuilding.
    pub fn indent_scope_cache_stale(&self) -> bool {
        self.buffer_cache.indent_scope_cache_stale()
    }

    /// Returns all cached indent scopes for the current buffer snapshot.
    pub fn cached_indent_scopes(&self) -> &[IndentScope] {
        self.buffer_cache.indent_scopes()
    }

    /// Returns cached containing indent scope ids for the requested line.
    pub fn cached_line_indent_scope_ids(&self, line: usize) -> Option<&[IndentScopeId]> {
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
        self.generations.syntax
    }

    /// Returns true when a syntax catch-up job has been queued for the current generation.
    pub fn syntax_background_pending(&self) -> bool {
        self.generations.syntax_background.is_some()
    }

    /// Returns the highlighted spans for a line, computing them on demand.
    pub fn syntax_spans_for_line(&mut self, line: usize) -> Option<Vec<SyntaxSpan>> {
        let syntax_name = self.syntax_name().to_owned();
        let spans = self
            .buffer_cache
            .spans_for_line(&syntax_name, &self.lines, line);
        if spans.is_some() {
            self.sync_undo_snapshot_cache_if_current();
        }
        spans
    }

    /// Applies a background buffer cache refresh result when it still matches this buffer.
    pub fn apply_buffer_cache_refresh_result(&mut self, result: BufferCacheRefreshResult) -> bool {
        if result.generation != self.generations.syntax {
            return false;
        }

        self.buffer_cache.replace_with(result.cache);
        self.sync_undo_snapshot_cache_if_current();
        if self.generations.syntax_background == Some(result.generation) {
            self.generations.syntax_background = None;
        }
        true
    }

    /// Applies a background syntax refresh result when it still matches this buffer.
    pub fn apply_syntax_refresh_result(&mut self, result: SyntaxRefreshResult) -> bool {
        if result.generation != self.generations.syntax {
            return false;
        }

        self.buffer_cache.replace_syntax_cache(result.syntax_cache);
        self.sync_undo_snapshot_cache_if_current();
        true
    }

    /// Marks a background syntax refresh as complete when it matches this buffer.
    pub fn finish_syntax_refresh(&mut self, generation: u64) -> bool {
        if generation != self.generations.syntax {
            return false;
        }

        if self.generations.syntax_background == Some(generation) {
            self.generations.syntax_background = None;
        }
        true
    }

    /// Applies a background indent scope refresh result when it still matches this buffer.
    pub fn apply_indent_scope_refresh_result(&mut self, result: IndentScopeRefreshResult) -> bool {
        if result.generation != self.generations.syntax {
            return false;
        }

        self.buffer_cache
            .replace_indent_scope_cache(result.indent_scope_cache);
        self.sync_undo_snapshot_cache_if_current();
        if self.generations.indent_background == Some(result.generation) {
            self.generations.indent_background = None;
        }
        true
    }

    /// Applies a background diff refresh result when it still matches this buffer.
    pub fn apply_diff_refresh_result(&mut self, result: crate::buffer::DiffRefreshResult) -> bool {
        if result.generation != self.generations.diff {
            return false;
        }

        self.diff_cache
            .replace_hunks_for_generation(result.generation, result.hunks);
        self.diff_tracked = Some(result.tracked);
        if self.generations.diff_background == Some(result.generation) {
            self.generations.diff_background = None;
        }
        true
    }

    /// Requests background buffer cache refresh when the cache is incomplete.
    pub fn request_buffer_cache_refresh(&mut self, buffer_id: BufferId) {
        let syntax_needed = !self.syntax_cache_complete()
            && self.generations.syntax_background != Some(self.generations.syntax);
        let indent_needed = self.indent_scope_cache_stale()
            && self.generations.indent_background != Some(self.generations.syntax);
        let diff_needed = self.diff_cache_stale()
            && self.generations.diff_background != Some(self.generations.diff);

        if !syntax_needed && !indent_needed && !diff_needed {
            return;
        }

        let generation = self.generations.syntax;

        if syntax_needed {
            let job = SyntaxRefreshJob::new(
                buffer_id,
                generation,
                self.syntax_name().to_owned().into(),
                self.buffer_cache.syntax_cache.clone(),
                self.lines.clone(),
            );
            let kind = JobKind::SyntaxRefresh(buffer_id);
            let token = JobToken::new(generation);

            let submitted = globals::with_buffer_pool(|buffer_pool| {
                buffer_pool.submit_background_job(kind, token, job).is_ok()
            });

            if submitted {
                self.generations.syntax_background = Some(generation);
            }
        }

        if indent_needed {
            let job = IndentScopeRefreshJob::new(
                buffer_id,
                generation,
                self.buffer_cache.indent_scope_cache.clone(),
                self.lines.clone(),
            );
            let kind = JobKind::IndentScopeRefresh(buffer_id);
            let token = JobToken::new(generation);

            let submitted = globals::with_buffer_pool(|buffer_pool| {
                buffer_pool.submit_background_job(kind, token, job).is_ok()
            });

            if submitted {
                self.generations.indent_background = Some(generation);
            }
        }

        if diff_needed {
            let Some(path) = self.path().cloned() else {
                return;
            };

            let job =
                DiffRefreshJob::new(buffer_id, self.generations.diff, path, self.line_texts());
            let kind = JobKind::DiffRefresh(buffer_id);
            let token = JobToken::new(self.generations.diff);

            let submitted = globals::with_buffer_pool(|buffer_pool| {
                buffer_pool.submit_background_job(kind, token, job).is_ok()
            });

            if submitted {
                self.generations.diff_background = Some(self.generations.diff);
            }
        }
    }

    /// Requests background syntax catch-up when the cache is incomplete.
    pub fn request_syntax_catch_up(&mut self, buffer_id: BufferId) {
        self.request_buffer_cache_refresh(buffer_id);
    }

    /// Ensures syntax data exists through a line without returning it.
    pub fn ensure_syntax_through(&mut self, line: usize) {
        let syntax_name = self.syntax_name().to_owned();
        self.buffer_cache
            .ensure_through(&syntax_name, &self.lines, line);
        self.sync_undo_snapshot_cache_if_current();
    }

    /// Warms syntax data through a line without requiring the whole range to complete.
    pub fn warm_syntax_through_with_budget(&mut self, line: usize, budget: std::time::Duration) {
        let syntax_name = self.syntax_name().to_owned();
        self.buffer_cache.ensure_syntax_through_with_budget(
            &syntax_name,
            &self.lines,
            line,
            budget,
        );
        self.buffer_cache.ensure_indent_through_with_budget(
            &self.lines,
            line,
            globals::with_config(|config| config.tab_width)
                .unwrap_or(4)
                .max(1),
            budget,
        );
        self.sync_undo_snapshot_cache_if_current();
    }

    /// Returns true when the complete buffer cache is available for the current text.
    pub fn buffer_cache_complete(&self) -> bool {
        self.buffer_cache
            .is_complete_for_line_count(self.line_count())
    }

    /// Returns true when the current syntax supports syntax-based folding.
    pub fn syntax_supports_folding(&self) -> bool {
        syntax_definition(self.syntax_name()).is_some_and(|definition| definition.supports_folding)
    }

    /// Returns all syntax fold regions for the current buffer.
    pub fn syntax_fold_regions(&mut self) -> &[SyntaxFoldRegion] {
        self.ensure_complete_syntax_folds();
        self.buffer_cache.syntax_fold_regions()
    }

    /// Returns the syntax fold region starting at the requested line, if any.
    pub fn syntax_fold_region_starting_at(&mut self, line: usize) -> Option<SyntaxFoldRegion> {
        self.ensure_complete_syntax_folds();
        self.buffer_cache.syntax_fold_region_starting_at(line)
    }

    /// Returns a cached syntax fold region starting at the requested line without warming syntax.
    pub fn cached_syntax_fold_region_starting_at(&self, line: usize) -> Option<SyntaxFoldRegion> {
        self.buffer_cache.syntax_fold_region_starting_at(line)
    }

    /// Returns the innermost syntax fold region containing the requested line.
    pub fn syntax_fold_region_containing(&mut self, line: usize) -> Option<SyntaxFoldRegion> {
        self.ensure_complete_syntax_folds();
        self.buffer_cache.syntax_fold_region_containing(line)
    }

    fn ensure_complete_syntax_folds(&mut self) {
        if !self.syntax_supports_folding() || self.line_count() == 0 || self.syntax_cache_complete()
        {
            return;
        }

        self.ensure_syntax_through(self.line_count().saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{BackgroundJob, JobEvent, JobHandle, JobKind, JobPayload, JobToken};
    use crate::buffer::TextStorage;
    use crate::buffer::{BufferId, Cursor};
    use crate::config::Config;
    use crate::globals;
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
            if let Some(event) = handle.poll_event() {
                return event;
            }
            assert!(Instant::now() < deadline, "timed out waiting for job event");
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn wait_for_syntax_refresh(handle: &JobHandle) -> (JobToken, SyntaxRefreshResult) {
        let mut latest = None;
        loop {
            match wait_for_event(handle) {
                JobEvent::Chunk {
                    token,
                    payload: JobPayload::SyntaxRefresh(result),
                    ..
                } => latest = Some((token, result)),
                JobEvent::Completed { payload: None, .. } => {
                    return latest.expect("syntax refresh should emit at least one chunk");
                }
                other => panic!(
                    "expected syntax refresh chunk or completion, got {:?}",
                    other
                ),
            }
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
    fn context_stack_top_is_checks_only_top_entry() {
        let mut stack = ContextStack::default();
        const OUTER: ContextId = ContextId::new("test", "outer");
        const INNER: ContextId = ContextId::new("test", "inner");
        stack.push(OUTER);
        stack.push(INNER);

        assert!(stack.top_is(INNER));
        assert!(!stack.top_is(OUTER));
        assert!(stack.contains_anywhere(OUTER));
    }

    #[test]
    fn context_stack_pop_top_only_removes_matching_top_entry() {
        let mut stack = ContextStack::default();
        const OUTER: ContextId = ContextId::new("test", "outer");
        const INNER: ContextId = ContextId::new("test", "inner");
        stack.push(OUTER);
        stack.push(INNER);

        assert!(!stack.pop_top(OUTER));
        assert!(stack.top_is(INNER));
        assert_eq!(stack.depth(OUTER), 1);
        assert_eq!(stack.depth(INNER), 1);

        assert!(stack.pop_top(INNER));
        assert!(stack.top_is(OUTER));
        assert_eq!(stack.depth(INNER), 0);
    }

    #[test]
    fn context_stack_depth_counts_duplicate_markers() {
        let mut stack = ContextStack::default();
        const COMMENT: ContextId = ContextId::new("test", "comment");
        const STRING: ContextId = ContextId::new("test", "string");
        const MISSING: ContextId = ContextId::new("test", "missing");
        stack.push(COMMENT);
        stack.push(STRING);
        stack.push(COMMENT);

        assert_eq!(stack.depth(COMMENT), 2);
        assert_eq!(stack.depth(STRING), 1);
        assert_eq!(stack.depth(MISSING), 0);
    }

    #[test]
    fn context_stack_payload_for_returns_nearest_matching_payload() {
        let mut stack = ContextStack::default();
        const HEREDOC: ContextId = ContextId::new("test", "heredoc");
        const BODY: ContextId = ContextId::new("test", "body");
        stack.push_with_payload(HEREDOC, "FIRST");
        stack.push(BODY);
        stack.push_with_payload(HEREDOC, "SECOND");

        assert_eq!(stack.payload_for(HEREDOC), Some("SECOND"));
        assert_eq!(stack.payload_for(BODY), None);
    }

    #[test]
    fn line_fingerprint_matches_contiguous_and_piece_table_chunks() {
        let mut lines = PieceTable::from_text("prefixsuffix");
        lines
            .insert_text(Cursor::new(0, "prefix".len()), "-")
            .expect("insert should split line into pieces");
        let line = lines.line(0).expect("line should exist");

        assert_eq!(
            LineFingerprint::new("prefix-suffix"),
            LineFingerprint::from_text_ref(&line)
        );
        assert_ne!(
            LineFingerprint::new("prefixsuffix"),
            LineFingerprint::from_text_ref(&line)
        );
    }

    #[test]
    fn cached_spans_can_be_replaced_and_read_without_recomputing() {
        let mut cache = SyntaxCache::new("plain");
        assert_eq!(cache.cached_spans_for_line(0), None);

        cache.push_cached_line_for_test(
            Arc::from("abc"),
            vec![SyntaxSpan::new(0, 3, tag("text.plain"))],
            SyntaxState::default(),
        );

        let cached = cache
            .cached_spans_for_line(0)
            .expect("cached spans should be available");
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].style, tag("text.plain"));

        let mut replacement = SyntaxCache::new("plain");
        replacement.push_cached_line_for_test(
            Arc::from("abcd"),
            vec![SyntaxSpan::new(0, 4, tag("text.replaced"))],
            SyntaxState::default(),
        );
        replacement.push_cached_line_for_test(
            Arc::from("abcde"),
            vec![SyntaxSpan::new(0, 5, tag("text.replaced"))],
            SyntaxState::default(),
        );

        cache = replacement;

        let cached = cache
            .cached_spans_for_line(1)
            .expect("replacement cache should have line 1");
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].style, tag("text.replaced"));
    }

    #[test]
    fn budgeted_syntax_refresh_can_stop_before_eof() {
        let lines = PieceTable::from_text(
            std::iter::repeat_n("fn main() { let value = Some(\"hi\"); }", 64)
                .collect::<Vec<_>>()
                .join("\n")
                .as_str(),
        );
        let mut cache = SyntaxCache::new("rust");

        cache.ensure_through("rust", &lines, lines.line_count() - 1);
        assert!(cache.is_complete_for_line_count(lines.line_count()));

        cache.invalidate_from(1, 0);
        cache.ensure_through_with("rust", &lines, lines.line_count() - 1, || false);

        assert_eq!(cache.cached_line_count(), 1);
        assert!(!cache.is_complete_for_line_count(lines.line_count()));
    }

    #[test]
    fn syntax_cache_rewrites_from_dirty_line_without_truncating() {
        let original = PieceTable::from_text("value = \"\"\"hello\nplanet\nworld\"\"\"\nafter");
        let edited = PieceTable::from_text("value = \"\"\"hello\ngalaxy\nworld\"\"\"\nafter");
        let mut cache = SyntaxCache::new("toml");

        cache.ensure_through("toml", &original, original.line_count() - 1);
        assert!(cache.is_complete_for_line_count(original.line_count()));

        cache.invalidate_from(1, 0);
        assert!(!cache.is_complete_for_line_count(original.line_count()));
        assert_eq!(cache.cached_line_count(), 1);
        assert!(cache.cached_spans_for_line(3).is_some());

        cache.ensure_through("toml", &edited, 2);

        assert_eq!(cache.cached_line_count(), original.line_count());
        assert!(cache.cached_spans_for_line(3).is_some());
        assert!(cache.is_complete_for_line_count(original.line_count()));
    }

    #[test]
    fn syntax_cache_invalidates_inside_cached_lines() {
        let text = (0..128)
            .map(|line| format!("value_{line} = {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = PieceTable::from_text(&text);
        let mut cache = SyntaxCache::new("rust");

        cache.ensure_through("rust", &lines, 65);
        assert!(cache.cached_spans_for_line(65).is_some());

        cache.invalidate_from(66, 0);
        assert_eq!(cache.cached_line_count(), 66);
        assert!(cache.cached_spans_for_line(64).is_some());
        assert!(cache.cached_spans_for_line(65).is_some());
        assert!(cache.cached_spans_for_line(66).is_none());

        cache.ensure_through("rust", &lines, lines.line_count() - 1);
        assert!(cache.is_complete_for_line_count(lines.line_count()));
    }

    #[test]
    fn syntax_cache_rehighlights_from_dirty_line_across_large_file() {
        let original = (0..128)
            .map(|line| format!("value_{line} = {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let edited = (0..128)
            .map(|line| {
                if line == 63 {
                    "changed = 1".to_owned()
                } else {
                    format!("value_{line} = {line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let original = PieceTable::from_text(&original);
        let edited = PieceTable::from_text(&edited);
        let mut cache = SyntaxCache::new("rust");

        cache.ensure_through("rust", &original, 67);
        cache.invalidate_from(63, 0);
        cache.ensure_through("rust", &edited, 64);

        assert!(cache.cached_spans_for_line(64).is_some());
        assert!(cache.cached_spans_for_line(67).is_some());
        assert!(!cache.is_complete_for_line_count(edited.line_count()));
        cache.ensure_through("rust", &edited, 67);
        assert!(cache.cached_spans_for_line(67).is_some());
    }

    #[test]
    fn undo_and_redo_restore_syntax_cache_snapshots() {
        let mut buffer = Buffer::from_str("root\n  child\nroot-close");

        buffer.ensure_syntax_through(2);
        let initial_result = BufferCacheRefreshResult {
            buffer_id: BufferId::new(1),
            generation: buffer.syntax_generation(),
            cache: buffer.buffer_cache.clone(),
        };
        assert!(buffer.apply_buffer_cache_refresh_result(initial_result));
        let initial_scopes = scope_tuples(&buffer);
        assert!(buffer.syntax_cache_complete());
        assert!(!buffer.indent_scope_cache_stale());

        buffer.insert_text(Cursor::new(2, 0), "  nested\n");
        buffer.push_snapshot(Cursor::new(3, 0));
        buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
        let edited_result = BufferCacheRefreshResult {
            buffer_id: BufferId::new(1),
            generation: buffer.syntax_generation(),
            cache: buffer.buffer_cache.clone(),
        };
        assert!(buffer.apply_buffer_cache_refresh_result(edited_result));
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
        assert_eq!(
            scope_tuples(&buffer),
            Vec::<(usize, Option<usize>, usize)>::new()
        );
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
        assert_eq!(
            scope_tuples(&buffer),
            Vec::<(usize, Option<usize>, usize)>::new()
        );
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
        assert_eq!(scope_ids.to_vec(), vec![0]);
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
        assert_eq!(line_scope_ids.to_vec(), vec![0]);
    }

    #[test]
    fn background_generation_is_cleared_when_syntax_is_invalidated() {
        let mut buffer = Buffer::from_str("line 1\nline 2");
        buffer.generations.syntax = 7;
        buffer.generations.syntax_background = Some(7);
        buffer.ensure_syntax_through(1);

        buffer.invalidate_syntax_from(1);

        assert_eq!(buffer.syntax_generation(), 8);
        assert_eq!(buffer.generations.syntax_background, None);
        assert!(buffer.cached_syntax_spans_for_line(0).is_some());
        assert!(buffer.cached_syntax_spans_for_line(1).is_none());
        assert!(!buffer.syntax_cache_complete());
    }

    #[test]
    fn complete_cache_requests_no_background_catch_up() {
        let mut buffer = Buffer::from_str("line 1");
        buffer.ensure_syntax_through(0);
        buffer.generations.syntax_background = None;

        buffer.request_syntax_catch_up(BufferId::new(1));

        assert_eq!(buffer.generations.syntax_background, None);
    }

    #[test]
    fn background_syntax_job_populates_offscreen_spans() {
        let path = temp_path_with_ext("background-syntax", "rs");
        let text = std::iter::repeat_n(
            "fn main() { let value: Option<String> = Some(\"hi\"); } // note",
            64,
        )
        .collect::<Vec<_>>()
        .join("\n");
        let buffer = Buffer::from_str_with_path(&text, path);
        let handle = JobHandle::new();
        let token = JobToken::new(buffer.syntax_generation());
        let job = SyntaxRefreshJob::new(
            BufferId::new(1),
            buffer.syntax_generation(),
            buffer.syntax_name().to_owned().into(),
            buffer.buffer_cache.syntax_cache.clone(),
            buffer.lines.clone(),
        );

        handle
            .submit(JobKind::SyntaxRefresh(BufferId::new(1)), token, job)
            .expect("syntax refresh job should submit");

        let (_, result) = wait_for_syntax_refresh(&handle);

        assert!(result.syntax_cache.cached_spans_for_line(50).is_some());
        assert!(result.syntax_cache.is_complete_for_line_count(64));

        handle.shutdown();
    }

    #[test]
    fn background_indent_job_populates_offscreen_scopes() {
        let text = std::iter::repeat_n("  indented line", 64)
            .collect::<Vec<_>>()
            .join("\n");
        let buffer = Buffer::from_str(&text);
        let handle = JobHandle::new();
        let token = JobToken::new(buffer.syntax_generation());
        let job = IndentScopeRefreshJob::new(
            BufferId::new(1),
            buffer.syntax_generation(),
            buffer.buffer_cache.indent_scope_cache.clone(),
            buffer.lines.clone(),
        );

        handle
            .submit(JobKind::IndentScopeRefresh(BufferId::new(1)), token, job)
            .expect("indent scope refresh job should submit");

        let event = wait_for_event(&handle);
        let result = match event {
            JobEvent::Completed {
                payload: Some(JobPayload::IndentScopeRefresh(result)),
                ..
            } => result,
            other => panic!("expected indent scope refresh completion, got {:?}", other),
        };

        assert!(!result.indent_scope_cache.is_stale());

        handle.shutdown();
    }

    #[test]
    fn latest_only_syntax_refresh_skips_stale_queue_entries() {
        let handle = JobHandle::new();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_blocker = Arc::clone(&gate);

        handle
            .submit(
                JobKind::TestGate,
                JobToken::new(1),
                BackgroundJob::Gate {
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
                JobKind::SyntaxRefresh(BufferId::new(1)),
                JobToken::new(1),
                SyntaxRefreshJob::new(
                    BufferId::new(1),
                    old_buffer.syntax_generation(),
                    old_buffer.syntax_name().to_owned().into(),
                    old_buffer.buffer_cache.syntax_cache.clone(),
                    old_buffer.lines.clone(),
                ),
            )
            .expect("old syntax job should submit");

        handle
            .submit_latest_only(
                JobKind::SyntaxRefresh(BufferId::new(1)),
                JobToken::new(2),
                SyntaxRefreshJob::new(
                    BufferId::new(1),
                    new_buffer.syntax_generation(),
                    new_buffer.syntax_name().to_owned().into(),
                    new_buffer.buffer_cache.syntax_cache.clone(),
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
        assert_eq!(blocker_event.kind(), &JobKind::TestGate);

        let (token, result) = wait_for_syntax_refresh(&handle);
        assert_eq!(token.generation(), 2);
        assert!(result.syntax_cache.is_complete_for_line_count(32));

        handle.shutdown();
    }

    #[test]
    fn syntax_refresh_after_edit_fills_missing_lines_and_reconverges() {
        let path = temp_path_with_ext("dirty-line-refresh", "rs");
        let text = std::iter::repeat_n("fn item() { let value = 1; }", 32)
            .collect::<Vec<_>>()
            .join("\n");
        let mut buffer = Buffer::from_str_with_path(&text, path);
        buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
        buffer.insert_char(Cursor::new(4, 0), '/');

        assert!(buffer.cached_syntax_spans_for_line(3).is_some());
        assert!(buffer.cached_syntax_spans_for_line(4).is_none());
        assert!(buffer.cached_syntax_spans_for_line(31).is_some());
        let unchanged_dirty_line = buffer.line_at(31).expect("line should exist");
        assert!(
            buffer
                .render_syntax_spans_for_line(31, &unchanged_dirty_line)
                .is_some()
        );
        assert!(!buffer.syntax_cache_complete());

        let job = SyntaxRefreshJob::new(
            BufferId::new(1),
            buffer.syntax_generation(),
            buffer.syntax_name().to_owned().into(),
            buffer.buffer_cache.syntax_cache.clone(),
            buffer.lines.clone(),
        );
        let handle = JobHandle::new();
        handle
            .submit(
                JobKind::SyntaxRefresh(BufferId::new(1)),
                JobToken::new(buffer.syntax_generation()),
                job,
            )
            .expect("syntax job should submit");

        let (_, result) = wait_for_syntax_refresh(&handle);

        assert!(buffer.apply_syntax_refresh_result(result));
        assert!(buffer.cached_syntax_spans_for_line(4).is_some());
        assert!(buffer.cached_syntax_spans_for_line(31).is_some());
        assert!(buffer.syntax_cache_complete());

        handle.shutdown();
    }

    #[test]
    fn stale_background_result_is_rejected_after_invalidation() {
        let path = temp_path_with_ext("stale-result", "rs");
        let mut buffer = Buffer::from_str_with_path("fn main() {}", path);
        let result = BufferCacheRefreshResult {
            buffer_id: BufferId::new(1),
            generation: buffer.syntax_generation(),
            cache: buffer.buffer_cache.clone(),
        };

        buffer.invalidate_syntax_from(0);

        assert!(!buffer.apply_buffer_cache_refresh_result(result));
    }
}
