use clap::Parser;
use rustix::fd::AsFd;
use std::io;

use urvim_core::Layout;
use urvim_core::buffer::Cursor;
use urvim_core::config::Config;
use urvim_core::editor::{Action, ActionKind, HandleKeyResult, ModeKind, RepeatReplay};
use urvim_core::globals;
use urvim_core::screen::Screen;
use urvim_core::ui::{Command, Intent, UiEvent};
use urvim_core::window::{Position, Size};
use urvim_plugin::{PluginConfigEntry, PluginRuntimeEvent};
use urvim_terminal::{Terminal, size::get_terminal_size};
use urvim_theme::{Theme, ThemeRegistry};

struct StartupPluginsAndThemes {
    #[cfg(test)]
    plugin_registry: urvim_plugin::PluginRegistry,
    plugin_runtime: urvim_plugin::PluginRuntime,
    theme_registry: ThemeRegistry,
    active_theme: Theme,
}

#[derive(Parser)]
#[command(name = "urvim")]
#[command(version = "0.1.0")]
#[command(about = "A terminal-based text editor", long_about = None)]
struct Cli {
    #[arg(long)]
    theme: Option<String>,
    #[arg(long = "no-syntax")]
    no_syntax: bool,
    files: Vec<urvim_core::cli::CliFileSpec>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let _guard = urvim_core::logger::init("debug.log");

    if !is_terminal::is_terminal(std::io::stdin()) {
        eprintln!("Error: Must be run from a terminal");
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "stdin is not a terminal",
        ));
    }

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let config =
        Config::load(cli.theme.as_deref(), cli.no_syntax.then_some(false)).map_err(|error| {
            eprintln!("Error: {}", error);
            io::Error::new(io::ErrorKind::InvalidData, error.to_string())
        })?;
    globals::set_config(config.clone());

    let StartupPluginsAndThemes {
        #[cfg(test)]
            plugin_registry: _,
        plugin_runtime,
        theme_registry: registry,
        active_theme,
    } = load_startup_plugins_and_themes(&config).map_err(|error| {
        eprintln!("Error: {}", error);
        io::Error::new(io::ErrorKind::InvalidData, error)
    })?;
    for failure in plugin_runtime.failures() {
        if let urvim_plugin::PluginProcessState::Failed(error) = failure.state {
            urvim_core::notify_warn!("Plugin process {} failed: {}", failure.plugin, error);
        }
    }
    globals::set_active_theme(active_theme);
    globals::set_theme_registry(registry);
    let mut plugin_runtime = plugin_runtime;

    let mut terminal = Terminal::new(stdin, stdout)?;

    let (mut rows, mut cols) = get_terminal_size().unwrap_or((24, 80));
    let mut screen = Screen::new(rows, cols);

    let mut layout = startup_layout(&cli.files);
    urvim_core::session::set_enabled(cli.files.is_empty());
    globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
    globals::with_buffer_pool(|pool| {
        pool.request_syntax_refresh_at_startup(
            Some(layout.active_buffer_view().buffer_id()),
            layout.active_buffer_view().scroll_offset().row as usize,
            rows.saturating_sub(1) as usize,
            config.syntax,
        );
    });
    globals::set_lsp_runtime(urvim_core::lsp::runtime::LspRuntime::new(&config));
    globals::with_lsp_runtime_mut(|runtime| runtime.sync());

    terminal.set_cursor_style(layout.active_window_cursor_style())?;

    let mut needs_redraw = true;
    loop {
        let background_requested_redraw = globals::with_buffer_pool(|pool| {
            let jobs = pool.process_background_jobs();
            let disk = pool.process_external_file_changes();
            jobs || disk
        }) || layout.process_background_jobs()
            || layout.process_workspace_file_operations()
            || globals::take_notification_redraw_requested();

        globals::try_with_lsp_runtime_mut(|runtime| runtime.sync());

        while let Some(event) = plugin_runtime.poll_event() {
            if let PluginRuntimeEvent::RequestReceived { plugin, request } = &event {
                handle_plugin_editor_request(&mut plugin_runtime, &mut layout, plugin, request);
            }
            handle_plugin_runtime_event(&event);
            tracing::debug!(?event, "plugin runtime event");
            needs_redraw = true;
        }

        if background_requested_redraw {
            needs_redraw = true;
        }

        if layout.has_stale_visible_visuals() {
            needs_redraw = true;
        }

        if globals::take_inlay_hint_retry_requested() {
            layout.retry_inlay_hints();
            needs_redraw = true;
        }

        if render_frame_if_needed(
            needs_redraw,
            &mut layout,
            &mut screen,
            &mut terminal,
            rows,
            cols,
        )? {
            needs_redraw = false;
        }

        let event = terminal.read_event()?;
        let ui_event: UiEvent = event.into();

        match ui_event {
            UiEvent::Tick => {
                let ui_result = layout.route_ui_event(&UiEvent::Tick);
                if handle_ui_result_with_plugin_runtime(&mut layout, &mut plugin_runtime, ui_result)
                {
                    needs_redraw = true;
                    if layout.should_exit() {
                        break;
                    }
                }
                urvim_core::session::maybe_autosave(&layout);
                continue;
            }
            UiEvent::Paste(text) => {
                let overlay_result = layout.route_ui_event(&UiEvent::Paste(text.clone()));
                if handle_ui_result_with_plugin_runtime(
                    &mut layout,
                    &mut plugin_runtime,
                    overlay_result,
                ) {
                    needs_redraw = true;
                    if layout.should_exit() {
                        break;
                    }
                    continue;
                }

                let Some(action) =
                    raw_paste_action_for_mode(layout.active_window_mode_kind(), text)
                else {
                    tracing::debug!("ignoring raw paste event in unsupported mode");
                    continue;
                };

                let handled =
                    process_intent_queue(&mut layout, vec![Intent::Action(action.clone())]);
                if handled {
                    if let Some(to_mode) = action.to_mode {
                        let repeat_text = {
                            let window = layout.active_window_group_mut().active_window_mut();
                            window.switch_mode(to_mode)
                        };
                        terminal.set_cursor_style(layout.active_window_cursor_style())?;
                        if let Some(repeat_text) = repeat_text.filter(|text| !text.is_empty())
                            && let Some(mut repeat_state) = globals::get_last_repeat()
                        {
                            repeat_state.insert_text = Some(repeat_text);
                            globals::set_last_repeat(repeat_state);
                        }
                    }

                    if action.is_snapshottable() {
                        let cursor = layout.active_buffer_view().cursor();
                        layout
                            .active_buffer_view()
                            .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
                            .unwrap_or(());
                    }

                    if action.updates_snapshot_cursor() {
                        let cursor = layout.active_buffer_view().cursor();
                        layout
                            .active_buffer_view()
                            .with_buffer_mut(|buffer| buffer.update_cursor(cursor))
                            .unwrap_or(());
                    }

                    needs_redraw = true;
                }

                if layout.should_exit() {
                    break;
                }

                terminal.set_cursor_style(layout.active_window_cursor_style())?;
            }
            UiEvent::Resize(new_rows, new_cols) => {
                rows = new_rows;
                cols = new_cols;
                handle_resize(&mut terminal, &mut screen, rows, cols)?;
                needs_redraw = true;
            }
            UiEvent::Key(key) => {
                let overlay_result = layout.route_ui_event(&UiEvent::Key(key));
                if handle_ui_result_with_plugin_runtime(
                    &mut layout,
                    &mut plugin_runtime,
                    overlay_result,
                ) {
                    needs_redraw = true;
                    if layout.should_exit() {
                        break;
                    }
                    terminal.set_cursor_style(layout.active_window_cursor_style())?;
                    continue;
                }

                let result = layout
                    .active_window_group_mut()
                    .active_window_mut()
                    .handle_key(&key);

                match result {
                    HandleKeyResult::Complete(intent) => match intent {
                        Intent::Action(action) => {
                            if execute_action_intent(&mut layout, action) {
                                needs_redraw = true;
                                terminal.set_cursor_style(layout.active_window_cursor_style())?;
                            }
                        }
                        Intent::Command(command) => {
                            if matches!(command, Command::Quit | Command::TryQuit) {
                                urvim_core::session::save_now(&layout);
                            }

                            if matches!(command, Command::Quit) {
                                break;
                            }

                            if let Command::OverwriteBuffer(target) = &command {
                                let handled = handle_save_buffer_action(&mut layout, *target, true);
                                if handled {
                                    needs_redraw = true;
                                }

                                if layout.should_exit() {
                                    break;
                                }

                                terminal.set_cursor_style(layout.active_window_cursor_style())?;
                                continue;
                            }

                            let handled = process_intent_queue_with_plugin_runtime(
                                &mut layout,
                                Some(&mut plugin_runtime),
                                vec![Intent::Command(command.clone())],
                            );
                            if handled {
                                needs_redraw = true;
                            }

                            if layout.should_exit() {
                                break;
                            }

                            terminal.set_cursor_style(layout.active_window_cursor_style())?;
                        }
                    },
                    HandleKeyResult::WaitForMore => {
                        // Continue waiting for more keys, no action taken
                    }
                    HandleKeyResult::InvalidSequence => {
                        // Ignore invalid sequences
                    }
                }
            }
        }
    }

    globals::shutdown_lsp_runtime();
    plugin_runtime.shutdown();
    urvim_core::session::save_now(&layout);
    terminal.reset_style()?;

    Ok(())
}

