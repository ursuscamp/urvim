use clap::Parser;
use rustix::fd::AsFd;
use std::io;

use urvim::Layout;
use urvim::buffer::Cursor;
use urvim::config::Config;
use urvim::editor::{Action, ActionKind, HandleKeyResult, ModeKind, RepeatReplay};
use urvim::globals;
use urvim::screen::Screen;
use urvim::terminal::{Terminal, size::get_terminal_size};
use urvim::theme::ThemeRegistry;
use urvim::ui::{Command, Intent, UiEvent};
use urvim::window::{Position, Size};

#[derive(Parser)]
#[command(name = "urvim")]
#[command(version = "0.1.0")]
#[command(about = "A terminal-based text editor", long_about = None)]
struct Cli {
    #[arg(long)]
    theme: Option<String>,
    #[arg(long = "no-syntax")]
    no_syntax: bool,
    files: Vec<urvim::cli::CliFileSpec>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let _guard = urvim::logger::init("debug.log");

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

    let registry = urvim::theme::ThemeRegistry::load_builtin().map_err(|error| {
        eprintln!("Error: {}", error);
        io::Error::new(io::ErrorKind::InvalidData, error.to_string())
    })?;

    let active_theme =
        select_active_theme(&registry, Some(config.theme.as_str())).map_err(|error| {
            eprintln!("Error: {}", error);
            io::Error::new(io::ErrorKind::InvalidInput, error)
        })?;
    globals::set_active_theme(active_theme);
    globals::set_theme_registry(registry);

    let mut terminal = Terminal::new(stdin, stdout)?;

    let (mut rows, mut cols) = get_terminal_size().unwrap_or((24, 80));
    let mut screen = Screen::new(rows, cols);

    let mut layout = startup_layout(&cli.files);
    urvim::session::set_enabled(cli.files.is_empty());
    globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
    globals::with_buffer_pool(|pool| {
        pool.warmup_syntax_at_startup(
            Some(layout.active_buffer_view().buffer_id()),
            layout.active_buffer_view().scroll_offset().row as usize,
            rows.saturating_sub(1) as usize,
            config.syntax,
        );
    });
    globals::set_lsp_runtime(urvim::lsp::runtime::LspRuntime::new(&config));
    globals::with_lsp_runtime_mut(|runtime| runtime.sync());

    terminal.set_cursor_style(layout.active_window_cursor_style())?;

