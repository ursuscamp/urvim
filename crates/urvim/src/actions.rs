use std::collections::VecDeque;
use std::io;

use crate::plugin::{BearscriptPluginRuntime, loaded_buffer_ids};
use urvim_core::buffer::Cursor;
use urvim_core::editor::{Action, ActionKind, ModeKind, RepeatReplay};
use urvim_core::event::EditorEvent;
use urvim_core::globals;
use urvim_core::layout::Layout;
use urvim_core::ui::{Command, Intent};

pub(super) struct SaveBufferOutcome {
    pub(super) handled: bool,
}

pub(super) fn handle_save_buffer_action(
    layout: &mut Layout,
    target: Option<urvim_core::buffer::BufferId>,
    force: bool,
) -> bool {
    handle_save_buffer_action_with_outcome(layout, target, force).handled
}

pub(super) fn handle_save_buffer_action_with_outcome(
    layout: &mut Layout,
    target: Option<urvim_core::buffer::BufferId>,
    force: bool,
) -> SaveBufferOutcome {
    let buffer_id = target.unwrap_or_else(|| layout.active_buffer_view().buffer_id());

    if !force
        && globals::with_buffer_pool(|pool| pool.buffer_needs_overwrite_confirmation(buffer_id))
    {
        layout.prompt_overwrite_buffer(buffer_id);
        return SaveBufferOutcome { handled: true };
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
            globals::enqueue_editor_event(EditorEvent::BufferSaved { buffer_id });
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            tracing::info!("Skipping save for unnamed buffer {:?}", buffer_id);
        }
        Err(error) => {
            urvim_core::notify_error!("Failed to save buffer {:?}: {}", buffer_id, error);
        }
    }

    SaveBufferOutcome { handled: true }
}

