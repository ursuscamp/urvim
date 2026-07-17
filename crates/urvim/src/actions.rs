use std::collections::VecDeque;
use std::io;

use crate::plugin::{BearscriptPluginRuntime, SharedLayout, loaded_buffer_ids};
use urvim_core::buffer::Cursor;
use urvim_core::editor::{EditorAction, EditorOperation, ModeKind, RepeatReplay};
use urvim_core::event::{
    BufferEventSnapshot, EditorEvent, EventSource, EventTransaction, capture_pane_state,
};
use urvim_core::globals;
use urvim_core::layout::Layout;
use urvim_core::ui::{Command, Intent};

pub(super) struct SaveBufferOutcome {
    pub(super) handled: bool,
    success: bool,
    error: Option<String>,
}

pub(super) fn handle_save_buffer_action_with_outcome(
    layout: &mut Layout,
    target: Option<urvim_core::buffer::BufferId>,
    force: bool,
) -> SaveBufferOutcome {
    let Some(buffer_id) = resolve_buffer_target(layout, target) else {
        return SaveBufferOutcome {
            handled: true,
            success: false,
            error: Some("unknown buffer".to_string()),
        };
    };

    if !force
        && globals::with_buffer_pool(|pool| pool.buffer_needs_overwrite_confirmation(buffer_id))
    {
        layout.prompt_overwrite_buffer(buffer_id);
        return SaveBufferOutcome {
            handled: true,
            success: true,
            error: None,
        };
    }

    let save_result = globals::with_buffer_pool(|pool| pool.save_buffer(buffer_id));

    match save_result {
        Ok(()) => {
            let label = globals::with_buffer(buffer_id, |buffer| {
                buffer
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".to_string())
            })
            .unwrap_or_else(|| "Untitled".to_string());
            globals::with_lsp_runtime_mut(|runtime| runtime.did_save_buffer(buffer_id));
            urvim_core::notify_info!("Saved {}", label);
            return SaveBufferOutcome {
                handled: true,
                success: true,
                error: None,
            };
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            tracing::info!("Skipping save for unnamed buffer {:?}", buffer_id);
            return SaveBufferOutcome {
                handled: true,
                success: false,
                error: Some(error.to_string()),
            };
        }
        Err(error) => {
            urvim_core::notify_error!("Failed to save buffer {:?}: {}", buffer_id, error);
            return SaveBufferOutcome {
                handled: true,
                success: false,
                error: Some(error.to_string()),
            };
        }
    }
}

pub(super) fn execute_action_intent_with_plugin_runtime(
    layout: &mut Layout,
    _plugin_runtime: &mut BearscriptPluginRuntime,
    action: EditorAction,
) -> bool {
    execute_action_transaction(layout, action)
}

fn execute_action_transaction(layout: &mut Layout, action: EditorAction) -> bool {
    let source = match action.kind.as_ref() {
        Some(EditorOperation::Undo) => EventSource::undo(),
        Some(EditorOperation::Redo) => EventSource::redo(),
        Some(EditorOperation::PasteAfter | EditorOperation::PasteBefore)
        | Some(EditorOperation::InsertRawPaste(_) | EditorOperation::ReplaceSelectionRawPaste(_)) => {
            EventSource::paste()
        }
        _ => EventSource::user(),
    };
    let transaction = EventTransaction::new(source);
    capture_pane_state(layout.event_pane_snapshots());
    let handled = execute_action_intent(layout, action);
    capture_pane_state(layout.event_pane_snapshots());
    drop(transaction);
    handled
}

pub(super) fn process_intent_queue(layout: &mut Layout, intents: Vec<Intent>) -> bool {
    process_intent_queue_with_plugin_runtime(layout, None, intents)
}

pub(super) fn process_intent_queue_with_plugin_runtime(
    layout: &mut Layout,
    mut plugin_runtime: Option<&mut BearscriptPluginRuntime>,
    intents: Vec<Intent>,
) -> bool {
    let mut queue: VecDeque<Intent> = intents.into();
    let mut handled_all = true;
    let mut saw_intent = false;

    while let Some(intent) = queue.pop_front() {
        saw_intent = true;
        handled_all &= match intent {
            Intent::Editor(action) => match plugin_runtime.as_deref_mut() {
                Some(runtime) => execute_action_intent_with_plugin_runtime(layout, runtime, action),
                None => execute_action_transaction(layout, action),
            },
            Intent::Command(command) => {
                execute_command_intent(layout, plugin_runtime.as_deref_mut(), command)
            }
        };
    }

    saw_intent && handled_all
}

