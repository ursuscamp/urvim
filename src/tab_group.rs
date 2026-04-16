//! Tab group module.
//!
//! This module provides the `TabGroup` container, which owns multiple windows,
//! renders a horizontal tab bar, and routes actions to the active window.

use crate::action::ActionResult;
use crate::buffer::{Buffer, BufferId, Cursor};
use crate::editor::Action;
use crate::globals;
use crate::jumplist::JumpList;
use crate::screen::Screen;
use crate::syntax::builtin_syntax_registry;
use crate::terminal::CursorStyle;
use crate::terminal::Style;
use crate::widget::Widget;
use crate::window::{BufferView, Position, Size, Window};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug)]
struct TabBarLayout {
    start: usize,
    end: usize,
    left_arrow: bool,
    right_arrow: bool,
}

/// Root tab-group container for urvim.
///
/// A tab group owns multiple editor windows, keeps track of the active tab,
/// and renders a tab bar above the active window content.
#[derive(Debug)]
pub struct TabGroup {
    tabs: Vec<Window>,
    active_tab: usize,
    tab_bar_start: usize,
    jumplist: RefCell<JumpList>,
}

impl TabGroup {
    /// Creates a new tab group from windows.
    pub fn new(mut tabs: Vec<Window>) -> Self {
        let mut seen_buffer_ids = HashSet::new();
        tabs.retain(|window| seen_buffer_ids.insert(window.buffer_view().buffer_id()));

        if tabs.is_empty() {
            let buffer_id = crate::globals::with_buffer_pool(|pool| pool.create_buffer());
            tabs.push(Window::from_buffer_id(buffer_id));
        }

        Self {
            tabs,
            active_tab: 0,
            tab_bar_start: 0,
            jumplist: RefCell::new(JumpList::new()),
        }
    }

    /// Creates a new tab group from buffers.
    pub fn from_buffers(buffers: Vec<Buffer>) -> Self {
        Self::new(buffers.into_iter().map(Window::new).collect())
    }

    /// Loads a tab group from CLI file paths.
    pub fn from_paths(paths: &[PathBuf]) -> Self {
        let mut tabs = Vec::new();

        for path in paths {
            match crate::globals::with_buffer_pool(|pool| pool.open_buffer(path)) {
                Ok(buffer_id) => {
                    tracing::info!("Opened file: {:?}", path);
                    tabs.push(Window::from_buffer_id(buffer_id));
                }
                Err(error) => {
                    tracing::warn!("Failed to open file {:?}: {}", path, error);
                }
            }
        }

        Self::new(tabs)
    }

    /// Returns the active tab index.
    pub fn active_tab_index(&self) -> usize {
        self.active_tab.min(self.tabs.len().saturating_sub(1))
    }

    /// Returns true when the tab group has no live tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// Returns the active window.
    pub fn active_window(&self) -> &Window {
        &self.tabs[self.active_tab_index()]
    }

    /// Returns the active window mutably.
    pub fn active_window_mut(&mut self) -> &mut Window {
        let index = self.active_tab_index();
        &mut self.tabs[index]
    }

    /// Returns the active window's current mode kind.
    pub fn active_window_mode_kind(&self) -> crate::editor::ModeKind {
        self.active_window().mode_kind()
    }

