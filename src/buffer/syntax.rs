//! Syntax highlighting tokenizers and indent-scope cache primitives.

use crate::background::{JobContext, JobEvent, JobKind, JobPayload, JobToken};
use crate::buffer::Buffer;
use crate::buffer::BufferId;
use crate::buffer::{ApplyEdit, DiffRefreshJob, LineEdit, PieceTable, TextRef, TextSnapshot};
use crate::globals;
use crate::syntax::{
    ContextControl, InjectedSyntaxFallback, InjectedSyntaxSelector, SyntaxDefinition, SyntaxRule,
    builtin_syntax_registry,
};
use crate::theme::Tag;
use regex::Regex;
use smol_str::SmolStr;
use std::sync::Arc;

const SYNTAX_CHUNK_SIZE: usize = 512;

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
struct SyntaxLineMeta {
    line: Arc<str>,
    span_start: usize,
    span_len: usize,
    state: SyntaxState,
}

#[derive(Debug, Clone)]
struct SyntaxChunk {
    start_line: usize,
    lines: Arc<[SyntaxLineMeta]>,
    spans: Arc<[SyntaxSpan]>,
}

#[derive(Debug, Clone)]
struct SyntaxChunkBuilder {
    start_line: usize,
    lines: Vec<SyntaxLineMeta>,
    spans: Vec<SyntaxSpan>,
}

struct SyntaxLineView<'a> {
    line: &'a str,
    spans: &'a [SyntaxSpan],
    state: &'a SyntaxState,
}

#[derive(Debug, Clone)]
struct RenderLineEdit {
    line: usize,
    line_delta: isize,
}

#[derive(Debug, Clone)]
struct RenderSyntaxLine {
    line: Arc<str>,
    spans: Arc<[SyntaxSpan]>,
}

#[derive(Debug, Clone)]
struct RenderSyntaxSnapshot {
    start_line: usize,
    lines: Vec<RenderSyntaxLine>,
    edits: Vec<RenderLineEdit>,
}

#[derive(Debug, Clone)]
pub struct SyntaxCache {
    syntax_name: SmolStr,
    chunks: Vec<SyntaxChunk>,
    tail: SyntaxChunkBuilder,
    dirty_suffix: Vec<SyntaxChunk>,
    dirty_start: Option<usize>,
    dirty_line_delta: isize,
    render_snapshot: Option<RenderSyntaxSnapshot>,
}

impl SyntaxChunkBuilder {
    fn new(start_line: usize) -> Self {
        Self {
            start_line,
            lines: Vec::new(),
            spans: Vec::new(),
        }
    }

    fn append_line(&mut self, line: Arc<str>, spans: Vec<SyntaxSpan>, state: SyntaxState) {
        let span_start = self.spans.len();
        let span_len = spans.len();
        self.spans.extend(spans);
        self.lines.push(SyntaxLineMeta {
            line,
            span_start,
            span_len,
            state,
        });
    }

    fn freeze(&mut self) -> Option<SyntaxChunk> {
        if self.lines.is_empty() {
            return None;
        }

        let start_line = self.start_line;
        let line_count = self.lines.len();
        Some(SyntaxChunk {
            start_line,
            lines: Arc::from(std::mem::take(&mut self.lines).into_boxed_slice()),
            spans: Arc::from(std::mem::take(&mut self.spans).into_boxed_slice()),
        })
        .inspect(|_| {
            self.start_line = start_line + line_count;
        })
    }
}

impl SyntaxCache {
    /// Creates an empty syntax cache for a syntax name.
    pub fn new(syntax_name: impl Into<SmolStr>) -> Self {
        Self {
            syntax_name: syntax_name.into(),
            chunks: Vec::new(),
            tail: SyntaxChunkBuilder::new(0),
            dirty_suffix: Vec::new(),
            dirty_start: None,
            dirty_line_delta: 0,
            render_snapshot: None,
        }
    }

    /// Updates the cached syntax name, clearing cached data when it changes.
    pub fn set_syntax_name(&mut self, syntax_name: impl Into<SmolStr>) {
        let syntax_name = syntax_name.into();
        if self.syntax_name != syntax_name {
            self.syntax_name = syntax_name;
            self.chunks = Vec::new();
            self.tail = SyntaxChunkBuilder::new(0);
            self.clear_dirty_suffix();
            self.clear_render_snapshot();
        }
    }

    /// Returns the canonical syntax name tracked by this cache.
    pub fn syntax_name(&self) -> &str {
        &self.syntax_name
    }

    fn clear_dirty_suffix(&mut self) {
        self.dirty_suffix = Vec::new();
        self.dirty_start = None;
        self.dirty_line_delta = 0;
    }

    fn clear_render_snapshot(&mut self) {
        self.render_snapshot = None;
    }

    fn capture_cached_lines_from(&self, start_line: usize) -> Vec<RenderSyntaxLine> {
        let mut lines = Vec::new();
        let mut line = start_line;
        while let Some(line_view) = self.line_view(line) {
            lines.push(RenderSyntaxLine {
                line: Arc::from(line_view.line),
                spans: Arc::from(line_view.spans.to_vec().into_boxed_slice()),
            });
            line += 1;
        }
        lines
    }

