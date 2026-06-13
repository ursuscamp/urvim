//! Layout rendering and focus navigation helpers.

use super::Layout;
use super::geometry::FocusDirection;
use super::geometry::PaneRegion;
use super::node::{LayoutNode, SplitAxis, SplitSize};
use crate::editor::ModeKind;
use crate::globals;
use crate::notification;
use crate::screen::Screen;
use crate::status_bar::StatusBarContext;
use crate::terminal::Style;
use crate::ui::floating_window::{FloatingAnchor, FloatingWindowFrame, FloatingWindowFrameLabel};
use crate::ui::{UiContext, UiRect};
use crate::widget::Widget;
use crate::window::{Position, Size};

#[derive(Clone, Copy, Default)]
struct BorderCell {
    horizontal: bool,
    vertical: bool,
}

impl Layout {
    pub(super) fn move_focus(&mut self, direction: FocusDirection) -> bool {
        let regions = self.pane_regions();
        let Some(current) = regions
            .iter()
            .copied()
            .find(|region| region.id == self.focused_pane)
        else {
            return false;
        };

        let candidate = match direction {
            FocusDirection::Left => regions
                .iter()
                .copied()
                .filter(|region| region.id != current.id)
                .filter(|region| region.right() <= current.left())
                .filter(|region| region.vertical_overlap(current) > 0)
                .min_by_key(|region| {
                    (
                        current.left().saturating_sub(region.right()),
                        current.top().abs_diff(region.top()),
                    )
                }),
            FocusDirection::Down => regions
                .iter()
                .copied()
                .filter(|region| region.id != current.id)
                .filter(|region| region.top() >= current.bottom())
                .filter(|region| region.horizontal_overlap(current) > 0)
                .min_by_key(|region| {
                    (
                        region.top().saturating_sub(current.bottom()),
                        current.left().abs_diff(region.left()),
                    )
                }),
            FocusDirection::Up => regions
                .iter()
                .copied()
                .filter(|region| region.id != current.id)
                .filter(|region| region.bottom() <= current.top())
                .filter(|region| region.horizontal_overlap(current) > 0)
                .min_by_key(|region| {
                    (
                        current.top().saturating_sub(region.bottom()),
                        current.left().abs_diff(region.left()),
                    )
                }),
            FocusDirection::Right => regions
                .iter()
                .copied()
                .filter(|region| region.id != current.id)
                .filter(|region| region.left() >= current.right())
                .filter(|region| region.vertical_overlap(current) > 0)
                .min_by_key(|region| {
                    (
                        region.left().saturating_sub(current.right()),
                        current.top().abs_diff(region.top()),
                    )
                }),
        };

        if let Some(candidate) = candidate {
            let target = self
                .resolve_directional_focus_target(current.id, candidate.id)
                .unwrap_or(candidate.id);
            return self.focus_pane(target);
        }

        false
    }

    pub(super) fn render_layout(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.prune_empty_panes();
        self.origin = origin;
        self.size = size;

        if size.rows == 0 {
            return;
        }

        let content_rows = size.rows.saturating_sub(1);
        let content_size = Size::new(content_rows, size.cols);

        if let Some(root) = self.root.as_mut() {
            Self::render_node(root, screen, origin, content_size);
        }

        if self.should_exit() {
            return;
        }

        self.render_split_borders(screen, origin, content_size);

        let buffer_view = self.active_buffer_view();
        let buffer_name = buffer_view
            .file_name()
            .unwrap_or_else(|| "Untitled".to_string());
        let syntax_name = buffer_view.syntax_name();
        let syntax_label = buffer_view.syntax_label();
        let cursor = buffer_view.cursor();
        let diagnostic_counts = globals::with_diagnostics_store(|store| {
            store.diagnostic_counts_for_buffer(buffer_view.buffer_id())
        })
        .unwrap_or_default();
        let context = StatusBarContext {
            mode_label: self.mode_label(),
            modified: buffer_view.is_modified(),
            syntax_name: syntax_name.as_str(),
            syntax_label: syntax_label.as_str(),
            buffer_name: buffer_name.as_str(),
            cursor_line: cursor.line,
            cursor_byte_col: cursor.col,
            line_count: buffer_view.line_count(),
            diagnostic_counts,
        };

        let footer_origin = Position::new(origin.row.saturating_add(content_rows), origin.col);
        self.status_bar
            .render(screen, footer_origin, Size::new(1, size.cols), &context);

        notification::render_active_banner(screen, origin, size, std::time::Instant::now());
        let mut progress_widget = notification::ProgressWidget::new();
        progress_widget.render_widget(screen, UiRect::new(origin, size), &UiContext);
        self.render_buffer_picker_overlay(screen, origin, size);
        self.render_colorscheme_picker_overlay(screen, origin, size);
        self.render_code_actions_picker_overlay(screen, origin, size);
        self.render_workspace_symbols_picker_overlay(screen, origin, size);
        self.render_references_picker_overlay(screen, origin, size);
        self.render_doc_symbols_picker_overlay(screen, origin, size);
        self.render_grep_picker_overlay(screen, origin, size);
        self.render_file_picker_overlay(screen, origin, size);
        self.render_git_picker_overlay(screen, origin, size);
        self.render_command_line_overlay(screen, origin, size);
        self.render_completion_overlay(screen, origin, size);
        self.render_lsp_rename_overlay(screen, origin, size);
        self.render_confirmation_box_overlay(screen, origin, size);
        self.render_diagnostic_hover_overlay(screen, origin, size);
        self.render_hover_overlay(screen, origin, size);

        if self.inlay_hint_request_pending() {
            self.request_active_buffer_inlay_hints();
        }
    }