pub(super) fn execute_command_intent(
    layout: &mut Layout,
    plugin_runtime: Option<&mut BearscriptPluginRuntime>,
    command: Command,
) -> bool {
    let name = command.event_name().into_owned();
    let emit_event = !command.is_internal_response();
    let outcome = execute_command_intent_inner(layout, plugin_runtime, command);
    if emit_event {
        enqueue_command_executed(&name, outcome.success, outcome.error.clone());
    }
    outcome.handled
}

struct CommandOutcome {
    handled: bool,
    success: bool,
    error: Option<String>,
}

impl CommandOutcome {
    fn success() -> Self {
        Self {
            handled: true,
            success: true,
            error: None,
        }
    }

    fn failure(error: impl Into<String>) -> Self {
        Self {
            handled: true,
            success: false,
            error: Some(error.into()),
        }
    }
}

pub(super) fn enqueue_command_executed(name: &str, success: bool, error: Option<String>) {
    globals::enqueue_editor_event(EditorEvent::CommandExecuted {
        command: name.to_string(),
        success,
        error,
    });
}

fn execute_command_intent_inner(
    layout: &mut Layout,
    plugin_runtime: Option<&mut BearscriptPluginRuntime>,
    command: Command,
) -> CommandOutcome {
    if let Command::SaveBuffer(target) = &command {
        let outcome = handle_save_buffer_action_with_outcome(layout, *target, false);
        return CommandOutcome {
            handled: outcome.handled,
            success: outcome.success,
            error: outcome.error,
        };
    }

    if let Command::OverwriteBuffer(target) = &command {
        let outcome = handle_save_buffer_action_with_outcome(layout, *target, true);
        return CommandOutcome {
            handled: outcome.handled,
            success: outcome.success,
            error: outcome.error,
        };
    }

    if let Command::PluginRequest {
        plugin,
        command,
        args,
    } = command
    {
        let Some(plugin_runtime) = plugin_runtime else {
            tracing::warn!(plugin, command, "plugin command has no runtime");
            urvim_core::notify_warn!("Plugin command {plugin} {command} could not run: no runtime");
            return CommandOutcome::failure("plugin runtime is not active");
        };
        match plugin_runtime.run_command(&plugin, &command, &args) {
            Ok(()) => {
                tracing::debug!(plugin, command, "ran BearScript plugin command");
                return CommandOutcome::success();
            }
            Err(error) => {
                tracing::warn!(plugin, command, error = %error, "BearScript plugin command failed");
                urvim_core::notify_warn!("Plugin command {plugin} {command} failed: {error}");
                return CommandOutcome::failure(error);
            }
        }
    }

    if let Command::PluginPickerSelect {
        plugin,
        picker_id,
        item_id,
    } = command
    {
        let Some(plugin_runtime) = plugin_runtime else {
            return CommandOutcome::success();
        };
        if let Err(error) = plugin_runtime.run_picker_selection(&plugin, picker_id, item_id) {
            tracing::warn!(plugin, picker_id, error = %error, "plugin picker selection failed");
            urvim_core::notify_warn!("Plugin {plugin} picker {picker_id} failed: {error}");
        }
        return CommandOutcome::success();
    }

    if let Command::PluginConfirmationSelect {
        plugin,
        confirmation_id,
        selection,
    } = command
    {
        let Some(plugin_runtime) = plugin_runtime else {
            return CommandOutcome::success();
        };
        if let Err(error) =
            plugin_runtime.run_confirmation_response(&plugin, confirmation_id, selection)
        {
            tracing::warn!(plugin, confirmation_id, error = %error, "plugin confirmation response failed");
            urvim_core::notify_warn!(
                "Plugin {plugin} confirmation {confirmation_id} failed: {error}"
            );
        }
        return CommandOutcome::success();
    }

    if let Command::PluginInputSubmit {
        plugin,
        input_id,
        text,
    } = command
    {
        let Some(plugin_runtime) = plugin_runtime else {
            return CommandOutcome::success();
        };
        if let Err(error) = plugin_runtime.run_input_submission(&plugin, input_id, text.clone()) {
            tracing::warn!(plugin, input_id, error = %error, "plugin input submission failed");
            urvim_core::notify_warn!("Plugin {plugin} input {input_id} failed: {error}");
        }
        return CommandOutcome::success();
    }

    if matches!(command, Command::PluginStatus) {
        let status = plugin_runtime
            .as_ref()
            .map(|runtime| runtime.status_summary())
            .unwrap_or_else(|| "BearScript plugin runtime inactive".to_string());
        urvim_core::notify_info!("{status}");
        return CommandOutcome::success();
    }

    if let Command::SaveBufferAs { buffer_id, path } = command {
        let Some(buffer_id) = resolve_buffer_target(layout, buffer_id) else {
            return CommandOutcome::failure("unknown buffer");
        };
        return match globals::with_buffer_pool(|pool| pool.save_buffer_to_path(buffer_id, &path)) {
            Ok(()) => {
                globals::with_lsp_runtime_mut(|runtime| runtime.did_save_buffer(buffer_id));
                urvim_core::notify_info!("Saved {}", path.display());
                CommandOutcome::success()
            }
            Err(error) => {
                urvim_core::notify_error!("Failed to write buffer to {:?}: {}", path, error);
                CommandOutcome::failure(error.to_string())
            }
        };
    }

    if let Command::CloseBuffer(buffer_id) = command {
        let Some(buffer_id) = resolve_buffer_target(layout, buffer_id) else {
            return CommandOutcome::failure("unknown buffer");
        };
        let closed = if buffer_id == layout.active_buffer_view().buffer_id() {
            layout.close_active_buffer_tab()
        } else {
            layout.close_buffer_tab_in_active_window(buffer_id)
        };
        if closed {
            cleanup_orphaned_buffers(layout);
        }
        return if closed {
            CommandOutcome::success()
        } else {
            CommandOutcome::failure("buffer is not open in the active window")
        };
    }

    if let Command::UnloadBuffer { buffer_id, force } = command {
        let Some(buffer_id) = resolve_buffer_target(layout, buffer_id) else {
            return CommandOutcome::failure("unknown buffer");
        };
        let modified =
            globals::with_buffer(buffer_id, |buffer| buffer.is_modified()).unwrap_or(false);
        if modified && !force {
            urvim_core::notify_warn!(
                "Buffer {:?} has unsaved changes; use force=true to unload",
                buffer_id
            );
            return CommandOutcome::failure("buffer has unsaved changes");
        }
        let _closed = layout.close_buffer_tabs_and_prune(buffer_id);
        let removed = globals::with_buffer_pool(|pool| pool.remove_buffer(buffer_id).is_some());
        return if removed {
            CommandOutcome::success()
        } else {
            CommandOutcome::failure("buffer is no longer loaded")
        };
    }

    let filetype_target = match &command {
        Command::SetBufferFiletype(buffer_id, _) => {
            let Some(buffer_id) = resolve_buffer_target(layout, *buffer_id) else {
                return CommandOutcome::failure("unknown buffer");
            };
            let syntax_name =
                globals::with_buffer(buffer_id, |buffer| buffer.syntax_name().to_string())
                    .expect("validated buffer should remain loaded");
            Some((buffer_id, syntax_name))
        }
        _ => None,
    };
    let handled = layout.dispatch_intent(&Intent::Command(command));
    if handled {
        if let Some((buffer_id, syntax_name)) = filetype_target
            && globals::with_buffer(buffer_id, |buffer| buffer.syntax_name() != syntax_name)
                .unwrap_or(false)
        {
            globals::enqueue_editor_event(EditorEvent::BufferFiletypeChanged {
                snapshot: buffer_event_snapshot(buffer_id),
            });
        }
        cleanup_orphaned_buffers(layout);
    }
    if handled {
        CommandOutcome::success()
    } else {
        CommandOutcome {
            handled: false,
            success: false,
            error: Some("command was not handled".to_string()),
        }
    }
}

