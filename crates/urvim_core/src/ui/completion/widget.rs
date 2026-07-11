//! Completion popup widget.

use super::render::{
    completion_item_display_width, completion_row_segments, completion_selection_prefix,
    render_segments, theme_style,
};
use super::{CompletionCandidate, CompletionSourceKind, configured_completion_sources};
use crate::background::{JobManager, JobToken};
use crate::buffer::{BufferId, Cursor};
use crate::screen::Screen;
use crate::ui::floating_window::{FloatingPlacement, FloatingWindowFrame};
use crate::ui::{Command, FocusPolicy, Intent, UiContext, UiEvent, UiEventResult, UiRect};
use crate::widget::Widget;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use urvim_terminal::KeyCode;

const MAX_VISIBLE_ITEMS: usize = 8;
const MIN_POPUP_CONTENT_WIDTH: usize = 20;
const MIN_POPUP_CONTENT_ROWS: u16 = 3;

/// Simple source-driven completion widget.
#[derive(Debug)]
pub struct CompletionWidget {
    items: Vec<CompletionCandidate>,
    highlighted: usize,
    visible_start: usize,
    cursor: Option<crate::window::Position>,
    open: bool,
    generation: u64,
    source_order: Vec<CompletionSourceKind>,
    pending_sources: BTreeSet<CompletionSourceKind>,
    source_results: BTreeMap<CompletionSourceKind, Vec<CompletionCandidate>>,
    jobs: Arc<JobManager>,
}

impl CompletionWidget {
    /// Creates a new completion widget.
    pub fn new(jobs: Arc<JobManager>) -> Self {
        Self {
            items: Vec::new(),
            highlighted: 0,
            visible_start: 0,
            cursor: None,
            open: false,
            generation: 0,
            source_order: Vec::new(),
            pending_sources: BTreeSet::new(),
            source_results: BTreeMap::new(),
            jobs,
        }
    }

    /// Returns true when the popup is open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns the current candidate list.
    pub fn results(&self) -> &[CompletionCandidate] {
        self.items.as_slice()
    }