pub(super) fn execute_action_intent_with_plugin_runtime(
    layout: &mut Layout,
    _plugin_runtime: &mut BearscriptPluginRuntime,
    action: Action,
) -> bool {
    // Plugin events are dispatched centrally from the editor event queue, so
    // action intent execution simply forwards to the non-plugin-aware helper.
    execute_action_intent(layout, action)
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
            Intent::Action(action) => match plugin_runtime.as_deref_mut() {
                Some(runtime) => execute_action_intent_with_plugin_runtime(layout, runtime, action),
                None => execute_action_intent(layout, action),
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
    if let Command::PluginRequest {
        plugin,
        command,
        args,
    } = command
    {
        let Some(plugin_runtime) = plugin_runtime else {
            tracing::warn!(plugin, command, "plugin command has no runtime");
            urvim_core::notify_warn!("Plugin command {plugin} {command} could not run: no runtime");
            return true;
        };
        match plugin_runtime.run_command(&plugin, &command, &args) {
            Ok(()) => tracing::debug!(plugin, command, "ran BearScript plugin command"),
            Err(error) => {
                tracing::warn!(plugin, command, error = %error, "BearScript plugin command failed");
                urvim_core::notify_warn!("Plugin command {plugin} {command} failed: {error}");
            }
        }
        return true;
    }

    if matches!(command, Command::PluginStatus) {
        let status = plugin_runtime
            .as_ref()
            .map(|runtime| runtime.status_summary())
            .unwrap_or_else(|| "BearScript plugin runtime inactive".to_string());
        urvim_core::notify_info!("{status}");
        return true;
    }

    if let Command::SaveBufferAs(path) = command {
        let buffer_id = layout.active_buffer_view().buffer_id();
        let handled =
            match globals::with_buffer_pool(|pool| pool.save_buffer_to_path(buffer_id, &path)) {
                Ok(()) => {
                    globals::with_lsp_runtime_mut(|runtime| runtime.did_save_buffer(buffer_id));
                    urvim_core::notify_info!("Saved {}", path.display());
                    globals::enqueue_editor_event(EditorEvent::BufferSaved { buffer_id });
                    true
                }
                Err(error) => {
                    urvim_core::notify_error!("Failed to write buffer to {:?}: {}", path, error);
                    true
                }
            };
        return handled;
    }

    if let Command::CloseBuffer(buffer_id) = command {
        let buffer_id = buffer_id.unwrap_or_else(|| layout.active_buffer_view().buffer_id());
        let closed = if buffer_id == layout.active_buffer_view().buffer_id() {
            layout.close_active_buffer_tab()
        } else {
            layout.active_window_group_mut().close_buffer_tab(buffer_id)
        };
        if closed {
            globals::enqueue_editor_event(EditorEvent::BufferClosed { buffer_id });
            cleanup_orphaned_buffers(layout);
        }
        return true;
    }

    if let Command::UnloadBuffer { buffer_id, force } = command {
        let buffer_id = buffer_id.unwrap_or_else(|| layout.active_buffer_view().buffer_id());
        let modified =
            globals::with_buffer(buffer_id, |buffer| buffer.is_modified()).unwrap_or(false);
        if modified && !force {
            urvim_core::notify_warn!(
                "Buffer {:?} has unsaved changes; use force=true to unload",
                buffer_id
            );
            return true;
        }
        let was_visible = layout.visible_buffer_ids().contains(&buffer_id);
        let _closed = layout.close_buffer_tabs_and_prune(buffer_id);
        if was_visible {
            globals::enqueue_editor_event(EditorEvent::BufferClosed { buffer_id });
        }
        globals::with_buffer_pool(|pool| {
            pool.remove_buffer(buffer_id);
        });
        return true;
    }

    let command_for_event = format!("{:?}", command);
    let opened_before = matches!(
        command,
        Command::OpenUnnamedBuffer | Command::OpenFile(_) | Command::OpenFileAtCursor(_, _)
    )
    .then(|| {
        layout
            .active_window_group()
            .buffer_ids()
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    });
    let filetype_target = match &command {
        Command::SetBufferFiletype(buffer_id, _) => {
            Some(buffer_id.unwrap_or_else(|| layout.active_buffer_view().buffer_id()))
        }
        _ => None,
    };
    let close_targets =
        matches!(command, Command::ClosePane).then(|| closed_pane_buffer_ids(layout));

    let handled = layout.dispatch_intent(&Intent::Command(command));
    if handled {
        if let Some(before) = opened_before.as_ref() {
            for buffer_id in layout
                .active_window_group()
                .buffer_ids()
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .difference(before)
                .copied()
            {
                globals::enqueue_editor_event(EditorEvent::BufferOpened { buffer_id });
            }
        }
        if let Some(buffer_id) = filetype_target {
            globals::enqueue_editor_event(EditorEvent::BufferFiletypeChanged { buffer_id });
        }
        for buffer_id in close_targets.unwrap_or_default() {
            globals::enqueue_editor_event(EditorEvent::BufferClosed { buffer_id });
        }
        cleanup_orphaned_buffers(layout);
        globals::enqueue_editor_event(EditorEvent::CommandExecuted {
            command: command_for_event,
        });
    }
    handled
}

fn closed_pane_buffer_ids(
    layout: &Layout,
) -> std::collections::BTreeSet<urvim_core::buffer::BufferId> {
    layout
        .active_window_group()
        .buffer_ids()
        .into_iter()
        .collect()
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

pub(super) fn execute_action_intent(layout: &mut Layout, action: Action) -> bool {
    let repeat_replay = action.resolve_dot_repeat();
    let dispatch_action = repeat_replay
        .as_ref()
        .map(|replay| replay.action.clone())
        .unwrap_or_else(|| {
            if action.is_repeat_command() {
                Action::none()
            } else {
                action.clone()
            }
        });

    match action.kind.as_ref() {
        Some(ActionKind::Undo) => apply_undo_redo(layout, false),
        Some(ActionKind::Redo) => apply_undo_redo(layout, true),
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
                let handled_by_layout = layout.dispatch_action(&dispatch_action);

                if !handled_by_layout {
                    match dispatch_action.kind.as_ref() {
                        Some(ActionKind::SaveBuffer(_)) => {
                            handled = handle_save_buffer_action(
                                layout,
                                dispatch_action.kind.as_ref().and_then(|kind| match kind {
                                    ActionKind::SaveBuffer(target) => *target,
                                    _ => None,
                                }),
                                false,
                            );
                        }
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
                            Some(ActionKind::InsertChar(_))
                            | Some(ActionKind::InsertText(_))
                            | Some(ActionKind::InsertNewline)
                            | Some(ActionKind::DeleteBackward)
                            | Some(ActionKind::DeleteForward) => {
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
        Action::count(replay.structural_count, Box::new(replay.action.clone()))
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
            _ => process_intent_queue(layout, vec![Intent::Action(structural_action.clone())]),
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
    use std::{cell::RefCell, rc::Rc};

    let mut plugin_runtime = BearscriptPluginRuntime::empty(Rc::new(RefCell::new(Layout::new(
        urvim_core::WindowGroup::from_buffers(vec![urvim_core::buffer::Buffer::new()]),
    ))));
    handle_ui_result_with_plugin_runtime(layout, &mut plugin_runtime, result)
}

pub(super) fn handle_ui_result_with_plugin_runtime(
    layout: &mut Layout,
    plugin_runtime: &mut BearscriptPluginRuntime,
    result: urvim_core::ui::UiEventResult,
) -> bool {
    if !result.handled() {
        return false;
    }

    let intents = result.into_intents();
    if !intents.is_empty() {
        process_intent_queue_with_plugin_runtime(layout, Some(plugin_runtime), intents);
    }

    true
}

pub(super) fn raw_paste_action_for_mode(mode: ModeKind, text: String) -> Option<Action> {
    match mode {
        ModeKind::Insert | ModeKind::Replace | ModeKind::Normal => {
            Some(Action::insert_raw_paste(text).with_from_mode(mode))
        }
        ModeKind::Visual | ModeKind::VisualLine => Some(
            Action::replace_selection_raw_paste(text).with_mode(Some(mode), Some(ModeKind::Normal)),
        ),
        ModeKind::Resizing => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use urvim_core::buffer::Buffer;
    use urvim_core::window_group::WindowGroup;

    #[test]
    fn closed_pane_buffer_ids_include_every_tab_in_focused_pane() {
        let layout = Layout::new(WindowGroup::from_buffers(vec![
            Buffer::from_str("one"),
            Buffer::from_str("two"),
            Buffer::from_str("three"),
        ]));

        let ids = closed_pane_buffer_ids(&layout);

        assert_eq!(ids.len(), 3);
        for buffer_id in layout.active_window_group().buffer_ids() {
            assert!(ids.contains(&buffer_id));
        }
    }
}