fn buffer_event_snapshot(buffer_id: urvim_core::buffer::BufferId) -> BufferEventSnapshot {
    globals::with_buffer(buffer_id, |buffer| {
        BufferEventSnapshot::from_buffer(buffer_id, buffer)
    })
    .expect("event buffer should remain loaded while its event is enqueued")
}

fn resolve_buffer_target(
    layout: &Layout,
    buffer_id: Option<urvim_core::buffer::BufferId>,
) -> Option<urvim_core::buffer::BufferId> {
    let buffer_id = buffer_id.unwrap_or_else(|| layout.active_buffer_view().buffer_id());
    if globals::with_buffer(buffer_id, |_| ()).is_none() {
        urvim_core::notify_error!("Unknown buffer: {}", buffer_id.get());
        return None;
    }
    Some(buffer_id)
}

pub(super) fn cleanup_orphaned_buffers(layout: &Layout) {
    let visible = layout
        .visible_buffer_ids()
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let loaded = loaded_buffer_ids();
    for buffer_id in loaded.difference(&visible).copied() {
        let should_unload =
            globals::with_buffer(buffer_id, |buffer| !buffer.is_modified()).unwrap_or(false);
        if !should_unload {
            tracing::debug!(?buffer_id, "keeping modified orphaned buffer loaded");
            continue;
        }
        globals::with_buffer_pool(|pool| {
            pool.remove_buffer(buffer_id);
        });
    }
}

