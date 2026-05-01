use super::*;
use crate::buffer::BufferId;
use crate::buffer::{configured_tab_width, display_grapheme_width, display_width_at};
use crate::config::ScrollMargin;
use crate::config::WrapMode;
use crate::theme::Tag;
use crate::window::wrap::WrappedLineSegment;
use imbl::Vector;
use smol_str::SmolStr;
use std::cell::RefCell;
use std::ops::Range;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub(super) enum BufferBacking {
    Pooled(BufferId),
    Owned(RefCell<Buffer>),
}

impl BufferView {
    /// Creates a new view and registers the buffer in the global pool.
    pub fn new(buffer: Buffer) -> Self {
        let buffer_id = crate::globals::with_buffer_pool(|pool| pool.register_buffer(buffer));
        Self::from_buffer_id(buffer_id)
    }

    /// Creates a view for an already-registered buffer ID.
    pub fn from_buffer_id(buffer_id: BufferId) -> Self {
        Self {
            buffer: BufferBacking::Pooled(buffer_id),
            scroll_offset: Position::new(0, 0),
            wrapped_row_offset: 0,
            cursor: Cursor::new(0, 0),
            remembered_visual_col: None,
            visual_selection: None,
            yank_flash: None,
        }
    }

    /// Creates a view backed by an owned buffer that is not stored in the global pool.
    pub fn from_owned_buffer(buffer: Buffer) -> Self {
        Self {
            buffer: BufferBacking::Owned(RefCell::new(buffer)),
            scroll_offset: Position::new(0, 0),
            wrapped_row_offset: 0,
            cursor: Cursor::new(0, 0),
            remembered_visual_col: None,
            visual_selection: None,
            yank_flash: None,
        }
    }

    /// Returns the buffer ID owned by this view, if it is pooled.
    pub fn buffer_id_opt(&self) -> Option<BufferId> {
        match &self.buffer {
            BufferBacking::Pooled(buffer_id) => Some(*buffer_id),
            BufferBacking::Owned(_) => None,
        }
    }

    /// Returns the buffer ID owned by this view.
    pub fn buffer_id(&self) -> BufferId {
        self.buffer_id_opt()
            .expect("owned preview buffers do not have a global buffer id")
    }

    /// Runs a closure with shared access to the shared buffer.
    pub fn with_buffer<R>(&self, f: impl FnOnce(&Buffer) -> R) -> Option<R> {
        match &self.buffer {
            BufferBacking::Pooled(buffer_id) => crate::globals::with_buffer(*buffer_id, f),
            BufferBacking::Owned(buffer) => Some(f(&buffer.borrow())),
        }
    }

    /// Runs a closure with mutable access to the shared buffer.
    pub fn with_buffer_mut<R>(&self, f: impl FnOnce(&mut Buffer) -> R) -> Option<R> {
        match &self.buffer {
            BufferBacking::Pooled(buffer_id) => crate::globals::with_buffer_mut(*buffer_id, f),
            BufferBacking::Owned(buffer) => Some(f(&mut buffer.borrow_mut())),
        }
    }

    /// Returns the number of lines in the shared buffer, or `0` if it no longer exists.
    pub fn line_count(&self) -> usize {
        self.with_buffer(|buffer| buffer.line_count()).unwrap_or(0)
    }

    /// Returns the length of a line in the shared buffer, or `0` if it no longer exists.
    pub fn line_len(&self, line: usize) -> usize {
        self.with_buffer(|buffer| buffer.line_len(line))
            .unwrap_or(0)
    }

