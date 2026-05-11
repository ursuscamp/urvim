//! Layout module.
//!
//! This module provides the `Layout` root container, which owns a binary split
//! tree of pane-hosted window groups, routes split-management actions, and renders
//! a footer status bar below the active editor region.

mod command_line;
mod confirmation;
mod dialogs;
mod geometry;
mod hover;
mod lsp;
mod lsp_rename;
mod node;
mod picker;
mod render;
mod session;
mod tree;

use self::dialogs::Dialogs;
use crate::action::ActionResult;
use crate::background::{JobEvent, JobKind, JobManager};
use crate::editor::{Action, ModeKind};
use crate::screen::Screen;
use crate::status_bar::StatusBar;
use crate::terminal::CursorStyle;
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
    dialogs: Dialogs,
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
            dialogs: Dialogs::default(),
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
            .dialogs
            .colorscheme_picker
            .as_ref()
            .and_then(|picker| picker.cursor())
        {
            return Some(position);
        }

        if let Some(position) = self
            .dialogs
            .grep_picker
            .as_ref()
            .and_then(|picker| picker.cursor())
        {
            return Some(position);
        }

        if let Some(position) = self
            .dialogs
            .code_actions_picker
            .as_ref()
            .and_then(|picker| picker.cursor())
        {
            return Some(position);
        }

        if let Some(position) = self
            .dialogs
            .doc_symbols_picker
            .as_ref()
            .and_then(|picker| picker.cursor())
        {
            return Some(position);
        }

        if let Some(position) = self
            .dialogs
            .workspace_symbols_picker
            .as_ref()
            .and_then(|picker| picker.cursor())
        {
            return Some(position);
        }

        if let Some(position) = self
            .dialogs
            .references_picker
            .as_ref()
            .and_then(|picker| picker.cursor())
        {
            return Some(position);
        }

        if let Some(position) = self
            .dialogs
            .file_picker
            .as_ref()
            .and_then(|picker| picker.cursor())
        {
            return Some(position);
        }

        if let Some(position) = self
            .dialogs
            .lsp_rename_prompt
            .as_ref()
            .and_then(|prompt| prompt.cursor())
        {
            return Some(position);
        }

        if let Some(position) = self.dialogs.command_line.cursor() {
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
                    if matches!(event.kind(), JobKind::LspRename(_)) {
                        self.dispatch_lsp_job_event(event);
                    } else {
                        self.dispatch_job_event(event);
                    }
                    accepted_redraw = true;
                }
            }
        }

        accepted_redraw
    }

    /// Processes workspace file-operation notifications.
    pub fn process_workspace_file_operations(&mut self) -> bool {
        let mut accepted_redraw = false;

        while let Some(event) = crate::globals::take_workspace_file_operation_notification() {
            match event {
                crate::globals::WorkspaceFileOperationNotification::Create { .. } => {
                    accepted_redraw = true;
                }
                crate::globals::WorkspaceFileOperationNotification::Rename { .. } => {
                    accepted_redraw = true;
                }
                crate::globals::WorkspaceFileOperationNotification::Delete {
                    buffer_id, ..
                } => {
                    if let Some(buffer_id) = buffer_id
                        && self.close_buffer_tabs(buffer_id)
                    {
                        self.prune_empty_panes();
                        accepted_redraw = true;
                    }
                }
            }
        }

        accepted_redraw
    }
}

impl Layout {
    /// Closes all open dialogs and overlays.
    pub(super) fn close_all_dialogs(&mut self) {
        self.dialogs.close_all();
    }

    /// Dispatches a unified intent through the root layout.
    pub fn dispatch_intent(&mut self, intent: &Intent) -> bool {
        match intent {
            Intent::Action(action) => self.dispatch_action(action),
            Intent::Command(command) => self.dispatch_command(command),
        }
    }