pub(super) fn execute_action_intent(layout: &mut Layout, action: EditorAction) -> bool {
    let repeat_replay = action.resolve_dot_repeat();
    let dispatch_action = repeat_replay
        .as_ref()
        .map(|replay| replay.action.clone())
        .unwrap_or_else(|| {
            if action.is_repeat_command() {
                EditorAction::none()
            } else {
                action.clone()
            }
        });

    match action.kind.as_ref() {
        Some(EditorOperation::Undo) => apply_undo_redo(layout, false),
        Some(EditorOperation::Redo) => apply_undo_redo(layout, true),
        _ => {
            let mut handled = false;
            if let Some(replay) = repeat_replay.as_ref() {
                handled = replay_repeat_action(layout, replay);
                if handled
                    && replay.action.kind.is_some()
                    && let Some(to_mode) = replay.action.to_mode
                {
                    let repeat_text = {
                        let window = layout.active_window_group_mut().active_window_mut();
                        window.switch_mode(to_mode)
                    };
                    if let Some(repeat_text) = repeat_text.filter(|text| !text.is_empty())
                        && let Some(mut repeat_state) = globals::get_last_repeat()
                    {
                        repeat_state.insert_text = Some(repeat_text);
                        globals::set_last_repeat(repeat_state);
                    }
                }
            } else {
                let handled_by_layout =
                    layout.dispatch_intent(&Intent::Editor(dispatch_action.clone()));

                if !handled_by_layout {
                    match dispatch_action.kind.as_ref() {
                        None => {
                            handled = true;
                        }
                        _ => {}
                    }
                } else {
                    let pending_repeat_suffix = layout.take_pending_repeat_suffix();
                    if let Some(suffix) = pending_repeat_suffix.as_deref() {
                        layout
                            .active_window_group_mut()
                            .active_window_mut()
                            .append_repeat_text(suffix);
                    }
                    handled = true;
                }

                if handled && let Some(to_mode) = dispatch_action.to_mode {
                    let repeat_text = {
                        let window = layout.active_window_group_mut().active_window_mut();
                        window.switch_mode(to_mode)
                    };
                    if let Some(repeat_text) = repeat_text.filter(|text| !text.is_empty())
                        && let Some(mut repeat_state) = globals::get_last_repeat()
                    {
                        repeat_state.insert_text = Some(repeat_text);
                        globals::set_last_repeat(repeat_state);
                    }
                }

                if handled {
                    if (dispatch_action.from_mode == Some(ModeKind::Insert)
                        || dispatch_action.from_mode == Some(ModeKind::Replace))
                        && dispatch_action.to_mode == Some(ModeKind::Normal)
                    {
                        commit_insert_exit_snapshot(layout);
                    }

                    if dispatch_action.is_snapshottable() {
                        let cursor = layout.active_buffer_view().cursor();
                        layout
                            .active_buffer_view()
                            .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
                            .unwrap_or(());
                    }

                    if dispatch_action.updates_snapshot_cursor() {
                        let cursor = layout.active_buffer_view().cursor();
                        layout
                            .active_buffer_view_mut()
                            .with_buffer_mut(|buffer| buffer.update_cursor(cursor))
                            .unwrap_or(());
                    }

                    if dispatch_action.from_mode == Some(ModeKind::Insert) {
                        match dispatch_action.kind.as_ref() {
                            Some(EditorOperation::InsertChar(_))
                            | Some(EditorOperation::InsertText(_))
                            | Some(EditorOperation::InsertNewline)
                            | Some(EditorOperation::DeleteBackward)
                            | Some(EditorOperation::DeleteForward) => {
                                layout.handle_insert_completion_change();
                            }
                            _ => layout.cancel_autocomplete(),
                        }
                    }

                    if let Some((repeat_action, repeat_count)) = action.dot_repeat_source() {
                        globals::set_last_repeat(globals::RepeatState {
                            action: repeat_action,
                            count: repeat_count,
                            insert_text: None,
                        });
                    }
                }
            }

            handled
        }
    }
}