    fn render_buffer_picker_overlay(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        let Some(picker) = self.buffer_picker_mut() else {
            return;
        };

        let ctx = UiContext;
        let rect = UiRect::new(origin, size);
        picker.render_widget(screen, rect, &ctx);
    }

    fn render_colorscheme_picker_overlay(
        &mut self,
        screen: &mut Screen,
        origin: Position,
        size: Size,
    ) {
        let Some(picker) = self.colorscheme_picker_mut() else {
            return;
        };

        let ctx = UiContext;
        let rect = UiRect::new(origin, size);
        picker.render_widget(screen, rect, &ctx);
    }

    fn render_code_actions_picker_overlay(
        &mut self,
        screen: &mut Screen,
        origin: Position,
        size: Size,
    ) {
        let Some(picker) = self.code_actions_picker_mut() else {
            return;
        };

        let ctx = UiContext;
        let rect = UiRect::new(origin, size);
        picker.render_widget(screen, rect, &ctx);
    }

    fn render_doc_symbols_picker_overlay(
        &mut self,
        screen: &mut Screen,
        origin: Position,
        size: Size,
    ) {
        let Some(picker) = self.doc_symbols_picker_mut() else {
            return;
        };

        let ctx = UiContext;
        let rect = UiRect::new(origin, size);
        picker.render_widget(screen, rect, &ctx);
    }

    fn render_workspace_symbols_picker_overlay(
        &mut self,
        screen: &mut Screen,
        origin: Position,
        size: Size,
    ) {
        let Some(picker) = self.workspace_symbols_picker_mut() else {
            return;
        };

        let ctx = UiContext;
        let rect = UiRect::new(origin, size);
        picker.render_widget(screen, rect, &ctx);
    }

    fn render_references_picker_overlay(
        &mut self,
        screen: &mut Screen,
        origin: Position,
        size: Size,
    ) {
        let Some(picker) = self.references_picker_mut() else {
            return;
        };

        let ctx = UiContext;
        let rect = UiRect::new(origin, size);
        picker.render_widget(screen, rect, &ctx);
    }

    fn render_grep_picker_overlay(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        let Some(picker) = self.grep_picker_mut() else {
            return;
        };

        let ctx = UiContext;
        let rect = UiRect::new(origin, size);
        picker.render_widget(screen, rect, &ctx);
    }

    fn render_file_picker_overlay(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        let Some(picker) = self.file_picker_mut() else {
            return;
        };

        let ctx = UiContext;
        let rect = UiRect::new(origin, size);
        picker.render_widget(screen, rect, &ctx);
    }

    fn render_git_picker_overlay(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        let Some(picker) = self.git_picker_mut() else {
            return;
        };

        let ctx = UiContext;
        let rect = UiRect::new(origin, size);
        picker.render_widget(screen, rect, &ctx);
    }

    fn render_command_line_overlay(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        let Some(input) = self.command_line_input_widget_mut() else {
            return;
        };

        let border_style: Style = globals::with_active_theme(|theme| {
            theme
                .map(|theme| theme.resolve_name_with_default("ui.window.lines.border"))
                .unwrap_or_default()
        });
        let body_style: Style = globals::with_active_theme(|theme| {
            theme
                .map(|theme| theme.resolve_name_with_default("ui.window"))
                .unwrap_or_default()
        });

        let frame_cols = size.cols.min(55);
        let content_cols = frame_cols.saturating_sub(2);
        let frame =
            FloatingWindowFrame::resolve(origin, size, 1, content_cols, FloatingAnchor::Center);
        let Some(frame) = frame else {
            return;
        };

        frame.render_bordered_with_label(
            screen,
            border_style,
            body_style,
            Some(FloatingWindowFrameLabel::top_center("Command")),
        );
        let cursor = {
            input.set_text_style(body_style);
            input.render_widget(
                screen,
                UiRect::new(frame.content_origin, frame.content_size),
                &UiContext,
            );
            input.render_cursor()
        };

        if let Some(cursor) = cursor {
            self.dialogs
                .command_line
                .set_cursor(Some(Position::new(cursor.row, cursor.col)));
        } else {
            self.dialogs.command_line.set_cursor(None);
        }
    }