    fn dispatch_command(&mut self, command: &Command) -> bool {
        if !matches!(command, Command::EnqueueNotification { .. }) {
            self.close_hover();
            self.close_diagnostic_hover();
        }

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
            Command::LspCodeActions => {
                if self.lsp_code_actions_supported() {
                    self.open_lsp_code_actions_picker();
                }
                true
            }
            Command::LspApplyCodeAction { buffer_id, action } => self
                .execute_lsp_code_action(*buffer_id, action.clone())
                .map_or_else(
                    |error| {
                        crate::notify_error!("LSP code action failed: {}", error);
                        true
                    },
                    |_| true,
                ),
            Command::OpenDocumentSymbolsPicker => {
                self.open_doc_symbols_picker();
                true
            }
            Command::OpenWorkspaceSymbolsPicker => {
                self.open_workspace_symbols_picker();
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
            Command::WriteAll => self.execute_write_all().map_or_else(
                |error| {
                    crate::notify_error!("Write all failed: {}", error);
                    true
                },
                |_| true,
            ),
            Command::LspHover => {
                if !self.lsp_hover_supported() {
                    return true;
                }
                self.execute_lsp_hover().map_or_else(
                    |error| {
                        crate::notify_error!("LSP hover failed: {}", error);
                        true
                    },
                    |_| true,
                )
            }
            Command::LspDefinition => {
                if !self.lsp_definition_supported() {
                    return true;
                }
                self.execute_lsp_definition().map_or_else(
                    |error| {
                        crate::notify_error!("LSP definition failed: {}", error);
                        true
                    },
                    |_| true,
                )
            }
            Command::LspReferences => {
                if !self.lsp_references_supported() {
                    return true;
                }
                self.open_lsp_references_picker();
                true
            }
            Command::LspPreviousDiagnostic => self.execute_lsp_previous_diagnostic().map_or_else(
                |error| {
                    crate::notify_error!("LSP diagnostic navigation failed: {}", error);
                    true
                },
                |_| true,
            ),
            Command::LspNextDiagnostic => self.execute_lsp_next_diagnostic().map_or_else(
                |error| {
                    crate::notify_error!("LSP diagnostic navigation failed: {}", error);
                    true
                },
                |_| true,
            ),
            Command::LspPreviousErrorDiagnostic => {
                self.execute_lsp_previous_error_diagnostic().map_or_else(
                    |error| {
                        crate::notify_error!("LSP diagnostic navigation failed: {}", error);
                        true
                    },
                    |_| true,
                )
            }
            Command::LspNextErrorDiagnostic => {
                self.execute_lsp_next_error_diagnostic().map_or_else(
                    |error| {
                        crate::notify_error!("LSP diagnostic navigation failed: {}", error);
                        true
                    },
                    |_| true,
                )
            }
            Command::LspRenamePrompt => {
                if self.lsp_rename_supported() {
                    self.open_lsp_rename_prompt();
                }
                true
            }
            Command::LspRename(name) => {
                if !self.lsp_rename_supported() {
                    return true;
                }
                self.execute_lsp_rename(name.clone()).map_or_else(
                    |error| {
                        crate::notify_error!("LSP rename failed: {}", error);
                        true
                    },
                    |_| true,
                )
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
                let modified_count =
                    crate::globals::with_buffer_pool(|pool| pool.modified_buffer_count());
                if modified_count > 0 {
                    let suffix = if modified_count == 1 { "" } else { "s" };
                    self.open_confirmation_box(
                        format!("Quit without saving {} buffer{}?", modified_count, suffix),
                        Command::Quit,
                    );
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
        self.close_hover();
        self.close_diagnostic_hover();
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

        let hover = self.route_hover_ui_event(event);
        if hover.handled() {
            return hover;
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
                } else if self.lsp_rename_prompt_is_open() {
                    self.handle_lsp_rename_event(event)
                } else if self.command_line_should_capture_events() {
                    self.handle_command_line_key(key)
                } else {
                    UiEventResult::NotHandled
                }
            }
            UiEvent::Paste(text) => {
                if self.confirmation_box_is_open() {
                    self.handle_confirmation_box_event(event)
                } else if self.lsp_rename_prompt_is_open() {
                    self.handle_lsp_rename_event(event)
                } else if self.command_line_should_capture_events() {
                    self.handle_command_line_paste(text.as_str())
                } else {
                    UiEventResult::NotHandled
                }
            }
            UiEvent::Resize(_, _) => UiEventResult::NotHandled,
        }
    }

    fn route_hover_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        let (result, should_close) = {
            if let Some(hover) = self.diagnostic_hover_mut() {
                let mut ctx = crate::ui::UiContext;
                let result = hover.handle_ui_event(event, &mut ctx);
                let should_close = result.handled() && !hover.is_open();
                (result, should_close)
            } else {
                let Some(hover) = self.hover_mut() else {
                    return UiEventResult::NotHandled;
                };

                let mut ctx = crate::ui::UiContext;
                let result = hover.handle_ui_event(event, &mut ctx);
                let should_close = result.handled() && !hover.is_open();
                (result, should_close)
            }
        };

        if should_close {
            if self.diagnostic_hover_is_open() {
                self.close_diagnostic_hover();
            } else {
                self.close_hover();
            }
        }

        result
    }