pub(super) fn apply_undo_redo(layout: &mut Layout, redo: bool) -> bool {
    let cursor = if redo {
        layout
            .active_buffer_view()
            .with_buffer_mut(|buffer| buffer.redo())
    } else {
        layout
            .active_buffer_view()
            .with_buffer_mut(|buffer| buffer.undo())
    };

    let Some(cursor) = cursor.flatten() else {
        return false;
    };

    layout.active_buffer_view_mut().set_cursor_synced(cursor);
    layout.active_window_group_mut().record_cursor_position();
    true
}

pub(super) fn commit_insert_exit_snapshot(layout: &mut Layout) {
    let cursor = layout.active_buffer_view().cursor();
    let should_snapshot = layout
        .active_buffer_view()
        .with_buffer(|buffer| !buffer.current_text_matches_undo_head())
        .unwrap_or(false);

    if should_snapshot {
        layout
            .active_buffer_view()
            .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
            .unwrap_or(());
    }
}

pub(super) fn replay_repeat_action(layout: &mut Layout, replay: &RepeatReplay) -> bool {
    if replay.action.kind.is_none()
        && replay.action.to_mode == Some(ModeKind::Insert)
        && replay.insert_text.as_deref().is_none_or(str::is_empty)
    {
        return false;
    }

    let structural_action = if replay.structural_count > 1 {
        EditorAction::count(replay.structural_count, Box::new(replay.action.clone()))
    } else {
        replay.action.clone()
    };

    for _ in 0..replay.repeat_count {
        let handled = match replay.action {
            _ if replay.action.kind.is_none()
                && replay.action.to_mode == Some(ModeKind::Insert) =>
            {
                true
            }
            _ => process_intent_queue(layout, vec![Intent::Editor(structural_action.clone())]),
        };

        if !handled {
            return false;
        }

        if let Some(text) = replay.insert_text.as_deref() {
            insert_replay_text(layout, text);
        }
    }

    true
}

fn insert_replay_text(layout: &mut Layout, text: &str) {
    if text.is_empty() {
        return;
    }

    let cursor = layout.active_buffer_view().cursor();
    layout
        .active_buffer_view_mut()
        .with_buffer_mut(|buffer| buffer.insert_text(cursor, text))
        .unwrap_or(());
    layout
        .active_buffer_view_mut()
        .set_cursor(cursor_after_text(cursor, text));
}

fn cursor_after_text(mut cursor: Cursor, text: &str) -> Cursor {
    for ch in text.chars() {
        if ch == '\n' {
            cursor = Cursor::new(cursor.line + 1, 0);
        } else {
            cursor.col += ch.len_utf8();
        }
    }

    cursor
}

#[cfg(test)]
pub(super) fn handle_ui_result(layout: &mut Layout, result: urvim_core::ui::UiEventResult) -> bool {
    if !result.handled() {
        return false;
    }

    let intents = result.into_intents();
    if !intents.is_empty() {
        process_intent_queue(layout, intents);
    }

    true
}