    let mut needs_redraw = true;
    loop {
        let background_requested_redraw =
            globals::with_buffer_pool(|pool| pool.process_background_jobs())
                || layout.process_background_jobs()
                || layout.process_workspace_file_operations()
                || globals::take_notification_redraw_requested();

        globals::try_with_lsp_runtime_mut(|runtime| runtime.sync());

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
                if handle_ui_result(&mut layout, ui_result) {
                    needs_redraw = true;
                    if layout.should_exit() {
                        break;
                    }
                }
                urvim::session::maybe_autosave(&layout);
                continue;
            }
            UiEvent::Paste(text) => {
                let overlay_result = layout.route_ui_event(&UiEvent::Paste(text.clone()));
                if handle_ui_result(&mut layout, overlay_result) {
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
                if handle_ui_result(&mut layout, overlay_result) {
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
                    HandleKeyResult::Complete(intent) => {
                        match intent {
                            Intent::Action(action) => {
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
                                    Some(ActionKind::Undo) => {
                                        if apply_undo_redo(&mut layout, false) {
                                            needs_redraw = true;
                                        }
                                    }
                                    Some(ActionKind::Redo) => {
                                        if apply_undo_redo(&mut layout, true) {
                                            needs_redraw = true;
                                        }
                                    }
                                    _ => {
                                        let mut handled = false;
                                        if let Some(replay) = repeat_replay.as_ref() {
                                            handled = replay_repeat_action(&mut layout, replay);
                                            if handled
                                                && replay.action.kind.is_some()
                                                && let Some(to_mode) = replay.action.to_mode
                                            {
                                                let repeat_text = {
                                                    let window = layout
                                                        .active_window_group_mut()
                                                        .active_window_mut();
                                                    window.switch_mode(to_mode)
                                                };
                                                terminal.set_cursor_style(
                                                    layout.active_window_cursor_style(),
                                                )?;
                                                if let Some(repeat_text) =
                                                    repeat_text.filter(|text| !text.is_empty())
                                                    && let Some(mut repeat_state) =
                                                        globals::get_last_repeat()
                                                {
                                                    repeat_state.insert_text = Some(repeat_text);
                                                    globals::set_last_repeat(repeat_state);
                                                }
                                            }
                                        } else {
                                            let handled_by_layout = process_intent_queue(
                                                &mut layout,
                                                vec![Intent::Action(dispatch_action.clone())],
                                            );

                                            if !handled_by_layout {
                                                // Fall back to app-level handling
                                                match dispatch_action {
                                                    _ if matches!(
                                                        dispatch_action.kind.as_ref(),
                                                        Some(ActionKind::SaveBuffer(_))
                                                    ) =>
                                                    {
                                                        handled = handle_save_buffer_action(
                                                            &mut layout,
                                                            dispatch_action.kind.as_ref(),
                                                        );
                                                    }
                                                    _ if dispatch_action.kind.is_none() => {
                                                        handled = true;
                                                    }
                                                    _ => { /* Should have been handled by window */
                                                    }
                                                }
                                            } else {
                                                let pending_repeat_suffix =
                                                    layout.take_pending_repeat_suffix();
                                                if let Some(suffix) =
                                                    pending_repeat_suffix.as_deref()
                                                {
                                                    layout
                                                        .active_window_group_mut()
                                                        .active_window_mut()
                                                        .append_repeat_text(suffix);
                                                }
                                                handled = true;
                                            }

                                            if handled
                                                && let Some(to_mode) = dispatch_action.to_mode
                                            {
                                                let repeat_text = {
                                                    let window = layout
                                                        .active_window_group_mut()
                                                        .active_window_mut();
                                                    window.switch_mode(to_mode)
                                                };
                                                terminal.set_cursor_style(
                                                    layout.active_window_cursor_style(),
                                                )?;
                                                if let Some(repeat_text) =
                                                    repeat_text.filter(|text| !text.is_empty())
                                                    && let Some(mut repeat_state) =
                                                        globals::get_last_repeat()
                                                {
                                                    repeat_state.insert_text = Some(repeat_text);
                                                    globals::set_last_repeat(repeat_state);
                                                }
                                            }
                                        }

                                        if handled {
                                            if dispatch_action.from_mode == Some(ModeKind::Insert)
                                                && dispatch_action.to_mode == Some(ModeKind::Normal)
                                            {
                                                commit_insert_exit_snapshot(&mut layout);
                                            }

                                            if dispatch_action.is_snapshottable() {
                                                let cursor = layout.active_buffer_view().cursor();
                                                layout
                                                    .active_buffer_view()
                                                    .with_buffer_mut(|buffer| {
                                                        buffer.push_snapshot(cursor)
                                                    })
                                                    .unwrap_or(());
                                            }

                                            if dispatch_action.updates_snapshot_cursor() {
                                                let cursor = layout.active_buffer_view().cursor();
                                                layout
                                                    .active_buffer_view()
                                                    .with_buffer_mut(|buffer| {
                                                        buffer.update_cursor(cursor)
                                                    })
                                                    .unwrap_or(());
                                            }

                                            if let Some((repeat_action, repeat_count)) =
                                                action.dot_repeat_source()
                                            {
                                                globals::set_last_repeat(globals::RepeatState {
                                                    action: repeat_action,
                                                    count: repeat_count,
                                                    insert_text: None,
                                                });
                                            }

                                            needs_redraw = true;
                                        }
                                    }
                                }

                                if layout.should_exit() {
                                    break;
                                }

                                terminal.set_cursor_style(layout.active_window_cursor_style())?;
                            }
                            Intent::Command(command) => {
                                if matches!(command, Command::Quit | Command::TryQuit) {
                                    urvim::session::save_now(&layout);
                                }

                                if matches!(command, Command::Quit) {
                                    break;
                                }

                                let handled = process_intent_queue(
                                    &mut layout,
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
                        }
                    }
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
    urvim::session::save_now(&layout);
    terminal.reset_style()?;

    Ok(())
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
) -> Result<urvim::theme::Theme, String> {
    let theme_name = requested.unwrap_or("Friday Night");
    registry.get(theme_name).cloned().ok_or_else(|| {
        format!(
            "unknown theme {theme_name:?}; available themes: {}",
            registry.names().join(", ")
        )
    })
}

fn handle_save_buffer_action(layout: &mut Layout, kind: Option<&ActionKind>) -> bool {
    let target = match kind {
        Some(ActionKind::SaveBuffer(target)) => *target,
        _ => None,
    };

    let buffer_id = target.unwrap_or_else(|| layout.active_buffer_view().buffer_id());
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
            urvim::notify_info!("Saved {}", label);
        }
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
            tracing::info!("Skipping save for unnamed buffer {:?}", buffer_id);
        }
        Err(error) => {
            urvim::notify_error!("Failed to save buffer {:?}: {}", buffer_id, error);
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
    let mut queue: std::collections::VecDeque<Intent> = intents.into();
    let mut handled_all = true;
    let mut saw_intent = false;

    while let Some(intent) = queue.pop_front() {
        saw_intent = true;
        handled_all &= layout.dispatch_intent(&intent);
    }

    saw_intent && handled_all
}

fn handle_ui_result(layout: &mut Layout, result: urvim::ui::UiEventResult) -> bool {
    if !result.handled() {
        return false;
    }

    let intents = result.into_intents();
    if !intents.is_empty() {
        process_intent_queue(layout, intents);
    }

    true
}

fn raw_paste_action_for_mode(mode: ModeKind, text: String) -> Option<Action> {
    match mode {
        ModeKind::Insert | ModeKind::Normal => {
            Some(Action::insert_raw_paste(text).with_from_mode(mode))
        }
        ModeKind::Visual | ModeKind::VisualLine => Some(
            Action::replace_selection_raw_paste(text).with_mode(Some(mode), Some(ModeKind::Normal)),
        ),
        ModeKind::Resizing => None,
    }
}

fn startup_layout(files: &[urvim::cli::CliFileSpec]) -> Layout {
    let Ok(cwd) = std::env::current_dir() else {
        tracing::warn!("failed to resolve current directory for startup");
        return Layout::from_cli_files(files);
    };

    startup_layout_for_cwd(&cwd, files)
}

fn startup_layout_for_cwd(cwd: &std::path::Path, files: &[urvim::cli::CliFileSpec]) -> Layout {
    if files.is_empty() {
        match urvim::session::load_session_for_cwd(cwd) {
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
    use urvim::buffer::{Buffer, BufferId};
    use urvim::cli::CliFileSpec;
    use urvim::editor::ModeKind;
    use urvim::terminal::{Event, Key, KeyCode};
    use urvim::window::VisualSelectionKind;
    use urvim::window_group::WindowGroup;

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
    fn terminal_event_adapter_converts_event_variants() {
        let key_event = Event::Key(urvim::terminal::Key::new(urvim::terminal::KeyCode::Char(
            'x',
        )));
        assert!(matches!(
            UiEvent::from(key_event),
            UiEvent::Key(key) if key.code == urvim::terminal::KeyCode::Char('x')
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
    fn process_intent_queue_returns_false_when_any_intent_is_unhandled() {
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));

        let handled = process_intent_queue(
            &mut layout,
            vec![
                Intent::Command(urvim::ui::Command::EnqueueNotification {
                    level: urvim::notification::NotificationLevel::Info,
                    message: "queued".to_string(),
                }),
                Intent::Action(Action::new(ActionKind::SaveBuffer(None))),
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
        let absolute_path = urvim::AbsolutePath::from_path(path.as_path())
            .expect("temp path should resolve absolutely");

        let mut buffer = Buffer::with_path(absolute_path);
        buffer.insert_text(Cursor::new(0, 0), "hello");

        let mut layout = Layout::new(WindowGroup::from_buffers(vec![buffer]));
        assert!(handle_save_buffer_action(
            &mut layout,
            Some(&ActionKind::SaveBuffer(None))
        ));

        let saved_text = std::fs::read_to_string(path).expect("saved file should be readable");
        assert_eq!(saved_text, "hello");
    }

    #[test]
    fn handle_save_buffer_action_emits_error_notification_for_missing_buffer() {
        let _guard = notification_test_lock();
        globals::clear_notifications();
        let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::new()]));

        assert!(handle_save_buffer_action(
            &mut layout,
            Some(&ActionKind::SaveBuffer(Some(BufferId::new(usize::MAX))))
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
            urvim::session::set_enabled(true);

            let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
            assert!(process_intent_queue(
                &mut layout,
                vec![Intent::Command(Command::SplitVertical)],
            ));

            urvim::session::save_now(&layout);
            let session_file = urvim::session::load_current_cwd()
                .expect("session should load")
                .expect("session should exist");

            match session_file.root {
                urvim::session::SessionNode::Split(_) => {}
                other => panic!("expected split session root, got {other:?}"),
            }

            assert!(process_intent_queue(
                &mut layout,
                vec![Intent::Command(Command::TryQuit)],
            ));

            let saved = urvim::session::load_current_cwd()
                .expect("session should still load")
                .expect("session should still exist");
            match saved.root {
                urvim::session::SessionNode::Split(_) => {}
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
            urvim::session::set_enabled(true);

            let mut layout = Layout::new(WindowGroup::from_buffers(vec![Buffer::from_str("one")]));
            assert!(process_intent_queue(
                &mut layout,
                vec![Intent::Command(Command::SplitVertical)],
            ));

            urvim::session::save_now(&layout);
            let session_file = urvim::session::load_current_cwd()
                .expect("session should load")
                .expect("session should exist");
            assert!(matches!(
                session_file.root,
                urvim::session::SessionNode::Split(_)
            ));

            assert!(process_intent_queue(
                &mut layout,
                vec![Intent::Command(Command::Quit)],
            ));

            let saved = urvim::session::load_current_cwd()
                .expect("session should still load")
                .expect("session should still exist");
            assert!(matches!(saved.root, urvim::session::SessionNode::Split(_)));
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
        let session = urvim::session::SessionFile {
            version: 1,
            cwd: temp_dir.display().to_string(),
            label: temp_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("cwd")
                .to_string(),
            focused_pane: 0,
            root: urvim::session::SessionNode::Pane(urvim::session::SessionPane {
                pane_id: 0,
                window_group: urvim::session::SessionWindowGroup {
                    active_tab: 0,
                    tabs: vec![urvim::session::SessionWindow {
                        path: path.display().to_string(),
                        cursor: urvim::session::SessionCursor { row: 1, col: 0 },
                        scroll_offset: urvim::session::SessionPosition { row: 0, col: 0 },
                        wrapped_row_offset: 0,
                        wrap_enabled: false,
                    }],
                },
            }),
        };
        urvim::session::save_session_for_cwd(&temp_dir, &session).unwrap();

        let loaded = urvim::session::load_session_for_cwd(&temp_dir).unwrap();
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
        urvim::session::save_session_for_cwd(&temp_dir, &saved_layout.to_session()).unwrap();

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
        use urvim::editor::ActionKind;

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
        use urvim::editor::ActionKind;

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
