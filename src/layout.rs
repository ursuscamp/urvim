//! Layout module.
//!
//! This module provides the `Layout` root container, which owns the top-level
//! terminal region and delegates rendering and actions to the active child
//! `TabGroup`.

use crate::action::ActionResult;
use crate::editor::Action;
use crate::screen::Screen;
use crate::tab_group::TabGroup;
use crate::widget::Widget;
use crate::window::{BufferView, Position, Size};
use std::path::PathBuf;

/// Root layout container for urvim.
///
/// The layout owns the top-level terminal region and, in this first stage,
/// forwards that region to a single tab group child.
#[derive(Debug)]
pub struct Layout {
    tab_group: TabGroup,
    origin: Position,
    size: Size,
}

impl Layout {
    /// Creates a layout from an existing tab group.
    pub fn new(tab_group: TabGroup) -> Self {
        Self {
            tab_group,
            origin: Position::default(),
            size: Size::default(),
        }
    }

    /// Creates a layout from CLI file paths.
    pub fn from_paths(paths: &[PathBuf]) -> Self {
        Self::new(TabGroup::from_paths(paths))
    }

    /// Returns the active tab group.
    pub fn tab_group(&self) -> &TabGroup {
        &self.tab_group
    }

    /// Returns the active tab group mutably.
    pub fn tab_group_mut(&mut self) -> &mut TabGroup {
        &mut self.tab_group
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
        self.tab_group.visual_cursor()
    }

    /// Renders the layout and forwards the available region to the child tab group.
    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.origin = origin;
        self.size = size;
        self.tab_group.render(screen, origin, size);
    }
}

impl Widget for Layout {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        self.tab_group.process_action(action)
    }
}

#[cfg(test)]
mod tests;
