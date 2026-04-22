//! Layout module.
//!
//! This module provides the `Layout` root container, which owns a binary split
//! tree of pane-hosted window groups, routes split-management actions, and renders
//! a footer status bar below the active editor region.

mod geometry;
mod node;
mod render;
mod tree;

use crate::action::ActionResult;
use crate::editor::{Action, ActionKind, ModeKind};
use crate::screen::Screen;
use crate::status_bar::StatusBar;
use crate::terminal::CursorStyle;
use crate::widget::Widget;
use crate::window::{BufferView, Position, Size};
use std::path::PathBuf;

use self::tree::ResizeDirection;
pub use node::{LayoutNode, PaneId, PaneNode, SplitAxis, SplitNode, SplitSize};

/// Root layout container for urvim.
///
/// The layout owns a binary split tree of panes, tracks the focused pane,
/// routes split-management actions, and renders a footer status bar beneath
/// the editor content area.
#[derive(Debug)]
pub struct Layout {
    root: Option<LayoutNode>,
    focused_pane: PaneId,
    next_pane_id: usize,
    status_bar: StatusBar,
    origin: Position,
    size: Size,
}

impl Layout {
    /// Creates a layout from an existing window group.
    pub fn new(window_group: crate::window_group::WindowGroup) -> Self {
        let focused_pane = PaneId(0);
        Self {
            root: Some(LayoutNode::Pane(PaneNode::new(focused_pane, window_group))),
            focused_pane,
            next_pane_id: 1,
            status_bar: StatusBar::new(),
            origin: Position::default(),
            size: Size::default(),
        }
    }

    /// Creates a layout from CLI file paths.
    pub fn from_paths(paths: &[PathBuf]) -> Self {
        Self::new(crate::window_group::WindowGroup::from_paths(paths))
    }

    /// Creates a layout from CLI file arguments with optional initial cursor positions.
    pub fn from_cli_files(files: &[crate::cli::CliFileSpec]) -> Self {
        Self::new(crate::window_group::WindowGroup::from_cli_files(files))
    }

    /// Returns true when the layout has no panes left to render.
    pub fn should_exit(&self) -> bool {
        self.root.is_none()
    }

    /// Returns the active window group for the focused pane.
    pub fn active_window_group(&self) -> &crate::window_group::WindowGroup {
        let root = self
            .root
            .as_ref()
            .expect("layout should contain a focused pane");
        Self::find_pane(root, self.focused_pane)
            .map(|pane| &pane.window_group)
            .expect("focused pane should exist")
    }

    /// Returns the active window group mutably for the focused pane.
    pub fn active_window_group_mut(&mut self) -> &mut crate::window_group::WindowGroup {
        let focused_pane = self.focused_pane;
        let root = self
            .root
            .as_mut()
            .expect("layout should contain a focused pane");
        Self::find_pane_mut(root, focused_pane)
            .map(|pane| &mut pane.window_group)
            .expect("focused pane should exist")
    }

    /// Returns the active window group.
    pub fn window_group(&self) -> &crate::window_group::WindowGroup {
        self.active_window_group()
    }

    /// Returns the active window group mutably.
    pub fn window_group_mut(&mut self) -> &mut crate::window_group::WindowGroup {
        self.active_window_group_mut()
    }

    /// Returns the current layout mode label.
    pub fn mode_label(&self) -> &'static str {
        self.active_window_group().active_window_mode_label()
    }

    /// Returns the current mode kind of the focused pane's active window.
    pub fn active_window_mode_kind(&self) -> ModeKind {
        self.active_window_group().active_window_mode_kind()
    }

    /// Returns the cursor style of the focused pane's active window.
    pub fn active_window_cursor_style(&self) -> CursorStyle {
        self.active_window_group().active_window_cursor_style()
    }

    /// Returns the last rendered layout origin.
    pub fn origin(&self) -> Position {
        self.origin
    }

    /// Returns the last rendered layout size.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Returns the active buffer view from the focused pane.
    pub fn active_buffer_view(&self) -> &BufferView {
        self.active_window_group().active_buffer_view()
    }

    /// Returns the active buffer view mutably from the focused pane.
    pub fn active_buffer_view_mut(&mut self) -> &mut BufferView {
        self.active_window_group_mut().active_buffer_view_mut()
    }

    /// Clears expired yank-flash highlights from all visible panes.
    pub fn prune_expired_yank_flashes(&mut self) -> bool {
        self.prune_expired_yank_flashes_at(std::time::Instant::now())
    }

    /// Returns and clears any repeat-text suffix produced by the active child window.
    pub fn take_pending_repeat_suffix(&mut self) -> Option<String> {
        if self.should_exit() {
            return None;
        }

        self.active_window_group_mut().take_pending_repeat_suffix()
    }

    /// Returns the visual cursor for the focused pane, if any.
    pub fn visual_cursor(&self) -> Option<Position> {
        let pane_region = self.pane_region(self.focused_pane)?;
        let mut pos = self.active_window_group().visual_cursor()?;
        pos.row = pos.row.saturating_add(pane_region.origin.row);
        pos.col = pos.col.saturating_add(pane_region.origin.col);
        Some(pos)
    }

    /// Renders the layout tree and footer status bar.
    pub fn render(&mut self, screen: &mut Screen, origin: Position, size: Size) {
        self.render_layout(screen, origin, size);
    }
}

