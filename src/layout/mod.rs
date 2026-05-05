//! Layout module.
//!
//! This module provides the `Layout` root container, which owns a binary split
//! tree of pane-hosted window groups, routes split-management actions, and renders
//! a footer status bar below the active editor region.

mod colorscheme_picker;
mod command_line;
mod confirmation;
mod geometry;
mod grep_picker;
mod node;
mod picker;
mod render;
mod session;
mod tree;

use self::command_line::CommandLineState;
use crate::action::ActionResult;
use crate::background::{JobEvent, JobManager};
use crate::editor::{Action, ModeKind};
use crate::screen::Screen;
use crate::status_bar::StatusBar;
use crate::terminal::CursorStyle;
use crate::ui::colorscheme_picker::ColorschemePickerWidget;
use crate::ui::confirmation_box::ConfirmationBox;
use crate::ui::file_picker::FilePickerWidget;
use crate::ui::grep_picker::GrepPickerWidget;
use crate::ui::{Command, Intent, UiEvent, UiEventResult};
use crate::window::{BufferView, Position, Size};
use std::path::PathBuf;
use std::sync::Arc;

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
    command_line: CommandLineState,
    command_line_open: bool,
    colorscheme_picker: Option<ColorschemePickerWidget>,
    file_picker: Option<FilePickerWidget>,
    grep_picker: Option<GrepPickerWidget>,
    confirmation_box: Option<ConfirmationBox>,
    jobs: Arc<JobManager>,
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
            command_line: CommandLineState::new(),
            command_line_open: false,
            colorscheme_picker: None,
            file_picker: None,
            grep_picker: None,
            confirmation_box: None,
            jobs: Arc::new(JobManager::new()),
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

    /// Returns true when the layout still has session state to persist.
    pub fn can_save_session(&self) -> bool {
        self.root.is_some()
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
        if let Some(position) = self
            .colorscheme_picker
            .as_ref()
            .and_then(|picker| picker.cursor())
        {
            return Some(position);
        }

        if let Some(position) = self.grep_picker.as_ref().and_then(|picker| picker.cursor()) {
            return Some(position);
        }

        if let Some(position) = self.file_picker.as_ref().and_then(|picker| picker.cursor()) {
            return Some(position);
        }

        if let Some(position) = self.command_line.cursor() {
            return Some(position);
        }

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

    /// Processes picker-owned background jobs.
    pub fn process_background_jobs(&mut self) -> bool {
        let mut accepted_redraw = false;

        while let Some(event) = self.jobs.poll_event() {
            match event {
                JobEvent::Started { .. } => {}
                event @ JobEvent::Chunk { .. }
                | event @ JobEvent::Completed { .. }
                | event @ JobEvent::Failed { .. } => {
                    self.dispatch_job_event(event);
                    accepted_redraw = true;
                }
            }
        }

        accepted_redraw
    }
}

impl Layout {
    /// Closes all open pickers.
    pub(super) fn close_all_pickers(&mut self) {
        self.close_colorscheme_picker();
        self.close_file_picker();
        self.close_grep_picker();
    }

    /// Dispatches a unified intent through the root layout.
    pub fn dispatch_intent(&mut self, intent: &Intent) -> bool {
        match intent {
            Intent::Action(action) => self.dispatch_action(action),
            Intent::Command(command) => self.dispatch_command(command),
        }
    }

    fn dispatch_command(&mut self, command: &Command) -> bool {
        match command {
            Command::EnqueueNotification { level, message } => {
                crate::globals::enqueue_notification(*level, message.clone())
            }
            Command::OpenCommandLine => {
                self.open_command_line();
                true
            }
            Command::OpenColorschemePicker => {
                self.open_colorscheme_picker();
                true
            }
            Command::OpenFilePicker => {
                self.open_file_picker();
                true
            }
            Command::OpenGrepPicker => {
                self.open_grep_picker();
                true
            }
            Command::OpenFile(path) => {
                match crate::globals::with_buffer_pool(|pool| pool.open_buffer(path)) {
                    Ok(buffer_id) => {
                        self.active_window_group_mut()
                            .activate_or_open_buffer(buffer_id);
                        true
                    }
                    Err(error) => {
                        crate::notify_error!("Failed to open file {:?}: {}", path, error);
                        true
                    }
                }
            }
            Command::OpenFileAtCursor(path, cursor) => {
                match crate::globals::with_buffer_pool(|pool| pool.open_buffer(path)) {
                    Ok(buffer_id) => {
                        let window_group = self.active_window_group_mut();
                        window_group.activate_or_open_buffer(buffer_id);
                        window_group.active_window_mut().set_cursor_synced(*cursor);
                        true
                    }
                    Err(error) => {
                        crate::notify_error!("Failed to open file {:?}: {}", path, error);
                        true
                    }
                }
            }
            Command::ResizePaneLeft(count) => {
                self.resize_counted_pane(*count, SplitAxis::Vertical, ResizeDirection::Left)
            }
            Command::ResizePaneRight(count) => {
                self.resize_counted_pane(*count, SplitAxis::Vertical, ResizeDirection::Right)
            }
            Command::ResizePaneUp(count) => {
                self.resize_counted_pane(*count, SplitAxis::Horizontal, ResizeDirection::Up)
            }
            Command::ResizePaneDown(count) => {
                self.resize_counted_pane(*count, SplitAxis::Horizontal, ResizeDirection::Down)
            }
            Command::EqualizeSplits => self.equalize_splits(),
            Command::ToggleWrap => {
                if self.should_exit() {
                    false
                } else {
                    self.active_window_group_mut()
                        .active_window_mut()
                        .toggle_wrap();
                    true
                }
            }
            Command::SplitVertical => self.split_focused_pane(SplitAxis::Vertical),
            Command::SplitHorizontal => self.split_focused_pane(SplitAxis::Horizontal),
            Command::FocusPaneLeft => self.move_focus(geometry::FocusDirection::Left),
            Command::FocusPaneDown => self.move_focus(geometry::FocusDirection::Down),
            Command::FocusPaneUp => self.move_focus(geometry::FocusDirection::Up),
            Command::FocusPaneRight => self.move_focus(geometry::FocusDirection::Right),
            Command::ClosePane => self.close_focused_pane(),
            Command::TryQuit => {
                if self.has_modified_buffers() {
                    self.open_confirmation_box("Quit without saving?", Command::Quit);
                    true
                } else {
                    self.close_confirmation_box();
                    self.root = None;
                    true
                }
            }
            Command::Quit => {
                self.close_confirmation_box();
                self.root = None;
                true
            }
        }
    }

