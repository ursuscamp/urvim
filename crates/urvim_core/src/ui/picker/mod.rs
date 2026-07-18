//! Reusable fuzzy picker widget.
//!
//! This module provides a generic overlay picker that can stream results from a
//! background source and emit selection intents for different result types.

pub mod buffer;
pub mod code_actions;
pub mod colorscheme;
pub mod doc_symbols;
pub mod file;
pub mod filetype;
pub mod git;
pub mod grep;
pub mod line;
pub mod plugin;
pub mod preview;
pub mod query;
pub mod references;

use crate::background::JobManager;
use crate::screen::Screen;
use crate::ui::geometry::{Position, Size};
use crate::ui::inputs::{InputWidget, PromptSegment};
pub use crate::ui::line_format::{
    EllipsisPlacement, FormattedLineSection, FormattedLineSegment, FormattedLineTemplate,
    LineSectionAlignment, LineSectionOverflow, LineSectionWidth,
};
use crate::ui::overlay::frame::{
    OverlayAnchor, OverlayFrame, OverlayFrameLabel, OverlayMargins, OverlayPlacement,
};
use crate::ui::picker::preview::PickerPreviewAdapter;
use crate::ui::picker::query::PickerQueryMode;
use crate::ui::text_width::{ClipSide, EllipsisSide, clip_text, ellipsize_text};
use crate::ui::{FocusPolicy, Intent, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use unicode_width::UnicodeWidthStr;
use urvim_terminal::{KeyCode, Style};

const MAX_VISIBLE_RESULTS: usize = 8;
const PICKER_CONTENT_COLS: u16 = 80;
const PICKER_TOP_MARGIN: u16 = 5;
const PROMPT_ROWS: u16 = 1;
const SEPARATOR_ROWS: u16 = 1;
static NEXT_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);
static NEXT_PICKER_PREVIEW_GENERATION: AtomicU64 = AtomicU64::new(1);

const PREVIEW_MIN_COLS: u16 = 48;
const PREVIEW_PREFERRED_COLS: u16 = 80;
const PREVIEW_MIN_ROWS: u16 = 5;
const PREVIEW_PREFERRED_CONTENT_ROWS: u16 = 20;

/// Picker search events streamed from the background worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerSearchEvent<T> {
    /// A search generation has started.
    PickerSearchStarted { generation: u64, query: String },
    /// A chunk of results is available.
    PickerChunk { generation: u64, chunk: Vec<T> },
    /// A ranked snapshot of the current results is available.
    PickerResults { generation: u64, results: Vec<T> },
    /// The search became stale before completion.
    PickerSearchStale { generation: u64 },
    /// The search finished for the current generation.
    PickerSearchComplete { generation: u64 },
}

/// Preview loading events streamed from a picker source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerPreviewEvent {
    /// A preview request has started.
    Started { generation: u64 },
    /// A preview request finished successfully.
    Loaded {
        /// Preview generation.
        generation: u64,
        /// Loaded preview contents.
        preview: PickerPreview,
    },
    /// A preview request failed.
    Failed {
        /// Preview generation.
        generation: u64,
        /// Human-readable failure message.
        message: String,
    },
}

/// Syntax-highlightable preview content for a picker item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerPreview {
    /// Preview title displayed above the text.
    pub title: String,
    /// One-based line number for the first preview line.
    pub start_line: usize,
    /// One-based line number to draw with active-line styling.
    pub highlighted_line: Option<usize>,
}

impl PickerPreview {
    /// Creates picker preview content.
    pub fn new(
        title: impl Into<String>,
        start_line: usize,
        highlighted_line: Option<usize>,
    ) -> Self {
        Self {
            title: title.into(),
            start_line,
            highlighted_line,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PickerPreviewState {
    Empty,
    Loading,
    Ready(PickerPreview),
    Error(String),
}

/// A styled text segment rendered inside a picker result row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerRenderSegment {
    /// Segment text.
    pub text: String,
    /// Segment style.
    pub style: Style,
}

impl PickerRenderSegment {
    /// Creates a styled picker render segment.
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

impl From<FormattedLineSegment> for PickerRenderSegment {
    fn from(segment: FormattedLineSegment) -> Self {
        Self::new(segment.text, segment.style)
    }
}

/// A formatted picker row described by a reusable line template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerFormattedLine {
    template: FormattedLineTemplate,
    values: Vec<String>,
}

impl PickerFormattedLine {
    /// Creates a picker row from a template and matching values.
    pub fn new(template: FormattedLineTemplate, values: Vec<String>) -> Self {
        Self { template, values }
    }

    /// Renders the picker row into styled segments.
    pub fn render_segments(&self, available_cols: usize) -> Vec<PickerRenderSegment> {
        self.template
            .render_segments(
                self.values.iter().map(String::as_str),
                available_cols.min(u16::MAX as usize) as u16,
            )
            .expect("picker item line template")
            .into_iter()
            .map(Into::into)
            .collect()
    }
}

impl PickerItem for String {
    fn formatted_line(&self, base_style: Style) -> PickerFormattedLine {
        PickerFormattedLine::new(
            FormattedLineTemplate::new(vec![
                FormattedLineSection::measured(base_style)
                    .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::Start)),
            ]),
            vec![self.clone()],
        )
    }
}

/// An item that can render itself for display inside a picker result row.
pub trait PickerItem: Clone + Send + 'static {
    /// Returns a formatted line description for the item.
    fn formatted_line(&self, base_style: Style) -> PickerFormattedLine;

    /// Returns styled segments for the item using the provided width budget.
    fn render_segments(
        &self,
        available_cols: usize,
        base_style: Style,
    ) -> Vec<PickerRenderSegment> {
        self.formatted_line(base_style)
            .render_segments(available_cols)
    }

    /// Returns whether the picker should pad the row to full width.
    fn pad_to_full_width(&self) -> bool {
        true
    }
}

/// Source of picker results and selection behavior.
pub trait PickerSource: Send + 'static {
    /// The item type displayed by the picker.
    type Item: PickerItem;

    /// Marks the active search generation.
    fn set_generation(&self, generation: u64);

    /// Starts an asynchronous search for the given query.
    fn start_search(
        &self,
        query: &str,
        generation: u64,
        sender: Sender<PickerSearchEvent<Self::Item>>,
    );

    /// Returns the job manager used by this picker source.
    fn job_manager(&self) -> std::sync::Arc<JobManager>;

    /// Cancels any active search, if the source supports it.
    fn cancel_search(&self) {}

    /// Toggles the query mode, if the source supports mode switching.
    fn toggle_query_mode(&self) -> Option<PickerQueryMode> {
        None
    }

    /// Returns the prompt segments for the given query mode, if supported.
    fn query_prompt_segments_for_mode(&self, _mode: PickerQueryMode) -> Option<Vec<PromptSegment>> {
        None
    }

    /// Returns a stable key for the selected item's preview.
    fn preview_key(&self, _item: &Self::Item) -> Option<String> {
        None
    }

    /// Returns a stable key for preserving selection across result refreshes.
    fn result_key(&self, _item: &Self::Item) -> Option<String> {
        None
    }

    /// Returns an intent for staging or unstaging the given item, if supported.
    fn stage_intent(&self, _item: &Self::Item) -> Option<Intent> {
        None
    }

    /// Returns an intent for discarding the given item, if supported.
    fn discard_intent(&self, _item: &Self::Item) -> Option<Intent> {
        None
    }

    /// Starts loading preview content for an item without blocking the UI thread.
    fn start_preview(
        &self,
        _item: Self::Item,
        _generation: u64,
        _sender: Sender<PickerPreviewEvent>,
    ) {
    }

    /// Cancels any active preview work, if the source supports it.
    fn cancel_preview(&self) {}

    /// Converts a selected item into an editor intent.
    fn select(&self, item: &Self::Item) -> Intent;
}

