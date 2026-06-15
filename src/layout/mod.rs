//! Layout module.
//!
//! This module provides the `Layout` root container, which owns a binary split
//! tree of pane-hosted window groups, routes split-management actions, and renders
//! a footer status bar below the active editor region.

mod command_line;
mod completion;
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
use crate::buffer::BufferId;
use crate::editor::{Action, ModeKind};
use crate::screen::Screen;
use crate::status_bar::StatusBar;
use crate::terminal::CursorStyle;
use crate::ui::{Command, Intent, UiEvent, UiEventResult};
use crate::window::{BufferView, Position, Size};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

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
    inlay_hints: InlayHintState,
    autocomplete: AutocompleteState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InlayHintRequestParams {
    pub buffer_id: BufferId,
    pub start_line: usize,
    pub syntax_generation: u64,
}

#[derive(Debug)]
pub(super) struct InFlightInlayHintRequest {
    pub params: InlayHintRequestParams,
    pub received_hints: bool,
}

#[derive(Debug)]
pub(super) enum InlayHintState {
    Idle,
    Pending,
    InFlight(InFlightInlayHintRequest),
}

#[derive(Debug)]
pub(super) struct AutocompleteState {
    pending_since: Option<Instant>,
    debounce: std::time::Duration,
}

impl Default for AutocompleteState {
    fn default() -> Self {
        Self {
            pending_since: None,
            debounce: std::time::Duration::from_millis(150),
        }
    }
}

impl AutocompleteState {
    fn schedule(&mut self, now: Instant) {
        self.pending_since = Some(now);
    }

    fn cancel(&mut self) {
        self.pending_since = None;
    }

    fn due(&self, now: Instant) -> bool {
        self.pending_since
            .is_some_and(|started| now.duration_since(started) >= self.debounce)
    }
}

