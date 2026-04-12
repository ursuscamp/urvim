use super::*;
use crate::buffer::BufferId;
use crate::buffer::{configured_tab_width, display_grapheme_width, display_width_at};
use crate::theme::Tag;
use imbl::Vector;
use smol_str::SmolStr;

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
        let syntax_enabled = globals::with_config(|config| config.syntax).unwrap_or(true);
        let todo_markers: Vector<SmolStr> = if syntax_enabled {
            globals::with_config(|config| config.todo_markers.clone()).unwrap_or_default()
        } else {
            Vector::new()
        };
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
                        &todo_markers,
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
        todo_markers: &Vector<SmolStr>,
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
                if is_comment_tag(&span.style) && !todo_markers.is_empty() {
                    Self::build_comment_chunks(
                        line_text,
                        span.start_byte,
                        span.end_byte,
                        visible_start,
                        visible_end,
                        todo_markers,
                        default_style,
                        syntax_styles,
                    )
                    .into_iter()
                    .for_each(|chunk| chunks.push(chunk));
                } else {
                    // Convert the syntax category into the active theme's concrete style.
                    let syntax_style = syntax_styles
                        .map(|styles| styles.style_for_tag(&span.style, default_style))
                        .unwrap_or_default();
                    chunks.push(RenderChunk::new(
                        &line_text[span_start..span_end],
                        default_style.overlay(syntax_style),
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
                default_style,
            ));
        }

        // If no spans applied, fall back to a single plain chunk.
        if chunks.is_empty() {
            chunks.push(RenderChunk::new(visible_text, default_style));
        }

        chunks
    }

    fn build_comment_chunks(
        line_text: &str,
        span_start: usize,
        span_end: usize,
        visible_start: usize,
        visible_end: usize,
        todo_markers: &Vector<SmolStr>,
        default_style: Style,
        syntax_styles: Option<&crate::theme::SyntaxTagStyles>,
    ) -> Vec<RenderChunk> {
        let render_start = span_start.max(visible_start);
        let render_end = span_end.min(visible_end);
        if render_start >= render_end {
            return Vec::new();
        }

        // Scan the full comment span first so offscreen markers still
        // contribute to the final split points when the viewport clips in the
        // middle of a comment.
        let comment_text = &line_text[span_start..span_end];
        let comment_style = syntax_styles
            .map(|styles| styles.style_for_tag(&comment_tag(), default_style))
            .unwrap_or_default();
        // Marker matches are computed in comment-local coordinates, then
        // shifted back into line coordinates so the visible slice can be
        // clipped cleanly below.
        let matches = Self::find_todo_matches(comment_text, todo_markers.iter())
            .into_iter()
            .map(|marker_match| TodoMatch {
                start_byte: span_start + marker_match.start_byte,
                end_byte: span_start + marker_match.end_byte,
                marker: marker_match.marker,
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return vec![RenderChunk::new(
                &line_text[render_start..render_end],
                default_style.overlay(comment_style),
            )];
        }

        let mut chunks = Vec::with_capacity(matches.len() * 2 + 1);
        let mut chunk_start = render_start;
        for marker_match in matches {
            if marker_match.end_byte <= render_start {
                continue;
            }
            if marker_match.start_byte >= render_end {
                break;
            }

            let marker_start = marker_match.start_byte.max(render_start);
            let marker_end = marker_match.end_byte.min(render_end);

            // Emit the ordinary comment text before the marker with the
            // regular comment style, then emit the marker itself with its
            // marker-specific theme tag.
            if chunk_start < marker_start {
                chunks.push(RenderChunk::new(
                    &line_text[chunk_start..marker_start],
                    default_style.overlay(comment_style),
                ));
            }

            let marker_tag = todo_marker_tag(&marker_match.marker);
            let marker_style = syntax_styles
                .map(|styles| styles.style_for_tag(&marker_tag, default_style))
                .unwrap_or_default();
            chunks.push(RenderChunk::new(
                &line_text[marker_start..marker_end],
                default_style.overlay(marker_style),
            ));
            chunk_start = marker_end;
        }

        // Flush any trailing comment text after the last visible marker so the
        // comment styling remains contiguous across the rendered slice.
        if chunk_start < render_end {
            chunks.push(RenderChunk::new(
                &line_text[chunk_start..render_end],
                default_style.overlay(comment_style),
            ));
        }

        // If the scan found no visible markers, still return the comment text
        // with the ordinary comment style instead of leaving the region plain.
        if chunks.is_empty() {
            chunks.push(RenderChunk::new(
                &line_text[render_start..render_end],
                default_style.overlay(comment_style),
            ));
        }

        chunks
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
    !matches!(before, Some(ch) if is_word_char(ch)) && !matches!(after, Some(ch) if is_word_char(ch))
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
    use crate::theme::{SyntaxTagStyles, Theme, ThemeKind, UiStyles};
    use std::collections::BTreeMap;
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
        let ui_styles = UiStyles::new(
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
            Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
            Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
            Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
            Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
            Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
            Style::new().fg(Color::ansi(13)).bg(Color::ansi(14)),
        );
        let mut syntax_map = BTreeMap::new();
        syntax_map.insert(
            Tag::parse("comment").expect("valid tag"),
            Style::new().fg(Color::ansi(20)),
        );

        Theme::new(
            "comment-only",
            ThemeKind::Ansi256,
            default_style,
            ui_styles,
            SyntaxTagStyles::new(syntax_map),
        )
    }

    fn marker_theme() -> Theme {
        let default_style = Style::new().fg(Color::ansi(15)).bg(Color::ansi(30));
        let ui_styles = UiStyles::new(
            Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
            Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
            Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
            Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
            Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
            Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
            Style::new().fg(Color::ansi(13)).bg(Color::ansi(14)),
        );
        let mut syntax_map = BTreeMap::new();
        syntax_map.insert(
            Tag::parse("comment").expect("valid tag"),
            Style::new().fg(Color::ansi(20)),
        );
        syntax_map.insert(
            Tag::parse("comment.todo").expect("valid tag"),
            Style::new().fg(Color::ansi(31)),
        );
        syntax_map.insert(
            Tag::parse("comment.fixme").expect("valid tag"),
            Style::new().fg(Color::ansi(32)),
        );
        syntax_map.insert(
            Tag::parse("comment.bug").expect("valid tag"),
            Style::new().fg(Color::ansi(33)),
        );
        syntax_map.insert(
            Tag::parse("comment.note").expect("valid tag"),
            Style::new().fg(Color::ansi(34)),
        );

        Theme::new(
            "marker-demo",
            ThemeKind::Ansi256,
            default_style,
            ui_styles,
            SyntaxTagStyles::new(syntax_map),
        )
    }

    #[test]
    fn find_todo_matches_requires_standalone_case_sensitive_markers() {
        let markers = imbl::Vector::from_iter(
            ["TODO", "FIXME", "BUG", "NOTE"].into_iter().map(SmolStr::new),
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
        let expected_comment_style = theme
            .default_style()
            .overlay(theme.syntax_style_for_tag(&Tag::parse("comment").expect("valid tag")));
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
        let line = render_data
            .get_line(0)
            .expect("rendered line should exist");
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
        let expected_default_style = theme.default_style();
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
        assert!(line.iter().all(|chunk| chunk.style == expected_default_style));
    }
}