/// Processes UI intents without holding a mutable layout borrow across a
/// plugin callback. Plugin commands can call back into the UI module, so they
/// must be dispatched through the shared layout rather than a borrowed layout.
pub(super) fn handle_ui_result_with_shared_layout(
    layout: &SharedLayout,
    plugin_runtime: &mut BearscriptPluginRuntime,
    result: urvim_core::ui::UiEventResult,
) -> bool {
    if !result.handled() {
        return false;
    }

    let intents = result.into_intents();
    if !intents.is_empty() {
        process_intents_with_shared_layout(layout, Some(plugin_runtime), intents);
    }

    true
}

pub(super) fn process_intents_with_shared_layout(
    layout: &SharedLayout,
    plugin_runtime: Option<&mut BearscriptPluginRuntime>,
    intents: Vec<Intent>,
) -> bool {
    let mut handled_all = true;
    let mut saw_intent = false;
    let mut plugin_runtime = plugin_runtime;

    for intent in intents {
        saw_intent = true;
        handled_all &= match intent {
            Intent::Command(Command::PluginRequest {
                plugin,
                command,
                args,
            }) => {
                let event_name = Command::PluginRequest {
                    plugin: plugin.clone(),
                    command: command.clone(),
                    args: Vec::new(),
                }
                .event_name()
                .into_owned();
                match plugin_runtime.as_deref_mut() {
                    Some(runtime) => {
                        let (success, error) = match runtime.run_command(&plugin, &command, &args) {
                            Ok(()) => {
                                tracing::debug!(plugin, command, "ran BearScript plugin command");
                                (true, None)
                            }
                            Err(error) => {
                                tracing::warn!(plugin, command, error = %error, "BearScript plugin command failed");
                                urvim_core::notify_warn!(
                                    "Plugin command {plugin} {command} failed: {error}"
                                );
                                (false, Some(error))
                            }
                        };
                        enqueue_command_executed(&event_name, success, error);
                        true
                    }
                    None => {
                        tracing::warn!(plugin, command, "plugin command has no runtime");
                        urvim_core::notify_warn!(
                            "Plugin command {plugin} {command} could not run: no runtime"
                        );
                        enqueue_command_executed(
                            &event_name,
                            false,
                            Some("plugin runtime is not active".to_string()),
                        );
                        true
                    }
                }
            }
            Intent::Command(Command::PluginPickerSelect {
                plugin,
                picker_id,
                item_id,
            }) => match plugin_runtime.as_deref_mut() {
                Some(runtime) => {
                    if let Err(error) = runtime.run_picker_selection(&plugin, picker_id, item_id) {
                        tracing::warn!(plugin, picker_id, error = %error, "plugin picker selection failed");
                        urvim_core::notify_warn!(
                            "Plugin {plugin} picker {picker_id} failed: {error}"
                        );
                    }
                    true
                }
                None => true,
            },
            Intent::Command(Command::PluginConfirmationSelect {
                plugin,
                confirmation_id,
                selection,
            }) => match plugin_runtime.as_deref_mut() {
                Some(runtime) => {
                    if let Err(error) =
                        runtime.run_confirmation_response(&plugin, confirmation_id, selection)
                    {
                        tracing::warn!(plugin, confirmation_id, error = %error, "plugin confirmation response failed");
                        urvim_core::notify_warn!(
                            "Plugin {plugin} confirmation {confirmation_id} failed: {error}"
                        );
                    }
                    true
                }
                None => true,
            },
            other => process_intent_queue_with_plugin_runtime(
                &mut layout.borrow_mut(),
                plugin_runtime.as_deref_mut(),
                vec![other],
            ),
        };
    }

    saw_intent && handled_all
}