    /// Returns the active window's current mode label.
    pub fn active_window_mode_label(&self) -> &'static str {
        self.active_window().mode_label()
    }

    /// Returns the active window's current cursor style.
    pub fn active_window_cursor_style(&self) -> CursorStyle {
        self.active_window().cursor_style()
    }

    /// Records the active cursor position in the tab-group jumplist.
    pub fn record_cursor_position(&mut self) {
        let view = self.active_buffer_view();
        let buffer_id = view.buffer_id();
        let cursor = view.cursor();
        let cursor = view
            .with_buffer(|buffer| buffer.sync_cursor(cursor))
            .unwrap_or(cursor);
        self.jumplist.borrow_mut().record_cursor(buffer_id, cursor);
    }

    /// Returns the active buffer view.
    pub fn active_buffer_view(&self) -> &BufferView {
        self.active_window().buffer_view()
    }

    /// Returns the active buffer view mutably.
    pub fn active_buffer_view_mut(&mut self) -> &mut BufferView {
        self.active_window_mut().buffer_view_mut()
    }

    /// Closes the active tab and returns true when the tab group becomes empty.
    pub fn close_active_tab(&mut self) -> bool {
        if self.tabs.is_empty() {
            return true;
        }

        let index = self.active_tab_index();
        self.tabs.remove(index);
        self.normalize_state();
        self.tabs.is_empty()
    }

    /// Returns and clears any repeat-text suffix produced by the active window.
    pub fn take_pending_repeat_suffix(&mut self) -> Option<String> {
        self.active_window_mut().take_pending_repeat_suffix()
    }

    fn active_cursor_snapshot(&self) -> (BufferId, Cursor) {
        let view = self.active_buffer_view();
        (view.buffer_id(), view.cursor())
    }

    fn record_cursor_after_action(
        &mut self,
        before: (BufferId, Cursor),
        after: (BufferId, Cursor),
    ) {
        if before == after {
            return;
        }

        self.record_cursor_position();
    }

    fn active_buffer_exists(&self, buffer_id: BufferId) -> bool {
        globals::with_buffer_pool(|pool| pool.get(buffer_id).is_some())
    }

    fn tab_index_for_buffer_id(&self, buffer_id: BufferId) -> Option<usize> {
        self.tabs
            .iter()
            .position(|window| window.buffer_view().buffer_id() == buffer_id)
    }

    fn open_buffer_tab(&mut self, buffer_id: BufferId) -> usize {
        self.tabs.push(Window::from_buffer_id(buffer_id));
        self.tabs.len() - 1
    }

    fn activate_jump_target(&mut self, buffer_id: BufferId, cursor: Cursor) -> bool {
        if !self.active_buffer_exists(buffer_id) {
            return false;
        }

        let index = self
            .tab_index_for_buffer_id(buffer_id)
            .unwrap_or_else(|| self.open_buffer_tab(buffer_id));
        self.active_tab = index;
        self.active_window_mut().set_cursor_synced(cursor);
        let restored_cursor = self.active_buffer_view().cursor();
        self.jumplist
            .borrow_mut()
            .sync_current_cursor(restored_cursor);
        true
    }

    fn jump_back_count(&mut self, count: usize) -> bool {
        let mut handled = false;
        for _ in 0..count {
            handled = self.jump_list_back() || handled;
        }
        handled
    }

    fn jump_forward_count(&mut self, count: usize) -> bool {
        let mut handled = false;
        for _ in 0..count {
            handled = self.jump_list_forward() || handled;
        }
        handled
    }

    /// Moves backward in the tab-group jumplist, restoring the selected tab.
    pub fn jump_list_back(&mut self) -> bool {
        let Some((buffer_id, _cursor)) = self.jumplist.borrow().peek_back() else {
            return false;
        };
        if !self.active_buffer_exists(buffer_id) {
            return false;
        }

        let Some((buffer_id, cursor)) = self.jumplist.borrow_mut().jump_back() else {
            return false;
        };
        self.activate_jump_target(buffer_id, cursor)
    }

    /// Moves forward in the tab-group jumplist, restoring the selected tab.
    pub fn jump_list_forward(&mut self) -> bool {
        let Some((buffer_id, _cursor)) = self.jumplist.borrow().peek_forward() else {
            return false;
        };
        if !self.active_buffer_exists(buffer_id) {
            return false;
        }

        let Some((buffer_id, cursor)) = self.jumplist.borrow_mut().jump_forward() else {
            return false;
        };
        self.activate_jump_target(buffer_id, cursor)
    }

    /// Renders the tab group.
    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.normalize_state();

        if self.tabs.is_empty() || size.rows == 0 {
            return;
        }

        let nerdfont_enabled =
            globals::with_config(|config| config.nerdfont_enabled()).unwrap_or(false);
        self.ensure_active_visible(size.cols as usize, nerdfont_enabled);
        let active_index = self.active_tab_index();

        self.render_tab_bar(screen, origin, size.cols, active_index, nerdfont_enabled);

        let content_origin = Position::new(origin.row + 1, origin.col);
        let content_rows = size.rows.saturating_sub(1);
        let content_size = Size::new(content_rows, size.cols);
        self.tabs[active_index].render(screen, content_origin, content_size);
    }

    /// Returns the cursor position for the active tab, offset by the tab bar row.
    pub fn visual_cursor(&self) -> Option<Position> {
        if self.tabs.is_empty() {
            return None;
        }
        let mut pos = self.active_window().visual_cursor()?;
        pos.row += 1;
        Some(pos)
    }

    fn normalize_state(&mut self) {
        if self.tabs.is_empty() {
            self.active_tab = 0;
            self.tab_bar_start = 0;
            return;
        }

        if self.active_tab >= self.tabs.len() {
            self.active_tab = 0;
        }

        if self.tab_bar_start >= self.tabs.len() {
            self.tab_bar_start = self.active_tab;
        }
    }

    fn render_tab_bar(
        &self,
        screen: &mut Screen,
        origin: Position,
        cols: u16,
        active_index: usize,
        nerdfont_enabled: bool,
    ) {
        let cols = cols as usize;
        if cols == 0 {
            return;
        }

        let layout = self.compute_layout(self.tab_bar_start, cols, active_index, nerdfont_enabled);
        let (base_style, active_style, indicator_style, modified_style) =
            globals::with_active_theme(|theme| {
                theme
                    .map(|theme| {
                        (
                            theme.ui.tab_inactive,
                            theme.ui.tab_active,
                            theme.ui.tab_scroll_indicator,
                            theme.ui.modified_marker,
                        )
                    })
                    .unwrap_or_else(|| {
                        let base_style = Style::new()
                            .bg(crate::terminal::Color::ansi(237))
                            .fg(crate::terminal::Color::ansi(250));
                        let active_style = base_style.reverse().bold();
                        (base_style, active_style, active_style, active_style)
                    })
            });
        let content_end = cols.saturating_sub(layout.right_arrow as usize);

        screen.write_string(origin.row, origin.col, base_style, &" ".repeat(cols));

        let mut current_col = origin.col;
        if layout.left_arrow {
            screen.write_string(origin.row, current_col, indicator_style, "<");
            current_col += 1;
        }

        let content_limit = origin.col + content_end as u16;

        if layout.start == active_index && layout.end == layout.start {
            let available = content_limit.saturating_sub(current_col);
            self.render_tab_entry(
                screen,
                origin.row,
                current_col,
                active_style,
                modified_style,
                active_index,
                available,
                nerdfont_enabled,
                true,
            );
        } else {
            for index in layout.start..layout.end {
                if current_col >= content_limit {
                    break;
                }

                let is_active = index == active_index;
                let style = if is_active { active_style } else { base_style };
                let available = content_limit.saturating_sub(current_col);
                if usize::from(available) < self.tab_entry_width(index, nerdfont_enabled) {
                    break;
                }

                let width = self.render_tab_entry(
                    screen,
                    origin.row,
                    current_col,
                    style,
                    modified_style,
                    index,
                    available,
                    nerdfont_enabled,
                    false,
                );
                current_col += width;
            }
        }

        if layout.right_arrow {
            screen.write_string(
                origin.row,
                origin.col + cols as u16 - 1,
                indicator_style,
                ">",
            );
        }
    }

    fn compute_layout(
        &self,
        start: usize,
        cols: usize,
        active_index: usize,
        nerdfont_enabled: bool,
    ) -> TabBarLayout {
        let left_arrow = start > 0;
        let mut right_arrow = false;

        loop {
            let mut available = cols;
            if left_arrow && available > 0 {
                available -= 1;
            }
            if right_arrow && available > 0 {
                available -= 1;
            }

            let mut used = 0usize;
            let mut end = start;
            while end < self.tabs.len() {
                let width = self.tab_entry_width(end, nerdfont_enabled);
                if used + width > available {
                    break;
                }
                used += width;
                end += 1;
            }

            let active_visible = if active_index < start {
                false
            } else if active_index == start {
                true
            } else {
                active_index < end
            };

            let new_right_arrow = end < self.tabs.len();
            if active_visible && new_right_arrow == right_arrow {
                return TabBarLayout {
                    start,
                    end,
                    left_arrow,
                    right_arrow,
                };
            }

            right_arrow = new_right_arrow;

            if !active_visible {
                break;
            }
        }

        TabBarLayout {
            start,
            end: start,
            left_arrow,
            right_arrow,
        }
    }

    fn ensure_active_visible(&mut self, cols: usize, nerdfont_enabled: bool) {
        if self.tabs.len() <= 1 {
            self.tab_bar_start = 0;
            return;
        }

        self.tab_bar_start = self.tab_bar_start.min(self.tabs.len() - 1);
        let active_index = self.active_tab_index();

        loop {
            if active_index < self.tab_bar_start {
                self.tab_bar_start = active_index;
                continue;
            }

            let layout =
                self.compute_layout(self.tab_bar_start, cols, active_index, nerdfont_enabled);
            if active_index >= layout.end && self.tab_bar_start < active_index {
                self.tab_bar_start += 1;
                continue;
            }

            break;
        }
    }

    fn move_tabs(&mut self, count: usize, direction: isize) {
        let len = self.tabs.len();
        if len <= 1 {
            return;
        }

        let offset = count % len;
        if offset == 0 {
            return;
        }

        let current = self.active_tab as isize;
        let len = len as isize;
        let next = (current + direction * offset as isize).rem_euclid(len) as usize;
        self.active_tab = next;
    }

    fn tab_label(&self, index: usize) -> String {
        let window = &self.tabs[index];
        window
            .buffer_view()
            .file_name()
            .unwrap_or_else(|| "Untitled".to_string())
    }

    fn tab_entry_width(&self, index: usize, nerdfont_enabled: bool) -> usize {
        let label = self.tab_label(index);
        let base_width = UnicodeWidthStr::width(label.as_str()) + 2;
        let metadata = self.tab_metadata(index);
        let glyph_width = metadata
            .as_ref()
            .and_then(|metadata| metadata.glyph.as_deref())
            .map(UnicodeWidthStr::width)
            .unwrap_or(0);

        if nerdfont_enabled && glyph_width > 0 {
            base_width + glyph_width + 1
        } else {
            base_width
        }
    }

    fn tab_metadata(&self, index: usize) -> Option<crate::syntax::SyntaxMetadata> {
        let syntax_name = self.tabs[index].buffer_view().syntax_name();
        builtin_syntax_registry()
            .ok()
            .and_then(|registry| registry.metadata(&syntax_name))
    }

    fn render_tab_entry(
        &self,
        screen: &mut Screen,
        row: u16,
        col: u16,
        style: Style,
        modified_style: Style,
        index: usize,
        available: u16,
        nerdfont_enabled: bool,
        clip_label: bool,
    ) -> u16 {
        if available == 0 {
            return 0;
        }

        let label = self.tab_label(index);
        let metadata = self.tab_metadata(index);
        let glyph = if nerdfont_enabled {
            metadata
                .as_ref()
                .and_then(|metadata| metadata.glyph.as_deref())
        } else {
            None
        };
        let glyph_color = metadata.as_ref().and_then(|metadata| metadata.glyph_color);
        let glyph_width = glyph.map(UnicodeWidthStr::width).unwrap_or(0);
        let prefix_width = if glyph.is_some() { glyph_width + 2 } else { 1 };
        let label_width_budget = if glyph.is_some() {
            usize::from(available).saturating_sub(glyph_width + 3)
        } else {
            usize::from(available).saturating_sub(2)
        };
        let rendered_label = if clip_label {
            self.clip_to_width(&label, label_width_budget)
        } else {
            label.clone()
        };
        let rendered_label_width = UnicodeWidthStr::width(rendered_label.as_str());

        screen.write_string(row, col, style, " ");
        let mut current_col = col + 1;

        if let Some(glyph) = glyph {
            let glyph_style = glyph_color.map(|color| style.fg(color)).unwrap_or(style);
            screen.write_string(row, current_col, glyph_style, glyph);
            current_col += glyph_width as u16;
            screen.write_string(row, current_col, style, " ");
            current_col += 1;
        }

        screen.write_string(row, current_col, style, rendered_label.as_str());
        current_col += rendered_label_width as u16;
        screen.write_string(row, current_col, style, " ");

        if self.tabs[index].buffer_view().is_modified() {
            let marker_col = col + prefix_width as u16 + rendered_label_width as u16;
            let marker_style = style.accent(modified_style);
            screen.write_string(row, marker_col, marker_style, "*");
        }

        prefix_width as u16 + rendered_label_width as u16 + 1
    }

    fn clip_to_width(&self, text: &str, max_width: usize) -> String {
        if max_width == 0 {
            return String::new();
        }

        let mut result = String::new();
        let mut width = 0usize;
        for grapheme in text.graphemes(true) {
            let grapheme_width = UnicodeWidthStr::width(grapheme);
            if width + grapheme_width > max_width {
                break;
            }
            width += grapheme_width;
            result.push_str(grapheme);
        }

        result
    }

    fn handle_previous_tab(&mut self, count: usize) {
        self.move_tabs(count, -1);
    }

    fn handle_next_tab(&mut self, count: usize) {
        self.move_tabs(count, 1);
    }
}