    /// Returns the highlighted index.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.items.get(self.highlighted).map(|_| self.highlighted)
    }

    /// Returns true when a completion request is waiting on the background job.
    pub fn is_pending(&self) -> bool {
        !self.pending_sources.is_empty()
    }

    /// Returns the rendered cursor position, if available.
    pub fn cursor(&self) -> Option<crate::window::Position> {
        self.cursor
    }

    /// Sets the popup anchor cursor.
    pub fn set_cursor(&mut self, cursor: Option<crate::window::Position>) {
        self.cursor = cursor;
    }

    /// Closes the popup.
    pub fn close(&mut self) {
        self.open = false;
        self.pending_sources.clear();
        self.source_results.clear();
        self.source_order.clear();
    }

    /// Starts a background completion request for the current buffer cursor.
    pub fn request_completion(&mut self, buffer_id: BufferId, cursor: Cursor) {
        let source_order = configured_completion_sources();
        self.generation = self.generation.saturating_add(1);
        self.open = false;
        self.source_order = source_order.clone();
        self.items.clear();
        self.highlighted = 0;
        self.visible_start = 0;
        self.pending_sources.clear();
        self.source_results.clear();

        if source_order.is_empty() {
            return;
        }

        let token = JobToken::new(self.generation);
        for source in source_order {
            self.pending_sources.insert(source);

            if self
                .jobs
                .submit_latest_only(
                    crate::background::JobKind::Completion(buffer_id, source),
                    token,
                    super::job::CompletionJob::new(buffer_id, cursor, source),
                )
                .is_err()
            {
                self.pending_sources.remove(&source);
            }
        }

        if self.pending_sources.is_empty() {
            self.items.clear();
        }
    }

    /// Handles a UI event while the popup is open.
    pub fn handle_ui_event(&mut self, event: &UiEvent, _ctx: &mut UiContext) -> UiEventResult {
        match event {
            UiEvent::Key(key) => match key.code {
                KeyCode::Esc => {
                    self.cancel();
                    UiEventResult::NotHandled
                }
                KeyCode::Char('c') if key.modifiers.has_ctrl() => self.cancel(),
                _ if !self.open => UiEventResult::NotHandled,
                KeyCode::Char('n') if key.modifiers.has_ctrl() => {
                    self.move_highlight(1);
                    UiEventResult::Handled(Vec::new())
                }
                KeyCode::Char('p') if key.modifiers.has_ctrl() => {
                    self.move_highlight(-1);
                    UiEventResult::Handled(Vec::new())
                }
                KeyCode::Char('y') if key.modifiers.has_ctrl() => self.accept(),
                _ => UiEventResult::NotHandled,
            },
            UiEvent::Paste(_) | UiEvent::Resize(_, _) | UiEvent::Tick => UiEventResult::NotHandled,
        }
    }

    /// Renders the completion popup.
    pub fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, _ctx: &UiContext) {
        if !self.open || rect.size.rows < 3 || rect.size.cols < 3 || self.items.is_empty() {
            return;
        }

        let border_style = theme_style("ui.window.lines.border");
        let body_style = theme_style("ui.window");
        let selected_style = theme_style("ui.window.active_line");

        let content_rows = self.visible_rows();
        let content_cols = self.content_cols(rect.size.cols);
        let Some(frame) = FloatingWindowFrame::resolve_placement(
            rect.origin,
            rect.size,
            content_rows,
            content_cols,
            FloatingPlacement::NearCursor {
                cursor: self.cursor.unwrap_or_default(),
            },
        ) else {
            return;
        };

        frame.render_bordered_with_label(screen, border_style, body_style, None);

        let start = self.visible_start.min(self.items.len());
        let end = (start + usize::from(frame.content_size.rows)).min(self.items.len());
        for (row_offset, (index, item)) in
            (start..end).zip(self.items[start..end].iter()).enumerate()
        {
            let row = frame.content_origin.row + row_offset as u16;
            let style = if index == self.highlighted {
                selected_style
            } else {
                body_style
            };
            if index == self.highlighted {
                screen.fill_region(
                    row,
                    frame.content_origin.col,
                    1,
                    frame.content_size.cols,
                    selected_style,
                );
            }
            let prefix = if index == self.highlighted {
                completion_selection_prefix()
            } else {
                String::from("  ")
            };
            let segments = completion_row_segments(item, style, prefix, frame.content_size.cols);
            render_segments(screen, row, frame.content_origin.col, segments);
        }
    }

    fn accept(&mut self) -> UiEventResult {
        let Some(item) = self.items.get(self.highlighted).cloned() else {
            return UiEventResult::Handled(Vec::new());
        };

        self.open = false;
        UiEventResult::Handled(vec![Intent::Command(Command::ApplyCompletion(
            crate::ui::ApplyCompletion {
                range: item.range,
                text: item.replacement,
                additional_text_edits: item.additional_text_edits,
                lsp_completion_item: item.lsp_completion_item,
                format: item
                    .insert_format
                    .unwrap_or(crate::ui::completion::CompletionInsertFormat::PlainText),
            },
        ))])
    }

    /// Applies a completion result snapshot from the background worker.
    pub fn apply_results(
        &mut self,
        generation: u64,
        source: CompletionSourceKind,
        items: Vec<CompletionCandidate>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }

        if !self.pending_sources.remove(&source) {
            return false;
        }

        self.source_results.insert(source, items);
        if !self.pending_sources.is_empty() {
            return true;
        }

        let source_order = self.source_order.clone();
        self.items = source_order
            .into_iter()
            .filter_map(|source| self.source_results.remove(&source))
            .flatten()
            .collect();
        self.source_results.clear();

        if self.items.is_empty() {
            self.open = false;
            self.highlighted = 0;
            self.visible_start = 0;
            return true;
        }

        self.open = true;
        self.highlighted = self.highlighted.min(self.items.len() - 1);
        self.ensure_highlight_visible();
        true
    }

    fn cancel(&mut self) -> UiEventResult {
        self.open = false;
        self.pending_sources.clear();
        self.source_results.clear();
        UiEventResult::Handled(Vec::new())
    }

    fn move_highlight(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }

        let len = self.items.len() as isize;
        self.highlighted = (self.highlighted as isize + delta).rem_euclid(len) as usize;
        self.ensure_highlight_visible();
    }

    fn ensure_highlight_visible(&mut self) {
        let max_visible = MAX_VISIBLE_ITEMS.max(1);
        if self.highlighted < self.visible_start {
            self.visible_start = self.highlighted;
        } else if self.highlighted >= self.visible_start.saturating_add(max_visible) {
            self.visible_start = self.highlighted + 1 - max_visible;
        }
    }

    fn visible_rows(&self) -> u16 {
        self.items
            .len()
            .min(MAX_VISIBLE_ITEMS)
            .max(usize::from(MIN_POPUP_CONTENT_ROWS)) as u16
    }

    fn content_cols(&self, max_cols: u16) -> u16 {
        let widest = self
            .items
            .iter()
            .map(completion_item_display_width)
            .max()
            .unwrap_or(0);
        let width = widest
            .saturating_add(2)
            .max(MIN_POPUP_CONTENT_WIDTH)
            .min(usize::from(max_cols.saturating_sub(2)));
        width.max(1) as u16
    }
}