    fn translate_render_snapshot_line(&self, line: usize) -> Option<usize> {
        let snapshot = self.render_snapshot.as_ref()?;
        let mut translated = line;
        for edit in &snapshot.edits {
            if translated < edit.line {
                break;
            }

            if edit.line_delta > 0 {
                let inserted_lines = edit.line_delta as usize;
                let inserted_end = edit.line.saturating_add(inserted_lines);
                if translated < inserted_end {
                    return None;
                }
                translated = translated.saturating_sub(inserted_lines);
            } else if edit.line_delta < 0 {
                translated = translated.saturating_add(edit.line_delta.unsigned_abs());
            }
        }

        Some(translated)
    }

    fn update_render_snapshot(&mut self, edits: &[(usize, isize)]) {
        let mut edits = edits
            .iter()
            .copied()
            .map(|(line, line_delta)| RenderLineEdit { line, line_delta })
            .collect::<Vec<_>>();

        if edits.is_empty() {
            return;
        }

        // Preserve simple stale render snapshots across zero-delta follow-up edits
        // so multi-step changes like `cw` keep pre-edit styling visible until
        // syntax highlighting catches up. Do not keep a snapshot that already
        // includes line-shifting edits: a later same-line insertion would reuse
        // stale line translations and can style the dirty suffix from the wrong
        // pre-edit line.
        if edits.iter().all(|edit| edit.line_delta == 0)
            && let Some(snapshot) = self.render_snapshot.as_ref()
        {
            if snapshot.edits.iter().all(|edit| edit.line_delta == 0) {
                return;
            }

            self.clear_render_snapshot();
        }

        if self.render_snapshot.is_some() && edits.iter().any(|edit| edit.line_delta != 0) {
            return;
        }

        edits.sort_by_key(|edit| edit.line);
        let start_line = 0;
        let lines = self.capture_cached_lines_from(start_line);
        if lines.is_empty() {
            self.clear_render_snapshot();
            return;
        }

        self.render_snapshot = Some(RenderSyntaxSnapshot {
            start_line,
            lines,
            edits,
        });
    }

    /// Applies one normalized edit to the cache.
    pub fn apply_edit(&mut self, edit: LineEdit) {
        self.apply_edits(&[edit]);
    }

    /// Applies a batch of normalized edits to the cache.
    pub fn apply_edits(&mut self, edits: &[LineEdit]) {
        if edits.is_empty() {
            return;
        }

        let line_edits: Vec<_> = edits
            .iter()
            .map(|edit| (edit.start_line, edit.line_delta))
            .collect();
        self.update_render_snapshot(&line_edits);

        let first_line = edits.iter().map(|edit| edit.start_line).min().unwrap();
        let line_delta: isize = edits.iter().map(|edit| edit.line_delta).sum();
        self.invalidate_from(first_line, line_delta);
    }

    /// Records the line edits that should preserve stale render spans.
    pub fn record_render_snapshot(&mut self, edits: &[(usize, isize)]) {
        self.update_render_snapshot(edits);
    }

    fn flush_tail(&mut self) {
        if let Some(chunk) = self.tail.freeze() {
            self.chunks.push(chunk);
        }
    }

    fn frozen_line_count(&self) -> usize {
        self.chunks
            .last()
            .map(|chunk| chunk.start_line + chunk.lines.len())
            .unwrap_or(0)
    }

    fn append_line(&mut self, line: Arc<str>, spans: Vec<SyntaxSpan>, state: SyntaxState) {
        if self.tail.lines.len() == SYNTAX_CHUNK_SIZE {
            self.flush_tail();
        }
        let frozen_line_count = self.frozen_line_count();
        if self.tail.lines.is_empty() && self.tail.start_line < frozen_line_count {
            self.tail.start_line = frozen_line_count;
        }
        self.tail.append_line(line, spans, state);
    }