/// Generic reusable fuzzy picker widget.
#[derive(Debug)]
pub struct PickerWidget<S: PickerSource> {
    source: S,
    query_input: InputWidget,
    results: Vec<S::Item>,
    highlighted: Option<usize>,
    visible_start: usize,
    generation: u64,
    open: bool,
    search_active: bool,
    search_complete: bool,
    pending_result_replacement: bool,
    cursor: Option<Position>,
    receiver: Receiver<PickerSearchEvent<S::Item>>,
    sender: Sender<PickerSearchEvent<S::Item>>,
    preview_generation: u64,
    preview_key: Option<String>,
    preview_highlighted: Option<usize>,
    preview_state: PickerPreviewState,
    preview_adapter: PickerPreviewAdapter,
    preview_receiver: Receiver<PickerPreviewEvent>,
    preview_sender: Sender<PickerPreviewEvent>,
    label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PickerLayout {
    picker: OverlayFrame,
    preview: Option<OverlayFrame>,
}

fn frame_from_outer(origin: Position, size: Size) -> OverlayFrame {
    OverlayFrame {
        origin,
        size,
        content_origin: Position::new(origin.row.saturating_add(1), origin.col.saturating_add(1)),
        content_size: Size::new(size.rows.saturating_sub(2), size.cols.saturating_sub(2)),
    }
}

impl<S: PickerSource> PickerWidget<S> {
    /// Creates a new picker widget backed by a source.
    pub fn new(source: S) -> Self {
        let jobs = source.job_manager();
        let (sender, receiver) = std::sync::mpsc::channel();
        let (preview_sender, preview_receiver) = std::sync::mpsc::channel();
        let mut query_input = InputWidget::new("");
        query_input.set_prompt(">");
        Self {
            source,
            query_input,
            results: Vec::new(),
            highlighted: None,
            visible_start: 0,
            generation: 0,
            open: true,
            search_active: false,
            search_complete: false,
            pending_result_replacement: false,
            cursor: None,
            receiver,
            sender,
            preview_generation: 0,
            preview_key: None,
            preview_highlighted: None,
            preview_state: PickerPreviewState::Empty,
            preview_adapter: PickerPreviewAdapter::with_jobs(jobs),
            preview_receiver,
            preview_sender,
            label: None,
        }
    }

    /// Returns true when the picker is active.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns the current query text.
    pub fn query(&self) -> &str {
        self.query_input.text()
    }

    /// Returns the current result list.
    pub fn results(&self) -> &[S::Item] {
        self.results.as_slice()
    }

    /// Returns the highlighted result index.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.highlighted
    }

    /// Closes the picker and cancels any active search.
    pub fn close(&mut self) {
        self.source.cancel_search();
        self.source.cancel_preview();
        self.preview_adapter.clear();
        self.open = false;
    }

    /// Returns the last rendered cursor position, if any.
    pub fn cursor(&self) -> Option<Position> {
        self.cursor
    }

    /// Returns the cursor position at the end of the search input, if open.
    pub fn cursor_position(&self, rect: UiRect) -> Option<Position> {
        if !self.open {
            return None;
        }

        let frame = self.resolve_frame(rect)?;
        let (_, cursor_col) = self
            .query_input
            .render_segments(frame.content_size.cols, Style::default());
        Some(Position::new(
            frame.content_origin.row,
            frame.content_origin.col.saturating_add(cursor_col),
        ))
    }

    /// Returns a mutable reference to the backing picker source.
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    /// Sets the label rendered in the picker's top border.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }

    fn reset_preview_scroll_for_key(&mut self, key: &str) {
        if let Some(pane) = self.preview_adapter.preview_pane_mut(key) {
            pane.set_follow_highlight(true);
        }
    }

    fn scroll_current_preview(&mut self, upwards: bool) -> bool {
        let Some(key) = self.preview_key.clone() else {
            return false;
        };
        let Some(pane) = self.preview_adapter.preview_pane_mut(key.as_str()) else {
            return false;
        };

        if upwards {
            pane.page_up();
        } else {
            pane.page_down();
        }
        true
    }

    /// Sets the search prompt text.
    pub fn set_query_prompt(&mut self, prompt: impl Into<String>) {
        self.query_input.set_prompt(prompt);
    }

    /// Sets the search prompt as styled segments.
    pub fn set_query_prompt_segments(&mut self, prompt: Vec<PromptSegment>) {
        self.query_input.set_prompt_segments(prompt);
        self.sync_query_right_prompt();
    }

    fn sync_query_right_prompt(&mut self) {
        self.query_input
            .set_right_prompt_segments(self.query_right_prompt_segments());
    }

    fn query_right_prompt_segments(&self) -> Vec<PromptSegment> {
        let Some(current) = self.highlighted.map(|index| index.saturating_add(1)) else {
            return Vec::new();
        };

        let total = self.results.len();
        if total == 0 {
            return Vec::new();
        }

        let count_style = self
            .query_input
            .prompt_segments()
            .first()
            .map(|segment| segment.style)
            .unwrap_or_else(Style::default);
        let separator_style = self
            .query_input
            .prompt_segments()
            .get(1)
            .map(|segment| segment.style)
            .unwrap_or_else(|| theme_style("ui.input.prompt.separator"));

        vec![
            PromptSegment::new(
                format!(" {} ", picker_indicator_glyph_backwards()),
                separator_style,
            ),
            PromptSegment::new(format!("{current}/{total}"), count_style),
        ]
    }

    fn resolve_frame(&self, rect: UiRect) -> Option<OverlayFrame> {
        self.resolve_layout(rect).map(|layout| layout.picker)
    }

    fn resolve_layout(&self, rect: UiRect) -> Option<PickerLayout> {
        if rect.size.rows < 3 || rect.size.cols < 3 {
            return None;
        }

        let page_size = self.page_size(rect.size.rows);
        let visible_results = self.visible_results(page_size);
        let status_line = self.status_line();
        let picker_content_cols = PICKER_CONTENT_COLS.min(rect.size.cols.saturating_sub(2).max(1));
        let result_rows = visible_results
            .len()
            .max(usize::from(status_line.is_some()));
        let picker_content_rows = usize::from(PROMPT_ROWS + SEPARATOR_ROWS) + result_rows;
        let picker_outer = Size::new(
            (picker_content_rows as u16)
                .saturating_add(2)
                .min(rect.size.rows),
            picker_content_cols.saturating_add(2).min(rect.size.cols),
        );

        if self.highlighted.is_some()
            && self.preview_key.is_some()
            && rect.size.cols >= picker_outer.cols + PREVIEW_MIN_COLS + 2
        {
            let preview_outer_cols = PREVIEW_PREFERRED_COLS
                .min(rect.size.cols.saturating_sub(picker_outer.cols))
                .max(PREVIEW_MIN_COLS)
                .min(rect.size.cols.saturating_sub(picker_outer.cols));
            let combined_cols = picker_outer.cols.saturating_add(preview_outer_cols);
            let top = rect.origin.row.saturating_add(
                PICKER_TOP_MARGIN.min(rect.size.rows.saturating_sub(picker_outer.rows)),
            );
            let left = rect
                .origin
                .col
                .saturating_add(rect.size.cols.saturating_sub(combined_cols) / 2);
            let picker = frame_from_outer(Position::new(top, left), picker_outer);
            let preview_outer_rows = picker_outer
                .rows
                .max(PREVIEW_PREFERRED_CONTENT_ROWS.saturating_add(2))
                .min(
                    rect.size
                        .rows
                        .saturating_sub(top.saturating_sub(rect.origin.row)),
                );
            let preview = frame_from_outer(
                Position::new(top, left.saturating_add(picker_outer.cols)),
                Size::new(preview_outer_rows, preview_outer_cols),
            );
            return Some(PickerLayout {
                picker,
                preview: Some(preview),
            });
        }

        let picker = OverlayFrame::resolve_placement(
            rect.origin,
            rect.size,
            picker_content_rows as u16,
            picker_content_cols,
            OverlayPlacement::Anchored {
                anchor: OverlayAnchor::TopCenter,
                margins: OverlayMargins {
                    top: PICKER_TOP_MARGIN,
                    ..OverlayMargins::default()
                },
            },
        )?;

        let below_rows = rect
            .origin
            .row
            .saturating_add(rect.size.rows)
            .saturating_sub(picker.origin.row.saturating_add(picker.size.rows));
        let preview = if self.highlighted.is_some()
            && self.preview_key.is_some()
            && below_rows >= PREVIEW_MIN_ROWS
        {
            Some(frame_from_outer(
                Position::new(
                    picker.origin.row.saturating_add(picker.size.rows),
                    picker.origin.col,
                ),
                Size::new(below_rows, picker.size.cols),
            ))
        } else {
            None
        };

        Some(PickerLayout { picker, preview })
    }