    /// Dispatches an action intent through the layout tree.
    pub fn dispatch_action(&mut self, action: &Action) -> bool {
        self.prune_empty_panes();
        match action.kind.as_ref() {
            _ => {
                if self.should_exit() {
                    false
                } else {
                    let handled = self.active_window_group_mut().dispatch_action(action)
                        == ActionResult::Handled;
                    if handled && self.active_window_group().is_empty() {
                        self.close_focused_pane();
                    }
                    handled
                }
            }
        }
    }

    /// Routes a UI event with overlay-first precedence.
    pub fn route_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        if matches!(event, UiEvent::Tick) {
            let picker = self.route_picker_ui_event(event);
            let picker_handled = picker.handled();
            let overlay = if picker_handled {
                UiEventResult::NotHandled
            } else {
                self.route_overlay_ui_event(event)
            };
            let overlay_handled = overlay.handled();
            let base = self.route_base_ui_event(event);
            let base_handled = base.handled();

            let mut intents = picker.into_intents();
            intents.extend(overlay.into_intents());
            intents.extend(base.into_intents());
            if picker_handled || overlay_handled || base_handled {
                return UiEventResult::Handled(intents);
            }

            return UiEventResult::NotHandled;
        }

        let picker = self.route_picker_ui_event(event);
        if picker.handled() {
            return picker;
        }

        let overlay = self.route_overlay_ui_event(event);
        if overlay.handled() {
            return overlay;
        }

        self.route_base_ui_event(event)
    }

    fn route_overlay_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        match event {
            UiEvent::Tick => {
                if crate::globals::prune_notifications() {
                    UiEventResult::Handled(Vec::new())
                } else {
                    UiEventResult::NotHandled
                }
            }
            UiEvent::Key(key) => {
                if self.confirmation_box_is_open() {
                    self.handle_confirmation_box_event(event)
                } else if self.command_line_should_capture_events() {
                    self.handle_command_line_key(key)
                } else {
                    UiEventResult::NotHandled
                }
            }
            UiEvent::Paste(text) => {
                if self.confirmation_box_is_open() {
                    self.handle_confirmation_box_event(event)
                } else if self.command_line_should_capture_events() {
                    self.handle_command_line_paste(text.as_str())
                } else {
                    UiEventResult::NotHandled
                }
            }
            UiEvent::Resize(_, _) => UiEventResult::NotHandled,
        }
    }

    fn route_picker_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        if self.colorscheme_picker_is_open() {
            return self.handle_colorscheme_picker_event(event);
        }

        if self.grep_picker_is_open() {
            return self.handle_grep_picker_event(event);
        }

        if self.file_picker_is_open() {
            return self.handle_file_picker_event(event);
        }

        UiEventResult::NotHandled
    }

    fn route_base_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        match event {
            UiEvent::Tick => {
                if self.prune_expired_yank_flashes() {
                    UiEventResult::Handled(Vec::new())
                } else {
                    UiEventResult::NotHandled
                }
            }
            UiEvent::Key(_) | UiEvent::Paste(_) | UiEvent::Resize(_, _) => {
                UiEventResult::NotHandled
            }
        }
    }

    fn resize_counted_pane(
        &mut self,
        count: usize,
        axis: SplitAxis,
        direction: ResizeDirection,
    ) -> bool {
        let mut handled = false;
        for _ in 0..count {
            handled |= self.resize_focused_pane(axis, direction);
        }

        handled
    }
}

#[cfg(test)]
mod tests;
