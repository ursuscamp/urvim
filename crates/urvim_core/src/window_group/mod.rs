//! Window group module.
//!
//! This module provides the `WindowGroup` container, which owns multiple windows,
//! renders a horizontal tab bar, and routes actions to the active window.

use crate::action::ActionResult;
use crate::buffer::{Buffer, BufferId, Cursor};
use crate::editor::EditorAction;
use crate::globals;
use crate::icon::FiletypeIcon;
use crate::jumplist::JumpList;
use crate::screen::Screen;
use crate::ui::text_width::{ClipSide, clip_text};
use crate::window::{BufferView, Position, Size, Window};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;
use unicode_width::UnicodeWidthStr;
use urvim_syntax::builtin_syntax_registry;
use urvim_terminal::CursorStyle;
use urvim_terminal::Style;

mod session;

#[derive(Debug)]
struct TabBarLayout {
    start: usize,
    end: usize,
    left_arrow: bool,
    right_arrow: bool,
}

#[derive(Clone, Copy, Debug)]
struct TabEntryRenderOptions {
    style: Style,
    modified_style: Style,
    available: u16,
    nerdfont_enabled: bool,
    clip_label: bool,
}

/// Root window-group container for urvim.
///
/// A window group owns multiple editor windows, keeps track of the active tab,
/// and renders a tab bar above the active window content.
#[derive(Debug)]
pub struct WindowGroup {
    tabs: Vec<Window>,
    active_tab: usize,
    tab_bar_start: usize,
    jumplist: RefCell<JumpList>,
}

impl WindowGroup {
    /// Creates a new window group from windows.
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

    /// Creates a new window group from buffers.
    pub fn from_buffers(buffers: Vec<Buffer>) -> Self {
        Self::new(buffers.into_iter().map(Window::new).collect())
    }

    /// Loads a window group from CLI file paths.
    pub fn from_paths(paths: &[PathBuf]) -> Self {
        let mut tabs = Vec::new();

        for path in paths {
            match crate::globals::with_buffer_pool(|pool| pool.open_buffer(path)) {
                Ok(buffer_id) => {
                    tracing::info!("Opened file: {:?}", path);
                    tabs.push(Window::from_buffer_id(buffer_id));
                }
                Err(error) => {
                    crate::notify_error!("Failed to open file {:?}: {}", path, error);
                }
            }
        }

        Self::new(tabs)
    }

    /// Loads a window group from CLI file arguments with optional initial cursor positions.
    pub fn from_cli_files(files: &[crate::cli::CliFileSpec]) -> Self {
        let mut tabs: Vec<Window> = Vec::new();

        for file in files {
            match crate::globals::with_buffer_pool(|pool| pool.open_buffer(&file.path)) {
                Ok(buffer_id) => {
                    tracing::info!("Opened file: {:?}", file.path);
                    if let Some(window) = tabs
                        .iter_mut()
                        .find(|window| window.buffer_view().buffer_id() == buffer_id)
                    {
                        if let Some(cursor) = file.cursor {
                            window.set_cursor_synced(cursor);
                        }
                    } else {
                        let mut window = Window::from_buffer_id(buffer_id);
                        if let Some(cursor) = file.cursor {
                            window.set_cursor_synced(cursor);
                        }
                        tabs.push(window);
                    }
                }
                Err(error) => {
                    crate::notify_error!("Failed to open file {:?}: {}", file.path, error);
                }
            }
        }

        Self::new(tabs)
    }

    /// Returns the active tab index.
    pub fn active_tab_index(&self) -> usize {
        self.active_tab.min(self.tabs.len().saturating_sub(1))
    }

    /// Returns true when the window group has no live tabs.
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

    /// Records the active cursor position in the window-group jumplist.
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

    /// Returns the buffer identifiers for every tab in this window group.
    pub fn buffer_ids(&self) -> Vec<BufferId> {
        self.tabs
            .iter()
            .map(|window| window.buffer_view().buffer_id())
            .collect()
    }