pub(super) fn raw_paste_action_for_mode(mode: ModeKind, text: String) -> Option<EditorAction> {
    match mode {
        ModeKind::Insert | ModeKind::Replace | ModeKind::Normal => {
            Some(EditorAction::insert_raw_paste(text).with_from_mode(mode))
        }
        ModeKind::Visual | ModeKind::VisualLine => Some(
            EditorAction::replace_selection_raw_paste(text)
                .with_mode(Some(mode), Some(ModeKind::Normal)),
        ),
        ModeKind::Resizing => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use urvim_core::buffer::Buffer;
    use urvim_core::ui::Command;
    use urvim_core::window_group::WindowGroup;

    fn drain_editor_events() -> Vec<EditorEvent> {
        std::iter::from_fn(globals::take_editor_event).collect()
    }

    fn action_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|error| error.into_inner())
    }

    #[test]
    fn save_as_targets_non_active_buffer() {
        let _pool_guard = action_test_lock();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![
            Buffer::from_str("first"),
            Buffer::from_str("second"),
        ]));
        let active = layout.active_buffer_view().buffer_id();
        let target = layout
            .active_window_group()
            .buffer_ids()
            .into_iter()
            .find(|buffer_id| *buffer_id != active)
            .expect("layout should contain a non-active buffer");
        let expected = globals::with_buffer(target, |buffer| buffer.as_str().to_string())
            .expect("target buffer should exist");
        let path = std::env::temp_dir().join(format!(
            "urvim-targeted-save-as-{}-{}.txt",
            std::process::id(),
            target.get()
        ));
        std::fs::remove_file(&path).ok();

        assert!(process_intent_queue(
            &mut layout,
            vec![Intent::Command(Command::SaveBufferAs {
                buffer_id: Some(target),
                path: path.clone(),
            })],
        ));

        assert_eq!(
            std::fs::read_to_string(&path).expect("target should be written"),
            expected
        );
        assert!(globals::with_buffer(active, |buffer| buffer.path().is_none()).unwrap_or(false));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn save_as_emits_domain_events_before_command_completion() {
        let _pool_guard = action_test_lock();
        globals::clear_editor_events_for_tests();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("text")]));
        let path = std::env::temp_dir().join(format!(
            "urvim-action-save-as-events-{}.txt",
            std::process::id()
        ));
        std::fs::remove_file(&path).ok();
        drain_editor_events();

        assert!(execute_command_intent(
            &mut layout,
            None,
            Command::SaveBufferAs {
                buffer_id: None,
                path: path.clone(),
            },
        ));

        let events = drain_editor_events();
        assert!(
            matches!(
                events.as_slice(),
                [
                    EditorEvent::BufferPathChanged { .. },
                    EditorEvent::BufferSaved { .. },
                    EditorEvent::CommandExecuted {
                        command,
                        success: true,
                        error: None,
                    }
                ] if command == "buffer.save-as"
            ),
            "unexpected events: {events:?}"
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn failed_save_emits_domain_failure_before_command_failure() {
        let _pool_guard = action_test_lock();
        globals::clear_editor_events_for_tests();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("text")]));
        drain_editor_events();

        assert!(execute_command_intent(
            &mut layout,
            None,
            Command::SaveBuffer(None),
        ));

        let events = drain_editor_events();
        assert!(
            matches!(
                events.as_slice(),
                [
                    EditorEvent::BufferSaveFailed { .. },
                    EditorEvent::CommandExecuted {
                        command,
                        success: false,
                        error: Some(error),
                    }
                ] if command == "buffer.save" && error == "buffer has no path"
            ),
            "unexpected events: {events:?}"
        );
    }

    #[test]
    fn internal_plugin_responses_do_not_emit_command_events() {
        let _pool_guard = action_test_lock();
        globals::clear_editor_events_for_tests();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
        drain_editor_events();

        assert!(execute_command_intent(
            &mut layout,
            None,
            Command::PluginInputSubmit {
                plugin: "demo".to_string(),
                input_id: 1,
                text: "answer".to_string(),
            },
        ));
        assert!(execute_command_intent(
            &mut layout,
            None,
            Command::EnqueueNotification {
                level: urvim_core::notification::NotificationLevel::Info,
                message: "internal".to_string(),
            },
        ));

        assert!(
            drain_editor_events()
                .iter()
                .all(|event| !matches!(event, EditorEvent::CommandExecuted { .. }))
        );
    }

    #[test]
    fn plugin_command_event_preserves_plugin_identity() {
        let _pool_guard = action_test_lock();
        globals::clear_editor_events_for_tests();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
        drain_editor_events();

        assert!(execute_command_intent(
            &mut layout,
            None,
            Command::PluginRequest {
                plugin: "acme-tools".to_string(),
                command: "sync.now".to_string(),
                args: Vec::new(),
            },
        ));

        assert!(matches!(
            drain_editor_events().as_slice(),
            [EditorEvent::CommandExecuted {
                command,
                success: false,
                error: Some(_),
            }] if command == "plugin.acme-tools.sync.now"
        ));
    }
}