fn handle_plugin_editor_request(
    plugin_runtime: &mut urvim_plugin::PluginRuntime,
    layout: &mut Layout,
    plugin: &str,
    request: &urvim_plugin::PluginRequest,
) {
    let response = resolve_plugin_editor_request(layout, request);
    if let Err(error) = plugin_runtime.send_response(plugin, response) {
        tracing::warn!(plugin, request_id = request.id, error = %error, "failed to send plugin editor response");
    }
}

fn handle_plugin_runtime_event(event: &PluginRuntimeEvent) -> bool {
    match event {
        PluginRuntimeEvent::NotificationReceived {
            plugin,
            notification,
        } if notification.method == "editor/notify" => {
            handle_plugin_notify_notification(plugin, notification)
        }
        PluginRuntimeEvent::NotificationReceived {
            plugin,
            notification,
        } => {
            tracing::debug!(
                plugin,
                method = notification.method,
                "ignoring unknown plugin notification"
            );
            false
        }
        PluginRuntimeEvent::RequestReceived { .. }
        | PluginRuntimeEvent::ResponseReceived { .. }
        | PluginRuntimeEvent::ProcessExited { .. }
        | PluginRuntimeEvent::ProtocolError { .. }
        | PluginRuntimeEvent::RuntimeError { .. }
        | PluginRuntimeEvent::RequestTimedOut { .. }
        | PluginRuntimeEvent::RequestFailed { .. } => false,
    }
}

fn handle_plugin_notify_notification(
    plugin: &str,
    notification: &urvim_plugin::PluginNotification,
) -> bool {
    let Some(message) = notification
        .params
        .get("message")
        .and_then(|value| value.as_str())
    else {
        tracing::warn!(plugin, "plugin notification missing message");
        return false;
    };
    let level = match notification
        .params
        .get("level")
        .and_then(|value| value.as_str())
    {
        Some("info") | None => urvim_core::notification::NotificationLevel::Info,
        Some("warn") | Some("warning") => urvim_core::notification::NotificationLevel::Warn,
        Some("error") => urvim_core::notification::NotificationLevel::Error,
        Some(other) => {
            tracing::warn!(
                plugin,
                level = other,
                "plugin notification used unknown level"
            );
            urvim_core::notification::NotificationLevel::Warn
        }
    };

    globals::enqueue_notification(level, format!("{plugin}: {message}"))
}

fn resolve_plugin_editor_request(
    layout: &mut Layout,
    request: &urvim_plugin::PluginRequest,
) -> urvim_plugin::PluginResponse {
    match request.method.as_str() {
        "editor/getActiveBuffer" => plugin_active_buffer_response(layout, request.id),
        "editor/getBufferText" => plugin_buffer_text_response(layout, request),
        "editor/getConfig" => plugin_config_response(request.id),
        "editor/applyEdit" => plugin_apply_edit_response(layout, request),
        method => urvim_plugin::PluginResponse::error(
            request.id,
            format!("unsupported editor request method: {method}"),
        ),
    }
}

fn plugin_active_buffer_response(layout: &Layout, request_id: u64) -> urvim_plugin::PluginResponse {
    let view = layout.active_buffer_view();
    let buffer_id = view.buffer_id();
    let cursor = view.cursor();
    match view.with_buffer(|buffer| {
        serde_json::json!({
            "id": buffer_id.get(),
            "path": buffer.path().map(|path| path.as_path().to_string_lossy().into_owned()),
            "file_name": buffer.file_name().map(|name| name.to_string_lossy().into_owned()),
            "filetype": buffer.syntax_name(),
            "line_count": buffer.line_count(),
            "modified": buffer.is_modified(),
            "cursor": {
                "line": cursor.line,
                "col": cursor.col,
            },
        })
    }) {
        Some(result) => urvim_plugin::PluginResponse::success(request_id, result),
        None => urvim_plugin::PluginResponse::error(request_id, "active buffer is missing"),
    }
}

fn plugin_buffer_text_response(
    _layout: &Layout,
    request: &urvim_plugin::PluginRequest,
) -> urvim_plugin::PluginResponse {
    let Some(buffer_id) = request
        .params
        .get("buffer_id")
        .or_else(|| request.params.get("id"))
        .and_then(|value| value.as_u64())
    else {
        return urvim_plugin::PluginResponse::error(
            request.id,
            "editor/getBufferText requires buffer_id",
        );
    };
    let buffer_id = urvim_core::buffer::BufferId::new(buffer_id as usize);

    match globals::with_buffer(buffer_id, |buffer| {
        serde_json::json!({
            "buffer_id": buffer_id.get(),
            "text": buffer.as_str(),
        })
    }) {
        Some(result) => urvim_plugin::PluginResponse::success(request.id, result),
        None => urvim_plugin::PluginResponse::error(
            request.id,
            format!("unknown buffer_id {}", buffer_id.get()),
        ),
    }
}