    /// Returns the shared buffer's file name as display text, if available.
    pub fn file_name(&self) -> Option<String> {
        self.with_buffer(|buffer| {
            buffer
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .flatten()
    }

    /// Returns the shared buffer's resolved syntax name, if it still exists.
    pub fn syntax_name(&self) -> String {
        self.with_buffer(|buffer| buffer.syntax_name().to_string())
            .unwrap_or_else(|| crate::syntax::fallback_syntax_name().to_string())
    }

    /// Returns the shared buffer's syntax label for display purposes.
    pub fn syntax_label(&self) -> String {
        self.with_buffer(|buffer| buffer.syntax_label())
            .unwrap_or_else(|| {
                crate::syntax::builtin_syntax_registry()
                    .ok()
                    .and_then(|registry| {
                        registry.display_name(crate::syntax::fallback_syntax_name())
                    })
                    .map(|label| label.to_string())
                    .unwrap_or_else(|| "Plain Text".to_string())
            })
    }

    /// Returns true when the shared buffer differs from its last saved baseline.
    pub fn is_modified(&self) -> bool {
        self.with_buffer(|buffer| buffer.is_modified())
            .unwrap_or(false)
    }

    fn current_visual_col(&self) -> usize {
        self.with_buffer(|buffer| buffer.visual_col_at(self.cursor))
            .unwrap_or(0)
    }

    /// Returns the active indent guide as `(column, start_exclusive, end_exclusive)`.
    ///
    /// The returned line bounds are exclusive boundary lines, so callers render
    /// on `start_exclusive + 1..end_exclusive`.
    pub(super) fn active_indent_guide(&self) -> Option<(usize, usize, usize)> {
        self.with_buffer(|buffer| {
            if buffer.line_count() == 0 {
                return None;
            }

            let cursor_line = self.cursor.line.min(buffer.line_count().saturating_sub(1));
            let cursor_visual_col = buffer.visual_col_at(self.cursor);
            let scope_ids = buffer.cached_line_indent_scope_ids(cursor_line)?;
            let scopes = buffer.cached_indent_scopes();

            let active_scope = scope_ids
                .iter()
                .filter_map(|scope_id| scopes.get(*scope_id))
                .filter(|scope| scope.is_active() && scope.indent_width <= cursor_visual_col)
                .max_by_key(|scope| (scope.indent_width, scope.start_line))?;

            let start_exclusive = active_scope.start_line;
            let line_count = buffer.line_count();
            let eof_line = line_count.saturating_sub(1);
            let end_exclusive = match active_scope.end_line {
                Some(end_line)
                    if end_line == eof_line
                        && buffer
                            .line_at(eof_line)
                            .map(|line| {
                                leading_indent_width(line.as_ref()) != active_scope.indent_width
                            })
                            .unwrap_or(false) =>
                {
                    line_count
                }
                Some(end_line) => end_line,
                None => line_count,
            };
            if start_exclusive.saturating_add(1) >= end_exclusive {
                return None;
            }

            Some((active_scope.indent_width, start_exclusive, end_exclusive))
        })
        .flatten()
    }

    pub fn scroll_offset(&self) -> Position {
        self.scroll_offset
    }

    pub fn set_scroll_offset(&mut self, offset: Position) {
        self.scroll_offset = offset;
        self.wrapped_row_offset = offset.row;
    }

    pub fn wrapped_row_offset(&self) -> u16 {
        self.wrapped_row_offset
    }

    pub fn set_wrapped_row_offset(&mut self, offset: u16) {
        self.wrapped_row_offset = offset;
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
        self.log_cursor_indent_scopes(cursor);
    }

    /// Starts a new visual selection anchored at the current cursor.
    pub fn begin_visual_selection(&mut self, kind: VisualSelectionKind) {
        self.visual_selection = Some(VisualSelection {
            anchor: self.cursor,
            kind,
        });
    }

    /// Clears the active visual selection.
    pub fn clear_visual_selection(&mut self) {
        self.visual_selection = None;
    }

    /// Starts a transient yank flash anchored at the supplied selection.
    pub fn begin_yank_flash(&mut self, selection: YankFlashSelection, duration: Duration) {
        self.yank_flash = Some(YankFlash {
            selection,
            expires_at: Instant::now() + duration,
        });
    }

    /// Clears the active yank flash, if any.
    pub fn clear_yank_flash(&mut self) {
        self.yank_flash = None;
    }

    /// Returns the active yank flash, if any.
    pub fn yank_flash(&self) -> Option<YankFlash> {
        self.yank_flash
    }

    /// Clears the active yank flash once it expires.
    pub fn prune_yank_flash(&mut self, now: Instant) -> bool {
        let Some(flash) = self.yank_flash else {
            return false;
        };

        if now >= flash.expires_at {
            self.yank_flash = None;
            return true;
        }

        false
    }

    /// Returns the active visual selection record, if any.
    pub fn visual_selection(&self) -> Option<VisualSelection> {
        self.visual_selection
    }

    /// Replaces the active character-wise visual selection with the given range.
    pub fn set_visual_selection_range(&mut self, range: crate::buffer::TextObjectRange) -> bool {
        let Some(selection) = self.visual_selection else {
            return false;
        };
        if selection.kind != VisualSelectionKind::Character {
            return false;
        }
        if self
            .visual_selection_range()
            .is_some_and(|current| current == range)
        {
            return false;
        }

        self.visual_selection = Some(VisualSelection {
            anchor: range.start,
            kind: VisualSelectionKind::Character,
        });
        self.cursor = self
            .with_buffer(|buffer| buffer.prev_cursor(range.end))
            .flatten()
            .unwrap_or(range.start);
        self.remembered_visual_col = Some(self.current_visual_col());
        true
    }

    /// Sets the cursor from stored state after syncing it against the current buffer.
    pub fn set_cursor_synced(&mut self, cursor: Cursor) {
        let synced_cursor = self
            .with_buffer(|buffer| buffer.sync_cursor(cursor))
            .unwrap_or(cursor);
        self.cursor = synced_cursor;
        self.remembered_visual_col = Some(self.current_visual_col());
        self.log_cursor_indent_scopes(synced_cursor);
    }

    pub fn get_or_compute_target_col(&self) -> usize {
        if let Some(col) = self.remembered_visual_col {
            return col;
        }
        self.current_visual_col()
    }

    pub fn update_remembered_to_current(&mut self) {
        self.remembered_visual_col = Some(self.current_visual_col());
    }

    pub fn set_remembered_visual_col(&mut self, col: usize) {
        self.remembered_visual_col = Some(col);
    }

    fn log_cursor_indent_scopes(&self, cursor: Cursor) {
        if !tracing::enabled!(tracing::Level::DEBUG) {
            return;
        }

        let Some((stale, scope_dump)) = self.with_buffer_mut(|buffer| {
            let stale = buffer.indent_scope_cache_stale();
            let scope_dump = buffer
                .cached_line_indent_scope_ids(cursor.line)
                .map(|scope_ids| {
                    scope_ids
                        .iter()
                        .filter_map(|scope_id| buffer.cached_indent_scopes().get(*scope_id))
                        .filter(|scope| scope.is_active())
                        .map(|scope| {
                            let end_line = scope
                                .end_line
                                .map(|end| end.to_string())
                                .unwrap_or_else(|| String::from("open"));
                            format!(
                                "#{}:{}-{}@{}",
                                scope.id, scope.start_line, end_line, scope.indent_width
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (stale, scope_dump)
        }) else {
            tracing::debug!(
                buffer_id = self
                    .buffer_id_opt()
                    .map(|buffer_id| buffer_id.get())
                    .unwrap_or(usize::MAX),
                line = cursor.line,
                col = cursor.col,
                "cursor moved but buffer is unavailable for indent scope logging"
            );
            return;
        };

        tracing::debug!(
            buffer_id = self.buffer_id_opt().map(|buffer_id| buffer_id.get()).unwrap_or(usize::MAX),
            line = cursor.line,
            col = cursor.col,
            indent_scope_cache_stale = stale,
            indent_scopes = %scope_dump.join(", "),
            "cursor line indent scopes"
        );
    }

    /// Returns the active visual selection as a normalized range.
    pub fn visual_selection_range(&self) -> Option<crate::buffer::TextObjectRange> {
        let selection = self.visual_selection?;
        if selection.kind == VisualSelectionKind::Line {
            return None;
        }
        let anchor = selection.anchor;
        let cursor = self.cursor;
        self.with_buffer(|buffer| Self::visual_selection_range_for(buffer, anchor, cursor))
            .flatten()
    }

    /// Returns the active linewise visual selection as a line span.
    pub fn visual_line_selection_range(&self) -> Option<(usize, usize)> {
        let selection = self.visual_selection?;
        if selection.kind != VisualSelectionKind::Line {
            return None;
        }
        self.with_buffer(|buffer| {
            Self::visual_line_selection_range_for(buffer, selection.anchor, self.cursor)
        })
        .flatten()
    }

    pub fn scroll_to_cursor(&mut self, viewport_size: Size, gutter_width: u16) {
        self.scroll_to_cursor_with_wrap(viewport_size, gutter_width, false, WrapMode::Hard);
    }

    /// Scrolls the viewport to keep the cursor visible.
    ///
    /// In non-wrapped mode, vertical and horizontal scrolling are based on
    /// logical buffer lines and visual columns.
    ///
    /// In wrapped mode, horizontal scrolling is disabled and vertical scrolling
    /// is driven by wrapped visual rows.
    pub fn scroll_to_cursor_with_wrap(
        &mut self,
        viewport_size: Size,
        gutter_width: u16,
        wrap_enabled: bool,
        wrap_mode: WrapMode,
    ) {
        let cursor = self.cursor;
        let Some((buffer_line_count, cursor_visual_col, line_width)) = self.with_buffer(|buffer| {
            (
                buffer.line_count(),
                buffer.visual_col_at(cursor),
                buffer.visual_line_width(cursor.line),
            )
        }) else {
            self.scroll_offset = Position::new(0, 0);
            return;
        };

        if buffer_line_count == 0 {
            self.scroll_offset = Position::new(0, 0);
            return;
        }

        let visible_rows = viewport_size.rows as usize;
        let visible_cols = viewport_size.cols.saturating_sub(gutter_width) as usize;
        let (effective_vertical_margin, effective_horizontal_margin) =
            Self::effective_scroll_margins(visible_rows, visible_cols);
        self.scroll_offset.row = Self::offset_to_keep_target_visible(
            self.scroll_offset.row as usize,
            cursor.line,
            visible_rows,
            effective_vertical_margin,
            buffer_line_count,
        ) as u16;

        if wrap_enabled {
            self.scroll_to_cursor_wrapped(viewport_size, gutter_width, wrap_mode);
            return;
        }

        self.wrapped_row_offset = self.scroll_offset.row;
        self.scroll_offset.col = Self::offset_to_keep_target_visible(
            self.scroll_offset.col as usize,
            cursor_visual_col,
            visible_cols,
            effective_horizontal_margin,
            line_width,
        ) as u16;
    }

    fn effective_scroll_margins(visible_rows: usize, visible_cols: usize) -> (usize, usize) {
        let scroll_margin = crate::globals::with_config(|config| config.scroll_margin)
            .unwrap_or_else(ScrollMargin::default);
        let effective_vertical_margin = scroll_margin
            .vertical
            .min(visible_rows.saturating_sub(1) / 2);
        let effective_horizontal_margin = scroll_margin
            .horizontal
            .min(visible_cols.saturating_sub(1) / 2);
        (effective_vertical_margin, effective_horizontal_margin)
    }

    fn offset_to_keep_target_visible(
        current_offset: usize,
        target: usize,
        visible_extent: usize,
        margin: usize,
        total_extent: usize,
    ) -> usize {
        let min_visible = current_offset.saturating_add(margin);
        let max_visible = current_offset
            .saturating_add(visible_extent.saturating_sub(1))
            .saturating_sub(margin);
        let mut next_offset = current_offset;
        if target < min_visible {
            next_offset = target.saturating_sub(margin);
        } else if target > max_visible {
            next_offset = target
                .saturating_add(margin)
                .saturating_add(1)
                .saturating_sub(visible_extent);
        }

        let max_offset = total_extent.saturating_sub(visible_extent);
        next_offset.min(max_offset)
    }

    fn scroll_to_cursor_wrapped(
        &mut self,
        viewport_size: Size,
        gutter_width: u16,
        wrap_mode: WrapMode,
    ) {
        let visible_rows = viewport_size.rows as usize;
        let visible_cols = viewport_size.cols.saturating_sub(gutter_width) as usize;
        if visible_rows == 0 || visible_cols == 0 {
            self.wrapped_row_offset = 0;
            self.scroll_offset.col = 0;
            return;
        }

        let (effective_vertical_margin, _) =
            Self::effective_scroll_margins(visible_rows, visible_cols);
        let Some((cursor_wrapped_row, total_wrapped_rows)) =
            self.wrapped_cursor_row_and_total_rows(visible_cols, wrap_mode)
        else {
            self.wrapped_row_offset = 0;
            self.scroll_offset.col = 0;
            return;
        };
        let wrapped_row = Self::offset_to_keep_target_visible(
            self.wrapped_row_offset as usize,
            cursor_wrapped_row,
            visible_rows,
            effective_vertical_margin,
            total_wrapped_rows,
        );

        // Components like gutter/layout still consume logical `scroll_offset.row`.
        // We therefore project the wrapped top row back to its owning logical line,
        // while preserving wrapped precision in `wrapped_row_offset`.
        let top_wrapped_line = self
            .logical_line_for_wrapped_row(wrapped_row, visible_cols, wrap_mode)
            .unwrap_or(0);
        self.wrapped_row_offset = wrapped_row as u16;
        self.scroll_offset.row = top_wrapped_line as u16;
        self.scroll_offset.col = 0;
    }

    fn wrapped_cursor_row_and_total_rows(
        &self,
        visible_cols: usize,
        wrap_mode: WrapMode,
    ) -> Option<(usize, usize)> {
        let cursor = self.cursor;
        self.with_buffer(|buffer| {
            let mut total_rows = 0usize;
            let mut cursor_row = 0usize;

            for line_idx in 0..buffer.line_count() {
                let line_text = buffer
                    .line_at(line_idx)
                    .map(|line| line.as_ref())
                    .unwrap_or("");
                let segments = Self::wrap_segments_for_line(line_text, visible_cols, wrap_mode);
                let segment_count = segments.len().max(1);

                if line_idx == cursor.line {
                    let segment_idx = Self::wrapped_segment_index_for_cursor(&segments, cursor.col);
                    cursor_row = total_rows + segment_idx;
                }

                total_rows += segment_count;
            }
            (cursor_row, total_rows.max(1))
        })
    }

    fn wrapped_segment_index_for_cursor(
        segments: &[WrappedLineSegment],
        cursor_col: usize,
    ) -> usize {
        let mut segment_idx = segments.len().saturating_sub(1);
        for (idx, segment) in segments.iter().enumerate() {
            if cursor_col < segment.start_byte || cursor_col > segment.end_byte {
                continue;
            }
            // At an exact segment boundary, prefer the continuation segment so
            // end-of-segment cursors display on the next visual row.
            if cursor_col == segment.end_byte
                && let Some(next) = segments.get(idx + 1)
                && next.start_byte == cursor_col
            {
                continue;
            }
            segment_idx = idx;
            break;
        }
        segment_idx
    }

    fn logical_line_for_wrapped_row(
        &self,
        wrapped_row: usize,
        visible_cols: usize,
        wrap_mode: WrapMode,
    ) -> Option<usize> {
        self.with_buffer(|buffer| {
            let mut accumulated_rows = 0usize;
            for line_idx in 0..buffer.line_count() {
                let line_text = buffer
                    .line_at(line_idx)
                    .map(|line| line.as_ref())
                    .unwrap_or("");
                let segment_count =
                    Self::wrap_segments_for_line(line_text, visible_cols, wrap_mode)
                        .len()
                        .max(1);
                if accumulated_rows + segment_count > wrapped_row {
                    return line_idx;
                }
                accumulated_rows += segment_count;
            }
            buffer.line_count().saturating_sub(1)
        })
    }

    pub fn build_render_data(&self, size: Size) -> RenderData {
        self.build_render_data_with_style(size, Style::default())
    }

    /// Builds render data for the visible buffer region using a base style.
    pub fn build_render_data_with_style(&self, size: Size, default_style: Style) -> RenderData {
        self.build_render_data_with_options(size, default_style, false, WrapMode::Hard, true)
    }

    /// Builds render data for the visible buffer region using a base style and
    /// optional visual wrapping.
    pub fn build_render_data_with_options(
        &self,
        size: Size,
        default_style: Style,
        wrap_enabled: bool,
        wrap_mode: WrapMode,
        warm_syntax: bool,
    ) -> RenderData {
        if size.rows == 0 || size.cols == 0 {
            return RenderData::new(0);
        }

        let mut render_data = RenderData::new(size.rows);
        let syntax_styles =
            globals::with_active_theme(|theme| theme.map(|theme| theme.highlights.clone()));
        let selection_style = globals::with_active_theme(|theme| {
            theme.map(|theme| theme.highlight_style_for_name("ui.selection"))
        });
        let syntax_enabled = globals::with_config(|config| config.syntax).unwrap_or(true);
        let todo_markers: Vector<SmolStr> = if syntax_enabled {
            globals::with_config(|config| config.todo_markers.clone()).unwrap_or_default()
        } else {
            Vector::new()
        };
        let mut request_cache_refresh = false;
        let _applied = self.with_buffer_mut(|buffer| {
            let start_line = self.scroll_offset.row as usize;
            let row_limit = size.rows as usize + 32;
            let mut rendered_rows = 0usize;
            let horizontal_offset = self.scroll_offset.col as usize;

            if syntax_enabled && warm_syntax {
                let visible_end_line = start_line + size.rows.saturating_sub(1) as usize;
                let cached_line_count = buffer.cached_syntax_line_count();
                let warmup_window = size.rows as usize + 32;
                let near_cached_frontier =
                    start_line <= cached_line_count.saturating_add(warmup_window);
                if near_cached_frontier {
                    buffer.ensure_syntax_through(visible_end_line);
                }
                if self.buffer_id_opt().is_some() {
                    request_cache_refresh = true;
                }
            }

            let mut buffer_line_idx = if wrap_enabled { 0 } else { start_line };
            let mut rows_to_skip = if wrap_enabled {
                self.wrapped_row_offset as usize
            } else {
                0
            };
            loop {
                if let Some(line_text) = buffer.line_at(buffer_line_idx).cloned() {
                    let line_text = line_text.as_ref();
                    let syntax_spans = if syntax_enabled {
                        buffer
                            .cached_syntax_spans_for_line(buffer_line_idx)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    if wrap_enabled {
                        let segments =
                            Self::wrap_segments_for_line(line_text, size.cols as usize, wrap_mode);
                        if rows_to_skip >= segments.len() {
                            rows_to_skip -= segments.len();
                            buffer_line_idx += 1;
                            continue;
                        }
                        for segment in segments.into_iter().skip(rows_to_skip) {
                            let visible_text = &line_text[segment.start_byte..segment.end_byte];
                            let chunks = Self::build_chunks_for_visible_line(
                                line_text,
                                segment.start_byte..segment.end_byte,
                                visible_text,
                                &syntax_spans,
                                &todo_markers,
                                default_style,
                                syntax_styles.as_ref(),
                            );
                            let line_data = LineData {
                                buffer_line: buffer_line_idx,
                                byte_offset: segment.start_byte,
                                end_byte: segment.end_byte,
                                width_offset: 0,
                                show_gutter_line_number: !segment.is_continuation,
                                base_style: Style::default(),
                                chunks,
                            };
                            render_data.line_data.push(line_data);
                            rendered_rows += 1;
                            if rendered_rows >= row_limit {
                                break;
                            }
                        }
                        rows_to_skip = 0;
                    } else {
                        let (byte_offset, width_offset, visible_text) =
                            Self::calculate_horizontal_offset(line_text, horizontal_offset);
                        let chunks = Self::build_chunks_for_visible_line(
                            line_text,
                            byte_offset..line_text.len(),
                            &visible_text,
                            &syntax_spans,
                            &todo_markers,
                            default_style,
                            syntax_styles.as_ref(),
                        );
                        let line_data = LineData {
                            buffer_line: buffer_line_idx,
                            byte_offset,
                            end_byte: line_text.len(),
                            width_offset,
                            show_gutter_line_number: true,
                            base_style: Style::default(),
                            chunks,
                        };
                        render_data.line_data.push(line_data);
                        rendered_rows += 1;
                    }
                } else {
                    break;
                }
                if rendered_rows >= row_limit {
                    break;
                }
                buffer_line_idx += 1;
            }
        });

        if request_cache_refresh && let Some(buffer_id) = self.buffer_id_opt() {
            globals::with_buffer_pool(|pool| pool.request_buffer_cache_refresh(buffer_id));
        }

        if let Some(selection_style) = selection_style {
            self.apply_visual_selection(&mut render_data, selection_style);
            self.apply_yank_flash(&mut render_data, selection_style);
        }

        render_data
    }

    fn apply_visual_selection(&self, render_data: &mut RenderData, selection_style: Style) {
        let Some(selection) = self.visual_selection() else {
            return;
        };

        match selection.kind {
            VisualSelectionKind::Character => {
                let Some(selection) = self.visual_selection_range() else {
                    return;
                };
                self.apply_characterwise_selection(render_data, selection_style, selection);
            }
            VisualSelectionKind::Line => {
                let Some((start_line, count)) = self.visual_line_selection_range() else {
                    return;
                };
                self.apply_linewise_selection(render_data, selection_style, start_line, count);
            }
        }
    }

    fn apply_yank_flash(&self, render_data: &mut RenderData, selection_style: Style) {
        let Some(flash) = self.yank_flash() else {
            return;
        };

        match flash.selection {
            YankFlashSelection::Character(selection) => {
                self.apply_characterwise_selection(render_data, selection_style, selection)
            }
            YankFlashSelection::Line { start_line, count } => {
                self.apply_linewise_selection(render_data, selection_style, start_line, count)
            }
        }
    }

    fn apply_characterwise_selection(
        &self,
        render_data: &mut RenderData,
        selection_style: Style,
        selection: crate::buffer::TextObjectRange,
    ) {
        for line_data in &mut render_data.line_data {
            let Some((line_start, line_end)) =
                Self::intersect_line_range(selection.start, selection.end, line_data.buffer_line)
            else {
                continue;
            };

            let mut selected_chunks = Vec::with_capacity(line_data.chunks.len());
            let mut chunk_start = line_data.byte_offset;
            for chunk in line_data.chunks.drain(..) {
                let selected_style = chunk.style.overlay(selection_style);
                Self::push_split_render_chunk(
                    &mut selected_chunks,
                    &chunk.text,
                    chunk_start,
                    line_start,
                    line_end,
                    chunk.style,
                    selected_style,
                );

                chunk_start += chunk.text.len();
            }

            line_data.chunks = selected_chunks;
        }
    }

    fn apply_linewise_selection(
        &self,
        render_data: &mut RenderData,
        selection_style: Style,
        start_line: usize,
        count: usize,
    ) {
        let end_line = start_line.saturating_add(count.saturating_sub(1));

        for line_data in &mut render_data.line_data {
            if line_data.buffer_line < start_line || line_data.buffer_line > end_line {
                continue;
            }

            for chunk in &mut line_data.chunks {
                chunk.style = chunk.style.overlay(selection_style);
            }
        }
    }

    fn build_chunks_for_visible_line(
        line_text: &str,
        visible: Range<usize>,
        visible_text: &str,
        syntax_spans: &[crate::buffer::SyntaxSpan],
        todo_markers: &Vector<SmolStr>,
        _default_style: Style,
        syntax_styles: Option<&crate::theme::HighlightStyles>,
    ) -> Vec<RenderChunk> {
        if visible_text.is_empty() {
            return vec![RenderChunk::new("", Style::default())];
        }

        let visible_start = visible.start;
        let visible_end = visible.end;
        let mut chunks = Vec::new();
        let mut chunk_start = visible_start;

        for span in syntax_spans {
            // Ignore spans that end before the visible slice starts.
            if span.end_byte <= visible_start {
                continue;
            }
            // Stop once spans move past the visible slice.
            if span.start_byte >= visible_end {
                break;
            }

            // Clamp the span to the visible slice so horizontal scrolling works.
            let span_start = span.start_byte.max(visible_start);
            let span_end = span.end_byte.min(visible_end);

            // Emit any plain text between the last emitted chunk and this span.
            if chunk_start < span_start {
                chunks.push(RenderChunk::new(
                    &line_text[chunk_start..span_start],
                    Style::default(),
                ));
            }

            if span_start < span_end {
                if is_comment_tag(&span.style) && !todo_markers.is_empty() {
                    Self::build_comment_chunks(
                        line_text,
                        span.start_byte..span.end_byte,
                        visible_start..visible_end,
                        todo_markers,
                        syntax_styles,
                    )
                    .into_iter()
                    .for_each(|chunk| chunks.push(chunk));
                } else {
                    // Convert the syntax category into the active theme's concrete style.
                    let syntax_style = syntax_styles
                        .map(|styles| styles.style_for_tag(&span.style, Style::default()))
                        .unwrap_or_default();
                    chunks.push(RenderChunk::new(
                        &line_text[span_start..span_end],
                        syntax_style,
                    ));
                }
                // Advance past the highlighted region so the next gap is computed correctly.
                chunk_start = span_end;
            }
        }

        // Emit any remaining plain text after the last highlighted span.
        if chunk_start < visible_end {
            chunks.push(RenderChunk::new(
                &line_text[chunk_start..visible_end],
                Style::default(),
            ));
        }

        // If no spans applied, fall back to a single plain chunk.
        if chunks.is_empty() {
            chunks.push(RenderChunk::new(visible_text, Style::default()));
        }

        chunks
    }

    fn build_comment_chunks(
        line_text: &str,
        span: Range<usize>,
        visible: Range<usize>,
        todo_markers: &Vector<SmolStr>,
        syntax_styles: Option<&crate::theme::HighlightStyles>,
    ) -> Vec<RenderChunk> {
        let Some((render_start, render_end)) =
            Self::intersect_byte_ranges(span.start, span.end, visible.start, visible.end)
        else {
            return Vec::new();
        };

        // Scan the full comment span first so offscreen markers still
        // contribute to the final split points when the viewport clips in the
        // middle of a comment.
        let comment_text = &line_text[span.start..span.end];
        let comment_style = syntax_styles
            .map(|styles| styles.style_for_tag(&comment_tag(), Style::default()))
            .unwrap_or_default();
        // Marker matches are computed in comment-local coordinates, then
        // shifted back into line coordinates so the visible slice can be
        // clipped cleanly below.
        let matches = Self::find_todo_matches(comment_text, todo_markers.iter())
            .into_iter()
            .map(|marker_match| TodoMatch {
                start_byte: span.start + marker_match.start_byte,
                end_byte: span.start + marker_match.end_byte,
                marker: marker_match.marker,
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return vec![RenderChunk::new(
                &line_text[render_start..render_end],
                comment_style,
            )];
        }

        let mut chunks = Vec::with_capacity(matches.len() * 2 + 1);
        let mut chunk_start = render_start;
        for marker_match in matches {
            let Some((marker_start, marker_end)) = Self::intersect_byte_ranges(
                marker_match.start_byte,
                marker_match.end_byte,
                render_start,
                render_end,
            ) else {
                continue;
            };
            let marker_tag = todo_marker_tag(&marker_match.marker);
            let marker_style = syntax_styles
                .map(|styles| styles.style_for_tag(&marker_tag, Style::default()))
                .unwrap_or_default();
            let segment_text = &line_text[chunk_start..marker_end];
            Self::push_split_render_chunk(
                &mut chunks,
                segment_text,
                chunk_start,
                marker_start,
                marker_end,
                comment_style,
                marker_style,
            );
            chunk_start = marker_end;
        }

        // Flush any trailing comment text after the last visible marker so the
        // comment styling remains contiguous across the rendered slice.
        if chunk_start < render_end {
            chunks.push(RenderChunk::new(
                &line_text[chunk_start..render_end],
                comment_style,
            ));
        }

        // If the scan found no visible markers, still return the comment text
        // with the ordinary comment style instead of leaving the region plain.
        if chunks.is_empty() {
            chunks.push(RenderChunk::new(
                &line_text[render_start..render_end],
                comment_style,
            ));
        }

        chunks
    }

    /// Splits one rendered text slice into prefix, selected middle, and suffix.
    ///
    /// The prefix and suffix keep `base_style`, while the selected middle uses
    /// `selected_style`. This lets callers reuse the same byte-range splitting
    /// logic for both visual selection and inline marker highlighting.
    fn push_split_render_chunk(
        chunks: &mut Vec<RenderChunk>,
        text: &str,
        chunk_start: usize,
        range_start: usize,
        range_end: usize,
        base_style: Style,
        selected_style: Style,
    ) {
        let chunk_end = chunk_start + text.len();
        if chunk_end <= range_start || chunk_start >= range_end {
            chunks.push(RenderChunk::new(text, base_style));
            return;
        }

        let local_start = range_start.saturating_sub(chunk_start).min(text.len());
        let local_end = range_end.saturating_sub(chunk_start).min(text.len());

        if local_start > 0 {
            chunks.push(RenderChunk::new(&text[..local_start], base_style));
        }
        if local_start < local_end {
            chunks.push(RenderChunk::new(
                &text[local_start..local_end],
                selected_style,
            ));
        }
        if local_end < text.len() {
            chunks.push(RenderChunk::new(&text[local_end..], base_style));
        }
    }

    fn find_todo_matches<'a, I>(comment_text: &str, todo_markers: I) -> Vec<TodoMatch>
    where
        I: IntoIterator<Item = &'a SmolStr>,
    {
        let mut matches: Vec<TodoMatch> = Vec::new();
        for marker in todo_markers {
            let mut search_start = 0;
            while let Some(relative_start) = comment_text[search_start..].find(marker.as_str()) {
                let start_byte = search_start + relative_start;
                let end_byte = start_byte + marker.len();
                if is_standalone_word(comment_text, start_byte, end_byte) {
                    let marker_match = TodoMatch {
                        start_byte,
                        end_byte,
                        marker: marker.to_string(),
                    };
                    if !matches.iter().any(|existing| {
                        existing.start_byte == marker_match.start_byte
                            && existing.end_byte == marker_match.end_byte
                            && existing.marker == marker_match.marker
                    }) {
                        matches.push(marker_match);
                    }
                }
                search_start = end_byte;
            }
        }

        matches.sort_by(|left, right| {
            left.start_byte
                .cmp(&right.start_byte)
                .then(left.end_byte.cmp(&right.end_byte))
                .then(left.marker.cmp(&right.marker))
        });
        matches
    }

    fn visual_selection_range_for(
        buffer: &crate::buffer::Buffer,
        anchor: Cursor,
        cursor: Cursor,
    ) -> Option<crate::buffer::TextObjectRange> {
        // Sync both endpoints against the current buffer so the range stays
        // valid after edits that may have shifted or invalidated the cursors.
        let anchor = buffer.sync_cursor(anchor);
        let cursor = buffer.sync_cursor(cursor);

        // Normalize the selection so start is always before end, regardless
        // of which side the cursor is on.
        let (start, end) = if (anchor.line, anchor.col) <= (cursor.line, cursor.col) {
            (anchor, cursor)
        } else {
            (cursor, anchor)
        };

        // Visual selections behave like Vim: the end is inclusive of the last
        // character, so extend it one cursor past the final selected grapheme.
        let end = if let Some(ch) = buffer.char_at_cursor(end) {
            buffer
                .next_cursor(end)
                .unwrap_or_else(|| Cursor::new(end.line, end.col + ch.len_utf8()))
        } else {
            end
        };

        if start.line > end.line || (start.line == end.line && start.col >= end.col) {
            return None;
        }

        Some(crate::buffer::TextObjectRange { start, end })
    }

    fn visual_line_selection_range_for(
        buffer: &crate::buffer::Buffer,
        anchor: Cursor,
        cursor: Cursor,
    ) -> Option<(usize, usize)> {
        let anchor = buffer.sync_cursor(anchor);
        let cursor = buffer.sync_cursor(cursor);
        let start_line = anchor.line.min(cursor.line);
        let end_line = anchor.line.max(cursor.line);
        let count = end_line.saturating_sub(start_line).saturating_add(1);
        (count > 0).then_some((start_line, count))
    }

    fn intersect_line_range(start: Cursor, end: Cursor, line_idx: usize) -> Option<(usize, usize)> {
        // Reject lines outside the selection span outright so callers can
        // skip them without any extra range math.
        if line_idx < start.line || line_idx > end.line {
            return None;
        }

        // For the first and last selected lines, clamp to the visible
        // endpoints. Middle lines are fully selected.
        let line_range = if start.line == end.line {
            (start.col, end.col)
        } else if line_idx == start.line {
            (start.col, usize::MAX)
        } else if line_idx == end.line {
            (0, end.col)
        } else {
            (0, usize::MAX)
        };

        Some(line_range)
    }

    /// Returns the overlap between two byte ranges, if any.
    fn intersect_byte_ranges(
        left_start: usize,
        left_end: usize,
        right_start: usize,
        right_end: usize,
    ) -> Option<(usize, usize)> {
        let start = left_start.max(right_start);
        let end = left_end.min(right_end);
        (start < end).then_some((start, end))
    }

    fn calculate_horizontal_offset(
        line_text: &str,
        visual_width_offset: usize,
    ) -> (usize, usize, String) {
        let tab_width = configured_tab_width();
        if visual_width_offset == 0 {
            return (0, 0, line_text.to_string());
        }

        let mut current_width = 0;
        let mut byte_offset = 0;

        for grapheme in line_text.graphemes(true) {
            let grapheme_width = display_grapheme_width(grapheme, current_width, tab_width);
            if current_width + grapheme_width > visual_width_offset {
                break;
            }
            current_width += grapheme_width;
            byte_offset += grapheme.len();
        }

        let actual_line_width = display_width_at(line_text, 0, tab_width);
        if byte_offset >= line_text.len() {
            return (line_text.len(), actual_line_width, String::new());
        }

        let visible_text = line_text[byte_offset..].to_string();
        (byte_offset, current_width, visible_text)
    }
}

fn leading_indent_width(line: &str) -> usize {
    let tab_width = configured_tab_width();
    line.chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .map(|ch| if ch == '\t' { tab_width } else { 1 })
        .sum()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TodoMatch {
    start_byte: usize,
    end_byte: usize,
    marker: String,
}

fn todo_marker_tag(marker: &str) -> Tag {
    Tag::parse(&format!("comment.{}", marker.to_ascii_lowercase())).expect("valid todo marker tag")
}

fn comment_tag() -> Tag {
    Tag::parse("comment").expect("valid comment tag")
}

fn is_comment_tag(tag: &Tag) -> bool {
    tag.parent_chain().any(|candidate| candidate == "comment")
}

fn is_standalone_word(text: &str, start_byte: usize, end_byte: usize) -> bool {
    let before = text[..start_byte].chars().next_back();
    let after = text[end_byte..].chars().next();
    !matches!(before, Some(ch) if is_word_char(ch))
        && !matches!(after, Some(ch) if is_word_char(ch))
}

fn is_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::config::Config;
    use crate::globals;
    use crate::path::AbsolutePath;
    use crate::terminal::{Color, Style};
    use crate::theme::{HighlightStyles, Theme, ThemeKind};
    use std::collections::BTreeSet;

    fn temp_path_with_ext(name: &str, ext: &str) -> AbsolutePath {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "urvim-view-tests-{}-{}-{}.{}",
            std::process::id(),
            nanos,
            name,
            ext
        ));
        AbsolutePath::from_path(path.as_path()).unwrap()
    }

    fn comment_only_theme() -> Theme {
        let default_style = Style::new().fg(Color::ansi(15)).bg(Color::ansi(30));
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.selection").expect("valid tag"),
            Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
        );
        highlights.insert(
            Tag::parse("ui.status_bar").expect("valid tag"),
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
        );
        highlights.insert(
            Tag::parse("ui.status_bar.modified_marker").expect("valid tag"),
            Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
        );
        highlights.insert(
            Tag::parse("ui.window.active_line").expect("valid tag"),
            Style::new().bg(Color::ansi(21)),
        );
        highlights.insert(
            Tag::parse("ui.tab.active").expect("valid tag"),
            Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
        );
        highlights.insert(
            Tag::parse("ui.tab.inactive").expect("valid tag"),
            Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
        );
        highlights.insert(
            Tag::parse("ui.tab.scroll_indicator").expect("valid tag"),
            Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
        );
        highlights.insert(
            Tag::parse("ui.window.gutter").expect("valid tag"),
            Style::new().fg(Color::ansi(13)).bg(Color::ansi(14)),
        );
        highlights.insert(
            Tag::parse("ui.window").expect("valid tag"),
            Style::new().fg(Color::ansi(15)).bg(Color::ansi(16)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines").expect("valid tag"),
            Style::new().fg(Color::ansi(17)).bg(Color::ansi(18)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines.resize").expect("valid tag"),
            Style::new().fg(Color::ansi(19)).bg(Color::ansi(20)),
        );
        highlights.insert(
            Tag::parse("syntax.comment").expect("valid tag"),
            Style::new().fg(Color::ansi(20)),
        );

        Theme::new(
            "comment-only",
            ThemeKind::Ansi256,
            default_style,
            highlights,
        )
    }

    fn marker_theme() -> Theme {
        let default_style = Style::new().fg(Color::ansi(15)).bg(Color::ansi(30));
        let mut highlights = HighlightStyles::default();
        highlights.insert(
            Tag::parse("ui.selection").expect("valid tag"),
            Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
        );
        highlights.insert(
            Tag::parse("ui.status_bar").expect("valid tag"),
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
        );
        highlights.insert(
            Tag::parse("ui.status_bar.modified_marker").expect("valid tag"),
            Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
        );
        highlights.insert(
            Tag::parse("ui.window.active_line").expect("valid tag"),
            Style::new().bg(Color::ansi(21)),
        );
        highlights.insert(
            Tag::parse("ui.tab.active").expect("valid tag"),
            Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
        );
        highlights.insert(
            Tag::parse("ui.tab.inactive").expect("valid tag"),
            Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
        );
        highlights.insert(
            Tag::parse("ui.tab.scroll_indicator").expect("valid tag"),
            Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
        );
        highlights.insert(
            Tag::parse("ui.window.gutter").expect("valid tag"),
            Style::new().fg(Color::ansi(13)).bg(Color::ansi(14)),
        );
        highlights.insert(
            Tag::parse("ui.window").expect("valid tag"),
            Style::new().fg(Color::ansi(15)).bg(Color::ansi(16)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines").expect("valid tag"),
            Style::new().fg(Color::ansi(17)).bg(Color::ansi(18)),
        );
        highlights.insert(
            Tag::parse("ui.window.lines.resize").expect("valid tag"),
            Style::new().fg(Color::ansi(19)).bg(Color::ansi(20)),
        );
        highlights.insert(
            Tag::parse("syntax.comment").expect("valid tag"),
            Style::new().fg(Color::ansi(20)),
        );
        highlights.insert(
            Tag::parse("syntax.comment.todo").expect("valid tag"),
            Style::new().fg(Color::ansi(31)),
        );
        highlights.insert(
            Tag::parse("syntax.comment.fixme").expect("valid tag"),
            Style::new().fg(Color::ansi(32)),
        );
        highlights.insert(
            Tag::parse("syntax.comment.bug").expect("valid tag"),
            Style::new().fg(Color::ansi(33)),
        );
        highlights.insert(
            Tag::parse("syntax.comment.note").expect("valid tag"),
            Style::new().fg(Color::ansi(34)),
        );

        Theme::new("marker-demo", ThemeKind::Ansi256, default_style, highlights)
    }

    #[test]
    fn find_todo_matches_requires_standalone_case_sensitive_markers() {
        let markers = imbl::Vector::from_iter(
            ["TODO", "FIXME", "BUG", "NOTE"]
                .into_iter()
                .map(SmolStr::new),
        );
        let matches = BufferView::find_todo_matches(
            "todo TODO FIXME FIXME123 BUG BUG123 NOTE NOTE123",
            markers.iter(),
        );

        assert_eq!(
            matches
                .into_iter()
                .map(|marker_match| (marker_match.start_byte, marker_match.marker))
                .collect::<Vec<_>>(),
            vec![
                (5, "TODO".to_string()),
                (10, "FIXME".to_string()),
                (25, "BUG".to_string()),
                (36, "NOTE".to_string()),
            ]
        );
    }

    #[test]
    fn build_comment_chunks_falls_back_to_comment_style_when_marker_styles_are_missing() {
        let path = temp_path_with_ext("todo-fallback", "rs");
        let buffer = Buffer::from_str_with_path("fn main() { // TODO }", path);
        let view = BufferView::new(buffer);
        let theme = comment_only_theme();
        let theme_default_style = theme.default_style();
        let expected_comment_style =
            theme.highlight_style_for_tag(&Tag::parse("comment").expect("valid tag"));
        let _theme_guard = globals::set_test_active_theme(theme);
        let _config_guard = globals::set_test_config(Config {
            theme: "comment-only".to_string(),
            insert_escape: None,
            syntax: true,
            auto_close_pairs: true,
            auto_indent: crate::config::AutoIndentMode::Off,
            advanced_glyphs: BTreeSet::new(),
            ..Default::default()
        });

        let render_data = view.build_render_data_with_style(Size::new(1, 80), theme_default_style);
        let line = render_data.get_line(0).expect("rendered line should exist");
        assert!(
            line.iter()
                .any(|chunk| chunk.text == "TODO" && chunk.style == expected_comment_style)
        );
    }

    #[test]
    fn todo_markers_do_not_highlight_outside_comments() {
        let buffer = Buffer::from_str("TODO = 1");
        let view = BufferView::new(buffer);
        let theme = marker_theme();
        let theme_default_style = theme.default_style();
        let expected_default_style = Style::default();
        let _theme_guard = globals::set_test_active_theme(theme);
        let _config_guard = globals::set_test_config(Config {
            theme: "marker-demo".to_string(),
            insert_escape: None,
            syntax: true,
            auto_close_pairs: true,
            auto_indent: crate::config::AutoIndentMode::Off,
            advanced_glyphs: BTreeSet::new(),
            ..Default::default()
        });

        let render_data = view.build_render_data_with_style(Size::new(1, 80), theme_default_style);
        let line = render_data.get_line(0).expect("rendered line should exist");

        assert!(line.iter().any(|chunk| chunk.text.contains("TODO")));
        assert!(
            line.iter()
                .all(|chunk| chunk.style == expected_default_style)
        );
    }
}
