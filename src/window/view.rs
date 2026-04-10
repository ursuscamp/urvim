use super::*;
use crate::buffer::BufferId;

impl BufferView {
    /// Creates a new view and registers the buffer in the global pool.
    pub fn new(buffer: Buffer) -> Self {
        let buffer_id = crate::globals::with_buffer_pool(|pool| pool.register_buffer(buffer));
        Self::from_buffer_id(buffer_id)
    }

    /// Creates a view for an already-registered buffer ID.
    pub fn from_buffer_id(buffer_id: BufferId) -> Self {
        Self {
            buffer_id,
            scroll_offset: Position::new(0, 0),
            cursor: Cursor::new(0, 0),
            remembered_visual_col: None,
        }
    }

    /// Returns the buffer ID owned by this view.
    pub fn buffer_id(&self) -> BufferId {
        self.buffer_id
    }

    /// Runs a closure with shared access to the shared buffer.
    pub fn with_buffer<R>(&self, f: impl FnOnce(&Buffer) -> R) -> Option<R> {
        crate::globals::with_buffer(self.buffer_id, f)
    }

    /// Runs a closure with mutable access to the shared buffer.
    pub fn with_buffer_mut<R>(&self, f: impl FnOnce(&mut Buffer) -> R) -> Option<R> {
        crate::globals::with_buffer_mut(self.buffer_id, f)
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

    pub fn scroll_offset(&self) -> Position {
        self.scroll_offset
    }

    pub fn set_scroll_offset(&mut self, offset: Position) {
        self.scroll_offset = offset;
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
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

    pub fn scroll_to_cursor(&mut self, viewport_size: Size, gutter_width: u16) {
        let cursor = self.cursor;
        let Some((buffer_line_count, cursor_visual_col, line_width)) = self.with_buffer(|buffer| {
            let line_width = buffer
                .line_at(cursor.line)
                .map(|line| UnicodeWidthStr::width(line.as_ref()))
                .unwrap_or(0);
            (
                buffer.line_count(),
                buffer.visual_col_at(cursor),
                line_width,
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

        if cursor.line < self.scroll_offset.row as usize {
            self.scroll_offset.row = cursor.line as u16;
        } else if cursor.line >= self.scroll_offset.row as usize + visible_rows {
            self.scroll_offset.row = (cursor.line + 1 - visible_rows) as u16;
        }

        let max_row = buffer_line_count.saturating_sub(visible_rows);
        if self.scroll_offset.row as usize > max_row {
            self.scroll_offset.row = max_row as u16;
        }

        if cursor_visual_col < self.scroll_offset.col as usize {
            self.scroll_offset.col = cursor_visual_col as u16;
        } else if cursor_visual_col >= self.scroll_offset.col as usize + visible_cols {
            self.scroll_offset.col = (cursor_visual_col + 1 - visible_cols) as u16;
        }

        let max_col = line_width.saturating_sub(visible_cols);
        if self.scroll_offset.col as usize > max_col {
            self.scroll_offset.col = max_col as u16;
        }
    }

    pub fn build_render_data(&self, size: Size) -> RenderData {
        self.build_render_data_with_style(size, Style::default())
    }

    /// Builds render data for the visible buffer region using a base style.
    pub fn build_render_data_with_style(&self, size: Size, default_style: Style) -> RenderData {
        let mut render_data = RenderData::new(size.rows);
        let syntax_styles =
            globals::with_active_theme(|theme| theme.map(|theme| theme.syntax.clone()));
        let syntax_enabled =
            globals::with_config(|config| config.map(|config| config.syntax)).unwrap_or(true);
        let _ = self.with_buffer_mut(|buffer| {
            let start_line = self.scroll_offset.row as usize;
            let total_lines_needed = size.rows as usize + 10;
            let horizontal_offset = self.scroll_offset.col as usize;

            for screen_line in 0..total_lines_needed {
                let buffer_line_idx = start_line + screen_line;
                if let Some(line_text) = buffer.line_at(buffer_line_idx).cloned() {
                    let line_text = line_text.as_ref();
                    let (byte_offset, width_offset, visible_text) =
                        Self::calculate_horizontal_offset(line_text, horizontal_offset);
                    let syntax_spans = if syntax_enabled {
                        buffer
                            .syntax_spans_for_line(buffer_line_idx)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let chunks = Self::build_chunks_for_visible_line(
                        line_text,
                        byte_offset,
                        &visible_text,
                        &syntax_spans,
                        default_style,
                        syntax_styles.as_ref(),
                    );
                    let line_data = LineData {
                        buffer_line: buffer_line_idx,
                        byte_offset,
                        width_offset,
                        chunks,
                    };
                    render_data.line_data.push(line_data);
                } else {
                    break;
                }
            }
        });

        for line_data in &mut render_data.line_data {
            for chunk in &mut line_data.chunks {
                chunk.style = default_style.overlay(chunk.style);
            }
        }

        render_data
    }

    fn build_chunks_for_visible_line(
        line_text: &str,
        visible_start: usize,
        visible_text: &str,
        syntax_spans: &[crate::buffer::SyntaxSpan],
        default_style: Style,
        syntax_styles: Option<&crate::theme::SyntaxTagStyles>,
    ) -> Vec<RenderChunk> {
        if visible_text.is_empty() {
            return vec![RenderChunk::new("", default_style)];
        }

        let visible_end = line_text.len();
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
                    default_style,
                ));
            }

            if span_start < span_end {
                // Convert the syntax category into the active theme's concrete style.
                let syntax_style = syntax_styles
                    .map(|styles| styles.style_for_tag(&span.style, default_style))
                    .unwrap_or_default();
                chunks.push(RenderChunk::new(
                    &line_text[span_start..span_end],
                    default_style.overlay(syntax_style),
                ));
                // Advance past the highlighted region so the next gap is computed correctly.
                chunk_start = span_end;
            }
        }

        // Emit any remaining plain text after the last highlighted span.
        if chunk_start < visible_end {
            chunks.push(RenderChunk::new(
                &line_text[chunk_start..visible_end],
                default_style,
            ));
        }

        // If no spans applied, fall back to a single plain chunk.
        if chunks.is_empty() {
            chunks.push(RenderChunk::new(visible_text, default_style));
        }

        chunks
    }

    fn calculate_horizontal_offset(
        line_text: &str,
        visual_width_offset: usize,
    ) -> (usize, usize, String) {
        if visual_width_offset == 0 {
            return (0, 0, line_text.to_string());
        }

        let mut current_width = 0;
        let mut byte_offset = 0;

        for grapheme in line_text.graphemes(true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            if current_width + grapheme_width > visual_width_offset {
                break;
            }
            current_width += grapheme_width;
            byte_offset += grapheme.len();
        }

        let actual_line_width = UnicodeWidthStr::width(line_text);
        if byte_offset >= line_text.len() {
            return (line_text.len(), actual_line_width, String::new());
        }

        let visible_text = line_text[byte_offset..].to_string();
        (byte_offset, current_width, visible_text)
    }
}
