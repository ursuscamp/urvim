use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use crate::actions::{execute_action_intent_with_plugin_runtime, process_intent_queue};
use crate::plugin::BearscriptPluginRuntime;
use crate::render::{handle_resize, render_frame_if_needed};
use crate::startup::{StartupPluginsAndThemes, load_startup_plugins_and_themes, startup_layout};
use urvim_core::config::Config;
use urvim_core::editor::HandleKeyResult;
use urvim_core::event::EditorEvent;
use urvim_core::globals;
use urvim_core::screen::Screen;
use urvim_core::ui::{Command, Intent, UiEvent};
use urvim_terminal::{Terminal, size::get_terminal_size};

use super::Cli;

/// Drains queued editor events and dispatches them to plugin event hooks in
/// FIFO order. Returns `true` when at least one event was dispatched.
fn drain_editor_events(plugin_runtime: &mut BearscriptPluginRuntime) -> bool {
    let mut dispatched = false;
    while let Some(event) = globals::take_editor_event() {
        if plugin_runtime.dispatch_editor_event(event) {
            dispatched = true;
        }
    }
    dispatched
}

pub(super) fn run(cli: Cli) -> io::Result<()> {
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

    let mut terminal = Terminal::new(stdin, stdout)?;

    let (mut rows, mut cols) = get_terminal_size().unwrap_or((24, 80));
    let mut screen = Screen::new(rows, cols);

    let layout = Rc::new(RefCell::new(startup_layout(&cli.files)));

    let StartupPluginsAndThemes {
        #[cfg(test)]
            plugin_registry: _,
        plugin_runtime,
        theme_registry: registry,
        active_theme,
    } = load_startup_plugins_and_themes(&config, Rc::clone(&layout)).map_err(|error| {
        eprintln!("Error: {}", error);
        io::Error::new(io::ErrorKind::InvalidData, error)
    })?;
    globals::set_active_theme(active_theme);
    globals::set_theme_registry(registry);
    let mut plugin_runtime = plugin_runtime;

    urvim_core::session::set_enabled(cli.files.is_empty());
    globals::set_active_buffer_id(layout.borrow().active_buffer_view().buffer_id());
    globals::with_buffer_pool(|pool| {
        let layout = layout.borrow();
        pool.request_syntax_refresh_at_startup(
            Some(layout.active_buffer_view().buffer_id()),
            layout.active_buffer_view().scroll_offset().row as usize,
            rows.saturating_sub(1) as usize,
            config.syntax,
        );
    });
    globals::set_lsp_runtime(urvim_core::lsp::runtime::LspRuntime::new(&config));
    globals::with_lsp_runtime_mut(|runtime| runtime.sync());
    globals::enqueue_editor_event(EditorEvent::EditorStarted);

    terminal.set_cursor_style(layout.borrow().active_window_cursor_style())?;

    let mut needs_redraw = true;
    loop {
        let background_requested_redraw = globals::with_buffer_pool(|pool| {
            let jobs = pool.process_background_jobs();
            let disk = pool.process_external_file_changes();
            jobs || disk
        }) || layout.borrow_mut().process_background_jobs()
            || layout.borrow_mut().process_workspace_file_operations()
            || globals::take_notification_redraw_requested();

        globals::try_with_lsp_runtime_mut(|runtime| runtime.sync());

        if drain_editor_events(&mut plugin_runtime) {
            needs_redraw = true;
        }

        if plugin_runtime.dispatch_job_events() {
            needs_redraw = true;
        }

        if plugin_runtime.dispatch_fs_events() {
            needs_redraw = true;
        }

        if plugin_runtime.dispatch_picker_events() {
            needs_redraw = true;
        }
        if plugin_runtime.dispatch_confirmation_events() {
            needs_redraw = true;
        }
        if plugin_runtime.dispatch_input_events() {
            needs_redraw = true;
        }

        if plugin_runtime.refresh_plugin_syntax() {
            needs_redraw = true;
        }

        if background_requested_redraw {
            needs_redraw = true;
        }

        if layout.borrow().has_stale_visible_visuals() {
            needs_redraw = true;
        }

        if globals::take_inlay_hint_retry_requested() {
            layout.borrow_mut().retry_inlay_hints();
            needs_redraw = true;
        }

        if render_frame_if_needed(
            needs_redraw,
            &mut layout.borrow_mut(),
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
                let ui_result = layout.borrow_mut().route_ui_event(&UiEvent::Tick);
                if crate::actions::handle_ui_result_with_shared_layout(
                    &layout,
                    &mut plugin_runtime,
                    ui_result,
                ) {
                    needs_redraw = true;
                    if layout.borrow().should_exit() {
                        break;
                    }
                }
                urvim_core::session::maybe_autosave(&layout.borrow());
                if drain_editor_events(&mut plugin_runtime) {
                    needs_redraw = true;
                }
                if plugin_runtime.dispatch_job_events() {
                    needs_redraw = true;
                }
                if plugin_runtime.dispatch_fs_events() {
                    needs_redraw = true;
                }
                if plugin_runtime.dispatch_timer_events() {
                    needs_redraw = true;
                }
                if plugin_runtime.dispatch_picker_events() {
                    needs_redraw = true;
                }
                if plugin_runtime.dispatch_confirmation_events() {
                    needs_redraw = true;
                }
                if plugin_runtime.dispatch_input_events() {
                    needs_redraw = true;
                }
                if plugin_runtime.refresh_plugin_syntax() {
                    needs_redraw = true;
                }
                continue;
            }
            UiEvent::Paste(text) => {
                let overlay_result = layout
                    .borrow_mut()
                    .route_ui_event(&UiEvent::Paste(text.clone()));
                if crate::actions::handle_ui_result_with_shared_layout(
                    &layout,
                    &mut plugin_runtime,
                    overlay_result,
                ) {
                    needs_redraw = true;
                    if layout.borrow().should_exit() {
                        break;
                    }
                    if drain_editor_events(&mut plugin_runtime) {
                        needs_redraw = true;
                    }
                    continue;
                }

                let Some(action) = crate::actions::raw_paste_action_for_mode(
                    layout.borrow().active_window_mode_kind(),
                    text,
                ) else {
                    tracing::debug!("ignoring raw paste event in unsupported mode");
                    if drain_editor_events(&mut plugin_runtime) {
                        needs_redraw = true;
                    }
                    continue;
                };

                let handled = process_intent_queue(
                    &mut layout.borrow_mut(),
                    vec![Intent::Editor(action.clone())],
                );
                if handled {
                    if let Some(to_mode) = action.to_mode {
                        let repeat_text = {
                            let mut layout = layout.borrow_mut();
                            let window = layout.active_window_group_mut().active_window_mut();
                            window.switch_mode(to_mode)
                        };
                        terminal.set_cursor_style(layout.borrow().active_window_cursor_style())?;
                        if let Some(repeat_text) = repeat_text.filter(|text| !text.is_empty())
                            && let Some(mut repeat_state) = globals::get_last_repeat()
                        {
                            repeat_state.insert_text = Some(repeat_text);
                            globals::set_last_repeat(repeat_state);
                        }
                    }

                    if action.is_snapshottable() {
                        let layout = layout.borrow();
                        let cursor = layout.active_buffer_view().cursor();
                        layout
                            .active_buffer_view()
                            .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
                            .unwrap_or(());
                    }

                    if action.updates_snapshot_cursor() {
                        let layout = layout.borrow();
                        let cursor = layout.active_buffer_view().cursor();
                        layout
                            .active_buffer_view()
                            .with_buffer_mut(|buffer| buffer.update_cursor(cursor))
                            .unwrap_or(());
                    }

                    needs_redraw = true;
                }

                if layout.borrow().should_exit() {
                    break;
                }

                if drain_editor_events(&mut plugin_runtime) {
                    needs_redraw = true;
                }

                terminal.set_cursor_style(layout.borrow().active_window_cursor_style())?;
            }
            UiEvent::Resize(new_rows, new_cols) => {
                rows = new_rows;
                cols = new_cols;
                handle_resize(&mut terminal, &mut screen, rows, cols)?;
                needs_redraw = true;
            }
            UiEvent::Key(key) => {
                let overlay_result = layout.borrow_mut().route_ui_event(&UiEvent::Key(key));
                if crate::actions::handle_ui_result_with_shared_layout(
                    &layout,
                    &mut plugin_runtime,
                    overlay_result,
                ) {
                    needs_redraw = true;
                    if layout.borrow().should_exit() {
                        break;
                    }
                    terminal.set_cursor_style(layout.borrow().active_window_cursor_style())?;
                    if drain_editor_events(&mut plugin_runtime) {
                        needs_redraw = true;
                    }
                    continue;
                }

                let result = layout
                    .borrow_mut()
                    .active_window_group_mut()
                    .active_window_mut()
                    .handle_key(&key);

                match result {
                    HandleKeyResult::Complete(intent) => match intent {
                        Intent::Editor(action) => {
                            if execute_action_intent_with_plugin_runtime(
                                &mut layout.borrow_mut(),
                                &mut plugin_runtime,
                                action,
                            ) {
                                needs_redraw = true;
                                terminal.set_cursor_style(
                                    layout.borrow().active_window_cursor_style(),
                                )?;
                            }
                        }
                        Intent::Command(command) => {
                            if matches!(command, Command::Quit | Command::TryQuit) {
                                urvim_core::session::save_now(&layout.borrow());
                            }

                            if matches!(command, Command::Quit) {
                                break;
                            }

                            if let Command::OverwriteBuffer(target) = &command {
                                let save = crate::actions::handle_save_buffer_action_with_outcome(
                                    &mut layout.borrow_mut(),
                                    *target,
                                    true,
                                );
                                if save.handled {
                                    needs_redraw = true;
                                }

                                if layout.borrow().should_exit() {
                                    break;
                                }

                                terminal.set_cursor_style(
                                    layout.borrow().active_window_cursor_style(),
                                )?;
                                if drain_editor_events(&mut plugin_runtime) {
                                    needs_redraw = true;
                                }
                                continue;
                            }

                            let handled = crate::actions::process_intents_with_shared_layout(
                                &layout,
                                Some(&mut plugin_runtime),
                                vec![Intent::Command(command.clone())],
                            );
                            if handled {
                                needs_redraw = true;
                            }

                            if layout.borrow().should_exit() {
                                break;
                            }

                            terminal
                                .set_cursor_style(layout.borrow().active_window_cursor_style())?;
                            if drain_editor_events(&mut plugin_runtime) {
                                needs_redraw = true;
                            }
                        }
                    },
                    HandleKeyResult::WaitForMore => {}
                    HandleKeyResult::InvalidSequence => {}
                }
            }
        }
    }

    globals::shutdown_lsp_runtime();
    urvim_core::session::save_now(&layout.borrow());
    terminal.reset_style()?;

    Ok(())
}