impl InlayHintState {
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }
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
            inlay_hints: InlayHintState::Idle,
            autocomplete: AutocompleteState::default(),
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
        if let Some(position) = self.dialogs.visual_cursor() {
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

    /// Returns true when any visible pane has visual buffer changes not yet rendered.
    pub fn has_stale_visible_visuals(&self) -> bool {
        let Some(root) = self.root.as_ref() else {
            return false;
        };

        Self::node_has_stale_visible_visuals(root)
    }

    /// Allows the active inlay hint request to be submitted again.
    pub fn retry_inlay_hints(&mut self) {
        let buffer_id = self.active_buffer_view().buffer_id();
        if crate::globals::try_with_lsp_runtime_mut(|runtime| {
            runtime.buffer_has_active_progress(buffer_id)
        })
        .unwrap_or(true)
        {
            return;
        }

        self.inlay_hints = InlayHintState::Pending;
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
                    if !self.jobs.is_accepted(event.kind(), event.token()) {
                        continue;
                    }

                    if matches!(event.kind(), JobKind::Completion(_, _)) {
                        self.handle_completion_job_event(&event);
                        accepted_redraw = true;
                        continue;
                    }

                    if matches!(
                        event.kind(),
                        JobKind::LspRename(_) | JobKind::LspInlayHints(_)
                    ) {
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
            Command::OpenCompletion => {
                self.open_completion();
                true
            }
            Command::OpenBufferPicker => {
                self.open_buffer_picker();
                true
            }
            Command::OpenColorschemePicker => {
                self.open_colorscheme_picker();
                true
            }
            Command::OpenFiletypePicker => {
                self.open_filetype_picker();
                true
            }
            Command::OpenGitPicker => {
                self.open_git_picker();
                true
            }
            Command::GitPickerToggleStage(action) => self.execute_git_picker_toggle_stage(action),
            Command::GitPickerDiscard(action) => {
                self.open_git_picker_discard_confirmation(action);
                true
            }
            Command::GitPickerDiscardConfirmed(action) => self.execute_git_picker_discard(action),
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
            Command::SetBufferFiletype(buffer_id, filetype) => {
                self.execute_set_buffer_filetype(*buffer_id, filetype);
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
            Command::OverwriteBuffer(target) => {
                let buffer_id = (*target).unwrap_or_else(|| self.active_buffer_view().buffer_id());
                match crate::globals::with_buffer_pool(|pool| pool.save_buffer(buffer_id)) {
                    Ok(()) => {
                        let label = crate::globals::with_buffer(buffer_id, |buffer| {
                            buffer
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "Untitled".to_string())
                        })
                        .unwrap_or_else(|| "Untitled".to_string());
                        crate::globals::with_lsp_runtime_mut(|runtime| {
                            runtime.did_save_buffer(buffer_id)
                        });
                        crate::notify_info!("Saved {}", label);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
                        tracing::info!("Skipping save for unnamed buffer {:?}", buffer_id);
                    }
                    Err(error) => {
                        crate::notify_error!("Failed to save buffer {:?}: {}", buffer_id, error);
                    }
                }
                true
            }
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
            Command::ApplyCompletion(apply_completion) => self.apply_completion(apply_completion),
            Command::FocusBuffer(buffer_id) => self.focus_buffer(*buffer_id),
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

    fn execute_set_buffer_filetype(&mut self, buffer_id: Option<BufferId>, filetype: &str) {
        let Some(canonical) = crate::syntax::builtin_syntax_registry()
            .ok()
            .and_then(|registry| registry.resolve_label(filetype))
        else {
            crate::notify_error!("Unknown filetype: {}", filetype);
            return;
        };

        let target_buffer_id = buffer_id.unwrap_or_else(|| self.active_buffer_view().buffer_id());
        if crate::globals::with_buffer_mut(target_buffer_id, |buffer| {
            buffer.set_syntax_name(canonical.clone())
        })
        .is_none()
        {
            crate::notify_error!("Missing buffer: {:?}", target_buffer_id);
            return;
        }

        let label = crate::syntax::builtin_syntax_registry()
            .ok()
            .and_then(|registry| registry.display_name(canonical.as_str()))
            .unwrap_or(canonical)
            .to_string();
        crate::notify_info!("filetype: {}", label);
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
                    if handled {
                        self.request_inlay_hints_for_active_viewport();
                    }
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
            let overlay = self.route_overlay_ui_event(event);
            let overlay_handled = overlay.handled();
            let picker = self.route_picker_ui_event(event);
            let picker_handled = picker.handled();
            let base = self.route_base_ui_event(event);
            let base_handled = base.handled();

            let mut intents = overlay.into_intents();
            intents.extend(picker.into_intents());
            intents.extend(base.into_intents());
            if overlay_handled || picker_handled || base_handled {
                return UiEventResult::Handled(intents);
            }

            return UiEventResult::NotHandled;
        }

        let overlay = self.route_overlay_ui_event(event);
        if overlay.handled() {
            return overlay;
        }

        let picker = self.route_picker_ui_event(event);
        if picker.handled() {
            return picker;
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
                } else if self.dialogs.completion.is_some() {
                    self.handle_completion_event(event)
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
                } else if self.completion_is_open() {
                    self.handle_completion_event(event)
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
        if self.buffer_picker_is_open() {
            return self.handle_buffer_picker_event(event);
        }

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

        if self.git_picker_is_open() {
            return self.handle_git_picker_event(event);
        }

        if self.file_picker_is_open() {
            return self.handle_file_picker_event(event);
        }

        if self.filetype_picker_is_open() {
            return self.handle_filetype_picker_event(event);
        }

        UiEventResult::NotHandled
    }

    fn route_base_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        match event {
            UiEvent::Tick => {
                let autocomplete = self.maybe_fire_autocomplete(Instant::now());
                if self.prune_expired_yank_flashes() {
                    UiEventResult::Handled(Vec::new())
                } else if autocomplete {
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

    fn apply_completion(&mut self, apply_completion: &crate::ui::ApplyCompletion) -> bool {
        let (replacement, cursor_offset) =
            apply_completion_text(apply_completion.text.as_str(), apply_completion.format);
        let resolved_additional_text_edits = if apply_completion.additional_text_edits.is_empty() {
            apply_completion
                .lsp_completion_item
                .as_ref()
                .and_then(|item| self.resolve_completion_additional_text_edits(item))
                .unwrap_or_default()
        } else {
            apply_completion.additional_text_edits.clone()
        };

        let window = self.active_window_group_mut().active_window_mut();

        let next_cursor = window.buffer_view_mut().with_buffer_mut(|buffer| {
            buffer.apply_completion(
                apply_completion.range,
                replacement.as_str(),
                cursor_offset,
                resolved_additional_text_edits.as_slice(),
            )
        });

        if let Some(next_cursor) = next_cursor.flatten() {
            window.buffer_view_mut().set_cursor(next_cursor);
        }

        true
    }

    fn resolve_completion_additional_text_edits(
        &mut self,
        item: &serde_json::Value,
    ) -> Option<Vec<crate::ui::completion::CompletionTextEdit>> {
        let buffer_id = self.active_buffer_view().buffer_id();
        crate::globals::with_lsp_runtime_mut(|runtime| {
            runtime.resolve_completion_additional_text_edits(buffer_id, item)
        })
        .and_then(Result::ok)
        .flatten()
    }
}

fn apply_completion_text(
    text: &str,
    format: crate::ui::completion::CompletionInsertFormat,
) -> (String, usize) {
    if !matches!(
        format,
        crate::ui::completion::CompletionInsertFormat::Snippet
    ) {
        return (text.to_string(), text.len());
    }

    let mut replacement = String::with_capacity(text.len());
    let mut first_tabstop_offset = None;
    let mut final_tabstop_offset = None;
    let mut chars = text.char_indices().peekable();

    while let Some((_, ch)) = chars.next() {
        if ch == '$' {
            match chars.peek().copied() {
                Some((_, '0')) => {
                    final_tabstop_offset.get_or_insert(replacement.len());
                    chars.next();
                    continue;
                }
                Some((_, '1'..='9')) => {
                    let mut number = 0usize;
                    while let Some(&(_, digit)) = chars.peek() {
                        if !digit.is_ascii_digit() {
                            break;
                        }
                        number = number
                            .saturating_mul(10)
                            .saturating_add(digit.to_digit(10).unwrap_or(0) as usize);
                        chars.next();
                    }
                    if number > 0 {
                        first_tabstop_offset.get_or_insert(replacement.len());
                        continue;
                    }
                }
                Some((_, '{')) => {
                    chars.next();
                    let mut number = 0usize;
                    let mut has_digits = false;
                    while let Some(&(_, digit)) = chars.peek() {
                        if !digit.is_ascii_digit() {
                            break;
                        }
                        has_digits = true;
                        number = number
                            .saturating_mul(10)
                            .saturating_add(digit.to_digit(10).unwrap_or(0) as usize);
                        chars.next();
                    }
                    if has_digits {
                        match chars.peek().copied() {
                            Some((_, '}')) => {
                                chars.next();
                                if number == 0 {
                                    final_tabstop_offset.get_or_insert(replacement.len());
                                } else {
                                    first_tabstop_offset.get_or_insert(replacement.len());
                                }
                                continue;
                            }
                            Some((_, ':')) => {
                                chars.next();
                                let mut default_text = String::new();
                                let mut depth = 1usize;
                                while let Some((_, next_ch)) = chars.next() {
                                    match next_ch {
                                        '{' => {
                                            depth = depth.saturating_add(1);
                                            default_text.push(next_ch);
                                        }
                                        '}' => {
                                            depth -= 1;
                                            if depth == 0 {
                                                break;
                                            }
                                            default_text.push(next_ch);
                                        }
                                        _ => default_text.push(next_ch),
                                    }
                                }
                                if number == 0 {
                                    final_tabstop_offset.get_or_insert(replacement.len());
                                } else {
                                    first_tabstop_offset.get_or_insert(replacement.len());
                                    replacement.push_str(default_text.as_str());
                                }
                                continue;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        replacement.push(ch);
    }

    let cursor_offset = first_tabstop_offset
        .or(final_tabstop_offset)
        .unwrap_or(replacement.len());

    (replacement, cursor_offset)
}

#[cfg(test)]
mod tests;