    fn chunk_line_view(chunk: &SyntaxChunk, line: usize) -> Option<SyntaxLineView<'_>> {
        let line_offset = line.checked_sub(chunk.start_line)?;
        let meta = chunk.lines.get(line_offset)?;
        Some(SyntaxLineView {
            line: meta.line.as_ref(),
            spans: &chunk.spans[meta.span_start..meta.span_start + meta.span_len],
            state: &meta.state,
        })
    }

    fn line_view(&self, line: usize) -> Option<SyntaxLineView<'_>> {
        let chunk_idx = self
            .chunks
            .partition_point(|chunk| chunk.start_line + chunk.lines.len() <= line);
        if let Some(chunk) = self.chunks.get(chunk_idx)
            && line >= chunk.start_line
        {
            return Self::chunk_line_view(chunk, line);
        }

        if line >= self.tail.start_line && line < self.tail.start_line + self.tail.lines.len() {
            let line_offset = line - self.tail.start_line;
            let meta = self.tail.lines.get(line_offset)?;
            return Some(SyntaxLineView {
                line: meta.line.as_ref(),
                spans: &self.tail.spans[meta.span_start..meta.span_start + meta.span_len],
                state: &meta.state,
            });
        }

        None
    }

    fn dirty_line_view(dirty_suffix: &[SyntaxChunk], line: usize) -> Option<SyntaxLineView<'_>> {
        let chunk_idx =
            dirty_suffix.partition_point(|chunk| chunk.start_line + chunk.lines.len() <= line);
        dirty_suffix
            .get(chunk_idx)
            .filter(|chunk| line >= chunk.start_line)
            .and_then(|chunk| Self::chunk_line_view(chunk, line))
    }

    fn dirty_line_view_by_index(
        dirty_suffix: &[SyntaxChunk],
        dirty_idx: usize,
    ) -> Option<SyntaxLineView<'_>> {
        let first_line = dirty_suffix.first()?.start_line;
        Self::dirty_line_view(dirty_suffix, first_line + dirty_idx)
    }

    fn split_chunk_at(chunk: &SyntaxChunk, line_offset: usize) -> (SyntaxChunk, SyntaxChunk) {
        let span_split = chunk.lines[line_offset].span_start;
        let prefix_lines: Vec<_> = chunk.lines[..line_offset].to_vec();
        let suffix_lines: Vec<_> = chunk.lines[line_offset..]
            .iter()
            .cloned()
            .map(|mut line| {
                line.span_start -= span_split;
                line
            })
            .collect();

        (
            SyntaxChunk {
                start_line: chunk.start_line,
                lines: Arc::from(prefix_lines.into_boxed_slice()),
                spans: Arc::from(chunk.spans[..span_split].to_vec().into_boxed_slice()),
            },
            SyntaxChunk {
                start_line: chunk.start_line + line_offset,
                lines: Arc::from(suffix_lines.into_boxed_slice()),
                spans: Arc::from(chunk.spans[span_split..].to_vec().into_boxed_slice()),
            },
        )
    }

    fn shift_chunk_lines(chunks: &mut [SyntaxChunk], line_delta: isize) {
        if line_delta == 0 {
            return;
        }

        for chunk in chunks {
            chunk.start_line = chunk.start_line.saturating_add_signed(line_delta);
        }
    }

    fn append_dirty_suffix_after_index(
        &mut self,
        dirty_suffix: &mut Vec<SyntaxChunk>,
        dirty_idx: usize,
    ) {
        self.flush_tail();

        let Some(first_line) = dirty_suffix.first().map(|chunk| chunk.start_line) else {
            return;
        };
        let line = first_line + dirty_idx;

        let split_idx =
            dirty_suffix.partition_point(|chunk| chunk.start_line + chunk.lines.len() <= line);
        if split_idx >= dirty_suffix.len() {
            return;
        }

        if dirty_suffix[split_idx].start_line < line {
            let chunk = dirty_suffix.remove(split_idx);
            let (_, suffix) = Self::split_chunk_at(&chunk, line - chunk.start_line);
            self.chunks.push(suffix);
            self.chunks.extend(dirty_suffix.drain(split_idx..));
        } else {
            self.chunks.extend(dirty_suffix.drain(split_idx..));
        }
    }

    fn truncate_cached_lines(&mut self, line_count: usize) {
        if self.cached_line_count() <= line_count {
            return;
        }

        self.invalidate_from(line_count, 0);
        self.clear_dirty_suffix();
    }

    fn truncate_cached_prefix(&mut self, line: usize) {
        if self.cached_line_count() <= line {
            return;
        }

        if line >= self.frozen_line_count() && self.truncate_tail_from(line) {
            return;
        }

        self.truncate_tail_from(line);
        self.flush_tail();
        let split_idx = self
            .chunks
            .partition_point(|chunk| chunk.start_line + chunk.lines.len() <= line);
        if self
            .chunks
            .get(split_idx)
            .is_some_and(|chunk| chunk.start_line < line)
        {
            let chunk = self.chunks.remove(split_idx);
            let (prefix, _) = Self::split_chunk_at(&chunk, line - chunk.start_line);
            self.chunks.truncate(split_idx);
            self.chunks.push(prefix);
        } else {
            self.chunks.truncate(split_idx);
        }
        self.tail = SyntaxChunkBuilder::new(line);
    }

    fn truncate_tail_from(&mut self, line: usize) -> bool {
        if line < self.tail.start_line || line > self.tail.start_line + self.tail.lines.len() {
            return false;
        }

        let line_offset = line - self.tail.start_line;
        let span_len = self
            .tail
            .lines
            .get(line_offset)
            .map(|line| line.span_start)
            .unwrap_or(self.tail.spans.len());
        self.tail.lines.truncate(line_offset);
        self.tail.spans.truncate(span_len);
        true
    }

    #[cfg(test)]
    fn push_cached_line_for_test(
        &mut self,
        line: impl Into<Arc<str>>,
        spans: Vec<SyntaxSpan>,
        state: SyntaxState,
    ) {
        self.append_line(line.into(), spans, state);
    }

    /// Invalidates cached syntax data from the provided line onward.
    pub fn invalidate_from(&mut self, line: usize, line_delta: isize) {
        if let Some(dirty_start) = self.dirty_start
            && line >= dirty_start
        {
            self.truncate_cached_prefix(line);
            self.dirty_suffix = Vec::new();
            self.dirty_start = Some(line);
            self.dirty_line_delta += line_delta;
            return;
        }

        if line >= self.cached_line_count() {
            if self.has_dirty_suffix() {
                let dirty_start = self.dirty_start.unwrap_or(line).min(line);
                self.dirty_suffix = Vec::new();
                self.dirty_start = Some(dirty_start);
                self.dirty_line_delta += line_delta;
            } else {
                self.clear_dirty_suffix();
            }
            return;
        }

        if line >= self.frozen_line_count() && self.truncate_tail_from(line) {
            self.clear_dirty_suffix();
            self.dirty_start = Some(line);
            self.dirty_line_delta = line_delta;
            return;
        }

        self.truncate_tail_from(line);

        if line_delta != 0 {
            self.flush_tail();
            let split_idx = self
                .chunks
                .partition_point(|chunk| chunk.start_line + chunk.lines.len() <= line);
            if self
                .chunks
                .get(split_idx)
                .is_some_and(|chunk| chunk.start_line < line)
            {
                let chunk = self.chunks.remove(split_idx);
                let (prefix, _) = Self::split_chunk_at(&chunk, line - chunk.start_line);
                self.chunks.truncate(split_idx);
                self.chunks.push(prefix);
            } else {
                self.chunks.truncate(split_idx);
            }

            self.tail = SyntaxChunkBuilder::new(line);
            self.clear_dirty_suffix();
            self.dirty_start = Some(line);
            self.dirty_line_delta = line_delta;
            return;
        }

        self.flush_tail();
        let split_idx = self
            .chunks
            .partition_point(|chunk| chunk.start_line + chunk.lines.len() <= line);

        if self.chunks[split_idx].start_line < line {
            let chunk = self.chunks.remove(split_idx);
            let (prefix, suffix) = Self::split_chunk_at(&chunk, line - chunk.start_line);
            self.chunks.push(prefix);
            self.dirty_suffix = Vec::with_capacity(self.chunks.len() - split_idx);
            self.dirty_suffix.push(suffix);
            self.dirty_suffix.extend(self.chunks.drain(split_idx + 1..));
        } else {
            self.dirty_suffix = self.chunks.split_off(split_idx);
        }
        Self::shift_chunk_lines(&mut self.dirty_suffix, line_delta);

        self.tail = SyntaxChunkBuilder::new(line);
        self.dirty_start = Some(line);
        self.dirty_line_delta = line_delta;
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

    /// Returns cached spans for rendering, falling back to the last stale snapshot.
    pub fn render_spans_for_line_ref(
        &self,
        line: usize,
        current_line_text: &str,
    ) -> Option<&[SyntaxSpan]> {
        if let Some(spans) = self.cached_spans_for_line_ref(line) {
            return Some(spans);
        }

        if self
            .dirty_start
            .is_some_and(|dirty_start| line == dirty_start)
        {
            return None;
        }

        let snapshot = self.render_snapshot.as_ref()?;
        let old_line = self.translate_render_snapshot_line(line)?;
        let snapshot_line = old_line.checked_sub(snapshot.start_line)?;
        snapshot
            .lines
            .get(snapshot_line)
            .filter(|line| line.line.as_ref() == current_line_text)
            .map(|line| line.spans.as_ref())
    }

    /// Returns how many leading lines currently have cached syntax data.
    pub fn cached_line_count(&self) -> usize {
        self.chunks
            .last()
            .map(|chunk| chunk.start_line + chunk.lines.len())
            .unwrap_or(0)
            .max(self.tail.start_line + self.tail.lines.len())
    }

    fn has_dirty_suffix(&self) -> bool {
        !self.dirty_suffix.is_empty()
    }

    fn pending_dirty_suffix_start(&self) -> Option<usize> {
        self.dirty_start
    }

    /// Returns true when every line in the buffer has a cached syntax result.
    pub fn is_complete_for_line_count(&self, line_count: usize) -> bool {
        self.dirty_suffix.is_empty() && self.cached_line_count() >= line_count
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
        self.set_syntax_name(syntax_definition.name());

        if line_texts.line_count() == 0 {
            self.chunks = Vec::new();
            self.tail = SyntaxChunkBuilder::new(0);
            self.clear_dirty_suffix();
            return;
        }

        let target_line = line.min(line_texts.line_count().saturating_sub(1));
        self.truncate_cached_lines(line_texts.line_count());

        if target_line < self.cached_line_count() && self.dirty_start.is_none() {
            return;
        }

        let dirty_start = self.dirty_start.unwrap_or(self.cached_line_count());
        let dirty_line_delta = self.dirty_line_delta;
        let dirty_suffix = std::mem::take(&mut self.dirty_suffix);
        if self.cached_line_count() > dirty_start {
            self.dirty_suffix = dirty_suffix;
            self.dirty_start = Some(dirty_start);
            self.dirty_line_delta = dirty_line_delta;
            self.truncate_cached_lines(dirty_start);
            let dirty_suffix = std::mem::take(&mut self.dirty_suffix);
            self.dirty_start = Some(dirty_start);
            self.dirty_line_delta = dirty_line_delta;
            let mut dirty_suffix = dirty_suffix;
            return self.ensure_through_with_rescan(
                syntax_definition,
                line_texts,
                target_line,
                dirty_start,
                dirty_line_delta,
                &mut dirty_suffix,
                should_continue,
            );
        }
        let mut dirty_suffix = dirty_suffix;

        self.ensure_through_with_rescan(
            syntax_definition,
            line_texts,
            target_line,
            dirty_start,
            dirty_line_delta,
            &mut dirty_suffix,
            should_continue,
        );
    }

    fn ensure_through_with_rescan<F>(
        &mut self,
        syntax_definition: &SyntaxDefinition,
        line_texts: &PieceTable,
        target_line: usize,
        dirty_start: usize,
        dirty_line_delta: isize,
        dirty_suffix: &mut Vec<SyntaxChunk>,
        mut should_continue: F,
    ) where
        F: FnMut() -> bool,
    {
        let mut state = self
            .line_view(self.cached_line_count().saturating_sub(1))
            .map(|line| line.state.clone())
            .unwrap_or_default();
        let mut current_line = self.cached_line_count();
        let mut scratch = String::new();

        while current_line <= target_line {
            if !should_continue() {
                self.dirty_suffix = std::mem::take(dirty_suffix);
                self.dirty_start = Some(dirty_start);
                self.dirty_line_delta = dirty_line_delta;
                return;
            }

            let line_ref = line_texts.line(current_line).expect("target line exists");
            let line_text = line_ref.contiguous_text_with_scratch(&mut scratch);
            let (spans, next_state) = tokenize_line_definition(syntax_definition, line_text, state);
            self.append_line(Arc::from(line_text), spans, next_state.clone());

            let dirty_idx = current_line as isize - dirty_start as isize - dirty_line_delta;
            if dirty_idx >= 0 {
                let dirty_idx = dirty_idx as usize;
                if let Some(old_entry) = Self::dirty_line_view_by_index(dirty_suffix, dirty_idx)
                    && old_entry.line == line_text
                    && *old_entry.state == next_state
                {
                    self.append_dirty_suffix_after_index(dirty_suffix, dirty_idx + 1);
                    self.clear_dirty_suffix();
                    return;
                }
            }

            state = next_state;
            current_line += 1;
        }

        if target_line == line_texts.line_count() - 1 {
            self.clear_dirty_suffix();
        } else {
            self.dirty_suffix = std::mem::take(dirty_suffix);
            self.dirty_start = Some(dirty_start);
            self.dirty_line_delta = dirty_line_delta;
        }
    }
}