    fn render_completion_overlay(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        let cursor = self.editor_cursor_position();
        let Some(completion) = self.dialogs.completion_mut() else {
            return;
        };

        if let Some(cursor) = cursor {
            completion.set_cursor(Some(cursor));
        }

        let ctx = UiContext;
        completion.render_widget(screen, UiRect::new(origin, size), &ctx);
    }

    fn render_lsp_rename_overlay(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        let Some(prompt) = self.lsp_rename_prompt_mut() else {
            return;
        };

        let ctx = UiContext;
        prompt.render_widget(screen, UiRect::new(origin, size), &ctx);
    }

    fn render_confirmation_box_overlay(
        &mut self,
        screen: &mut Screen,
        origin: Position,
        size: Size,
    ) {
        let Some(prompt) = self.confirmation_box_mut() else {
            return;
        };

        let ctx = UiContext;
        prompt.render_widget(screen, UiRect::new(origin, size), &ctx);
    }

    fn render_hover_overlay(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        let Some(hover) = self.hover_mut() else {
            return;
        };

        let ctx = UiContext;
        hover.render_widget(screen, UiRect::new(origin, size), &ctx);
    }

    fn render_diagnostic_hover_overlay(
        &mut self,
        screen: &mut Screen,
        origin: Position,
        size: Size,
    ) {
        let Some(hover) = self.diagnostic_hover_mut() else {
            return;
        };

        let ctx = UiContext;
        hover.render_widget(screen, UiRect::new(origin, size), &ctx);
    }

    fn render_split_borders(&self, screen: &mut Screen, origin: Position, size: Size) {
        let regions = self.pane_regions();
        if regions.len() <= 1 || size.rows == 0 || size.cols == 0 {
            return;
        }

        let unicode_borders = crate::icon::unicode_borders_enabled();
        let mode = self.active_window_mode_kind();
        let border_style: Style = globals::with_active_theme(|theme| {
            theme
                .map(|theme| {
                    if mode == ModeKind::Resizing {
                        theme.resolve_name_with_default("ui.window.lines.resize")
                    } else {
                        theme.resolve_name_with_default("ui.window.lines.border")
                    }
                })
                .unwrap_or_default()
        });

        let mut cells =
            vec![BorderCell::default(); usize::from(size.rows) * usize::from(size.cols)];
        for region in &regions {
            self.mark_region_borders(&mut cells, origin, size, region, &regions);
        }

        for row_offset in 0..size.rows {
            for col_offset in 0..size.cols {
                let index = Self::border_index(size, row_offset, col_offset);
                let cell = cells[index];
                if !(cell.horizontal || cell.vertical) {
                    continue;
                }

                let glyph = Self::border_glyph(
                    unicode_borders,
                    &cells,
                    size,
                    origin,
                    row_offset,
                    col_offset,
                );
                screen.write_string(
                    origin.row + row_offset,
                    origin.col + col_offset,
                    border_style,
                    glyph,
                );
            }
        }
    }

    fn mark_region_borders(
        &self,
        cells: &mut [BorderCell],
        origin: Position,
        size: Size,
        region: &PaneRegion,
        regions: &[PaneRegion],
    ) {
        if region.size.rows == 0 || region.size.cols == 0 {
            return;
        }

        let right = region.right();
        let bottom = region.bottom();
        let border_col = right;
        let border_row = bottom;
        let content_row_end = origin.row.saturating_add(size.rows);
        let content_col_end = origin.col.saturating_add(size.cols);

        if border_col >= origin.col && border_col < content_col_end {
            for other in regions
                .iter()
                .filter(|other| other.id != region.id)
                .filter(|other| other.left() == right.saturating_add(1))
            {
                let top = region.top().max(other.top()).max(origin.row);
                let bottom = region.bottom().min(other.bottom()).min(content_row_end);
                for row in top..=bottom {
                    if let Some(cell) = self.border_cell_mut(cells, origin, size, row, border_col) {
                        cell.vertical = true;
                    }
                }
            }
        }

        if border_row >= origin.row && border_row < content_row_end {
            for other in regions
                .iter()
                .filter(|other| other.id != region.id)
                .filter(|other| other.top() == bottom.saturating_add(1))
            {
                let left = region.left().max(other.left()).max(origin.col);
                let right = region.right().min(other.right()).min(content_col_end);
                for col in left..=right {
                    if let Some(cell) = self.border_cell_mut(cells, origin, size, border_row, col) {
                        cell.horizontal = true;
                    }
                }
            }
        }
    }

