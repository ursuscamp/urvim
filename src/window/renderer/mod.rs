use super::*;
use crate::config::WrapMode;

/// Snapshot of the viewport state used when rendering a buffer view.
#[derive(Debug, Clone, Copy)]
pub struct BufferRenderState {
    /// Current cursor position.
    pub cursor: Cursor,
    /// Current scroll origin.
    pub scroll_offset: Position,
    /// Current wrapped-row offset.
    pub wrapped_row_offset: u16,
    /// Size of the rendered viewport.
    pub size: Size,
    /// Whether visual wrapping is enabled.
    pub wrap_enabled: bool,
    /// Wrap strategy used when wrapping is enabled.
    pub wrap_mode: WrapMode,
    /// Whether relative line numbers are shown in the gutter.
    pub relative_number: bool,
    /// Whether the viewport should be scrolled to keep the cursor visible.
    pub scroll_to_cursor: bool,
    /// Whether the active editor line should be highlighted.
    pub active_line_enabled: bool,
    /// Whether the window is in normal mode.
    pub is_normal_mode: bool,
    /// Whether syntax warmup should synchronously fill missing spans before rendering.
    pub syntax_warmup: bool,
}

/// Theme and style values used while rendering a window frame.
#[derive(Debug, Clone, Copy)]
pub struct WindowRenderTheme {
    /// Style used for the gutter background and text.
    pub gutter_style: Style,
    /// Default background style for the content area.
    pub default_style: Style,
    /// Optional style for the active gutter row.
    pub active_gutter_style: Option<Style>,
    /// Optional style for the active content row.
    pub active_line_style: Option<Style>,
    /// Style used for added diff markers in the gutter.
    pub diff_added_gutter_style: Style,
    /// Style used for deleted diff markers in the gutter.
    pub diff_deleted_gutter_style: Style,
    /// Style used for modified diff markers in the gutter.
    pub diff_modified_gutter_style: Style,
}

/// Renders a buffer view into the supplied screen region.
///
/// This contains the shared window rendering pipeline so higher-level callers
/// can reuse the same gutter, wrapping, cursor, and active-line behavior.
pub fn render_buffer_view(
    screen: &mut Screen,
    origin: Position,
    buffer_view: &mut BufferView,
    render_data: &mut RenderData,
    theme: WindowRenderTheme,
    state: &mut BufferRenderState,
) {
    buffer_view.prune_yank_flash(std::time::Instant::now());
    state.cursor = buffer_view.cursor();
    state.scroll_offset = buffer_view.scroll_offset();
    state.wrapped_row_offset = buffer_view.wrapped_row_offset();

    let total_lines = buffer_view.line_count();
    let diagnostic_sign_width = diagnostic_sign_width_for_buffer(buffer_view.buffer_id_opt());
    let gutter_width = Gutter::new_with_style(0, state.size.rows, total_lines, theme.gutter_style)
        .with_diagnostic_sign_width(diagnostic_sign_width)
        .with_diff_sign_width(diff_sign_width_for_buffer(buffer_view.buffer_id_opt()))
        .with_fold_sign_width(FOLD_SIGN_WIDTH)
        .calculate_width();

    // Resolve scrolling before building the gutter so line numbers and
    // visible content are derived from the same viewport.
    if state.scroll_to_cursor {
        buffer_view.scroll_to_cursor_with_wrap(
            state.size,
            gutter_width,
            state.wrap_enabled,
            state.wrap_mode,
        );
        state.cursor = buffer_view.cursor();
        state.scroll_offset = buffer_view.scroll_offset();
        state.wrapped_row_offset = buffer_view.wrapped_row_offset();
    }
    let start_line = state.scroll_offset.row as usize;

    // Create gutter with the finalized viewport state.
    let mut gutter =
        Gutter::new_with_style(start_line, state.size.rows, total_lines, theme.gutter_style)
            .with_diagnostic_sign_width(diagnostic_sign_width)
            .with_diff_sign_width(diff_sign_width_for_buffer(buffer_view.buffer_id_opt()))
            .with_fold_sign_width(FOLD_SIGN_WIDTH);

    // Render buffer content offset by gutter width.
    let content_origin = Position::new(origin.row, origin.col + gutter_width);
    let content_size = Size::new(
        state.size.rows,
        state.size.cols.saturating_sub(gutter_width),
    );
    screen.fill_region(
        content_origin.row,
        content_origin.col,
        content_size.rows,
        content_size.cols,
        theme.default_style,
    );

    *render_data = buffer_view.build_render_data_with_options(
        content_size,
        theme.default_style,
        state.wrap_enabled,
        state.wrap_mode,
        state.syntax_warmup,
    );

    let active_cursor_row = if state.active_line_enabled {
        render_data
            .cursor_screen_position(state.cursor)
            .map(|position| position.row as usize)
    } else {
        None
    };

    gutter.render_for_render_data(
        screen,
        origin,
        render_data,
        GutterRenderState {
            cursor_line: state.cursor.line,
            relative_number: state.relative_number,
            active_screen_row: active_cursor_row,
            active_line_style: theme.active_gutter_style,
            diff_markers: buffer_view
                .with_buffer(|buffer| {
                    buffer.diff_markers_for_visible_rows(start_line, state.size.rows as usize)
                })
                .unwrap_or_else(|| vec![None; state.size.rows as usize]),
            diff_sign_width: diff_sign_width_for_buffer(buffer_view.buffer_id_opt()),
            fold_sign_width: FOLD_SIGN_WIDTH,
            diff_added_sign_style: theme.diff_added_gutter_style,
            diff_deleted_sign_style: theme.diff_deleted_gutter_style,
            diff_modified_sign_style: theme.diff_modified_gutter_style,
            diagnostic_severities: visible_diagnostic_severities(
                buffer_view.buffer_id_opt(),
                start_line,
                state.size.rows as usize,
            ),
            diagnostic_sign_width,
        },
    );

    if state.active_line_enabled
        && state.is_normal_mode
        && let Some(cursor_row) = active_cursor_row
        && let Some(active_line_style) = theme.active_line_style
    {
        render_data.set_line_base_style(cursor_row, active_line_style);
    }

    render_data.render(screen, content_origin, content_size, theme.default_style);
    buffer_view.mark_visual_generation_rendered();
}