impl Widget for CompletionWidget {
    fn handle_ui_event(&mut self, event: &UiEvent, ctx: &mut UiContext) -> UiEventResult {
        CompletionWidget::handle_ui_event(self, event, ctx)
    }

    fn render_widget(&mut self, screen: &mut Screen, rect: UiRect, ctx: &UiContext) {
        CompletionWidget::render_widget(self, screen, rect, ctx)
    }

    fn focus_policy(&self) -> FocusPolicy {
        FocusPolicy::Focusable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::JobManager;
    use crate::buffer::{Cursor, TextObjectRange};
    use urvim_terminal::{Key, KeyCode, Modifiers};

    #[test]
    fn completion_widget_ignores_regular_editing_keys() {
        let mut widget = CompletionWidget::new(Arc::new(JobManager::new()));
        widget.open = true;
        widget.items = vec![CompletionCandidate::new(
            "alpha",
            "alpha",
            TextObjectRange {
                start: Cursor::new(0, 0),
                end: Cursor::new(0, 5),
            },
            None,
        )];
        let mut ctx = UiContext;

        assert_eq!(
            widget.handle_ui_event(&UiEvent::Key(Key::new(KeyCode::Char('a'))), &mut ctx),
            UiEventResult::NotHandled
        );
        assert_eq!(
            widget.handle_ui_event(&UiEvent::Key(Key::new(KeyCode::Backspace)), &mut ctx),
            UiEventResult::NotHandled
        );
        assert_eq!(
            widget.handle_ui_event(&UiEvent::Paste("x".to_string()), &mut ctx),
            UiEventResult::NotHandled
        );
        assert_eq!(
            widget.handle_ui_event(
                &UiEvent::Key(Key::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL)),
                &mut ctx
            ),
            UiEventResult::Handled(Vec::new())
        );
    }

    #[test]
    fn completion_widget_builds_closed() {
        let _widget = CompletionWidget::new(Arc::new(JobManager::new()));
        assert!(!_widget.is_open());
    }

    #[test]
    fn completion_widget_content_width_accounts_for_metadata_columns() {
        let mut widget = CompletionWidget::new(Arc::new(JobManager::new()));
        let item = CompletionCandidate {
            label: "alpha_with_a_very_long_completion_label_that_exceeds_the_old_cap".to_string(),
            replacement: "alpha_with_a_very_long_completion_label_that_exceeds_the_old_cap"
                .to_string(),
            range: TextObjectRange {
                start: Cursor::new(0, 0),
                end: Cursor::new(0, 65),
            },
            symbol: None,
            kind: Some(lsp_types::CompletionItemKind::FUNCTION),
            insert_format: None,
            detail: Some("fn()".to_string()),
            label_detail: Some("module".to_string()),
            label_description: Some("desc".to_string()),
            additional_text_edits: Vec::new(),
            lsp_completion_item: None,
            deprecated: false,
            preselect: false,
        };
        widget.items = vec![item.clone()];

        let expected = (completion_item_display_width(&item) + 2)
            .max(MIN_POPUP_CONTENT_WIDTH)
            .min(usize::from(200u16.saturating_sub(2))) as u16;
        assert!(completion_item_display_width(&item) > 64);
        assert_eq!(widget.content_cols(200), expected);
    }
}