fn plugin_config_response(request_id: u64) -> urvim_plugin::PluginResponse {
    let config = globals::with_config(Clone::clone).unwrap_or_default();
    urvim_plugin::PluginResponse::success(
        request_id,
        serde_json::json!({
            "theme": config.theme,
            "syntax": config.syntax,
            "active_line": config.active_line,
            "relative_number": config.relative_number,
            "indent_guides": config.indent_guides,
            "auto_close_pairs": config.auto_close_pairs,
            "tab_width": config.tab_width,
            "plugins": config.plugins.keys().cloned().collect::<Vec<_>>(),
        }),
    )
}

fn plugin_apply_edit_response(
    layout: &mut Layout,
    request: &urvim_plugin::PluginRequest,
) -> urvim_plugin::PluginResponse {
    match parse_plugin_edit_request(&request.params)
        .and_then(|edit| apply_plugin_edit(layout, edit))
    {
        Ok(result) => urvim_plugin::PluginResponse::success(request.id, result),
        Err(error) => {
            tracing::warn!(
                request_id = request.id,
                error,
                "invalid plugin edit request"
            );
            urvim_plugin::PluginResponse::error(request.id, error)
        }
    }
}

#[derive(Debug)]
struct PluginEditRequest {
    buffer_id: urvim_core::buffer::BufferId,
    kind: PluginEditKind,
    start: Cursor,
    end: Cursor,
    text: String,
}

#[derive(Debug, PartialEq, Eq)]
enum PluginEditKind {
    Insert,
    Delete,
    Replace,
}

fn parse_plugin_edit_request(params: &serde_json::Value) -> Result<PluginEditRequest, String> {
    let buffer_id = params
        .get("buffer_id")
        .or_else(|| params.get("id"))
        .and_then(|value| value.as_u64())
        .ok_or_else(|| "editor/applyEdit requires buffer_id".to_string())?;
    let kind = match params
        .get("kind")
        .or_else(|| params.get("operation"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| "editor/applyEdit requires kind".to_string())?
    {
        "insert" => PluginEditKind::Insert,
        "delete" => PluginEditKind::Delete,
        "replace" => PluginEditKind::Replace,
        other => return Err(format!("unsupported edit kind: {other}")),
    };
    let start = parse_plugin_cursor(params.get("start"), "start")?;
    let end = match kind {
        PluginEditKind::Insert => start,
        PluginEditKind::Delete | PluginEditKind::Replace => {
            parse_plugin_cursor(params.get("end"), "end")?
        }
    };
    let text_value = params.get("text").and_then(|value| value.as_str());
    if matches!(kind, PluginEditKind::Insert | PluginEditKind::Replace) && text_value.is_none() {
        return Err("insert and replace edits require text".to_string());
    }
    let text = text_value.unwrap_or("").to_string();

    Ok(PluginEditRequest {
        buffer_id: urvim_core::buffer::BufferId::new(buffer_id as usize),
        kind,
        start,
        end,
        text,
    })
}

fn parse_plugin_cursor(value: Option<&serde_json::Value>, name: &str) -> Result<Cursor, String> {
    let value = value.ok_or_else(|| format!("editor/applyEdit requires {name}"))?;
    let line = value
        .get("line")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| format!("{name}.line must be an unsigned integer"))?;
    let col = value
        .get("col")
        .or_else(|| value.get("column"))
        .and_then(|value| value.as_u64())
        .ok_or_else(|| format!("{name}.col must be an unsigned integer"))?;
    Ok(Cursor::new(line as usize, col as usize))
}

fn apply_plugin_edit(
    layout: &mut Layout,
    edit: PluginEditRequest,
) -> Result<serde_json::Value, String> {
    let active_buffer_id = layout.active_buffer_view().buffer_id();
    let applied = globals::with_buffer_mut(edit.buffer_id, |buffer| {
        if !buffer.is_valid_cursor(edit.start) {
            return Err(format!(
                "invalid start cursor {}:{}",
                edit.start.line, edit.start.col
            ));
        }
        if !buffer.is_valid_cursor(edit.end) {
            return Err(format!(
                "invalid end cursor {}:{}",
                edit.end.line, edit.end.col
            ));
        }
        if edit.start > edit.end {
            return Err("edit start must be before or equal to end".to_string());
        }

        match edit.kind {
            PluginEditKind::Insert => buffer.insert_text(edit.start, edit.text.as_str()),
            PluginEditKind::Delete => buffer.remove(edit.start, edit.end),
            PluginEditKind::Replace => {
                buffer.remove(edit.start, edit.end);
                buffer.insert_text(edit.start, edit.text.as_str());
            }
        }
        buffer.push_snapshot(edit.start);
        Ok(serde_json::json!({
            "buffer_id": edit.buffer_id.get(),
            "applied": true,
            "text": buffer.as_str(),
        }))
    })
    .ok_or_else(|| format!("unknown buffer_id {}", edit.buffer_id.get()))??;

    if edit.buffer_id == active_buffer_id {
        layout.active_buffer_view_mut().set_cursor(edit.start);
    }
    Ok(applied)
}

fn handle_resize<I: io::Read + AsFd, O: io::Write + AsFd>(
    terminal: &mut Terminal<I, O>,
    screen: &mut Screen,
    rows: u16,
    cols: u16,
) -> io::Result<()> {
    screen.resize(rows, cols);
    terminal.clear_screen()
}

fn select_active_theme(
    registry: &ThemeRegistry,
    requested: Option<&str>,
) -> Result<urvim_theme::Theme, String> {
    let theme_name = requested.unwrap_or("Friday Night");
    registry.get(theme_name).cloned().ok_or_else(|| {
        format!(
            "unknown theme {theme_name:?}; available themes: {}",
            registry.names().join(", ")
        )
    })
}