    fn page_size(&self, rows: u16) -> usize {
        usize::from(rows.saturating_sub(4)).clamp(1, MAX_VISIBLE_RESULTS)
    }

    fn visible_results(&self, page_size: usize) -> &[S::Item] {
        let start = self.visible_start.min(self.results.len());
        let end = (start + page_size).min(self.results.len());
        &self.results[start..end]
    }

    fn status_line(&self) -> Option<String> {
        if self.query_input.text().is_empty() && self.results.is_empty() {
            return Some("Type to search".to_string());
        }

        if self.search_active {
            return Some("Searching...".to_string());
        }

        if self.search_complete && self.results.is_empty() {
            return Some("No matches".to_string());
        }

        None
    }

    fn ensure_highlight_visible(&mut self) {
        let Some(index) = self.highlighted else {
            self.visible_start = 0;
            return;
        };

        let page_size = MAX_VISIBLE_RESULTS.max(1);
        if index < self.visible_start {
            self.visible_start = index;
            return;
        }

        if index >= self.visible_start.saturating_add(page_size) {
            self.visible_start = index + 1 - page_size;
        }
    }

    fn selected_result_key(&self) -> Option<String> {
        let index = self.highlighted?;
        self.results
            .get(index)
            .and_then(|item| self.source.result_key(item))
    }

    fn restore_highlight_after_replace(
        &mut self,
        previous_key: Option<String>,
        previous_index: Option<usize>,
    ) {
        if self.results.is_empty() {
            self.highlighted = None;
            self.visible_start = 0;
            self.preview_highlighted = None;
            self.refresh_preview_for_highlight();
            return;
        }

        if let Some(key) = previous_key.as_deref() {
            if let Some(index) = self
                .results
                .iter()
                .position(|item| self.source.result_key(item).as_deref() == Some(key))
            {
                self.highlighted = Some(index);
                self.ensure_highlight_visible();
                self.refresh_preview_for_highlight();
                return;
            }
        }

        self.highlighted = previous_index.map(|index| index.min(self.results.len() - 1));
        if self.highlighted.is_none() {
            self.visible_start = 0;
            self.preview_highlighted = None;
        }
        self.ensure_highlight_visible();
        self.refresh_preview_for_highlight();
    }

    fn move_highlight(&mut self, delta: isize) {
        if self.results.is_empty() {
            self.sync_query_right_prompt();
            return;
        }

        let len = self.results.len() as isize;
        let current = match self.highlighted {
            Some(idx) => idx as isize,
            None if delta > 0 => -1,
            None => 0,
        };
        let next = (current + delta).rem_euclid(len) as usize;
        self.highlighted = Some(next);
        self.ensure_highlight_visible();
        self.refresh_preview_for_highlight();
        self.sync_query_right_prompt();
    }

    /// Restarts the active search using the current query text.
    pub fn restart_search(&mut self) {
        self.generation = NEXT_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst);
        self.search_active = false;
        self.search_complete = false;
        self.source.set_generation(self.generation);