impl Widget for TabGroup {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        let before = self.active_cursor_snapshot();
        let result = match action.kind.as_ref() {
            Some(crate::editor::ActionKind::PreviousTab) => {
                self.handle_previous_tab(1);
                ActionResult::Handled
            }
            Some(crate::editor::ActionKind::NextTab) => {
                self.handle_next_tab(1);
                ActionResult::Handled
            }
            Some(crate::editor::ActionKind::JumpBackward) => {
                self.jump_list_back();
                ActionResult::Handled
            }
            Some(crate::editor::ActionKind::JumpForward) => {
                self.jump_list_forward();
                ActionResult::Handled
            }
            Some(crate::editor::ActionKind::Count(count, inner)) => match inner.kind.as_ref() {
                Some(crate::editor::ActionKind::PreviousTab) => {
                    self.handle_previous_tab(*count);
                    ActionResult::Handled
                }
                Some(crate::editor::ActionKind::NextTab) => {
                    self.handle_next_tab(*count);
                    ActionResult::Handled
                }
                Some(crate::editor::ActionKind::JumpBackward) => {
                    self.jump_back_count(*count);
                    ActionResult::Handled
                }
                Some(crate::editor::ActionKind::JumpForward) => {
                    self.jump_forward_count(*count);
                    ActionResult::Handled
                }
                _ => self.active_window_mut().process_action(action),
            },
            _ => self.active_window_mut().process_action(action),
        };

        if result == ActionResult::Handled && self.should_record_cursor_position(action) {
            let after = self.active_cursor_snapshot();
            self.record_cursor_after_action(before, after);
        }

        result
    }
}

impl TabGroup {
    fn should_record_cursor_position(&self, action: &Action) -> bool {
        match action.kind.as_ref() {
            Some(crate::editor::ActionKind::JumpBackward)
            | Some(crate::editor::ActionKind::JumpForward)
            | Some(crate::editor::ActionKind::PreviousTab)
            | Some(crate::editor::ActionKind::NextTab) => false,
            Some(crate::editor::ActionKind::Count(_, inner)) => {
                self.should_record_cursor_position(inner)
            }
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests;