fn load_startup_plugins_and_themes(config: &Config) -> Result<StartupPluginsAndThemes, String> {
    let plugin_config = config
        .plugins
        .iter()
        .map(|(name, plugin)| {
            (
                name.clone(),
                PluginConfigEntry {
                    enabled: plugin.enabled,
                    path: plugin.path.clone(),
                },
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let plugin_registry = urvim_plugin::PluginRegistry::load_from_config(&plugin_config)
        .map_err(|error| error.to_string())?;
    urvim_core::command::install_configured_commands_with_plugins(config, &plugin_registry)
        .map_err(|error| error.to_string())?;
    let plugin_runtime = urvim_plugin::PluginRuntime::start_from_registry(&plugin_registry);

    let mut theme_registry = ThemeRegistry::load_builtin().map_err(|error| error.to_string())?;
    urvim_plugin::load_plugin_themes(&mut theme_registry, &plugin_registry)
        .map_err(|error| error.to_string())?;
    let active_theme = select_active_theme(&theme_registry, Some(config.theme.as_str()))?;

    Ok(StartupPluginsAndThemes {
        #[cfg(test)]
        plugin_registry,
        plugin_runtime,
        theme_registry,
        active_theme,
    })
}

fn handle_save_buffer_action(
    layout: &mut Layout,
    target: Option<urvim_core::buffer::BufferId>,
    force: bool,
) -> bool {
    let buffer_id = target.unwrap_or_else(|| layout.active_buffer_view().buffer_id());

    if !force
        && globals::with_buffer_pool(|pool| pool.buffer_needs_overwrite_confirmation(buffer_id))
    {
        layout.prompt_overwrite_buffer(buffer_id);
        return true;
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
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            tracing::info!("Skipping save for unnamed buffer {:?}", buffer_id);
        }
        Err(error) => {
            urvim_core::notify_error!("Failed to save buffer {:?}: {}", buffer_id, error);
        }
    }

    true
}

fn render_frame_if_needed<I: io::Read + AsFd, O: io::Write + AsFd>(
    needs_redraw: bool,
    layout: &mut Layout,
    screen: &mut Screen,
    terminal: &mut Terminal<I, O>,
    rows: u16,
    cols: u16,
) -> io::Result<bool> {
    if !needs_redraw {
        return Ok(false);
    }

    render_frame(layout, screen, terminal, rows, cols)?;
    Ok(true)
}

fn apply_undo_redo(layout: &mut Layout, redo: bool) -> bool {
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

fn commit_insert_exit_snapshot(layout: &mut Layout) {
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

fn render_frame<I: io::Read + AsFd, O: io::Write + AsFd>(
    layout: &mut Layout,
    screen: &mut Screen,
    terminal: &mut Terminal<I, O>,
    rows: u16,
    cols: u16,
) -> io::Result<()> {
    globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
    screen.clear();
    layout.render(screen, Position::new(0, 0), Size::new(rows, cols));
    screen.render(terminal)?;

    if let Some(cursor_pos) = layout.visual_cursor() {
        terminal.set_cursor_position(cursor_pos.row + 1, cursor_pos.col + 1)?;
    }

    Ok(())
}

fn replay_repeat_action(layout: &mut Layout, replay: &RepeatReplay) -> bool {
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

fn process_intent_queue(layout: &mut Layout, intents: Vec<Intent>) -> bool {
    process_intent_queue_with_plugin_runtime(layout, None, intents)
}

fn process_intent_queue_with_plugin_runtime(
    layout: &mut Layout,
    mut plugin_runtime: Option<&mut urvim_plugin::PluginRuntime>,
    intents: Vec<Intent>,
) -> bool {
    let mut queue: std::collections::VecDeque<Intent> = intents.into();
    let mut handled_all = true;
    let mut saw_intent = false;

    while let Some(intent) = queue.pop_front() {
        saw_intent = true;
        handled_all &= match intent {
            Intent::Action(action) => execute_action_intent(layout, action),
            Intent::Command(command) => {
                execute_command_intent(layout, plugin_runtime.as_deref_mut(), command)
            }
        };
    }

    saw_intent && handled_all
}

fn execute_command_intent(
    layout: &mut Layout,
    plugin_runtime: Option<&mut urvim_plugin::PluginRuntime>,
    command: Command,
) -> bool {
    if let Command::PluginRequest {
        plugin,
        command,
        method,
        params,
    } = command
    {
        let Some(plugin_runtime) = plugin_runtime else {
            tracing::warn!(plugin, command, method, "plugin command has no runtime");
            urvim_core::notify_warn!("Plugin command {plugin} {command} could not run: no runtime");
            return true;
        };

        match plugin_runtime.send_request(&plugin, method.as_str(), params) {
            Ok(request_id) => {
                tracing::debug!(
                    plugin,
                    command,
                    method,
                    request_id,
                    "sent plugin command request"
                );
            }
            Err(error) => {
                tracing::warn!(plugin, command, method, error = %error, "failed to send plugin command request");
                urvim_core::notify_warn!("Plugin command {plugin} {command} failed: {error}");
            }
        }
        return true;
    }

    if matches!(command, Command::PluginStatus) {
        if let Some(plugin_runtime) = plugin_runtime {
            notify_plugin_statuses(plugin_runtime.status_entries());
        } else {
            urvim_core::notify_info!("No plugin runtime is available");
        }
        return true;
    }

    layout.dispatch_intent(&Intent::Command(command))
}

fn notify_plugin_statuses(statuses: Vec<urvim_plugin::PluginStatusEntry>) {
    if statuses.is_empty() {
        urvim_core::notify_info!("No plugins loaded");
        return;
    }

    let summary = statuses
        .into_iter()
        .map(|status| {
            let state = match &status.state {
                urvim_plugin::PluginProcessState::NotConfigured => "not configured".to_string(),
                urvim_plugin::PluginProcessState::Starting => "starting".to_string(),
                urvim_plugin::PluginProcessState::Running => {
                    if status.capabilities.is_empty() {
                        "running".to_string()
                    } else {
                        format!("running [{}]", status.capabilities.join(", "))
                    }
                }
                urvim_plugin::PluginProcessState::Failed(error) => format!("failed: {error}"),
                urvim_plugin::PluginProcessState::Stopped => "stopped".to_string(),
            };
            format!("{}: {}", status.plugin, state)
        })
        .collect::<Vec<_>>()
        .join("; ");
    urvim_core::notify_info!("Plugins: {}", summary);
}

fn execute_action_intent(layout: &mut Layout, action: Action) -> bool {
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
                        _ => {
                            // Should have been handled by the window.
                        }
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

#[cfg(test)]
fn handle_ui_result(layout: &mut Layout, result: urvim_core::ui::UiEventResult) -> bool {
    let mut plugin_runtime = urvim_plugin::PluginRuntime::default();
    handle_ui_result_with_plugin_runtime(layout, &mut plugin_runtime, result)
}

fn handle_ui_result_with_plugin_runtime(
    layout: &mut Layout,
    plugin_runtime: &mut urvim_plugin::PluginRuntime,
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

fn raw_paste_action_for_mode(mode: ModeKind, text: String) -> Option<Action> {
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

fn startup_layout(files: &[urvim_core::cli::CliFileSpec]) -> Layout {
    let Ok(cwd) = std::env::current_dir() else {
        tracing::warn!("failed to resolve current directory for startup");
        return Layout::from_cli_files(files);
    };

    startup_layout_for_cwd(&cwd, files)
}

fn startup_layout_for_cwd(cwd: &std::path::Path, files: &[urvim_core::cli::CliFileSpec]) -> Layout {
    if files.is_empty() {
        match urvim_core::session::load_session_for_cwd(cwd) {
            Ok(Some(session)) => Layout::from_session(session),
            Ok(None) => Layout::from_cli_files(&[]),
            Err(error) => {
                tracing::warn!(?error, "failed to load session");
                Layout::from_cli_files(&[])
            }
        }
    } else {
        Layout::from_cli_files(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex, OnceLock};
    use urvim_core::buffer::{Buffer, BufferId};
    use urvim_core::cli::CliFileSpec;
    use urvim_core::editor::ModeKind;
    use urvim_core::window::VisualSelectionKind;
    use urvim_core::window_group::WindowGroup;
    use urvim_terminal::{Event, Key, KeyCode};

    struct TestBackend {
        input: Arc<Mutex<VecDeque<u8>>>,
        output: Arc<Mutex<Vec<u8>>>,
    }

    fn repeat_state_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn notification_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    fn cwd_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn buffer_pool_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    impl TestBackend {
        fn new(data: Vec<u8>) -> Self {
            Self {
                input: Arc::new(Mutex::new(VecDeque::from(data))),
                output: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl Read for TestBackend {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut input = self.input.lock().unwrap();
            if input.is_empty() {
                return Ok(0);
            }
            let mut i = 0;
            while i < buf.len() {
                match input.pop_front() {
                    Some(b) => {
                        buf[i] = b;
                        i += 1;
                    }
                    None => break,
                }
            }
            Ok(i)
        }
    }

    impl Write for TestBackend {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut output = self.output.lock().unwrap();
            output.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl AsFd for TestBackend {
        fn as_fd(&self) -> rustix::fd::BorrowedFd<'_> {
            panic!("TestBackend does not have a valid file descriptor")
        }
    }

    #[test]
    fn test_handle_resize_clears_terminal() {
        let stdin = TestBackend::new(Vec::new());
        let stdout = TestBackend::new(Vec::new());
        let output = stdout.output.clone();
        let mut terminal = Terminal::new_for_testing(stdin, stdout);
        let mut screen = Screen::new(2, 2);

        handle_resize(&mut terminal, &mut screen, 3, 4).unwrap();

        assert_eq!(screen.size(), (3, 4));
        assert_eq!(output.lock().unwrap().as_slice(), b"\x1b[2J\x1b[H");
    }

    #[test]
    fn render_frame_if_needed_skips_idle_noop_frames() {
        let stdin = TestBackend::new(Vec::new());
        let stdout = TestBackend::new(Vec::new());
        let output = stdout.output.clone();
        let mut terminal = Terminal::new_for_testing(stdin, stdout);
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        let mut screen = Screen::new(1, 5);

        assert!(
            !render_frame_if_needed(false, &mut layout, &mut screen, &mut terminal, 1, 5).unwrap()
        );
        assert!(output.lock().unwrap().is_empty());
    }

    #[test]
    fn apply_undo_redo_requests_redraw_after_success() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));

        assert!(
            layout
                .active_buffer_view_mut()
                .with_buffer_mut(|buffer| {
                    buffer.insert_text(Cursor::new(0, 0), "hello");
                    buffer.push_snapshot(Cursor::new(0, 5));
                })
                .is_some()
        );

        assert!(apply_undo_redo(&mut layout, false));
        assert_eq!(layout.active_buffer_view().cursor(), Cursor::new(0, 0));
        assert_eq!(
            layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str().to_string()),
            Some(String::new())
        );

        assert!(apply_undo_redo(&mut layout, true));
        assert_eq!(layout.active_buffer_view().cursor(), Cursor::new(0, 5));
        assert_eq!(
            layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str().to_string()),
            Some("hello".to_string())
        );
    }

    #[test]
    fn plugin_active_buffer_request_returns_metadata() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        layout
            .active_buffer_view_mut()
            .with_buffer_mut(|buffer| buffer.set_syntax_name("rust"));
        let buffer_id = layout.active_buffer_view().buffer_id().get();
        let request =
            urvim_plugin::PluginRequest::new(7, "editor/getActiveBuffer", serde_json::json!({}));

        let response = resolve_plugin_editor_request(&mut layout, &request);

        assert_eq!(response.id, 7);
        assert!(response.error.is_none());
        let result = response.result.expect("response should include result");
        assert_eq!(result["id"], serde_json::json!(buffer_id));
        assert_eq!(result["filetype"], serde_json::json!("rust"));
        assert_eq!(result["line_count"], serde_json::json!(1));
    }

    #[test]
    fn plugin_buffer_text_request_returns_content() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "one\ntwo",
        )]));
        let buffer_id = layout.active_buffer_view().buffer_id().get();
        let request = urvim_plugin::PluginRequest::new(
            8,
            "editor/getBufferText",
            serde_json::json!({ "buffer_id": buffer_id }),
        );

        let response = resolve_plugin_editor_request(&mut layout, &request);

        assert_eq!(response.id, 8);
        assert!(response.error.is_none());
        let result = response.result.expect("response should include result");
        assert_eq!(result["buffer_id"], serde_json::json!(buffer_id));
        assert_eq!(result["text"], serde_json::json!("one\ntwo"));
    }

    #[test]
    fn plugin_config_request_returns_safe_subset() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
        let config = Config {
            theme: "Demo Night".to_string(),
            syntax: false,
            tab_width: 2,
            ..Config::default()
        };
        globals::set_config(config);
        let request =
            urvim_plugin::PluginRequest::new(9, "editor/getConfig", serde_json::json!({}));

        let response = resolve_plugin_editor_request(&mut layout, &request);

        assert_eq!(response.id, 9);
        assert!(response.error.is_none());
        let result = response.result.expect("response should include result");
        assert_eq!(result["theme"], serde_json::json!("Demo Night"));
        assert_eq!(result["syntax"], serde_json::json!(false));
        assert_eq!(result["tab_width"], serde_json::json!(2));
        assert!(result.get("keymaps").is_none());
    }

    #[test]
    fn plugin_buffer_text_request_errors_for_unknown_buffer() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
        let request = urvim_plugin::PluginRequest::new(
            10,
            "editor/getBufferText",
            serde_json::json!({ "buffer_id": 999999 }),
        );

        let response = resolve_plugin_editor_request(&mut layout, &request);

        assert_eq!(response.id, 10);
        assert!(response.error.unwrap().contains("unknown buffer_id"));
    }

    #[test]
    fn plugin_editor_request_errors_for_unsupported_method() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));
        let request = urvim_plugin::PluginRequest::new(11, "editor/mutate", serde_json::json!({}));

        let response = resolve_plugin_editor_request(&mut layout, &request);

        assert_eq!(response.id, 11);
        assert!(
            response
                .error
                .unwrap()
                .contains("unsupported editor request")
        );
    }

    #[test]
    fn plugin_apply_edit_inserts_text() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("world")]));
        let buffer_id = layout.active_buffer_view().buffer_id().get();
        let request = urvim_plugin::PluginRequest::new(
            12,
            "editor/applyEdit",
            serde_json::json!({
                "buffer_id": buffer_id,
                "kind": "insert",
                "start": { "line": 0, "col": 0 },
                "text": "hello ",
            }),
        );

        let response = resolve_plugin_editor_request(&mut layout, &request);

        assert!(response.error.is_none());
        assert_eq!(
            layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str()),
            Some("hello world".to_string())
        );
    }

    #[test]
    fn plugin_apply_edit_replaces_text() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "hello old",
        )]));
        let buffer_id = layout.active_buffer_view().buffer_id().get();
        let request = urvim_plugin::PluginRequest::new(
            13,
            "editor/applyEdit",
            serde_json::json!({
                "buffer_id": buffer_id,
                "kind": "replace",
                "start": { "line": 0, "col": 6 },
                "end": { "line": 0, "col": 9 },
                "text": "new",
            }),
        );

        let response = resolve_plugin_editor_request(&mut layout, &request);

        assert!(response.error.is_none());
        assert_eq!(
            layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str()),
            Some("hello new".to_string())
        );
    }

    #[test]
    fn plugin_apply_edit_rejects_invalid_range_without_mutating() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        let buffer_id = layout.active_buffer_view().buffer_id().get();
        let request = urvim_plugin::PluginRequest::new(
            14,
            "editor/applyEdit",
            serde_json::json!({
                "buffer_id": buffer_id,
                "kind": "delete",
                "start": { "line": 0, "col": 99 },
                "end": { "line": 0, "col": 100 },
            }),
        );

        let response = resolve_plugin_editor_request(&mut layout, &request);

        assert!(response.error.unwrap().contains("invalid start cursor"));
        assert_eq!(
            layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str()),
            Some("hello".to_string())
        );
    }

    #[test]
    fn plugin_apply_edit_participates_in_undo() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        let buffer_id = layout.active_buffer_view().buffer_id().get();
        let request = urvim_plugin::PluginRequest::new(
            15,
            "editor/applyEdit",
            serde_json::json!({
                "buffer_id": buffer_id,
                "kind": "insert",
                "start": { "line": 0, "col": 5 },
                "text": "!",
            }),
        );

        let response = resolve_plugin_editor_request(&mut layout, &request);

        assert!(response.error.is_none());
        assert_eq!(
            layout
                .active_buffer_view_mut()
                .with_buffer_mut(|buffer| buffer.undo())
                .flatten(),
            Some(Cursor::new(0, 0))
        );
        assert_eq!(
            layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str()),
            Some("hello".to_string())
        );
    }

    #[test]
    fn insert_exit_commits_single_undo_snapshot_when_text_changes() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("world")]));

        assert!(
            layout
                .active_buffer_view_mut()
                .with_buffer_mut(|buffer| buffer.insert_text(Cursor::new(0, 0), "hello"))
                .is_some()
        );
        layout
            .active_buffer_view_mut()
            .set_cursor(Cursor::new(0, 5));

        commit_insert_exit_snapshot(&mut layout);

        assert!(
            layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.can_undo())
                .unwrap_or(false)
        );

        assert_eq!(
            layout
                .active_buffer_view_mut()
                .with_buffer_mut(|buffer| buffer.undo())
                .flatten(),
            Some(Cursor::new(0, 0))
        );
        assert_eq!(
            layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str().to_string()),
            Some("world".to_string())
        );
    }

    #[test]
    fn insert_exit_does_not_commit_snapshot_without_text_changes() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("world")]));

        commit_insert_exit_snapshot(&mut layout);

        assert!(
            !layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.can_undo())
                .unwrap_or(true)
        );
    }

    #[test]
    fn select_active_theme_defaults_to_friday_night() {
        let registry = ThemeRegistry::load_builtin().expect("builtins should load");

        let theme = select_active_theme(&registry, None).expect("default theme should exist");

        assert_eq!(theme.name(), "Friday Night");
    }

    #[test]
    fn select_active_theme_can_select_nord() {
        let registry = ThemeRegistry::load_builtin().expect("builtins should load");

        let theme = select_active_theme(&registry, Some("Nord")).expect("Nord theme should exist");

        assert_eq!(theme.name(), "Nord");
    }

    #[test]
    fn select_active_theme_reports_unknown_theme() {
        let registry = ThemeRegistry::load_builtin().expect("builtins should load");

        let error =
            select_active_theme(&registry, Some("missing")).expect_err("unknown theme should fail");

        assert!(error.contains("missing"));
        assert!(error.contains("Friday Night"));
    }

    #[test]
    fn load_startup_plugins_and_themes_loads_example_plugin_theme_and_scripts() {
        let config = Config {
            theme: "Demo Night".to_string(),
            plugins: std::collections::BTreeMap::from([(
                "demo-plugin".to_string(),
                urvim_core::config::PluginConfig {
                    enabled: true,
                    path: std::path::PathBuf::from(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../examples/plugins/demo-plugin"
                    )),
                },
            )]),
            ..Config::default()
        };

        let startup =
            load_startup_plugins_and_themes(&config).expect("startup plugins should load");

        assert_eq!(startup.active_theme.name(), "Demo Night");
        assert!(startup.theme_registry.get("Demo Night").is_some());
        assert!(startup.plugin_registry.get("demo-plugin").is_some());

        assert_eq!(
            startup
                .plugin_registry
                .script("demo-plugin", "wq")
                .map(|script| script.len()),
            Some(2)
        );
    }

    #[test]
    fn load_startup_plugins_and_themes_skips_disabled_missing_plugins() {
        let config = Config {
            plugins: std::collections::BTreeMap::from([(
                "missing-demo".to_string(),
                urvim_core::config::PluginConfig {
                    enabled: false,
                    path: std::path::PathBuf::from("/tmp/urvim-missing-disabled-plugin"),
                },
            )]),
            ..Config::default()
        };

        let startup = load_startup_plugins_and_themes(&config)
            .expect("disabled missing plugin should not fail startup");

        assert_eq!(startup.active_theme.name(), "Friday Night");
        assert!(startup.plugin_registry.is_empty());
    }

    #[test]
    fn terminal_event_adapter_converts_event_variants() {
        let key_event = Event::Key(urvim_terminal::Key::new(urvim_terminal::KeyCode::Char('x')));
        assert!(matches!(
            UiEvent::from(key_event),
            UiEvent::Key(key) if key.code == urvim_terminal::KeyCode::Char('x')
        ));

        let resize_event = Event::Resize(40, 120);
        assert_eq!(UiEvent::from(resize_event), UiEvent::Resize(40, 120));

        let paste_event = Event::Paste("abc".to_string());
        assert_eq!(
            UiEvent::from(paste_event),
            UiEvent::Paste("abc".to_string())
        );

        assert_eq!(UiEvent::from(Event::Tick), UiEvent::Tick);
    }

    #[test]
    fn process_intent_queue_dispatches_actions() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![
            Buffer::new(),
            Buffer::new(),
        ]));

        assert!(process_intent_queue(
            &mut layout,
            vec![Intent::Action(Action::new(ActionKind::NextTab))],
        ));
        assert_eq!(layout.window_group().active_tab_index(), 1);
    }

    #[test]
    fn process_intent_queue_records_repeat_state_for_command_actions() {
        let _guard = repeat_state_lock();
        globals::set_last_repeat(globals::RepeatState {
            action: Action::new(ActionKind::NextTab),
            count: 99,
            insert_text: Some("stale".to_string()),
        });
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "alpha\nbeta",
        )]));

        assert!(process_intent_queue(
            &mut layout,
            vec![Intent::Action(Action::new(ActionKind::DeleteLine))],
        ));

        let repeat = globals::get_last_repeat().expect("repeat state should be recorded");
        assert!(matches!(
            repeat.action.kind.as_ref(),
            Some(ActionKind::DeleteLine)
        ));
        assert_eq!(repeat.count, 1);
        assert!(repeat.insert_text.is_none());

        let cursor = layout
            .active_buffer_view_mut()
            .with_buffer_mut(|buffer| buffer.undo())
            .flatten()
            .expect("undo should restore the deleted line");
        layout.active_buffer_view_mut().set_cursor_synced(cursor);
        assert_eq!(
            layout
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str())
                .expect("buffer should be available"),
            "alpha\nbeta"
        );
    }

    #[test]
    fn process_intent_queue_returns_false_when_any_intent_is_unhandled() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));

        let handled = process_intent_queue(
            &mut layout,
            vec![
                Intent::Command(urvim_core::ui::Command::EnqueueNotification {
                    level: urvim_core::notification::NotificationLevel::Info,
                    message: "queued".to_string(),
                }),
                Intent::Action(Action::new(ActionKind::VisualTextObject(
                    urvim_core::editor::TextObject::InnerWord,
                ))),
            ],
        );

        assert!(!handled);
    }

    #[test]
    fn confirmed_try_quit_flows_through_ui_result_handling_and_exits() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
        let cursor = Cursor::new(0, 1);
        layout
            .active_buffer_view_mut()
            .with_buffer_mut(|buffer| buffer.insert_text(cursor, "x"));

        assert!(layout.dispatch_intent(&Intent::Command(Command::TryQuit)));
        assert!(!layout.should_exit());

        let ui_result = layout.route_ui_event(&UiEvent::Key(Key::new(KeyCode::Char('y'))));
        assert!(handle_ui_result(&mut layout, ui_result));
        assert!(layout.should_exit());
    }

    #[test]
    fn handle_save_buffer_action_emits_success_notification() {
        let _guard = notification_test_lock();
        globals::clear_notifications();

        let unique = format!(
            "urvim-save-success-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        let absolute_path = urvim_core::AbsolutePath::from_path(path.as_path())
            .expect("temp path should resolve absolutely");

        let mut buffer = Buffer::with_path(absolute_path);
        buffer.insert_text(Cursor::new(0, 0), "hello");

        let mut layout = Layout::new(WindowGroup::from_buffers(vec![buffer]));
        assert!(handle_save_buffer_action(&mut layout, None, false,));

        let saved_text = std::fs::read_to_string(path).expect("saved file should be readable");
        assert_eq!(saved_text, "hello");
    }

    #[test]
    fn handle_save_buffer_action_prompts_when_disk_changed() {
        let _pool_guard = buffer_pool_lock();
        let _notification_guard = notification_test_lock();
        globals::clear_notifications();

        let unique = format!(
            "urvim-save-confirm-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::write(&path, "alpha").unwrap();
        let buffer = Buffer::load_from_file(&path).unwrap();

        let mut layout = Layout::new(WindowGroup::from_buffers(vec![buffer]));
        layout
            .active_buffer_view_mut()
            .with_buffer_mut(|buffer| buffer.insert_text(Cursor::new(0, 5), "-dirty"))
            .unwrap();
        std::fs::write(&path, "alpha-external").unwrap();

        let buffer_id = layout.active_buffer_view().buffer_id();
        assert!(handle_save_buffer_action(
            &mut layout,
            Some(buffer_id),
            false,
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha-external");

        let ui_result = layout.route_ui_event(&UiEvent::Key(Key::new(KeyCode::Enter)));
        assert!(handle_ui_result(&mut layout, ui_result));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha-dirty");

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn handle_save_buffer_action_emits_error_notification_for_missing_buffer() {
        let _guard = notification_test_lock();
        globals::clear_notifications();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));

        assert!(handle_save_buffer_action(
            &mut layout,
            Some(BufferId::new(usize::MAX)),
            false,
        ));
    }

    #[test]
    fn try_quit_saves_session_before_layout_is_cleared() {
        let _guard = cwd_lock();
        let temp_dir = std::env::temp_dir().join(format!(
            "urvim-try-quit-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        (|| {
            urvim_core::session::set_enabled(true);

            let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
            assert!(process_intent_queue(
                &mut layout,
                vec![Intent::Command(Command::SplitVertical)],
            ));

            urvim_core::session::save_now(&layout);
            let session_file = urvim_core::session::load_current_cwd()
                .expect("session should load")
                .expect("session should exist");

            match session_file.root {
                urvim_core::session::SessionNode::Split(_) => {}
                other => panic!("expected split session root, got {other:?}"),
            }

            assert!(process_intent_queue(
                &mut layout,
                vec![Intent::Command(Command::TryQuit)],
            ));

            let saved = urvim_core::session::load_current_cwd()
                .expect("session should still load")
                .expect("session should still exist");
            match saved.root {
                urvim_core::session::SessionNode::Split(_) => {}
                other => panic!("expected split session root after try-quit, got {other:?}"),
            }
        })();

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn quit_saves_session_before_layout_is_cleared() {
        let _guard = cwd_lock();
        let temp_dir = std::env::temp_dir().join(format!(
            "urvim-quit-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        (|| {
            urvim_core::session::set_enabled(true);

            let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
            assert!(process_intent_queue(
                &mut layout,
                vec![Intent::Command(Command::SplitVertical)],
            ));

            urvim_core::session::save_now(&layout);
            let session_file = urvim_core::session::load_current_cwd()
                .expect("session should load")
                .expect("session should exist");
            assert!(matches!(
                session_file.root,
                urvim_core::session::SessionNode::Split(_)
            ));

            assert!(process_intent_queue(
                &mut layout,
                vec![Intent::Command(Command::Quit)],
            ));

            let saved = urvim_core::session::load_current_cwd()
                .expect("session should still load")
                .expect("session should still exist");
            assert!(matches!(
                saved.root,
                urvim_core::session::SessionNode::Split(_)
            ));
        })();

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn startup_layout_restores_existing_session_when_no_files_are_passed() {
        let _guard = cwd_lock();
        let temp_dir = std::env::temp_dir().join(format!(
            "urvim-startup-restore-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let path = temp_dir.join("restore.txt");
        std::fs::write(&path, "saved session file\nsecond line").unwrap();
        let session = urvim_core::session::SessionFile {
            version: 1,
            cwd: temp_dir.display().to_string(),
            label: temp_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("cwd")
                .to_string(),
            focused_pane: 0,
            root: urvim_core::session::SessionNode::Pane(urvim_core::session::SessionPane {
                pane_id: 0,
                window_group: urvim_core::session::SessionWindowGroup {
                    active_tab: 0,
                    tabs: vec![urvim_core::session::SessionWindow {
                        path: path.display().to_string(),
                        cursor: urvim_core::session::SessionCursor { row: 1, col: 0 },
                        scroll_offset: urvim_core::session::SessionPosition { row: 0, col: 0 },
                        wrapped_row_offset: 0,
                        wrap_enabled: false,
                    }],
                },
            }),
        };
        urvim_core::session::save_session_for_cwd(&temp_dir, &session).unwrap();

        let loaded = urvim_core::session::load_session_for_cwd(&temp_dir).unwrap();
        assert!(loaded.is_some());

        let restored = startup_layout_for_cwd(&temp_dir, &[]);
        assert_eq!(
            restored
                .active_buffer_view()
                .with_buffer(|buffer| buffer
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned()))
                .unwrap(),
            Some("restore.txt".to_string())
        );
        assert_eq!(restored.active_buffer_view().cursor(), Cursor::new(1, 0));
    }

    #[test]
    fn startup_layout_uses_blank_buffer_when_no_session_exists() {
        let _guard = cwd_lock();
        let temp_dir = std::env::temp_dir().join(format!(
            "urvim-startup-blank-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let restored = startup_layout_for_cwd(&temp_dir, &[]);
        assert_eq!(
            restored
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str())
                .unwrap(),
            ""
        );
    }

    #[test]
    fn startup_layout_with_files_does_not_restore_session() {
        let _guard = cwd_lock();
        let temp_dir = std::env::temp_dir().join(format!(
            "urvim-startup-files-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let session_path = temp_dir.join("session.txt");
        std::fs::write(&session_path, "session state").unwrap();
        let saved_layout = Layout::new(WindowGroup::from_paths(&[session_path.clone()]));
        urvim_core::session::save_session_for_cwd(&temp_dir, &saved_layout.to_session()).unwrap();

        let cli_path = temp_dir.join("cli.txt");
        std::fs::write(&cli_path, "cli file").unwrap();
        let restored = startup_layout_for_cwd(
            &temp_dir,
            &[CliFileSpec {
                path: cli_path.clone(),
                cursor: None,
            }],
        );

        assert_eq!(
            restored
                .active_buffer_view()
                .with_buffer(|buffer| buffer.as_str())
                .unwrap(),
            "cli file"
        );
        assert_eq!(
            restored
                .active_buffer_view()
                .with_buffer(|buffer| buffer
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned()))
                .unwrap(),
            Some("cli.txt".to_string())
        );
    }

    #[test]
    fn raw_paste_action_for_insert_and_normal_modes_inserts_text() {
        let insert = raw_paste_action_for_mode(ModeKind::Insert, "hello".to_string())
            .expect("insert mode paste should be handled");
        let normal = raw_paste_action_for_mode(ModeKind::Normal, "hello".to_string())
            .expect("normal mode paste should be handled");

        assert!(matches!(
            insert.kind.as_ref(),
            Some(ActionKind::InsertRawPaste(text)) if text == "hello"
        ));
        assert_eq!(insert.from_mode, Some(ModeKind::Insert));
        assert_eq!(insert.to_mode, None);

        assert!(matches!(
            normal.kind.as_ref(),
            Some(ActionKind::InsertRawPaste(text)) if text == "hello"
        ));
        assert_eq!(normal.from_mode, Some(ModeKind::Normal));
        assert_eq!(normal.to_mode, None);
    }

    #[test]
    fn raw_paste_action_for_visual_modes_replaces_selection_then_exits() {
        let visual = raw_paste_action_for_mode(ModeKind::Visual, "hello".to_string())
            .expect("visual mode paste should be handled");
        let visual_line = raw_paste_action_for_mode(ModeKind::VisualLine, "hello".to_string())
            .expect("visual line mode paste should be handled");

        assert!(matches!(
            visual.kind.as_ref(),
            Some(ActionKind::ReplaceSelectionRawPaste(text)) if text == "hello"
        ));
        assert_eq!(visual.from_mode, Some(ModeKind::Visual));
        assert_eq!(visual.to_mode, Some(ModeKind::Normal));

        assert!(matches!(
            visual_line.kind.as_ref(),
            Some(ActionKind::ReplaceSelectionRawPaste(text)) if text == "hello"
        ));
        assert_eq!(visual_line.from_mode, Some(ModeKind::VisualLine));
        assert_eq!(visual_line.to_mode, Some(ModeKind::Normal));
    }

    #[test]
    fn resolve_repeat_action_uses_stored_repeat_state() {
        use urvim_core::editor::ActionKind;

        let _guard = repeat_state_lock();
        globals::set_last_repeat(globals::RepeatState {
            action: Action::new(ActionKind::DeleteLine),
            count: 3,
            insert_text: Some("hello".to_string()),
        });

        let replay = Action::new(ActionKind::RepeatLastChange)
            .resolve_dot_repeat()
            .expect("repeat should resolve");
        assert!(matches!(
            replay.action.kind.as_ref(),
            Some(ActionKind::DeleteLine)
        ));
        assert_eq!(replay.structural_count, 3);
        assert_eq!(replay.repeat_count, 1);
        assert_eq!(replay.insert_text.as_deref(), Some("hello"));
    }

    #[test]
    fn resolve_repeat_action_overrides_the_stored_count() {
        use urvim_core::editor::ActionKind;

        let _guard = repeat_state_lock();
        globals::set_last_repeat(globals::RepeatState {
            action: Action::new(ActionKind::DeleteLine),
            count: 3,
            insert_text: None,
        });

        let replay = Action::count(2, Box::new(Action::new(ActionKind::RepeatLastChange)))
            .resolve_dot_repeat()
            .expect("repeat should resolve");
        assert!(matches!(
            replay.action.kind.as_ref(),
            Some(ActionKind::DeleteLine)
        ));
        assert_eq!(replay.structural_count, 3);
        assert_eq!(replay.repeat_count, 2);
        assert_eq!(replay.insert_text, None);
    }

    #[test]
    fn replay_repeat_action_applies_structural_count_once_before_insert_text() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str(
            "line1\nline2\nline3",
        )]));
        let replay = RepeatReplay {
            action: Action::new(ActionKind::ChangeLine),
            structural_count: 2,
            repeat_count: 1,
            insert_text: Some("hello".to_string()),
        };

        assert!(replay_repeat_action(&mut layout, &replay));
        let text = layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str())
            .expect("buffer should exist");
        assert_eq!(text, "hello\nline3");
        assert_eq!(layout.active_buffer_view().cursor(), Cursor::new(0, 5));
    }

    #[test]
    fn replay_repeat_action_replays_direct_insert_text() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("world")]));
        let replay = RepeatReplay {
            action: Action::mode_transition(ModeKind::Insert),
            structural_count: 1,
            repeat_count: 1,
            insert_text: Some("hello ".to_string()),
        };

        assert!(replay_repeat_action(&mut layout, &replay));
        let text = layout
            .active_buffer_view()
            .with_buffer(|buffer| buffer.as_str())
            .expect("buffer should exist");
        assert_eq!(text, "hello world");
        assert_eq!(layout.active_buffer_view().cursor(), Cursor::new(0, 6));
    }

    #[test]
    fn switch_mode_clears_visual_selection_when_leaving_visual() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));
        let enter_repeat_text = layout
            .window_group_mut()
            .active_window_mut()
            .switch_mode(ModeKind::Visual);
        let repeat_text = layout
            .window_group_mut()
            .active_window_mut()
            .switch_mode(ModeKind::Normal);

        assert_eq!(
            layout.window_group().active_window_mode_kind(),
            ModeKind::Normal
        );
        assert!(enter_repeat_text.is_none());
        assert!(repeat_text.is_none());
        assert!(layout.active_buffer_view().visual_selection().is_none());
    }

    #[test]
    fn switch_mode_restarts_visual_selection_when_entering_visual() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));

        let repeat_text = layout
            .window_group_mut()
            .active_window_mut()
            .switch_mode(ModeKind::Visual);

        assert_eq!(
            layout.window_group().active_window_mode_kind(),
            ModeKind::Visual
        );
        assert!(repeat_text.is_none());
        assert!(layout.active_buffer_view().visual_selection().is_some());
    }

    #[test]
    fn switch_mode_starts_linewise_visual_selection_when_entering_visual_line() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("hello")]));

        let repeat_text = layout
            .window_group_mut()
            .active_window_mut()
            .switch_mode(ModeKind::VisualLine);

        assert_eq!(
            layout.window_group().active_window_mode_kind(),
            ModeKind::VisualLine
        );
        assert!(repeat_text.is_none());
        let selection = layout
            .active_buffer_view()
            .visual_selection()
            .expect("linewise selection should exist");
        assert_eq!(selection.kind, VisualSelectionKind::Line);
    }
}
