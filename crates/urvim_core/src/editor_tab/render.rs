use super::*;
use crate::buffer::{configured_tab_width, display_grapheme_width, display_width_at, expand_tabs};
use crate::ui::text_width::{ClipSide, clip_text};
use unicode_segmentation::UnicodeSegmentation;
use urvim_theme::StyleOverlay;

impl RenderChunk {
    /// Creates a run backed by real buffer text.
    pub fn new(text: &str, style: Style) -> Self {
        Self {
            text: text.to_string(),
            style,
            is_virtual_text: false,
        }
    }

    /// Creates a real-text run with the default style.
    pub fn default_text(text: &str) -> Self {
        Self {
            text: text.to_string(),
            style: Style::default(),
            is_virtual_text: false,
        }
    }

    /// Creates a virtual-text run that consumes no buffer bytes.
    pub fn virtual_text(text: &str, style: Style) -> Self {
        Self {
            text: text.to_string(),
            style,
            is_virtual_text: true,
        }
    }

    fn with_virtual_text_flag(text: &str, style: Style, is_virtual_text: bool) -> Self {
        Self {
            text: text.to_string(),
            style,
            is_virtual_text,
        }
    }
}

impl RenderData {
    pub fn new(visible_rows: u16) -> Self {
        Self {
            line_data: Vec::with_capacity(visible_rows as usize + 10),
            visible_rows,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_data.len()
    }

    pub fn get_line(&self, screen_line: usize) -> Option<&[RenderChunk]> {
        self.line_data
            .get(screen_line)
            .map(|ld| ld.chunks.as_slice())
    }

    pub fn visible_rows(&self) -> u16 {
        self.visible_rows
    }

    /// Renders the buffer content clipped to the supplied viewport size.
    pub fn render(&self, screen: &mut Screen, origin: Position, size: Size, base_style: Style) {
        let tab_width = configured_tab_width();
        let right_col = origin.col.saturating_add(size.cols);
        for (row_offset, line_data) in self.line_data.iter().take(size.rows as usize).enumerate() {
            let row = origin.row + row_offset as u16;
            let line_base_style = base_style.overlay(line_data.base_style);
            let mut line_visual_col = line_data.width_offset;
            let mut col_offset = origin.col;
            for chunk in &line_data.chunks {
                if col_offset >= right_col {
                    break;
                }

                let style = line_base_style.overlay(chunk.style);
                let rendered = expand_tabs(&chunk.text, line_visual_col, tab_width);
                let remaining_cols = usize::from(right_col.saturating_sub(col_offset));
                let clipped = clip_text(rendered.as_str(), remaining_cols, ClipSide::Start).text;
                screen.write_string(row, col_offset, style, clipped.as_str());
                let chunk_width = display_width_at(&chunk.text, line_visual_col, tab_width);
                line_visual_col += chunk_width;
                col_offset = col_offset.saturating_add(chunk_width as u16).min(right_col);
            }

            if line_data.base_style != Style::default() && col_offset < right_col {
                screen.fill_region(row, col_offset, 1, right_col - col_offset, line_base_style);
            }
        }
    }

    /// Applies an extra base style to one rendered line.
    pub fn set_line_base_style(&mut self, screen_line: usize, style: Style) {
        if let Some(line_data) = self.line_data.get_mut(screen_line) {
            line_data.base_style = style;
        }
    }

    /// Applies a style overlay to every chunk intersecting the cursor range.
    pub fn overlay_range(&mut self, start: Cursor, end: Cursor, style: Style) {
        self.apply_range_style(start, end, |base| base.overlay(style));
    }

    /// Applies a partial style overlay to real text in a cursor range.
    pub fn overlay_style_range(&mut self, start: Cursor, end: Cursor, style: StyleOverlay) {
        self.apply_range_style(start, end, |base| style.apply_to(base));
    }

    /// Applies a style accent to every chunk intersecting the cursor range.
    pub fn accent_range(&mut self, start: Cursor, end: Cursor, style: Style) {
        self.apply_range_style(start, end, |base| base.accent(style));
    }

    pub fn cursor_screen_position(&self, cursor: Cursor) -> Option<Position> {
        let tab_width = configured_tab_width();
        for (screen_row, line_data) in self
            .line_data
            .iter()
            .take(self.visible_rows as usize)
            .enumerate()
        {
            if line_data.buffer_line != cursor.line {
                continue;
            }

            let mut row_has_real_text = false;
            let mut row_byte_offset = line_data.byte_offset;
            let mut row_end_byte = line_data.byte_offset;
            for chunk in &line_data.chunks {
                if chunk.is_virtual_text {
                    continue;
                }
                row_has_real_text = true;
                row_end_byte = row_end_byte.max(row_byte_offset + chunk.text.len());
                row_byte_offset += chunk.text.len();
            }

            if !row_has_real_text {
                if cursor.col < line_data.byte_offset || cursor.col > line_data.end_byte {
                    continue;
                }
            } else if cursor.col < line_data.byte_offset || cursor.col > row_end_byte {
                continue;
            }

            let mut visual_col = 0;
            let mut original_col = line_data.byte_offset;
            for chunk in &line_data.chunks {
                if chunk.is_virtual_text {
                    if cursor.col < original_col {
                        break;
                    }
                    visual_col += display_width_at(&chunk.text, visual_col, tab_width);
                    continue;
                }
                let mut chunk_byte_pos = 0;
                for grapheme in chunk.text.graphemes(true) {
                    if original_col + chunk_byte_pos >= cursor.col {
                        break;
                    }
                    visual_col += display_grapheme_width(grapheme, visual_col, tab_width);
                    chunk_byte_pos += grapheme.len();
                }
                original_col += chunk.text.len();
            }
            return Some(Position::new(screen_row as u16, visual_col as u16));
        }
        None
    }

    fn apply_range_style(&mut self, start: Cursor, end: Cursor, combine: impl Fn(Style) -> Style) {
        if end.line < start.line || (end.line == start.line && end.col <= start.col) {
            return;
        }

        for line_data in &mut self.line_data {
            if line_data.buffer_line < start.line || line_data.buffer_line > end.line {
                continue;
            }

            let start_byte = if line_data.buffer_line == start.line {
                start.col
            } else {
                0
            };
            let end_byte = if line_data.buffer_line == end.line {
                end.col
            } else {
                line_data.end_byte
            };

            let visible_start = start_byte.max(line_data.byte_offset);
            let visible_end = end_byte.min(line_data.end_byte);
            if visible_start >= visible_end {
                continue;
            }

            let mut selected_chunks = Vec::with_capacity(line_data.chunks.len());
            let mut chunk_start = line_data.byte_offset;
            for chunk in line_data.chunks.drain(..) {
                if chunk.is_virtual_text {
                    selected_chunks.push(chunk);
                    continue;
                }

                let selected_style = combine(chunk.style);
                push_split_render_chunk(
                    &mut selected_chunks,
                    &chunk.text,
                    chunk_start,
                    visible_start,
                    visible_end,
                    chunk.style,
                    selected_style,
                    false,
                );
                chunk_start += chunk.text.len();
            }

            line_data.chunks = selected_chunks;
        }
    }
}

fn push_split_render_chunk(
    chunks: &mut Vec<RenderChunk>,
    text: &str,
    chunk_start: usize,
    range_start: usize,
    range_end: usize,
    base_style: Style,
    selected_style: Style,
    is_virtual_text: bool,
) {
    let chunk_end = chunk_start + text.len();
    if chunk_end <= range_start || chunk_start >= range_end {
        chunks.push(RenderChunk::with_virtual_text_flag(
            text,
            base_style,
            is_virtual_text,
        ));
        return;
    }

    let local_start = range_start.saturating_sub(chunk_start).min(text.len());
    let local_end = range_end.saturating_sub(chunk_start).min(text.len());

    if local_start > 0 {
        chunks.push(RenderChunk::with_virtual_text_flag(
            &text[..local_start],
            base_style,
            is_virtual_text,
        ));
    }
    if local_start < local_end {
        chunks.push(RenderChunk::with_virtual_text_flag(
            &text[local_start..local_end],
            selected_style,
            is_virtual_text,
        ));
    }
    if local_end < text.len() {
        chunks.push(RenderChunk::with_virtual_text_flag(
            &text[local_end..],
            base_style,
            is_virtual_text,
        ));
    }
}
