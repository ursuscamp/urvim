//! Tab group module.
//!
//! This module provides the `TabGroup` container, which owns multiple windows,
//! renders a horizontal tab bar, and routes actions to the active window.

use crate::action::ActionResult;
use crate::buffer::Buffer;
use crate::editor::Action;
use crate::globals;
use crate::screen::Screen;
use crate::terminal::Style;
use crate::widget::Widget;
use crate::window::{BufferView, Position, Size, Window};
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

    /// Returns the active window.
    pub fn active_window(&self) -> &Window {
        &self.tabs[self.active_tab_index()]
    }

    /// Returns the active window mutably.
    pub fn active_window_mut(&mut self) -> &mut Window {
        let index = self.active_tab_index();
        &mut self.tabs[index]
    }

    /// Returns the active buffer view.
    pub fn active_buffer_view(&self) -> &BufferView {
        self.active_window().buffer_view()
    }

    /// Returns the active buffer view mutably.
    pub fn active_buffer_view_mut(&mut self) -> &mut BufferView {
        self.active_window_mut().buffer_view_mut()
    }

    /// Renders the tab group.
    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.normalize_state();

        if size.rows == 0 {
            return;
        }

        self.ensure_active_visible(size.cols as usize);
        let active_index = self.active_tab_index();

        self.render_tab_bar(screen, origin, size.cols, active_index);

        let content_origin = Position::new(origin.row + 1, origin.col);
        let content_rows = size.rows.saturating_sub(1);
        let content_size = Size::new(content_rows, size.cols);
        self.tabs[active_index].render(screen, content_origin, content_size);
    }

    /// Returns the cursor position for the active tab, offset by the tab bar row.
    pub fn visual_cursor(&self) -> Option<Position> {
        let mut pos = self.active_window().visual_cursor()?;
        pos.row += 1;
        Some(pos)
    }

    fn normalize_state(&mut self) {
        if self.tabs.is_empty() {
            let buffer_id = crate::globals::with_buffer_pool(|pool| pool.create_buffer());
            self.tabs.push(Window::from_buffer_id(buffer_id));
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
    ) {
        let cols = cols as usize;
        if cols == 0 {
            return;
        }

        let layout = self.compute_layout(self.tab_bar_start, cols, active_index);
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
            if available >= 2 {
                let label = self.tab_label(active_index);
                let clipped = self.clip_to_width(&label, available.saturating_sub(2) as usize);
                let entry = format!(" {} ", clipped);
                screen.write_string(origin.row, current_col, active_style, &entry);
                if self.tabs[active_index].buffer_view().is_modified() {
                    let marker_col =
                        current_col + 1 + UnicodeWidthStr::width(clipped.as_str()) as u16;
                    let marker_style = active_style.accent(modified_style);
                    screen.write_string(origin.row, marker_col, marker_style, "*");
                }
            }
        } else {
            for index in layout.start..layout.end {
                if current_col >= content_limit {
                    break;
                }

                let is_active = index == active_index;
                let style = if is_active { active_style } else { base_style };
                let available = content_limit.saturating_sub(current_col);
                if available < 2 {
                    break;
                }

                let label = self.tab_label(index);
                let clipped = self.clip_to_width(&label, available.saturating_sub(2) as usize);
                let entry = format!(" {} ", clipped);
                screen.write_string(origin.row, current_col, style, &entry);
                if self.tabs[index].buffer_view().is_modified() {
                    let marker_col =
                        current_col + 1 + UnicodeWidthStr::width(clipped.as_str()) as u16;
                    let marker_style = style.accent(modified_style);
                    screen.write_string(origin.row, marker_col, marker_style, "*");
                }
                current_col += UnicodeWidthStr::width(entry.as_str()) as u16;
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

    fn compute_layout(&self, start: usize, cols: usize, active_index: usize) -> TabBarLayout {
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
                let width = self.tab_entry_width(end);
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

    fn ensure_active_visible(&mut self, cols: usize) {
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

            let layout = self.compute_layout(self.tab_bar_start, cols, active_index);
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

    fn tab_entry_width(&self, index: usize) -> usize {
        let label = self.tab_label(index);
        UnicodeWidthStr::width(label.as_str()) + 2
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
        match action {
            Action::PreviousTab => {
                self.handle_previous_tab(1);
                ActionResult::Handled
            }
            Action::NextTab => {
                self.handle_next_tab(1);
                ActionResult::Handled
            }
            Action::Count(count, inner) => match inner.as_ref() {
                Action::PreviousTab => {
                    self.handle_previous_tab(*count);
                    ActionResult::Handled
                }
                Action::NextTab => {
                    self.handle_next_tab(*count);
                    ActionResult::Handled
                }
                _ => self.active_window_mut().process_action(action),
            },
            _ => self.active_window_mut().process_action(action),
        }
    }
}

#[cfg(test)]
mod tests;
