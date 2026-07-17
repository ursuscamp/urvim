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
mod input_box;
mod keymap;
mod lsp;
mod lsp_rename;
mod node;
mod picker;
mod render;
mod session;
mod tree;

use self::dialogs::Dialogs;
use self::keymap::ModalKeySequence;
use crate::action::ActionResult;
use crate::background::{JobEvent, JobKind, JobManager};
use crate::buffer::BufferId;
use crate::editor::{EditorAction, InheritedKeymap, ModeKind, NormalMode};
use crate::screen::Screen;
use crate::status_bar::StatusBar;
use crate::ui::plugin_pane::{PluginPane, PluginPaneOptions};
use crate::ui::plugin_window::{
    PluginWindowContent, PluginWindowId, PluginWindowManager, PluginWindowOptions,
};
use crate::ui::{Command, Intent, KeymapInheritance, UiEvent, UiEventResult};
use crate::window::{BufferView, Position, Size};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use urvim_terminal::CursorStyle;

use self::tree::ResizeDirection;
pub use geometry::PaneRegion;
pub use node::{LayoutNode, PaneContent, PaneId, PaneNode, SplitAxis, SplitNode, SplitSize};

/// Snapshot of a visible buffer range in a pane.
#[derive(Debug, Clone)]
pub struct VisibleRangeSnapshot {
    /// Pane identifier.
    pub pane_id: PaneId,
    /// Buffer identifier shown in this pane.
    pub buffer_id: BufferId,
    /// Whether this pane is the focused pane.
    pub active: bool,
    /// Current cursor position.
    pub cursor: crate::buffer::Cursor,
    /// Approximate first visible line.
    pub start_line: usize,
    /// Approximate last visible line (exclusive).
    pub end_line: usize,
    /// Scroll offset from the buffer view.
    pub scroll_offset: Position,
    /// Pane content size.
    pub size: Size,
}

/// Snapshot of a tab in a pane.
#[derive(Debug, Clone)]
pub struct PaneTabSnapshot {
    /// Buffer identifier for this tab.
    pub buffer_id: BufferId,
    /// Whether this tab is the active tab in its pane.
    pub active: bool,
}