impl ApplyEdit for SyntaxCache {
    fn apply_edit(&mut self, edit: LineEdit) {
        SyntaxCache::apply_edit(self, edit);
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

    /// Applies one normalized edit to the indent scope cache.
    pub fn apply_edit(&mut self, edit: LineEdit) {
        self.apply_edits(&[edit]);
    }

    /// Applies a batch of normalized edits to the indent scope cache.
    pub fn apply_edits(&mut self, edits: &[LineEdit]) {
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

impl ApplyEdit for IndentScopeCache {
    fn apply_edit(&mut self, edit: LineEdit) {
        IndentScopeCache::apply_edit(self, edit);
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

    /// Replaces only the syntax cache portion.
    pub fn replace_syntax_cache(&mut self, syntax_cache: SyntaxCache) {
        self.syntax_cache = syntax_cache;
    }

    /// Applies one normalized edit to the buffer cache.
    pub fn apply_edit(&mut self, edit: LineEdit) {
        self.apply_edits(&[edit]);
    }

    /// Applies a batch of normalized edits to the buffer cache.
    pub fn apply_edits(&mut self, edits: &[LineEdit]) {
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
        self.apply_edit(LineEdit::new(line, line_delta));
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
        current_line_text: &str,
    ) -> Option<&[SyntaxSpan]> {
        self.syntax_cache
            .render_spans_for_line_ref(line, current_line_text)
    }

    /// Returns how many leading lines currently have cached syntax data.
    pub fn cached_line_count(&self) -> usize {
        self.syntax_cache.cached_line_count()
    }

    /// Returns true when every line in the buffer has a cached syntax result.
    pub fn is_complete_for_line_count(&self, line_count: usize) -> bool {
        self.syntax_cache.is_complete_for_line_count(line_count)
    }

    fn pending_syntax_dirty_suffix_start(&self) -> Option<usize> {
        self.syntax_cache.pending_dirty_suffix_start()
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

impl ApplyEdit for BufferCache {
    fn apply_edit(&mut self, edit: LineEdit) {
        BufferCache::apply_edit(self, edit);
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
                self.cache.ensure_through_with_definition(
                    syntax_definition.as_ref(),
                    &self.line_texts,
                    target_line,
                    || context.is_current(),
                );
            }
        }

        event_tx
            .send(JobEvent::Completed {
                kind: context.kind().clone(),
                token: context.token(),
                payload: Some(JobPayload::SyntaxRefresh(SyntaxRefreshResult {
                    buffer_id: self.buffer_id,
                    generation: self.generation,
                    syntax_cache: self.cache,
                })),
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
enum SyntaxState {
    #[default]
    Plain,
    Code(CodeState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuleListInjectionState {
    nested: Option<NestedState>,
    fallback: InjectedSyntaxFallback,
    parent_style: Option<Tag>,
}

#[derive(Debug, Clone)]
enum NestedState {
    Syntax {
        syntax_definition: Arc<SyntaxDefinition>,
        state: Box<SyntaxState>,
    },
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

fn tokenize_line_definition(
    definition: &SyntaxDefinition,
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    if !definition.rules().is_empty() {
        tokenize_rule_list_line(definition, line, state)
    } else {
        tokenize_code_line(definition, line, state)
    }
}

fn tokenize_rule_list_line(
    definition: &SyntaxDefinition,
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    let mut spans = Vec::new();
    let mut index = 0;
    let regex_rule_indexes = definition.regex_rule_indexes();
    let injection_rule_indexes = definition.injection_rule_indexes();
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

        if injection.is_some() && !has_active_injection(definition, contexts) {
            *injection = None;
        }

        if let Some(injection_state) = injection.as_mut() {
            if let Some(regex_match) = find_next_rule_list_regex_match(
                definition,
                regex_rule_indexes,
                line,
                index,
                contexts,
            ) {
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
        for rule_idx in regex_rule_indexes {
            if let SyntaxRule::Regex {
                regex,
                lookahead,
                tag,
                context,
            } = &definition.rules[*rule_idx]
            {
                if let Some(context) = context.as_ref()
                    && !contexts.contains_all(&context.requires)
                {
                    continue;
                }
                if let Some(hit) = regex_match_at(regex, lookahead.as_ref(), line, index, true) {
                    let matched_text = line.get(hit.start..hit.end).unwrap_or("");
                    if let Some(context) = context.as_ref()
                        && !context_payload_matches(
                            contexts,
                            context,
                            matched_text,
                            hit.captures.as_ref(),
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
                            hit.start,
                            hit.end,
                            hit.captures.expect("captures requested"),
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
            let mut chosen_spans = vec![SyntaxSpan::new(start, end, tag.clone())];
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
        for rule_idx in injection_rule_indexes {
            if let SyntaxRule::Injection {
                selector,
                fallback,
                context,
            } = &definition.rules[*rule_idx]
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

fn has_active_injection(definition: &SyntaxDefinition, contexts: &ContextStack) -> bool {
    definition.injection_rule_indexes().iter().any(|rule_idx| {
        matches!(
            &definition.rules[*rule_idx],
            SyntaxRule::Injection { context, .. }
                if context
                    .as_ref()
                    .is_some_and(|context| contexts.contains_all(&context.requires))
        )
    })
}

fn find_next_rule_list_regex_match(
    definition: &SyntaxDefinition,
    regex_rule_indexes: &[usize],
    line: &str,
    start: usize,
    contexts: &ContextStack,
) -> Option<(usize, usize)> {
    let mut index = start;
    while index < line.len() {
        for rule_idx in regex_rule_indexes {
            if let SyntaxRule::Regex {
                regex,
                lookahead,
                context,
                ..
            } = &definition.rules[*rule_idx]
            {
                if let Some(context) = context.as_ref()
                    && !contexts.contains_all(&context.requires)
                {
                    continue;
                }
                if contexts.contains("markdown_code_fence_body") && context.is_none() {
                    continue;
                }
                if let Some(hit) = regex_match_at(regex, lookahead.as_ref(), line, index, false) {
                    return Some((hit.start, hit.end));
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
    let state = initial_state_for_definition(definition.as_ref());
    Some(NestedState::Syntax {
        syntax_definition: definition,
        state: Box::new(state),
    })
}

fn tokenize_code_line(
    definition: &SyntaxDefinition,
    line: &str,
    state: SyntaxState,
) -> (Vec<SyntaxSpan>, SyntaxState) {
    let state = match state {
        SyntaxState::Code(code_state) => code_state,
        _ => CodeState::Normal {
            contexts: ContextStack::default(),
        },
    };

    match state {
        CodeState::RuleList {
            contexts,
            injection,
            parent_style,
        } => {
            let (spans, next_state) = tokenize_rule_list_line(
                definition,
                line,
                SyntaxState::Code(CodeState::RuleList {
                    contexts,
                    injection,
                    parent_style,
                }),
            );
            let state = match next_state {
                SyntaxState::Code(CodeState::RuleList {
                    contexts,
                    injection,
                    parent_style,
                }) => CodeState::RuleList {
                    contexts,
                    injection,
                    parent_style,
                },
                SyntaxState::Code(CodeState::Normal { contexts }) => CodeState::Normal { contexts },
                SyntaxState::Plain => CodeState::Normal {
                    contexts: ContextStack::default(),
                },
            };
            (spans, SyntaxState::Code(state))
        }
        CodeState::Normal { contexts } => (
            Vec::new(),
            SyntaxState::Code(CodeState::Normal { contexts }),
        ),
    }
}

fn tokenize_nested_body(nested: &mut NestedState, body: &str, offset: usize) -> Vec<SyntaxSpan> {
    match nested {
        NestedState::Syntax {
            syntax_definition,
            state,
        } => {
            let (spans, next_state) =
                tokenize_line_definition(syntax_definition.as_ref(), body, state.as_ref().clone());
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

struct RegexMatch<'a> {
    start: usize,
    end: usize,
    captures: Option<regex::Captures<'a>>,
}

fn regex_match_at<'a>(
    regex: &'a Regex,
    lookahead: Option<&'a Regex>,
    line: &'a str,
    index: usize,
    want_captures: bool,
) -> Option<RegexMatch<'a>> {
    let pattern = regex.as_str();
    let (start, end, captures) = if want_captures {
        if pattern.starts_with('^') || pattern.starts_with("\\A") {
            let tail = line.get(index..)?;
            let captures = regex.captures(tail)?;
            let matched = captures.get(0)?;
            if matched.start() != 0 {
                return None;
            }
            (index, index + matched.end(), Some(captures))
        } else {
            let captures = regex.captures_at(line, index)?;
            let matched = captures.get(0)?;
            if matched.start() != index {
                return None;
            }
            (matched.start(), matched.end(), Some(captures))
        }
    } else if pattern.starts_with('^') || pattern.starts_with("\\A") {
        let tail = line.get(index..)?;
        let matched = regex.find(tail)?;
        if matched.start() != 0 {
            return None;
        }
        (index, index + matched.end(), None)
    } else if let Some(matched) = regex.find_at(line, index) {
        if matched.start() != index {
            return None;
        }
        (matched.start(), matched.end(), None)
    } else {
        return None;
    };

    if let Some(lookahead) = lookahead {
        let tail = line.get(end..)?;
        let matched = lookahead.find(tail)?;
        if matched.start() != 0 {
            return None;
        }
    }

    Some(RegexMatch {
        start,
        end,
        captures,
    })
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
        self.apply_cache_edits(&[LineEdit::new(line, line_delta)]);
    }

    /// Applies normalized cache edits and refreshes the visible cache state.
    pub fn apply_cache_edits(&mut self, edits: &[LineEdit]) {
        if edits.is_empty() {
            return;
        }

        self.buffer_cache.apply_edits(edits);
        self.generations.syntax = self.generations.syntax.wrapping_add(1);
        self.generations.syntax_background = None;
        self.generations.indent_background = None;
        self.generations.diff = self.generations.diff.wrapping_add(1);
        self.generations.diff_background = None;
        if edits.iter().any(|edit| edit.start_line == 0) {
            self.refresh_syntax();
        }

        if !self.lines.is_empty() {
            let syntax_name = self.syntax_name().to_owned();
            self.buffer_cache.ensure_syntax_through_with_budget(
                &syntax_name,
                &self.lines,
                self.line_count().saturating_sub(1),
                std::time::Duration::from_millis(2),
            );
            self.buffer_cache.ensure_indent_through_with_budget(
                &self.lines,
                self.line_count().saturating_sub(1),
                globals::with_config(|config| config.tab_width)
                    .unwrap_or(4)
                    .max(1),
                std::time::Duration::from_millis(2),
            );
            self.sync_undo_snapshot_cache_if_current();
        }
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
        current_line_text: &str,
    ) -> Option<&[SyntaxSpan]> {
        self.buffer_cache
            .render_spans_for_line_ref(line, current_line_text)
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

    /// Returns where a pending invalidated syntax suffix starts, if one exists.
    pub fn pending_syntax_dirty_suffix_start(&self) -> Option<usize> {
        self.buffer_cache.pending_syntax_dirty_suffix_start()
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
        if self.generations.syntax_background == Some(result.generation) {
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
        let syntax_needed = (self.buffer_cache.syntax_cache.has_dirty_suffix()
            || !self.syntax_cache_complete())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{BackgroundJob, JobEvent, JobHandle, JobKind, JobPayload, JobToken};
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
        let definition = SyntaxDefinition::new(
            crate::syntax::SyntaxMetadata {
                name: SmolStr::new("example"),
                display_name: SmolStr::new("Example"),
                alias: Vec::new(),
                comment_prefix: None,
                glyph: None,
                glyph_color: None,
                filename: Vec::new(),
                shebang: Vec::new(),
            },
            vec![
                SyntaxRule::Regex {
                    regex: Regex::new(r"^```[A-Za-z]+$").expect("valid opener regex"),
                    lookahead: None,
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
                    lookahead: None,
                    tag: tag("markup.code"),
                    context: Some(ContextControl {
                        requires: vec![SmolStr::new("fence")],
                        push: Vec::new(),
                        pop: vec![SmolStr::new("fence")],
                        payload_match: None,
                    }),
                },
            ],
            vec![0, 2],
            vec![1],
        );

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

        cache.ensure_through("rust", &lines, SYNTAX_CHUNK_SIZE + 1);
        assert!(cache.is_complete_for_line_count(lines.line_count()));

        cache.invalidate_from(1, 0);
        cache.ensure_through_with("rust", &lines, lines.line_count() - 1, || false);

        assert_eq!(cache.cached_line_count(), 1);
        assert!(!cache.is_complete_for_line_count(lines.line_count()));
    }

    #[test]
    fn syntax_cache_stops_when_dirty_suffix_reconverges() {
        let original = PieceTable::from_text("value = \"\"\"hello\nplanet\nworld\"\"\"\nafter");
        let edited = PieceTable::from_text("value = \"\"\"hello\ngalaxy\nworld\"\"\"\nafter");
        let mut cache = SyntaxCache::new("toml");

        cache.ensure_through("toml", &original, original.line_count() - 1);
        assert!(cache.is_complete_for_line_count(original.line_count()));

        cache.invalidate_from(1, 0);
        assert!(!cache.is_complete_for_line_count(original.line_count()));

        cache.ensure_through("toml", &edited, 2);

        assert_eq!(cache.cached_line_count(), 3);
        assert!(cache.cached_spans_for_line(3).is_none());
    }

    #[test]
    fn syntax_cache_invalidates_inside_frozen_chunk() {
        let text = (0..SYNTAX_CHUNK_SIZE * 3 + 2)
            .map(|line| format!("value_{line} = {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let lines = PieceTable::from_text(&text);
        let mut cache = SyntaxCache::new("rust");

        cache.ensure_through("rust", &lines, SYNTAX_CHUNK_SIZE + 1);
        assert!(cache.cached_spans_for_line(SYNTAX_CHUNK_SIZE + 1).is_some());

        cache.invalidate_from(SYNTAX_CHUNK_SIZE + 2, 0);
        assert_eq!(cache.cached_line_count(), SYNTAX_CHUNK_SIZE + 2);
        assert!(cache.cached_spans_for_line(SYNTAX_CHUNK_SIZE - 2).is_some());
        assert!(cache.cached_spans_for_line(SYNTAX_CHUNK_SIZE + 1).is_some());
        assert!(cache.cached_spans_for_line(SYNTAX_CHUNK_SIZE + 2).is_none());

        cache.ensure_through("rust", &lines, lines.line_count() - 1);
        assert!(cache.is_complete_for_line_count(lines.line_count()));
    }

    #[test]
    fn syntax_cache_reconverges_across_frozen_chunk_boundary() {
        let original = (0..SYNTAX_CHUNK_SIZE * 3 + 4)
            .map(|line| format!("value_{line} = {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let edited = (0..SYNTAX_CHUNK_SIZE * 3 + 4)
            .map(|line| {
                if line == SYNTAX_CHUNK_SIZE - 1 {
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

        cache.ensure_through("rust", &original, SYNTAX_CHUNK_SIZE + 3);
        cache.invalidate_from(SYNTAX_CHUNK_SIZE - 1, 0);
        cache.ensure_through("rust", &edited, SYNTAX_CHUNK_SIZE);

        assert!(cache.cached_spans_for_line(SYNTAX_CHUNK_SIZE).is_some());
        assert!(cache.cached_spans_for_line(SYNTAX_CHUNK_SIZE + 3).is_none());
        cache.ensure_through("rust", &edited, SYNTAX_CHUNK_SIZE + 3);
        assert!(cache.cached_spans_for_line(SYNTAX_CHUNK_SIZE + 3).is_some());
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

        assert!(!buffer.indent_scope_cache_stale());
        assert_eq!(scope_tuples(&buffer), vec![(0, Some(2), 0)]);
        assert!(buffer.cached_line_indent_scope_ids(0).is_some());
        assert!(buffer.cached_line_indent_scope_ids(1).is_some());
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

        assert!(!buffer.indent_scope_cache_stale());
        assert_eq!(
            scope_tuples(&buffer),
            vec![(0, Some(4), 0), (1, Some(3), 2)]
        );
        assert!(buffer.cached_line_indent_scope_ids(0).is_some());
        assert!(buffer.cached_line_indent_scope_ids(1).is_some());

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
        assert!(buffer.cached_syntax_spans_for_line(1).is_some());
        assert!(buffer.syntax_cache_complete());
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

        let event = wait_for_event(&handle);
        let result = match event {
            JobEvent::Completed {
                payload: Some(JobPayload::SyntaxRefresh(result)),
                ..
            } => result,
            other => panic!("expected syntax refresh completion, got {:?}", other),
        };

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

        let syntax_event = wait_for_event(&handle);
        let (token, result) = match syntax_event {
            JobEvent::Completed {
                token,
                payload: Some(JobPayload::SyntaxRefresh(result)),
                ..
            } => (token, result),
            other => panic!("expected latest syntax completion, got {:?}", other),
        };
        assert_eq!(token.generation(), 2);
        assert!(result.syntax_cache.is_complete_for_line_count(32));

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