        self.pending_result_replacement = !self.results.is_empty();
        self.search_active = true;
        self.source.start_search(
            self.query_input.text(),
            self.generation,
            self.sender.clone(),
        );
        self.sync_query_right_prompt();
    }

    fn drain_search_events(&mut self) -> bool {
        let mut handled = false;
        loop {
            match self.receiver.try_recv() {
                Ok(event) => {
                    handled = true;
                    self.handle_search_event(event);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        handled
    }

    fn drain_preview_events(&mut self) -> bool {
        let mut handled = false;
        loop {
            match self.preview_receiver.try_recv() {
                Ok(event) => {
                    handled = true;
                    self.handle_preview_event(event);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }

        handled
    }

    fn refresh_preview_for_highlight(&mut self) {
        let Some(index) = self.highlighted else {
            self.source.cancel_preview();
            self.preview_key = None;
            self.preview_highlighted = None;
            self.preview_state = PickerPreviewState::Empty;
            return;
        };

        let Some(item) = self.results.get(index).cloned() else {
            self.source.cancel_preview();
            self.preview_key = None;
            self.preview_highlighted = None;
            self.preview_state = PickerPreviewState::Empty;
            return;
        };

        let Some(key) = self.source.preview_key(&item) else {
            self.source.cancel_preview();
            self.preview_key = None;
            self.preview_highlighted = None;
            self.preview_state = PickerPreviewState::Empty;
            return;
        };

        if self.preview_key.as_deref() == Some(key.as_str())
            && self.preview_highlighted == Some(index)
        {
            return;
        }

        self.source.cancel_preview();
        self.preview_generation = NEXT_PICKER_PREVIEW_GENERATION.fetch_add(1, Ordering::SeqCst);
        self.reset_preview_scroll_for_key(key.as_str());
        self.preview_key = Some(key.clone());
        self.preview_highlighted = Some(index);
        self.preview_state = PickerPreviewState::Loading;

        self.preview_adapter
            .request_syntax_refresh_for_path(std::path::Path::new(key.as_str()))
            .ok();

        self.source
            .start_preview(item, self.preview_generation, self.preview_sender.clone());
    }

    fn handle_preview_event(&mut self, event: PickerPreviewEvent) {
        match event {
            PickerPreviewEvent::Started { generation } if generation == self.preview_generation => {
                self.preview_state = PickerPreviewState::Loading;
            }
            PickerPreviewEvent::Loaded {
                generation,
                preview,
            } if generation == self.preview_generation => {
                self.preview_state = PickerPreviewState::Ready(preview);
            }
            PickerPreviewEvent::Failed {
                generation,
                message,
            } if generation == self.preview_generation => {
                self.preview_state = PickerPreviewState::Error(message);
            }
            _ => {}
        }
    }

    /// Applies a background syntax refresh to the currently highlighted preview.
    pub fn handle_preview_syntax_refresh(
        &mut self,
        _generation: u64,
        result: crate::ui::picker::preview::PreviewSyntaxRefreshResult,
    ) {
        let _ = self
            .preview_adapter
            .apply_syntax_refresh_result_for_key(result.key.as_str(), result.result);
    }

    /// Applies an in-progress background syntax snapshot to the currently highlighted preview.
    pub fn handle_preview_syntax_refresh_chunk(
        &mut self,
        _generation: u64,
        result: crate::ui::picker::preview::PreviewSyntaxRefreshResult,
    ) {
        let _ = self
            .preview_adapter
            .apply_syntax_refresh_chunk_for_key(result.key.as_str(), result.result);
    }

    /// Marks a background preview syntax refresh as failed.
    pub fn handle_preview_syntax_refresh_failed(&mut self, _generation: u64) {
        let Some(key) = self.preview_key.as_deref() else {
            return;
        };

        let _ = self
            .preview_adapter
            .clear_syntax_refresh_pending_for_key(key);
    }

    /// Applies a streamed search event to the picker state.
    pub fn handle_search_event(&mut self, event: PickerSearchEvent<S::Item>) {
        match event {
            PickerSearchEvent::PickerSearchStarted { generation, .. }
                if generation == self.generation =>
            {
                self.search_active = true;
                self.search_complete = false;
            }
            PickerSearchEvent::PickerChunk { generation, chunk }
                if generation == self.generation =>
            {
                if !chunk.is_empty() {
                    if self.pending_result_replacement {
                        self.results.clear();
                        self.highlighted = None;
                        self.visible_start = 0;
                        self.preview_highlighted = None;
                        self.pending_result_replacement = false;
                    }
                    self.results.extend(chunk);
                    self.ensure_highlight_visible();
                    self.refresh_preview_for_highlight();
                }
            }
            PickerSearchEvent::PickerResults {
                generation,
                results,
            } if generation == self.generation => {
                let previous_key = self.selected_result_key();
                let previous_index = self.highlighted;
                self.pending_result_replacement = false;
                self.results = results;
                self.restore_highlight_after_replace(previous_key, previous_index);
            }
            PickerSearchEvent::PickerSearchComplete { generation }
                if generation == self.generation =>
            {
                self.search_active = false;
                self.search_complete = true;
                if self.pending_result_replacement {
                    self.results.clear();
                    self.pending_result_replacement = false;
                    self.highlighted = None;
                    self.visible_start = 0;
                    self.preview_highlighted = None;
                } else if self.results.is_empty() {
                    self.highlighted = None;
                    self.visible_start = 0;
                }
                self.refresh_preview_for_highlight();
            }
            PickerSearchEvent::PickerSearchStale { .. } => {}
            _ => {}
        }

        self.sync_query_right_prompt();
    }

    fn submit_selection(&mut self) -> UiEventResult {
        let index = self.highlighted.unwrap_or(0);

        let Some(item) = self.results.get(index).cloned() else {
            return UiEventResult::Handled(Vec::new());
        };

        self.close();
        UiEventResult::Handled(vec![self.source.select(&item)])
    }
}

impl<S: PickerSource> Widget for PickerWidget<S> {
    fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        if !self.open {
            return UiEventResult::NotHandled;
        }

        match event {
            UiEvent::Tick => {
                let handled_search = self.drain_search_events();
                self.refresh_preview_for_highlight();
                let handled_preview = self.drain_preview_events();
                if handled_search || handled_preview {
                    UiEventResult::Handled(Vec::new())
                } else {
                    UiEventResult::NotHandled
                }
            }
            UiEvent::Key(key) => {
                match key.code {
                    KeyCode::Tab if !key.modifiers.has_shift() => {
                        let Some(mode) = self.source.toggle_query_mode() else {
                            return UiEventResult::NotHandled;
                        };

                        if let Some(prompt) = self.source.query_prompt_segments_for_mode(mode) {
                            self.set_query_prompt_segments(prompt);
                        }
                        self.restart_search();
                    }
                    KeyCode::PageUp => {
                        self.scroll_current_preview(true);
                    }
                    KeyCode::PageDown => {
                        self.scroll_current_preview(false);
                    }
                    KeyCode::Esc => {
                        self.close();
                    }
                    KeyCode::Enter => return self.submit_selection(),
                    KeyCode::Up | KeyCode::Char('p') if key.modifiers.has_ctrl() => {
                        self.move_highlight(-1);
                    }
                    KeyCode::Down | KeyCode::Char('n') if key.modifiers.has_ctrl() => {
                        self.move_highlight(1);
                    }
                    KeyCode::Char('y') if key.modifiers.has_ctrl() => {
                        return self.submit_selection();
                    }
                    KeyCode::Char('c') if key.modifiers.has_ctrl() => {
                        self.close();
                    }
                    KeyCode::Char('s') if key.modifiers.has_ctrl() => {
                        let Some(item) = self.highlighted.and_then(|index| self.results.get(index))
                        else {
                            return UiEventResult::Handled(Vec::new());
                        };
                        let Some(intent) = self.source.stage_intent(item) else {
                            return UiEventResult::Handled(Vec::new());
                        };
                        return UiEventResult::Handled(vec![intent]);
                    }
                    KeyCode::Char('d') if key.modifiers.has_ctrl() => {
                        let Some(item) = self.highlighted.and_then(|index| self.results.get(index))
                        else {
                            return UiEventResult::Handled(Vec::new());
                        };
                        let Some(intent) = self.source.discard_intent(item) else {
                            return UiEventResult::Handled(Vec::new());
                        };
                        return UiEventResult::Handled(vec![intent]);
                    }
                    _ => {
                        let before = self.query_input.text().to_string();
                        if !self.query_input.handle_key(*key) {
                            return UiEventResult::NotHandled;
                        }
                        if self.query_input.text() != before {
                            self.restart_search();
                        }
                    }
                }

                UiEventResult::Handled(Vec::new())
            }
            UiEvent::Paste(text) => {
                if !text.is_empty() {
                    self.query_input.insert_str(text);
                    self.restart_search();
                }
                UiEventResult::Handled(Vec::new())
            }
            UiEvent::Resize(_, _) => UiEventResult::Handled(Vec::new()),
        }
    }

    fn layout(&mut self, constraints: crate::ui::UiConstraints) -> crate::ui::geometry::Size {
        constraints.available
    }

    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        if !self.open {
            return;
        }

        let _ = self.drain_search_events();
        let _ = self.drain_preview_events();
        self.sync_query_right_prompt();
        let Some(layout) = self.resolve_layout(rect) else {
            self.cursor = None;
            return;
        };
        let frame = layout.picker;

        let border_style = theme_style("ui.window.lines.border");
        let body_style = theme_style("ui.window");
        let active_style = theme_style("ui.window.active_line");
        let label = self.label.as_deref().map(OverlayFrameLabel::top_center);
        frame.render_bordered_with_label(screen, border_style, body_style, label);

        self.query_input.set_text_style(body_style);
        {
            let input = &mut self.query_input;
            input.render_widget(
                screen,
                UiRect::new(frame.content_origin, frame.content_size),
                _ctx,
            );
        }
        let Some(cursor) = self.query_input.render_cursor() else {
            self.cursor = None;
            return;
        };

        let separator_row = frame.content_origin.row + PROMPT_ROWS;
        frame.render_separator(screen, separator_row, border_style);

        let results_start_row = separator_row + SEPARATOR_ROWS;
        let page_size = self.page_size(frame.size.rows);
        let visible_results = self.visible_results(page_size).to_vec();
        let start_index = self.visible_start;
        self.cursor = Some(cursor);

        if visible_results.is_empty() {
            if let Some(status) = self.status_line() {
                screen.write_string(
                    results_start_row,
                    frame.content_origin.col,
                    body_style,
                    status.as_str(),
                );
            }
            return;
        }

        let selected_prefix = selection_prefix();

        for (offset, item) in visible_results.iter().enumerate() {
            let index = start_index + offset;
            let row = results_start_row + offset as u16;
            let prefix = if Some(index) == self.highlighted {
                selected_prefix.as_str()
            } else {
                "  "
            };
            let style = if Some(index) == self.highlighted {
                active_style
            } else {
                body_style
            };
            let segments = render_result_line(
                prefix,
                item,
                usize::from(frame.content_size.cols),
                style,
                true,
            );
            let mut col = frame.content_origin.col;
            for segment in segments {
                screen.write_string(row, col, segment.style, segment.text.as_str());
                col = col.saturating_add(UnicodeWidthStr::width(segment.text.as_str()) as u16);
            }
        }

        if let Some(preview) = layout.preview {
            render_preview_frame(
                screen,
                preview,
                &self.preview_state,
                &mut self.preview_adapter,
                border_style,
                body_style,
            );
        }
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Focusable
    }
}

fn theme_style(name: &str) -> Style {
    crate::globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default(name))
            .unwrap_or_default()
    })
}