    fn border_cell_mut<'a>(
        &self,
        cells: &'a mut [BorderCell],
        origin: Position,
        size: Size,
        row: u16,
        col: u16,
    ) -> Option<&'a mut BorderCell> {
        if row < origin.row
            || col < origin.col
            || row >= origin.row.saturating_add(size.rows)
            || col >= origin.col.saturating_add(size.cols)
        {
            return None;
        }

        let index = Self::border_index(size, row - origin.row, col - origin.col);
        cells.get_mut(index)
    }

    fn border_index(size: Size, row: u16, col: u16) -> usize {
        usize::from(row) * usize::from(size.cols) + usize::from(col)
    }

    fn border_glyph(
        unicode: bool,
        cells: &[BorderCell],
        size: Size,
        origin: Position,
        row: u16,
        col: u16,
    ) -> &'static str {
        let cell_index = Self::border_index(size, row, col);
        let cell = cells[cell_index];
        let north = Self::border_cell_occupied(cells, size, origin, row.saturating_sub(1), col);
        let south = Self::border_cell_occupied(cells, size, origin, row.saturating_add(1), col);
        let west = Self::border_cell_occupied(cells, size, origin, row, col.saturating_sub(1));
        let east = Self::border_cell_occupied(cells, size, origin, row, col.saturating_add(1));

        crate::icon::split_border_glyph(
            unicode,
            north,
            south,
            west,
            east,
            cell.vertical,
            cell.horizontal,
        )
    }

    fn border_cell_occupied(
        cells: &[BorderCell],
        size: Size,
        origin: Position,
        row: u16,
        col: u16,
    ) -> bool {
        if row < origin.row
            || col < origin.col
            || row >= origin.row.saturating_add(size.rows)
            || col >= origin.col.saturating_add(size.cols)
        {
            return false;
        }

        let index = Self::border_index(size, row - origin.row, col - origin.col);
        cells
            .get(index)
            .map(|cell| cell.horizontal || cell.vertical)
            .unwrap_or(false)
    }

    pub(super) fn render_node(
        node: &mut LayoutNode,
        screen: &mut Screen,
        origin: Position,
        size: Size,
    ) {
        match node {
            LayoutNode::Pane(pane) => pane.window_group.render(screen, origin, size),
            LayoutNode::Split(split) => {
                let (first_origin, first_size, second_origin, second_size) =
                    Self::split_regions(origin, size, split.axis, split.split_size);
                Self::render_node(&mut split.first, screen, first_origin, first_size);
                Self::render_node(&mut split.second, screen, second_origin, second_size);
            }
        }
    }

    pub(super) fn split_regions(
        origin: Position,
        size: Size,
        axis: SplitAxis,
        split_size: SplitSize,
    ) -> (Position, Size, Position, Size) {
        match axis {
            SplitAxis::Horizontal => {
                let separator = size.rows.min(1);
                let usable_rows = size.rows.saturating_sub(separator);
                let first_rows = Self::weighted_extent(
                    usable_rows,
                    split_size.first_weight(),
                    split_size.second_weight(),
                );
                let second_rows = usable_rows.saturating_sub(first_rows);
                (
                    origin,
                    Size::new(first_rows, size.cols),
                    Position::new(
                        origin
                            .row
                            .saturating_add(first_rows)
                            .saturating_add(separator),
                        origin.col,
                    ),
                    Size::new(second_rows, size.cols),
                )
            }
            SplitAxis::Vertical => {
                let separator = size.cols.min(1);
                let usable_cols = size.cols.saturating_sub(separator);
                let first_cols = Self::weighted_extent(
                    usable_cols,
                    split_size.first_weight(),
                    split_size.second_weight(),
                );
                let second_cols = usable_cols.saturating_sub(first_cols);
                (
                    origin,
                    Size::new(size.rows, first_cols),
                    Position::new(
                        origin.row,
                        origin
                            .col
                            .saturating_add(first_cols)
                            .saturating_add(separator),
                    ),
                    Size::new(size.rows, second_cols),
                )
            }
        }
    }

    fn weighted_extent(total: u16, first_weight: u16, second_weight: u16) -> u16 {
        let total = u32::from(total);
        let denominator = u32::from(first_weight.max(1)) + u32::from(second_weight.max(1));
        ((total * u32::from(first_weight.max(1))) / denominator) as u16
    }
}