impl Widget for Layout {
    fn process_action(&mut self, action: &Action) -> ActionResult {
        self.prune_empty_panes();
        let handled = match action.kind.as_ref() {
            Some(ActionKind::SplitVertical) => self.split_focused_pane(SplitAxis::Vertical),
            Some(ActionKind::SplitHorizontal) => self.split_focused_pane(SplitAxis::Horizontal),
            Some(ActionKind::FocusPaneLeft) => self.move_focus(geometry::FocusDirection::Left),
            Some(ActionKind::FocusPaneDown) => self.move_focus(geometry::FocusDirection::Down),
            Some(ActionKind::FocusPaneUp) => self.move_focus(geometry::FocusDirection::Up),
            Some(ActionKind::FocusPaneRight) => self.move_focus(geometry::FocusDirection::Right),
            Some(ActionKind::ResizePaneLeft) => {
                self.resize_focused_pane(SplitAxis::Vertical, ResizeDirection::Left)
            }
            Some(ActionKind::ResizePaneRight) => {
                self.resize_focused_pane(SplitAxis::Vertical, ResizeDirection::Right)
            }
            Some(ActionKind::ResizePaneUp) => {
                self.resize_focused_pane(SplitAxis::Horizontal, ResizeDirection::Up)
            }
            Some(ActionKind::ResizePaneDown) => {
                self.resize_focused_pane(SplitAxis::Horizontal, ResizeDirection::Down)
            }
            Some(ActionKind::Count(count, inner))
                if matches!(
                    inner.kind.as_ref(),
                    Some(ActionKind::ResizePaneLeft)
                        | Some(ActionKind::ResizePaneRight)
                        | Some(ActionKind::ResizePaneUp)
                        | Some(ActionKind::ResizePaneDown)
                ) =>
            {
                self.resize_counted_pane(*count, inner.as_ref())
            }
            Some(ActionKind::EqualizeSplits) => self.equalize_splits(),
            Some(ActionKind::ClosePane) => self.close_focused_pane(),
            _ => {
                if self.should_exit() {
                    false
                } else {
                    let handled = self.active_window_group_mut().process_action(action)
                        == ActionResult::Handled;
                    if handled && self.active_window_group().is_empty() {
                        self.close_focused_pane();
                    }
                    handled
                }
            }
        };

        if handled {
            ActionResult::Handled
        } else {
            ActionResult::NotHandled
        }
    }
}

impl Layout {
    fn resize_counted_pane(&mut self, count: usize, action: &Action) -> bool {
        let mut handled = false;
        for _ in 0..count {
            handled |= match action.kind.as_ref() {
                Some(ActionKind::ResizePaneLeft) => {
                    self.resize_focused_pane(SplitAxis::Vertical, ResizeDirection::Left)
                }
                Some(ActionKind::ResizePaneRight) => {
                    self.resize_focused_pane(SplitAxis::Vertical, ResizeDirection::Right)
                }
                Some(ActionKind::ResizePaneUp) => {
                    self.resize_focused_pane(SplitAxis::Horizontal, ResizeDirection::Up)
                }
                Some(ActionKind::ResizePaneDown) => {
                    self.resize_focused_pane(SplitAxis::Horizontal, ResizeDirection::Down)
                }
                _ => false,
            };
        }

        handled
    }
}

#[cfg(test)]
mod tests;