    /// Closes the active tab and returns true when the window group becomes empty.
    pub fn close_active_tab(&mut self) -> bool {
        if self.tabs.is_empty() {
            return true;
        }

        let index = self.active_tab_index();
        self.tabs.remove(index);
        self.normalize_state();
        crate::session::mark_dirty();
        self.tabs.is_empty()
    }

    /// Closes every tab that shows `buffer_id` and returns true when any tab was removed.
    pub fn close_buffer_tab(&mut self, buffer_id: BufferId) -> bool {
        let before = self.tabs.len();
        self.tabs
            .retain(|window| window.buffer_view().buffer_id() != buffer_id);
        if self.tabs.len() == before {
            return false;
        }

        self.normalize_state();
        crate::session::mark_dirty();
        true
    }

    /// Returns and clears any repeat-text suffix produced by the active window.
    pub fn take_pending_repeat_suffix(&mut self) -> Option<String> {
        self.active_window_mut().take_pending_repeat_suffix()
    }

    /// Clears the active window's yank flash once it expires.
    pub fn prune_expired_yank_flash(&mut self, now: Instant) -> bool {
        if self.tabs.is_empty() {
            return false;
        }

        self.active_window_mut()
            .buffer_view_mut()
            .prune_yank_flash(now)
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

    /// Activates the tab for `buffer_id`, or opens a new tab for it when absent.
    pub fn activate_or_open_buffer(&mut self, buffer_id: BufferId) {
        let index = self
            .tab_index_for_buffer_id(buffer_id)
            .unwrap_or_else(|| self.open_buffer_tab(buffer_id));
        self.active_tab = index;
        crate::session::mark_dirty();
    }

    /// Opens a new unnamed buffer in a new tab and activates it.
    pub fn open_unnamed_buffer_tab(&mut self) -> BufferId {
        let buffer_id = crate::globals::with_buffer_pool(|pool| pool.create_buffer());
        self.active_tab = self.open_buffer_tab(buffer_id);
        crate::session::mark_dirty();
        buffer_id
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
        crate::session::mark_dirty();
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

    /// Moves backward in the window-group jumplist, restoring the selected tab.
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

    /// Moves forward in the window-group jumplist, restoring the selected tab.
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

    /// Renders the window group.
    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.normalize_state();

        if self.tabs.is_empty() || size.rows == 0 {
            return;
        }

        let nerdfont_enabled = crate::icon::nerdfont_enabled();
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
                            theme.resolve_name_with_default("ui.tab.inactive"),
                            theme.resolve_name_with_default("ui.tab.active"),
                            theme.resolve_name_with_default("ui.tab.scroll_indicator"),
                            theme.resolve_name_with_default("ui.status_bar.modified_marker"),
                        )
                    })
                    .unwrap_or_else(|| {
                        let base_style = Style::new()
                            .bg(urvim_terminal::Color::ansi(237))
                            .fg(urvim_terminal::Color::ansi(250));
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
                Position::new(origin.row, current_col),
                active_index,
                TabEntryRenderOptions {
                    style: active_style,
                    modified_style,
                    available,
                    nerdfont_enabled,
                    clip_label: true,
                },
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
                    Position::new(origin.row, current_col),
                    index,
                    TabEntryRenderOptions {
                        style,
                        modified_style,
                        available,
                        nerdfont_enabled,
                        clip_label: false,
                    },
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

    fn tab_metadata(&self, index: usize) -> Option<urvim_syntax::SyntaxMetadata> {
        let syntax_name = self.tabs[index].buffer_view().syntax_name();
        builtin_syntax_registry()
            .ok()
            .and_then(|registry| registry.metadata(&syntax_name))
    }

    fn render_tab_entry(
        &self,
        screen: &mut Screen,
        origin: Position,
        index: usize,
        options: TabEntryRenderOptions,
    ) -> u16 {
        if options.available == 0 {
            return 0;
        }

        let label = self.tab_label(index);
        let metadata = self.tab_metadata(index);
        let glyph = FiletypeIcon::from_metadata(metadata.as_ref(), options.nerdfont_enabled);
        let glyph_width = glyph
            .as_ref()
            .map(|glyph| UnicodeWidthStr::width(glyph.glyph.as_str()))
            .unwrap_or(0);
        let prefix_width = if glyph.is_some() { glyph_width + 2 } else { 1 };
        let label_width_budget = if glyph.is_some() {
            usize::from(options.available).saturating_sub(glyph_width + 3)
        } else {
            usize::from(options.available).saturating_sub(2)
        };
        let rendered_label = if options.clip_label {
            clip_text(&label, label_width_budget, ClipSide::Start).text
        } else {
            label.clone()
        };
        let rendered_label_width = UnicodeWidthStr::width(rendered_label.as_str());

        screen.write_string(origin.row, origin.col, options.style, " ");
        let mut current_col = origin.col + 1;

        if let Some(glyph) = glyph {
            screen.write_string(
                origin.row,
                current_col,
                options.style.accent(glyph.style),
                glyph.glyph.as_str(),
            );
            current_col += glyph_width as u16;
            screen.write_string(origin.row, current_col, options.style, " ");
            current_col += 1;
        }

        screen.write_string(
            origin.row,
            current_col,
            options.style,
            rendered_label.as_str(),
        );
        current_col += rendered_label_width as u16;
        screen.write_string(origin.row, current_col, options.style, " ");

        if self.tabs[index].buffer_view().is_modified() {
            let marker_col = origin.col + prefix_width as u16 + rendered_label_width as u16;
            let marker_style = options.style.accent(options.modified_style);
            screen.write_string(origin.row, marker_col, marker_style, "*");
        }

        prefix_width as u16 + rendered_label_width as u16 + 1
    }

    /// Switches backward through tabs by `count` positions.
    pub fn previous_tab(&mut self, count: usize) {
        self.move_tabs(count, -1);
    }

    /// Switches forward through tabs by `count` positions.
    pub fn next_tab(&mut self, count: usize) {
        self.move_tabs(count, 1);
    }
}

impl WindowGroup {
    /// Dispatches an editor action to the active tab/window.
    pub fn dispatch_action(&mut self, action: &EditorAction) -> ActionResult {
        let before = self.active_cursor_snapshot();
        let result = match action.kind.as_ref() {
            Some(crate::editor::EditorOperation::JumpBackward) => {
                self.jump_list_back();
                ActionResult::Handled
            }
            Some(crate::editor::EditorOperation::JumpForward) => {
                self.jump_list_forward();
                ActionResult::Handled
            }
            Some(crate::editor::EditorOperation::Count(count, inner)) => {
                match inner.kind.as_ref() {
                    Some(crate::editor::EditorOperation::JumpBackward) => {
                        self.jump_back_count(*count);
                        ActionResult::Handled
                    }
                    Some(crate::editor::EditorOperation::JumpForward) => {
                        self.jump_forward_count(*count);
                        ActionResult::Handled
                    }
                    _ => self.active_window_mut().dispatch_action(action),
                }
            }
            _ => self.active_window_mut().dispatch_action(action),
        };

        if result == ActionResult::Handled && self.should_record_cursor_position(action) {
            let after = self.active_cursor_snapshot();
            self.record_cursor_after_action(before, after);
        }

        result
    }
}

impl WindowGroup {
    fn should_record_cursor_position(&self, action: &EditorAction) -> bool {
        match action.kind.as_ref() {
            Some(crate::editor::EditorOperation::JumpBackward)
            | Some(crate::editor::EditorOperation::JumpForward) => false,
            Some(crate::editor::EditorOperation::Count(_, inner)) => {
                self.should_record_cursor_position(inner)
            }
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests;