pub fn picker_indicator_glyph() -> &'static str {
    crate::icon::selection_indicator()
}

fn picker_indicator_glyph_backwards() -> &'static str {
    crate::icon::backward_selection_indicator()
}

fn selection_prefix() -> String {
    crate::icon::selection_prefix()
}

fn render_result_line<T: PickerItem>(
    prefix: &str,
    item: &T,
    max_cols: usize,
    style: Style,
    pad: bool,
) -> Vec<PickerRenderSegment> {
    let prefix_cols = UnicodeWidthStr::width(prefix);
    if max_cols <= prefix_cols {
        return vec![PickerRenderSegment::new(
            pad_to_width(visible_tail_text(prefix, max_cols, false).0, max_cols, pad),
            style,
        )];
    }

    let item_cols = max_cols - prefix_cols;
    let mut segments = vec![PickerRenderSegment::new(prefix, style)];
    segments.extend(item.render_segments(item_cols, style));

    if pad && item.pad_to_full_width() {
        let width = segment_width(segments.as_slice());
        if width < max_cols {
            segments.push(PickerRenderSegment::new(
                " ".repeat(max_cols - width),
                style,
            ));
        }
    }

    segments
}

fn pad_to_width(mut text: String, max_cols: usize, pad: bool) -> String {
    if !pad {
        return text;
    }

    let width = UnicodeWidthStr::width(text.as_str());
    if width < max_cols {
        text.push_str(" ".repeat(max_cols - width).as_str());
    }

    text
}

fn segment_width(segments: &[PickerRenderSegment]) -> usize {
    segments
        .iter()
        .map(|segment| UnicodeWidthStr::width(segment.text.as_str()))
        .sum()
}

fn render_preview_frame(
    screen: &mut Screen,
    frame: OverlayFrame,
    state: &PickerPreviewState,
    preview_adapter: &mut PickerPreviewAdapter,
    border_style: Style,
    body_style: Style,
) {
    frame.render_bordered(screen, border_style, body_style);

    let PickerPreviewState::Ready(preview) = state else {
        return;
    };

    let Some(pane) = preview_adapter.preview_pane_mut(preview.title.as_str()) else {
        return;
    };

    pane.render(
        screen,
        frame.content_origin,
        frame.content_size,
        preview
            .highlighted_line
            .unwrap_or(preview.start_line)
            .saturating_sub(1),
        preview.highlighted_line.is_some(),
    );
}

/// Returns the visible head of text within a column budget.
pub fn visible_head_text(text: &str, max_cols: usize, ellipsize: bool) -> (String, u16) {
    let clipped = if ellipsize {
        ellipsize_text(text, max_cols, EllipsisSide::End)
    } else {
        clip_text(text, max_cols, ClipSide::Start)
    };
    (clipped.text, clipped.end_byte as u16)
}

