//! Layout module.
//!
//! This module provides the `Layout` root container, which owns the top-level
//! terminal region and delegates rendering and actions to the active child
//! `TabGroup` and footer status bar.

use crate::action::ActionResult;
use crate::editor::{Action, ModeKind};
use crate::screen::Screen;
use crate::status_bar::{StatusBar, StatusBarContext};
use crate::tab_group::TabGroup;
use crate::widget::Widget;
use crate::window::{BufferView, Position, Size};
use std::path::PathBuf;

/// Root layout container for urvim.
///
/// The layout owns the top-level terminal region, forwards the editor content
/// area to the tab group, and renders a footer status bar.
#[derive(Debug)]
pub struct Layout {
    tab_group: TabGroup,
    status_bar: StatusBar,
    mode_kind: ModeKind,
    origin: Position,
    size: Size,
}

impl Layout {
    /// Creates a layout from an existing tab group and initial mode kind.
    pub fn new(tab_group: TabGroup, mode_kind: ModeKind) -> Self {
        Self {
            tab_group,
            status_bar: StatusBar::new(),
            mode_kind,
            origin: Position::default(),
            size: Size::default(),
        }
    }

    /// Creates a layout from CLI file paths.
    pub fn from_paths(paths: &[PathBuf]) -> Self {
        Self::new(TabGroup::from_paths(paths), ModeKind::Normal)
    }

    /// Returns the active tab group.
    pub fn tab_group(&self) -> &TabGroup {
        &self.tab_group
    }

    /// Returns the active tab group mutably.
    pub fn tab_group_mut(&mut self) -> &mut TabGroup {
        &mut self.tab_group
    }

    /// Returns the current layout mode kind.
    pub fn mode_kind(&self) -> ModeKind {
        self.mode_kind
    }

    /// Returns the current layout mode label.
    pub fn mode_label(&self) -> &'static str {
        self.mode_kind.label()
    }

    /// Updates the current layout mode kind.
    pub fn set_mode_kind(&mut self, mode_kind: ModeKind) {
        self.mode_kind = mode_kind;
    }

    /// Returns the last rendered layout origin.
    pub fn origin(&self) -> Position {
        self.origin
    }

    /// Returns the last rendered layout size.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Returns the active buffer view from the child tab group.
    pub fn active_buffer_view(&self) -> &BufferView {
        self.tab_group.active_buffer_view()
    }

    /// Returns the active buffer view from the child tab group mutably.
    pub fn active_buffer_view_mut(&mut self) -> &mut BufferView {
        self.tab_group.active_buffer_view_mut()
    }

    /// Returns the visual cursor for the active child.
    pub fn visual_cursor(&self) -> Option<Position> {
        if self.size.rows <= 2 {
            return None;
        }

        self.tab_group.visual_cursor()
    }

    /// Renders the layout and forwards the available region to the child tab group.
    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.origin = origin;
        self.size = size;

        if size.rows == 0 {
            return;
        }

        let content_rows = size.rows.saturating_sub(1);
        let content_size = Size::new(content_rows, size.cols);
        self.tab_group.render(screen, origin, content_size);

        let buffer_view = self.active_buffer_view();
        let buffer = buffer_view.buffer();
        let buffer_name = buffer
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        let cursor = buffer_view.cursor();
        let context = StatusBarContext {
            mode_label: self.mode_label(),
            buffer_name: buffer_name.as_str(),
            cursor_line: cursor.line,
            cursor_byte_col: cursor.col,
            line_count: buffer.line_count(),
        };

        let footer_origin = Position::new(origin.row.saturating_add(content_rows), origin.col);
        self.status_bar
            .render(screen, footer_origin, Size::new(1, size.cols), &context);
    }
}

impl Widget for Layout {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        self.tab_group.process_action(action)
    }
}

#[cfg(test)]
mod tests;