    fn route_picker_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        if self.colorscheme_picker_is_open() {
            return self.handle_colorscheme_picker_event(event);
        }

        if self.code_actions_picker_is_open() {
            return self.handle_code_actions_picker_event(event);
        }

        if self.workspace_symbols_picker_is_open() {
            return self.handle_workspace_symbols_picker_event(event);
        }

        if self.references_picker_is_open() {
            return self.handle_references_picker_event(event);
        }

        if self.doc_symbols_picker_is_open() {
            return self.handle_doc_symbols_picker_event(event);
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

    fn lsp_hover_supported(&mut self) -> bool {
        let buffer_id = self.active_buffer_view().buffer_id();
        crate::globals::try_with_lsp_runtime_mut(|runtime| runtime.buffer_supports_hover(buffer_id))
            .unwrap_or(false)
    }

    fn editor_cursor_position(&self) -> Option<Position> {
        let pane_region = self.pane_region(self.focused_pane)?;
        let mut pos = self.active_window_group().visual_cursor()?;
        pos.row = pos.row.saturating_add(pane_region.origin.row);
        pos.col = pos.col.saturating_add(pane_region.origin.col);
        Some(pos)
    }

    fn lsp_definition_supported(&mut self) -> bool {
        let buffer_id = self.active_buffer_view().buffer_id();
        crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.buffer_supports_definition(buffer_id)
        })
        .unwrap_or(false)
    }

    fn lsp_code_actions_supported(&mut self) -> bool {
        let buffer_id = self.active_buffer_view().buffer_id();
        crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.buffer_supports_code_actions(buffer_id)
        })
        .unwrap_or(false)
    }

    fn lsp_references_supported(&mut self) -> bool {
        let buffer_id = self.active_buffer_view().buffer_id();
        crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.buffer_supports_references(buffer_id)
        })
        .unwrap_or(false)
    }

    fn lsp_rename_supported(&mut self) -> bool {
        let buffer_id = self.active_buffer_view().buffer_id();
        crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.buffer_supports_rename(buffer_id)
        })
        .unwrap_or(false)
    }

    fn execute_lsp_code_action(
        &mut self,
        buffer_id: crate::buffer::BufferId,
        action: crate::lsp::runtime::CodeActionApplication,
    ) -> Result<(), String> {
        crate::globals::with_lsp_runtime_mut(|runtime| runtime.apply_code_action(buffer_id, action))
            .ok_or_else(|| "LSP runtime is not available".to_string())?
    }
}

#[cfg(test)]
mod tests;