/// Returns the visible tail of text within a column budget.
pub fn visible_tail_text(text: &str, max_cols: usize, ellipsize: bool) -> (String, u16) {
    let clipped = if ellipsize {
        ellipsize_text(text, max_cols, EllipsisSide::Start)
    } else {
        clip_text(text, max_cols, ClipSide::End)
    };
    (clipped.text, clipped.width as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::JobManager;
    use crate::config::{AdvancedGlyphCapability, Config};
    use crate::ui::inputs::PromptSegment;
    use crate::ui::picker::query::query_prompt_segments;
    use crate::ui::{Intent, UiContext, UiEvent};
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Clone)]
    struct TestSource {
        generation: Arc<Mutex<u64>>,
        selected: Arc<Mutex<Vec<String>>>,
    }

    #[derive(Clone)]
    struct ModeSource {
        generation: Arc<Mutex<u64>>,
        mode: Arc<Mutex<crate::ui::picker::query::PickerQueryMode>>,
    }

    #[derive(Clone)]
    struct SamePreviewKeySource {
        generation: Arc<Mutex<u64>>,
    }

    impl TestSource {
        fn new() -> Self {
            Self {
                generation: Arc::new(Mutex::new(0)),
                selected: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl ModeSource {
        fn new() -> Self {
            Self {
                generation: Arc::new(Mutex::new(0)),
                mode: Arc::new(Mutex::new(crate::ui::picker::query::PickerQueryMode::Exact)),
            }
        }
    }

    impl SamePreviewKeySource {
        fn new() -> Self {
            Self {
                generation: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl PickerSource for TestSource {
        type Item = String;

        fn set_generation(&self, generation: u64) {
            *self.generation.lock().unwrap() = generation;
        }

        fn start_search(
            &self,
            query: &str,
            generation: u64,
            sender: Sender<PickerSearchEvent<Self::Item>>,
        ) {
            sender
                .send(PickerSearchEvent::PickerSearchStarted {
                    generation,
                    query: query.to_string(),
                })
                .ok();
            sender
                .send(PickerSearchEvent::PickerChunk {
                    generation,
                    chunk: vec![format!("{query}-one"), format!("{query}-two")],
                })
                .ok();
            sender
                .send(PickerSearchEvent::PickerSearchComplete { generation })
                .ok();
        }

        fn job_manager(&self) -> std::sync::Arc<JobManager> {
            std::sync::Arc::new(JobManager::new())
        }

        fn preview_key(&self, item: &Self::Item) -> Option<String> {
            Some(item.clone())
        }

        fn result_key(&self, item: &Self::Item) -> Option<String> {
            Some(item.clone())
        }

        fn select(&self, item: &Self::Item) -> Intent {
            self.selected.lock().unwrap().push(item.clone());
            Intent::Command(crate::ui::Command::Quit)
        }
    }

    impl PickerSource for ModeSource {
        type Item = String;

        fn set_generation(&self, generation: u64) {
            *self.generation.lock().unwrap() = generation;
        }

        fn start_search(
            &self,
            query: &str,
            generation: u64,
            sender: Sender<PickerSearchEvent<Self::Item>>,
        ) {
            sender
                .send(PickerSearchEvent::PickerSearchStarted {
                    generation,
                    query: query.to_string(),
                })
                .ok();
            sender
                .send(PickerSearchEvent::PickerChunk {
                    generation,
                    chunk: vec![format!("{query}-one")],
                })
                .ok();
            sender
                .send(PickerSearchEvent::PickerSearchComplete { generation })
                .ok();
        }

        fn job_manager(&self) -> std::sync::Arc<JobManager> {
            std::sync::Arc::new(JobManager::new())
        }

        fn toggle_query_mode(&self) -> Option<crate::ui::picker::query::PickerQueryMode> {
            let mut mode = self.mode.lock().unwrap();
            *mode = mode.toggled();
            Some(*mode)
        }

        fn query_prompt_segments_for_mode(
            &self,
            mode: crate::ui::picker::query::PickerQueryMode,
        ) -> Option<Vec<PromptSegment>> {
            Some(query_prompt_segments(mode))
        }

        fn preview_key(&self, item: &Self::Item) -> Option<String> {
            Some(item.clone())
        }

        fn result_key(&self, item: &Self::Item) -> Option<String> {
            Some(item.clone())
        }

        fn select(&self, _item: &Self::Item) -> Intent {
            Intent::Command(crate::ui::Command::Quit)
        }
    }

    impl PickerSource for SamePreviewKeySource {
        type Item = String;

        fn set_generation(&self, generation: u64) {
            *self.generation.lock().unwrap() = generation;
        }

        fn start_search(
            &self,
            _query: &str,
            _generation: u64,
            _sender: Sender<PickerSearchEvent<Self::Item>>,
        ) {
        }

        fn job_manager(&self) -> std::sync::Arc<JobManager> {
            std::sync::Arc::new(JobManager::new())
        }

        fn preview_key(&self, _item: &Self::Item) -> Option<String> {
            Some(String::from("same-file"))
        }

        fn result_key(&self, item: &Self::Item) -> Option<String> {
            Some(item.clone())
        }

        fn select(&self, _item: &Self::Item) -> Intent {
            Intent::Command(crate::ui::Command::Quit)
        }
    }

    #[test]
    fn picker_restarts_search_on_input() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        let mut ctx = UiContext;

        let handled = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::Char('a'))),
            &mut ctx,
        );
        assert!(handled.handled());
        assert_eq!(picker.query(), "a");

        let handled = picker.handle_ui_event(&UiEvent::Tick, &mut ctx);
        assert!(handled.handled());
        assert_eq!(
            picker.results(),
            &["a-one".to_string(), "a-two".to_string()]
        );
        assert_eq!(picker.highlighted_index(), None);
    }

    #[test]
    fn picker_toggles_query_mode_on_tab_when_supported() {
        let source = ModeSource::new();
        let mut picker = PickerWidget::new(source);
        let mut ctx = UiContext;

        picker.set_query_prompt_segments(query_prompt_segments(
            crate::ui::picker::query::PickerQueryMode::Exact,
        ));

        let handled = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::Tab)),
            &mut ctx,
        );

        assert!(handled.handled());
        assert_eq!(picker.query_input.prompt_segments()[0].text, "Fuzzy");
    }

    #[test]
    fn picker_refreshes_preview_when_same_file_has_new_selected_line() {
        let source = SamePreviewKeySource::new();
        let mut picker = PickerWidget::new(source);
        picker.results = vec!["first".to_string(), "second".to_string()];
        picker.highlighted = Some(1);
        picker.preview_key = Some(String::from("same-file"));
        picker.preview_highlighted = Some(0);
        picker.preview_state = PickerPreviewState::Ready(PickerPreview::new("same-file", 1, None));

        picker.refresh_preview_for_highlight();

        assert_eq!(picker.preview_highlighted, Some(1));
    }

    #[test]
    fn picker_keeps_old_results_until_new_chunk_arrives() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source.clone());
        let mut ctx = UiContext;

        let _ = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::Char('a'))),
            &mut ctx,
        );
        let generation = *source.generation.lock().unwrap();
        picker.handle_search_event(PickerSearchEvent::PickerChunk {
            generation,
            chunk: vec!["a-one".to_string(), "a-two".to_string()],
        });
        assert_eq!(
            picker.results(),
            &["a-one".to_string(), "a-two".to_string()]
        );

        let _ = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::Char('b'))),
            &mut ctx,
        );

        assert_eq!(
            picker.results(),
            &["a-one".to_string(), "a-two".to_string()]
        );

        let generation = *source.generation.lock().unwrap();
        picker.handle_search_event(PickerSearchEvent::PickerChunk {
            generation,
            chunk: vec!["ab-one".to_string()],
        });

        assert_eq!(picker.results(), &["ab-one".to_string()]);
        assert_eq!(picker.highlighted_index(), None);
    }

    #[test]
    fn picker_preserves_selection_when_results_are_replaced() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        picker.results = vec!["low".to_string(), "mid".to_string(), "high".to_string()];
        picker.highlighted = Some(1);

        let generation = picker.generation;
        picker.handle_search_event(PickerSearchEvent::PickerResults {
            generation,
            results: vec!["high".to_string(), "mid".to_string(), "low".to_string()],
        });

        assert_eq!(picker.highlighted_index(), Some(1));
        assert_eq!(
            picker.results(),
            &["high".to_string(), "mid".to_string(), "low".to_string()]
        );
    }

    #[test]
    fn picker_clears_old_results_when_new_search_has_no_matches() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source.clone());
        let mut ctx = UiContext;

        let _ = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::Char('a'))),
            &mut ctx,
        );
        let generation = *source.generation.lock().unwrap();
        picker.handle_search_event(PickerSearchEvent::PickerChunk {
            generation,
            chunk: vec!["a-one".to_string(), "a-two".to_string()],
        });
        assert!(!picker.results().is_empty());

        let _ = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::Char('b'))),
            &mut ctx,
        );
        assert!(!picker.results().is_empty());

        let generation = *source.generation.lock().unwrap();
        picker.handle_search_event(PickerSearchEvent::PickerSearchStarted {
            generation,
            query: "ab".to_string(),
        });
        picker.handle_search_event(PickerSearchEvent::PickerSearchComplete { generation });

        assert!(picker.results().is_empty());
        assert_eq!(picker.highlighted_index(), None);
    }

    #[test]
    fn picker_selects_highlighted_result() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        let mut ctx = UiContext;

        picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::Char('a'))),
            &mut ctx,
        );
        let _ = picker.handle_ui_event(&UiEvent::Tick, &mut ctx);

        // Move highlight to first result before selecting
        picker.move_highlight(1);

        let result = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::Enter)),
            &mut ctx,
        );
        assert!(result.handled());
        assert!(!picker.is_open());
    }

    #[test]
    fn picker_selects_first_result_when_none_is_highlighted() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source.clone());
        let mut ctx = UiContext;

        picker.results = vec!["first".to_string(), "second".to_string()];
        picker.highlighted = None;

        let result = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::Enter)),
            &mut ctx,
        );

        assert!(result.handled());
        assert!(!picker.is_open());
        assert_eq!(
            source.selected.lock().unwrap().as_slice(),
            &["first".to_string()]
        );
    }

    #[test]
    fn picker_wraps_highlight_when_moving_above_first_item() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        picker.results = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        picker.highlighted = Some(0);

        picker.move_highlight(-1);

        assert_eq!(picker.highlighted_index(), Some(2));
    }

    #[test]
    fn picker_wraps_highlight_when_moving_below_last_item() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        picker.results = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        picker.highlighted = Some(2);

        picker.move_highlight(1);

        assert_eq!(picker.highlighted_index(), Some(0));
    }

    #[test]
    fn picker_page_keys_scroll_the_visible_preview_without_changing_selection() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        let key = String::from("/tmp/example.txt");
        picker.results = vec![key.clone()];
        picker.highlighted = Some(0);
        picker.preview_key = Some(key.clone());
        picker.preview_highlighted = Some(0);
        picker.preview_state = PickerPreviewState::Ready(PickerPreview::new(key.clone(), 1, None));
        picker.preview_adapter.insert(
            key.clone(),
            crate::ui::picker::preview::PreviewPane::new(crate::buffer::Buffer::from_str(
                "one\ntwo\nthree\nfour\nfive\n",
            )),
        );
        picker
            .preview_adapter
            .preview_pane_mut(key.as_str())
            .unwrap()
            .render(
                &mut crate::screen::Screen::new(2, 20),
                Position::new(0, 0),
                Size::new(2, 20),
                0,
                false,
            );
        let mut ctx = UiContext;

        let result = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::PageDown)),
            &mut ctx,
        );
        assert!(result.handled());
        assert_eq!(picker.highlighted_index(), Some(0));
        assert_eq!(
            picker
                .preview_adapter
                .preview_pane_mut(key.as_str())
                .unwrap()
                .buffer_view()
                .scroll_offset()
                .row,
            2
        );

        let _ = picker.handle_ui_event(
            &UiEvent::Key(urvim_terminal::Key::new(KeyCode::PageUp)),
            &mut ctx,
        );
        assert_eq!(
            picker
                .preview_adapter
                .preview_pane_mut(key.as_str())
                .unwrap()
                .buffer_view()
                .scroll_offset()
                .row,
            0
        );
    }

    #[test]
    fn picker_resets_preview_follow_when_switching_items() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        let old_key = String::from("/tmp/old.txt");
        let new_key = String::from("/tmp/new.txt");
        picker.results = vec![old_key.clone(), new_key.clone()];
        picker.highlighted = Some(1);
        picker.preview_key = Some(old_key.clone());
        picker.preview_highlighted = Some(0);
        picker.preview_state =
            PickerPreviewState::Ready(PickerPreview::new(old_key.clone(), 1, None));
        picker.preview_adapter.insert(
            new_key.clone(),
            crate::ui::picker::preview::PreviewPane::new(crate::buffer::Buffer::from_str(
                "one\ntwo\nthree\nfour\n",
            )),
        );
        picker
            .preview_adapter
            .preview_pane_mut(new_key.as_str())
            .unwrap()
            .set_follow_highlight(false);

        picker.refresh_preview_for_highlight();

        assert!(
            picker
                .preview_adapter
                .preview_pane_mut(new_key.as_str())
                .unwrap()
                .follows_highlight()
        );
        assert_eq!(picker.preview_highlighted, Some(1));
        assert!(matches!(picker.preview_state, PickerPreviewState::Loading));
    }

    #[test]
    fn picker_clears_pending_preview_syntax_refresh_on_failure() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        let key = String::from("/tmp/example.txt");
        picker.results = vec![key.clone()];
        picker.highlighted = Some(0);
        picker.preview_key = Some(key.clone());
        picker.preview_highlighted = Some(0);
        picker.preview_state = PickerPreviewState::Ready(PickerPreview::new(key.clone(), 1, None));
        picker.preview_adapter.insert(
            key.clone(),
            crate::ui::picker::preview::PreviewPane::new(crate::buffer::Buffer::from_str(
                "one\ntwo\nthree\n",
            )),
        );
        picker
            .preview_adapter
            .preview_pane_mut(key.as_str())
            .unwrap()
            .request_syntax_refresh();

        picker.handle_preview_syntax_refresh_failed(picker.preview_generation);

        assert!(
            !picker
                .preview_adapter
                .preview_pane_mut(key.as_str())
                .unwrap()
                .syntax_refresh_pending()
        );
    }

    #[test]
    fn picker_applies_preview_syntax_refresh_for_current_preview_key() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        let key = String::from("/tmp/example.txt");
        picker.results = vec![key.clone()];
        picker.highlighted = Some(0);
        picker.preview_key = Some(key.clone());
        picker.preview_highlighted = Some(0);
        picker.preview_state = PickerPreviewState::Ready(PickerPreview::new(key.clone(), 1, None));
        picker.preview_adapter.insert(
            key.clone(),
            crate::ui::picker::preview::PreviewPane::new(crate::buffer::Buffer::from_str(
                "one\ntwo\nthree\n",
            )),
        );
        picker
            .preview_adapter
            .preview_pane_mut(key.as_str())
            .unwrap()
            .request_syntax_refresh();

        let result = crate::buffer::BufferCacheRefreshResult {
            buffer_id: crate::buffer::BufferId::new(0),
            generation: 999,
            cache: crate::buffer::BufferCache::new("rust"),
        };

        picker.handle_preview_syntax_refresh(
            1,
            crate::ui::picker::preview::PreviewSyntaxRefreshResult {
                key: key.clone(),
                result,
            },
        );

        assert!(
            picker
                .preview_adapter
                .preview_pane_mut(key.as_str())
                .unwrap()
                .syntax_refresh_pending()
        );
    }

    #[test]
    fn picker_uses_nerdfont_selection_prefix_when_enabled() {
        let _config_guard = crate::globals::set_test_config(Config {
            advanced_glyphs: std::collections::BTreeSet::from([AdvancedGlyphCapability::Nerdfont]),
            ..Config::default()
        });

        assert_eq!(picker_indicator_glyph(), "");
        assert_eq!(selection_prefix(), " ");
    }

    #[test]
    fn picker_uses_ascii_selection_prefix_when_nerdfont_is_disabled() {
        let _config_guard = crate::globals::set_test_config(Config::default());

        assert_eq!(picker_indicator_glyph(), ">");
        assert_eq!(selection_prefix(), "> ");
    }

    #[test]
    fn picker_query_uses_prompt_prefix() {
        let source = TestSource::new();
        let picker = PickerWidget::new(source);

        assert_eq!(picker.query_input.prompt(), ">");
        assert_eq!(picker.query(), "");
    }

    #[test]
    fn picker_count_prompt_is_right_aligned_and_one_based() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        picker.set_query_prompt_segments(vec![
            PromptSegment::new("Exact", Style::new().bold()),
            PromptSegment::new(" > ", Style::new().faint()),
        ]);
        picker.results = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        picker.highlighted = Some(1);
        picker.sync_query_right_prompt();

        let right_prompt = picker.query_input.right_prompt_segments();
        assert_eq!(right_prompt.len(), 2);
        assert_eq!(right_prompt[0].text, " < ");
        assert_eq!(right_prompt[0].style, Style::new().faint());
        assert_eq!(right_prompt[1].text, "2/3");
        assert_eq!(right_prompt[1].style, Style::new().bold());

        let rect = UiRect::new(Position::new(0, 0), crate::ui::geometry::Size::new(12, 40));
        let layout = picker.resolve_layout(rect).expect("picker layout");
        let mut screen = crate::screen::Screen::new(12, 40);
        picker.render_widget(&mut screen, rect, &UiContext);

        let prompt_row = row_text(
            &mut screen,
            layout.picker.content_origin.row,
            layout.picker.content_origin.col,
        );
        assert!(prompt_row.contains("< 2/3"));
    }

    #[test]
    fn picker_hides_count_prompt_when_no_results_exist() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        picker.set_query_prompt_segments(vec![
            PromptSegment::new("Exact", Style::new().bold()),
            PromptSegment::new(" > ", Style::new().faint()),
        ]);
        picker.highlighted = None;
        picker.results.clear();
        picker.sync_query_right_prompt();

        assert!(picker.query_input.right_prompt_segments().is_empty());
    }

    #[test]
    fn picker_uses_fixed_content_width_when_space_allows() {
        let source = TestSource::new();
        let picker = PickerWidget::new(source);
        let rect = UiRect::new(Position::new(0, 0), crate::ui::geometry::Size::new(20, 120));

        let frame = picker.resolve_frame(rect).expect("picker frame");

        assert_eq!(frame.content_size.cols, PICKER_CONTENT_COLS);
    }

    #[test]
    fn picker_uses_top_center_anchor() {
        let source = TestSource::new();
        let picker = PickerWidget::new(source);
        let rect = UiRect::new(Position::new(0, 0), crate::ui::geometry::Size::new(30, 120));

        let frame = picker.resolve_frame(rect).expect("picker frame");

        assert_eq!(frame.origin.row, PICKER_TOP_MARGIN);
        assert_eq!(frame.origin.col, 19);
    }

    #[test]
    fn picker_content_width_clamps_to_available_space() {
        let source = TestSource::new();
        let picker = PickerWidget::new(source);
        let rect = UiRect::new(Position::new(0, 0), crate::ui::geometry::Size::new(20, 40));

        let frame = picker.resolve_frame(rect).expect("picker frame");

        assert_eq!(frame.content_size.cols, 38);
    }

    #[test]
    fn picker_preview_attaches_to_right_when_wide() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        picker.results = vec!["src/main.rs".to_string()];
        picker.highlighted = Some(0);
        picker.preview_key = Some("src/main.rs".to_string());
        let rect = UiRect::new(Position::new(0, 0), crate::ui::geometry::Size::new(30, 180));

        let layout = picker.resolve_layout(rect).expect("picker layout");
        let preview = layout.preview.expect("preview frame");

        assert_eq!(preview.origin.row, layout.picker.origin.row);
        assert_eq!(
            preview.origin.col,
            layout.picker.origin.col + layout.picker.size.cols
        );
        assert_eq!(preview.content_size.rows, PREVIEW_PREFERRED_CONTENT_ROWS);
    }

    #[test]
    fn picker_preview_clamps_preferred_height_to_screen_space() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        picker.results = vec!["src/main.rs".to_string()];
        picker.highlighted = Some(0);
        picker.preview_key = Some("src/main.rs".to_string());
        let rect = UiRect::new(Position::new(0, 0), crate::ui::geometry::Size::new(18, 180));

        let layout = picker.resolve_layout(rect).expect("picker layout");
        let preview = layout.preview.expect("preview frame");

        assert_eq!(preview.size.rows, rect.size.rows - preview.origin.row);
    }

    #[test]
    fn picker_preview_attaches_to_bottom_when_slim() {
        let source = TestSource::new();
        let mut picker = PickerWidget::new(source);
        picker.results = vec!["src/main.rs".to_string()];
        picker.highlighted = Some(0);
        picker.preview_key = Some("src/main.rs".to_string());
        let rect = UiRect::new(Position::new(0, 0), crate::ui::geometry::Size::new(30, 90));

        let layout = picker.resolve_layout(rect).expect("picker layout");
        let preview = layout.preview.expect("preview frame");

        assert_eq!(
            preview.origin.row,
            layout.picker.origin.row + layout.picker.size.rows
        );
        assert_eq!(preview.origin.col, layout.picker.origin.col);
    }

    #[test]
    fn result_line_preserves_prefix_and_ellipsizes_label_tail() {
        let line = render_result_line(
            "> ",
            &"src/deeply/nested/path/filename.rs".to_string(),
            16,
            Style::default(),
            false,
        );

        assert_eq!(segment_text(line.as_slice()), "> …h/filename.rs");
    }

    #[test]
    fn result_line_keeps_short_label_unchanged() {
        let line = render_result_line(
            "  ",
            &"src/main.rs".to_string(),
            16,
            Style::default(),
            false,
        );

        assert_eq!(segment_text(line.as_slice()), "  src/main.rs");
    }

    #[test]
    fn result_line_pads_to_full_width() {
        let line = render_result_line("> ", &"main.rs".to_string(), 12, Style::default(), true);

        assert_eq!(segment_text(line.as_slice()), "> main.rs   ");
    }

    #[test]
    fn preview_render_matches_editor_tab_body_layout() {
        let temp_root = unique_temp_dir();
        std::fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("preview.rs");
        std::fs::write(&file_path, "alpha\nbeta\n").unwrap();

        let mut adapter = crate::ui::picker::preview::PickerPreviewAdapter::new();
        let pane = adapter.preview_for_path(file_path.as_path()).unwrap();
        let mut preview_screen = crate::screen::Screen::new(5, 20);
        pane.render(
            &mut preview_screen,
            Position::new(1, 1),
            Size::new(3, 18),
            0,
            false,
        );

        let mut tab = crate::editor_tab::EditorTab::new(crate::buffer::Buffer::from_str_with_path(
            "alpha\nbeta\n",
            crate::path::AbsolutePath::from_path(file_path.as_path()).unwrap(),
        ));
        let mut tab_screen = crate::screen::Screen::new(3, 18);
        tab.set_cursor(crate::buffer::Cursor::new(0, 0));
        tab.render(&mut tab_screen, Position::new(1, 1), Size::new(3, 18));

        assert_eq!(
            row_text(&mut preview_screen, 1, 0).trim_start().trim_end(),
            row_text(&mut tab_screen, 1, 0).trim_start().trim_end()
        );
        assert_eq!(
            row_text(&mut preview_screen, 2, 0).trim_start().trim_end(),
            row_text(&mut tab_screen, 2, 0).trim_start().trim_end()
        );

        std::fs::remove_file(file_path).ok();
        std::fs::remove_dir_all(temp_root).ok();
    }

    #[test]
    fn preview_render_is_separate_from_widget_focus() {
        let preview = PickerPreview::new("/tmp/example.txt", 1, None);
        let mut adapter = crate::ui::picker::preview::PickerPreviewAdapter::new();
        adapter.insert(
            preview.title.clone(),
            crate::ui::picker::preview::PreviewPane::new(crate::buffer::Buffer::from_str(
                "hello\nworld\n",
            )),
        );
        let frame = crate::ui::overlay::frame::OverlayFrame::resolve_placement(
            Position::new(0, 0),
            Size::new(4, 16),
            2,
            14,
            crate::ui::overlay::frame::OverlayPlacement::Anchored {
                anchor: crate::ui::overlay::frame::OverlayAnchor::Center,
                margins: crate::ui::overlay::frame::OverlayMargins::default(),
            },
        )
        .expect("preview frame");
        let mut screen = crate::screen::Screen::new(4, 16);

        let preview_pane = adapter
            .preview_for_path(std::path::Path::new(preview.title.as_str()))
            .unwrap();
        preview_pane.render(
            &mut screen,
            Position::new(frame.content_origin.row + 1, frame.content_origin.col),
            Size::new(
                frame.content_size.rows.saturating_sub(1),
                frame.content_size.cols,
            ),
            0,
            false,
        );

        assert!(!row_text(&mut screen, 2, 1).trim().is_empty());
    }

    fn row_text(screen: &mut crate::screen::Screen, row: u16, start_col: u16) -> String {
        let (_, cols) = screen.size();
        (start_col..cols)
            .map(|col| screen.get_cell_mut(row, col).unwrap().text.clone())
            .collect()
    }

    fn segment_text(segments: &[PickerRenderSegment]) -> String {
        segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }

    fn unique_temp_dir() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("urvim-picker-tests-{nanos}"))
    }
}