/// Snapshot of a pane's state.
#[derive(Debug, Clone)]
pub struct PaneStateSnapshot {
    /// Pane identifier.
    pub id: PaneId,
    /// Whether this pane is the focused pane.
    pub focused: bool,
    /// Active buffer identifier in this pane.
    pub active_buffer_id: BufferId,
    /// Active tab index.
    pub active_tab_index: usize,
    /// All tabs in this pane.
    pub tabs: Vec<PaneTabSnapshot>,
    /// Pane origin (top-left corner).
    pub origin: Position,
    /// Pane content size.
    pub size: Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PersistentFocusTarget {
    Pane(PaneId),
    Plugin(PluginWindowId),
    PluginPane(PaneId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluginPaneKeySequence {
    None,
    Local,
    Inherited,
}

/// Root layout container for urvim.
///
/// The layout owns a binary split tree of panes, tracks the focused pane,
/// routes split-management actions, and renders a footer status bar beneath
/// the editor content area.
#[derive(Debug)]
pub struct Layout {
    root: Option<LayoutNode>,
    focused_pane: PaneId,
    last_editor_pane: PaneId,
    next_pane_id: usize,
    status_bar: StatusBar,
    origin: Position,
    size: Size,
    dialogs: Dialogs,
    jobs: Arc<JobManager>,
    inlay_hints: InlayHintState,
    autocomplete: AutocompleteState,
    plugin_windows: PluginWindowManager,
    plugin_pane_inherited_keymap: InheritedKeymap,
    plugin_pane_key_sequence: PluginPaneKeySequence,
    modal_inherited_keymap: InheritedKeymap,
    modal_key_sequence: ModalKeySequence,
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
            root: Some(LayoutNode::Pane(PaneNode::new_editor(
                focused_pane,
                window_group,
            ))),
            focused_pane,
            last_editor_pane: focused_pane,
            next_pane_id: 1,
            status_bar: StatusBar::new(),
            origin: Position::default(),
            size: Size::default(),
            dialogs: Dialogs::default(),
            jobs: Arc::new(JobManager::new()),
            inlay_hints: InlayHintState::Idle,
            autocomplete: AutocompleteState::default(),
            plugin_windows: PluginWindowManager::new(),
            plugin_pane_inherited_keymap: InheritedKeymap::new(NormalMode::keymap()),
            plugin_pane_key_sequence: PluginPaneKeySequence::None,
            modal_inherited_keymap: InheritedKeymap::new(NormalMode::keymap()),
            modal_key_sequence: ModalKeySequence::None,
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
        Self::find_pane(root, self.last_editor_pane)
            .and_then(PaneNode::editor_window_group)
            .expect("focused pane should exist")
    }

    /// Returns the active window group mutably for the focused pane.
    pub fn active_window_group_mut(&mut self) -> &mut crate::window_group::WindowGroup {
        let focused_pane = self.last_editor_pane;
        let root = self
            .root
            .as_mut()
            .expect("layout should contain a focused pane");
        Self::find_pane_mut(root, focused_pane)
            .and_then(PaneNode::editor_window_group_mut)
            .expect("focused pane should exist")
    }

    /// Returns the active window group.
    pub fn window_group(&self) -> &crate::window_group::WindowGroup {
        self.active_window_group()
    }

    /// Returns every buffer identifier currently shown in any pane.
    pub fn visible_buffer_ids(&self) -> Vec<BufferId> {
        let Some(root) = self.root.as_ref() else {
            return Vec::new();
        };
        let mut ids = Vec::new();
        Self::collect_buffer_ids(root, &mut ids);
        ids.sort_unstable();
        ids.dedup();
        ids
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

        if self.plugin_windows.focused().is_some() || self.focused_plugin_pane().is_some() {
            return None;
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

    /// Returns the registry of plugin-owned floating windows.
    pub fn plugin_windows(&self) -> &PluginWindowManager {
        &self.plugin_windows
    }

    /// Returns the mutable registry of plugin-owned floating windows.
    pub fn plugin_windows_mut(&mut self) -> &mut PluginWindowManager {
        &mut self.plugin_windows
    }

    /// Returns the focused plugin pane, if any.
    pub fn focused_plugin_pane(&self) -> Option<PaneId> {
        self.is_plugin_pane(self.focused_pane)
            .then_some(self.focused_pane)
    }

    /// Returns an owned plugin pane.
    pub fn plugin_pane(&self, owner: &str, id: PaneId) -> Result<&PluginPane, String> {
        let pane = self
            .plugin_pane_node(id)
            .ok_or_else(|| format!("unknown plugin pane_id {}", id.0))?;
        let window = pane
            .plugin_pane()
            .ok_or_else(|| format!("pane_id {} is not a plugin pane", id.0))?;
        if window.owner() != owner {
            return Err(format!("plugin {owner:?} does not own pane_id {}", id.0));
        }
        Ok(window)
    }

    /// Returns all plugin pane IDs owned by a plugin.
    pub fn plugin_pane_ids(&self, owner: &str) -> Vec<PaneId> {
        self.plugin_pane_ids_in_tree()
            .into_iter()
            .filter(|id| self.plugin_pane(owner, *id).is_ok())
            .collect()
    }

    /// Returns the configured options for an owned plugin pane.
    pub fn plugin_pane_options(
        &self,
        owner: &str,
        id: PaneId,
    ) -> Result<PluginPaneOptions, String> {
        Ok(self.plugin_pane(owner, id)?.options().clone())
    }

    /// Replaces an owned plugin pane's content.
    pub fn set_plugin_pane_content(
        &mut self,
        owner: &str,
        id: PaneId,
        content: PluginWindowContent,
    ) -> Result<(), String> {
        self.plugin_pane(owner, id)?;
        self.plugin_pane_node_mut(id)
            .and_then(PaneNode::plugin_pane_mut)
            .expect("owned plugin pane should exist")
            .set_content(content);
        Ok(())
    }

    /// Updates an owned plugin pane's presentation options.
    pub fn configure_plugin_pane(
        &mut self,
        owner: &str,
        id: PaneId,
        options: PluginPaneOptions,
    ) -> Result<(), String> {
        self.plugin_pane(owner, id)?;
        self.plugin_pane_node_mut(id)
            .and_then(PaneNode::plugin_pane_mut)
            .expect("owned plugin pane should exist")
            .set_options(options);
        Ok(())
    }

    /// Installs a keymap on an owned plugin pane.
    pub fn set_plugin_pane_keymap(
        &mut self,
        owner: &str,
        id: PaneId,
        keys: Vec<String>,
        rhs: String,
        intent: Intent,
    ) -> Result<(), String> {
        self.plugin_pane(owner, id)?;
        self.plugin_pane_node_mut(id)
            .and_then(PaneNode::plugin_pane_mut)
            .expect("owned plugin pane should exist")
            .set_keymap(keys, rhs, intent);
        if self.focused_plugin_pane() == Some(id) {
            self.plugin_pane_inherited_keymap.clear_pending();
            self.plugin_pane_key_sequence = PluginPaneKeySequence::None;
        }
        Ok(())
    }

    /// Removes a keymap from an owned plugin pane.
    pub fn delete_plugin_pane_keymap(
        &mut self,
        owner: &str,
        id: PaneId,
        keys: &[String],
    ) -> Result<(), String> {
        self.plugin_pane(owner, id)?;
        self.plugin_pane_node_mut(id)
            .and_then(PaneNode::plugin_pane_mut)
            .expect("owned plugin pane should exist")
            .delete_keymap(keys);
        if self.focused_plugin_pane() == Some(id) {
            self.plugin_pane_inherited_keymap.clear_pending();
            self.plugin_pane_key_sequence = PluginPaneKeySequence::None;
        }
        Ok(())
    }

    /// Returns keymaps installed on an owned plugin pane.
    pub fn plugin_pane_keymaps(
        &self,
        owner: &str,
        id: PaneId,
    ) -> Result<Vec<(Vec<String>, String)>, String> {
        Ok(self.plugin_pane(owner, id)?.keymaps())
    }

    /// Focuses an owned plugin pane.
    pub fn focus_plugin_pane(&mut self, owner: &str, id: PaneId) -> Result<(), String> {
        self.plugin_pane(owner, id)?;
        self.plugin_windows.blur_focused();
        self.focus_pane(id);
        Ok(())
    }

    /// Closes an owned plugin pane and removes its layout leaf.
    pub fn close_plugin_pane(&mut self, owner: &str, id: PaneId) -> Result<(), String> {
        self.plugin_pane(owner, id)?;
        let Some(root) = self.root.take() else {
            return Err("layout has no panes".to_string());
        };
        let (root, removed) = Self::remove_pane(root, id);
        self.root = root;
        if !removed {
            return Err(format!("unknown pane_id {}", id.0));
        }
        if self.focused_pane == id {
            if let Some(next) = self.first_editor_pane_id() {
                self.focus_pane(next);
            }
        }
        crate::session::mark_dirty();
        Ok(())
    }

    /// Closes every layout pane owned by a plugin.
    pub fn close_plugin_panes_owned(&mut self, owner: &str) {
        let ids = self.plugin_pane_ids(owner);
        let focused = self.focused_plugin_pane();
        for id in ids {
            if let Some(root) = self.root.take() {
                let (root, removed) = Self::remove_pane(root, id);
                self.root = root;
                debug_assert!(removed, "owned plugin pane should be present in the tree");
            }
        }
        if focused.is_some() {
            if let Some(next) = self.first_editor_pane_id() {
                self.focus_pane(next);
            }
        }
        crate::session::mark_dirty();
    }

    /// Focuses the next visible editor pane or plugin window in visual order.
    pub fn focus_next_window(&mut self) -> bool {
        self.cycle_persistent_focus(true)
    }

    /// Focuses the previous visible editor pane or plugin window in visual order.
    pub fn focus_previous_window(&mut self) -> bool {
        self.cycle_persistent_focus(false)
    }

    fn cycle_persistent_focus(&mut self, forward: bool) -> bool {
        let mut pane_regions = self.pane_regions();
        pane_regions.sort_by_key(|region| (region.origin.row, region.origin.col, region.id.0));

        let mut targets = pane_regions
            .into_iter()
            .map(|region| {
                if self.is_plugin_pane(region.id) {
                    PersistentFocusTarget::PluginPane(region.id)
                } else {
                    PersistentFocusTarget::Pane(region.id)
                }
            })
            .collect::<Vec<_>>();
        targets.extend(
            self.plugin_windows
                .visible_ids()
                .map(PersistentFocusTarget::Plugin),
        );

        let current = self
            .focused_plugin_pane()
            .map(PersistentFocusTarget::PluginPane)
            .or_else(|| {
                self.plugin_windows
                    .focused()
                    .map(PersistentFocusTarget::Plugin)
            })
            .unwrap_or(PersistentFocusTarget::Pane(self.focused_pane));
        if targets.is_empty() {
            return false;
        }

        let current_index = targets.iter().position(|target| *target == current);
        let target_index = match current_index {
            Some(index) if forward => (index + 1) % targets.len(),
            Some(0) if !forward => targets.len() - 1,
            Some(index) => index - 1,
            None if forward => 0,
            None => targets.len() - 1,
        };

        match targets[target_index] {
            PersistentFocusTarget::Pane(id) => {
                self.plugin_windows.blur_focused();
                self.focus_pane(id)
            }
            PersistentFocusTarget::Plugin(id) => self.plugin_windows.focus_id(id),
            PersistentFocusTarget::PluginPane(id) => {
                self.plugin_windows.blur_focused();
                self.focus_pane(id)
            }
        }
    }

    pub(super) fn focus_layout_pane(&mut self, id: PaneId) -> bool {
        if self.is_plugin_pane(id) {
            self.plugin_windows.blur_focused();
            return self.focus_pane(id);
        }
        self.focus_pane(id)
    }

    /// Creates a plugin-owned floating window.
    pub fn create_plugin_window(
        &mut self,
        owner: String,
        options: PluginWindowOptions,
    ) -> PluginWindowId {
        self.plugin_windows.create(owner, options)
    }

    /// Replaces a plugin window's retained content.
    pub fn set_plugin_window_content(
        &mut self,
        owner: &str,
        id: PluginWindowId,
        content: PluginWindowContent,
    ) -> Result<(), String> {
        self.plugin_windows.set_content(owner, id, content)
    }

    /// Creates a plugin pane beside an existing pane and focuses it.
    pub fn create_plugin_pane(
        &mut self,
        owner: String,
        target: Option<PaneId>,
        axis: SplitAxis,
        split_size: SplitSize,
        options: PluginPaneOptions,
    ) -> Result<PaneId, String> {
        let target = target.unwrap_or(self.focused_pane);
        let new_id = self.allocate_pane_id();
        let Some(root) = self.root.take() else {
            return Err("layout has no panes".to_string());
        };
        let mut plugin = Some(PaneNode::new_plugin(new_id, owner, options));
        let (root, changed) =
            Self::split_node_with_content(root, target, axis, split_size, &mut plugin);
        if !changed {
            self.root = Some(root);
            return Err(format!("unknown pane_id {}", target.0));
        }
        self.root = Some(root);
        self.plugin_windows.blur_focused();
        self.focus_pane(new_id);
        crate::session::mark_dirty();
        Ok(new_id)
    }

    /// Closes the focused plugin pane and collapses its parent split.
    pub fn close_focused_plugin_pane(&mut self) -> bool {
        let Some(id) = self.focused_plugin_pane() else {
            return false;
        };
        let Some(root) = self.root.take() else {
            return false;
        };
        let (root, removed) = Self::remove_pane(root, id);
        self.root = root;
        if removed {
            if let Some(next) = self.first_pane_id() {
                self.focus_pane(next);
            }
        }
        removed
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
        self.clear_modal_inherited_keys();
    }

    /// Dispatches a unified intent through the root layout.
    pub fn dispatch_intent(&mut self, intent: &Intent) -> bool {
        match intent {
            Intent::Editor(action) => self.dispatch_action(action),
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
            Command::OpenUnnamedBuffer => {
                self.active_window_group_mut().open_unnamed_buffer_tab();
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
            // Saving a single buffer is coordinated by the application layer so it can
            // perform overwrite confirmation and emit plugin-facing editor events.
            Command::SaveBuffer(_) => false,
            // Plugin picker values and callbacks live in the application plugin runtime.
            Command::PluginPickerSelect { .. }
            | Command::PluginConfirmationSelect { .. }
            | Command::PluginInputSubmit { .. } => false,
            Command::SaveBufferAs { buffer_id, path } => self
                .execute_save_as(*buffer_id, path.as_path())
                .map_or_else(
                    |error| {
                        crate::notify_error!("Failed to write buffer to {:?}: {}", path, error);
                        true
                    },
                    |_| true,
                ),
            Command::CloseBuffer(_) | Command::UnloadBuffer { .. } => true,
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
                        let snapshot = crate::globals::with_buffer(buffer_id, |buffer| {
                            crate::event::BufferEventSnapshot::from_buffer(buffer_id, buffer)
                        })
                        .expect("saved buffer should remain loaded");
                        crate::globals::enqueue_editor_event(
                            crate::event::EditorEvent::BufferSaved { snapshot },
                        );
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
            Command::PluginRequest { .. } | Command::PluginStatus => false,
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
            Command::PreviousTab(count) => {
                self.active_window_group_mut().previous_tab(*count);
                true
            }
            Command::NextTab(count) => {
                self.active_window_group_mut().next_tab(*count);
                true
            }
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
            Command::FocusNextWindow => self.focus_next_window(),
            Command::FocusPreviousWindow => self.focus_previous_window(),
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
        let canonical = urvim_syntax::builtin_syntax_registry()
            .ok()
            .and_then(|registry| registry.resolve_label(filetype))
            .map(|name| name.to_string())
            .or_else(|| {
                crate::globals::plugin_filetypes()
                    .into_iter()
                    .find(|name| name == filetype)
            });
        let Some(canonical) = canonical else {
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

        let label = urvim_syntax::builtin_syntax_registry()
            .ok()
            .and_then(|registry| {
                registry
                    .display_name(canonical.as_str())
                    .map(|label| label.to_string())
            })
            .unwrap_or_else(|| canonical.clone());
        crate::notify_info!("filetype: {}", label);
    }

    /// Dispatches an editor action through the active editor pane.
    pub fn dispatch_action(&mut self, action: &EditorAction) -> bool {
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
        if !self.modal_dialog_is_open() {
            self.clear_modal_inherited_keys();
        } else if matches!(event, UiEvent::Paste(_) | UiEvent::Resize(_, _)) {
            self.clear_modal_inherited_keys();
        }

        // Once an inherited sequence starts, it owns subsequent keys until the
        // sequence completes or fails. Local dialog handling only gets first
        // refusal when no inherited sequence is pending.
        if self.modal_key_sequence == ModalKeySequence::Inherited
            && let UiEvent::Key(key) = event
        {
            return self.route_modal_inherited_key(key);
        }

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
        // Confirmation and rename controls are local; only keys they reject may
        // participate in application-level normal-mode mappings.
        if self.overlay_modal_is_open()
            && let UiEvent::Key(key) = event
        {
            return self.route_modal_inherited_key(key);
        }

        let picker = self.route_picker_ui_event(event);
        if picker.handled() {
            return picker;
        }
        // Picker query editing is local as well, so printable mappings never
        // delay or replace text input.
        if self.picker_is_open()
            && let UiEvent::Key(key) = event
        {
            return self.route_modal_inherited_key(key);
        }

        let plugin_pane = self.route_plugin_pane_ui_event(event);
        if plugin_pane.handled() {
            return plugin_pane;
        }
        let plugin_window = self.plugin_windows.route_ui_event(event);
        if plugin_window.handled() {
            return plugin_window;
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
                } else if self.input_box_is_open() {
                    self.handle_input_box_event(event)
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
                } else if self.input_box_is_open() {
                    self.handle_input_box_event(event)
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
        if self.plugin_picker_is_open() {
            return self.handle_plugin_picker_event(event);
        }

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

    fn route_plugin_pane_ui_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(id) = self.focused_plugin_pane() else {
            return UiEventResult::NotHandled;
        };

        match event {
            UiEvent::Key(key) => {
                if self.plugin_pane_key_sequence == PluginPaneKeySequence::Inherited {
                    return self.route_plugin_pane_inherited_key(key);
                }

                let local_result = {
                    let Some(pane) = self.plugin_pane_node_mut(id) else {
                        return UiEventResult::Handled(Vec::new());
                    };
                    pane.plugin_pane_mut()
                        .expect("focused plugin pane should contain plugin content")
                        .handle_key(key)
                };

                match local_result {
                    crate::editor::HandleKeyResult::Complete(intent) => {
                        self.plugin_pane_key_sequence = PluginPaneKeySequence::None;
                        UiEventResult::Handled(vec![intent])
                    }
                    crate::editor::HandleKeyResult::WaitForMore => {
                        self.plugin_pane_key_sequence = PluginPaneKeySequence::Local;
                        UiEventResult::Handled(Vec::new())
                    }
                    crate::editor::HandleKeyResult::InvalidSequence
                        if self.plugin_pane_key_sequence == PluginPaneKeySequence::Local =>
                    {
                        self.plugin_pane_key_sequence = PluginPaneKeySequence::None;
                        UiEventResult::Handled(Vec::new())
                    }
                    crate::editor::HandleKeyResult::InvalidSequence => {
                        self.route_plugin_pane_inherited_key(key)
                    }
                }
            }
            UiEvent::Paste(_) | UiEvent::Resize(_, _) => {
                self.plugin_pane_inherited_keymap.clear_pending();
                self.plugin_pane_key_sequence = PluginPaneKeySequence::None;
                if let Some(pane) = self.plugin_pane_node_mut(id) {
                    pane.plugin_pane_mut()
                        .expect("focused plugin pane should contain plugin content")
                        .clear_pending_keys();
                    UiEventResult::Handled(Vec::new())
                } else {
                    UiEventResult::NotHandled
                }
            }
            UiEvent::Tick => UiEventResult::NotHandled,
        }
    }

    fn route_plugin_pane_inherited_key(&mut self, key: &urvim_terminal::Key) -> UiEventResult {
        let result = self
            .plugin_pane_inherited_keymap
            .handle_key(key, |inheritance| {
                matches!(
                    inheritance,
                    KeymapInheritance::Focus | KeymapInheritance::Application
                )
            });
        match result {
            crate::editor::HandleKeyResult::Complete(intent) => {
                self.plugin_pane_key_sequence = PluginPaneKeySequence::None;
                UiEventResult::Handled(vec![intent])
            }
            crate::editor::HandleKeyResult::WaitForMore => {
                self.plugin_pane_key_sequence = PluginPaneKeySequence::Inherited;
                UiEventResult::Handled(Vec::new())
            }
            crate::editor::HandleKeyResult::InvalidSequence => {
                self.plugin_pane_key_sequence = PluginPaneKeySequence::None;
                UiEventResult::Handled(Vec::new())
            }
        }
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

impl Layout {
    /// Returns visible range snapshots for all panes, optionally filtered by buffer.
    pub fn visible_range_snapshots(
        &self,
        buffer_filter: Option<BufferId>,
    ) -> Vec<VisibleRangeSnapshot> {
        let regions = self.pane_regions();
        let focused_pane = self.focused_pane;
        let Some(root) = self.root.as_ref() else {
            return Vec::new();
        };
        let mut snapshots = Vec::new();

        for region in &regions {
            let Some(pane) = Self::find_pane(root, region.id) else {
                continue;
            };
            let Some(window_group) = pane.editor_window_group() else {
                continue;
            };
            let view = window_group.active_buffer_view();
            let buffer_id = view.buffer_id();

            if let Some(filter) = buffer_filter {
                if buffer_id != filter {
                    continue;
                }
            }

            let cursor = view.cursor();
            let scroll_offset = view.scroll_offset();
            let pane_height = region.size.rows as usize;
            let start_line = scroll_offset.row as usize;
            let end_line = start_line.saturating_add(pane_height);

            snapshots.push(VisibleRangeSnapshot {
                pane_id: region.id,
                buffer_id,
                active: region.id == focused_pane,
                cursor,
                start_line,
                end_line,
                scroll_offset,
                size: region.size,
            });
        }

        snapshots
    }

    /// Returns a snapshot of all panes' state.
    pub fn pane_state_snapshots(&self) -> Vec<PaneStateSnapshot> {
        let regions = self.pane_regions();
        let focused_pane = self.focused_pane;
        let Some(root) = self.root.as_ref() else {
            return Vec::new();
        };
        let mut snapshots = Vec::new();

        for region in &regions {
            let Some(pane) = Self::find_pane(root, region.id) else {
                continue;
            };
            let Some(window_group) = pane.editor_window_group() else {
                continue;
            };
            let active_tab_index = window_group.active_tab_index();
            let buffer_ids = window_group.buffer_ids();

            let tabs: Vec<PaneTabSnapshot> = buffer_ids
                .iter()
                .enumerate()
                .map(|(index, &buffer_id)| PaneTabSnapshot {
                    buffer_id,
                    active: index == active_tab_index,
                })
                .collect();

            let active_buffer_id = buffer_ids
                .get(active_tab_index)
                .copied()
                .unwrap_or_else(|| buffer_ids.first().copied().unwrap_or(BufferId::new(0)));

            snapshots.push(PaneStateSnapshot {
                id: region.id,
                focused: region.id == focused_pane,
                active_buffer_id,
                active_tab_index,
                tabs,
                origin: region.origin,
                size: region.size,
            });
        }

        snapshots
    }
}

#[cfg(test)]
mod tests;
