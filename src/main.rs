use clap::Parser;
use rustix::fd::AsFd;
use std::io;

use urvim::Layout;
use urvim::action::ActionResult;
use urvim::buffer::{Cursor, SyntaxCatchUpResult};
use urvim::config::Config;
use urvim::editor::{
    Action, ActionKind, HandleKeyResult, InsertMode, Mode, ModeKind, NormalMode, RepeatReplay,
    VisualLineMode, VisualMode,
};
use urvim::globals;
use urvim::screen::Screen;
use urvim::terminal::{Event, Terminal, size::get_terminal_size};
use urvim::theme::ThemeRegistry;
use urvim::widget::Widget;
use urvim::window::{Position, Size, VisualSelectionKind};

#[derive(Parser)]
#[command(name = "urvim")]
#[command(version = "0.1.0")]
#[command(about = "A terminal-based text editor", long_about = None)]
struct Cli {
    #[arg(long)]
    theme: Option<String>,
    #[arg(long = "no-syntax")]
    no_syntax: bool,
    files: Vec<std::path::PathBuf>,
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
    globals::set_job_manager(urvim::job::JobManager::new());

    let mut terminal = Terminal::new(stdin, stdout)?;

    let (mut rows, mut cols) = get_terminal_size().unwrap_or((24, 80));
    let mut screen = Screen::new(rows, cols);

    let mut layout = Layout::from_paths(&cli.files);
    globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
    globals::with_buffer_pool(|pool| {
        pool.warmup_syntax_at_startup(
            Some(layout.active_buffer_view().buffer_id()),
            layout.active_buffer_view().scroll_offset().row as usize,
            rows.saturating_sub(1) as usize,
            config.syntax,
        );
    });

    // Initialize with Normal mode and set cursor style
    let mut mode: Box<dyn Mode> = Box::new(NormalMode::new());
    terminal.set_cursor_style(mode.cursor_style())?;
    layout.set_mode_kind(mode.kind());

    loop {
        globals::with_job_manager(|job_manager| {
            if let Some(job_manager) = job_manager {
                let _ = job_manager.process_completed(|event| {
                    if let Ok((_kind, _token, result)) =
                        event.into_completed_output::<SyntaxCatchUpResult>()
                    {
                        globals::with_buffer_mut(result.buffer_id, |buffer| {
                            buffer.apply_syntax_catch_up_result(result);
                        });
                    }
                });
            }
        });

        globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
        screen.clear();
        layout.render(&mut screen, Position::new(0, 0), Size::new(rows, cols));
        screen.render(&mut terminal)?;

        if let Some(cursor_pos) = layout.visual_cursor() {
            terminal.set_cursor_position(cursor_pos.row + 1, cursor_pos.col + 1)?;
        }

        let event = terminal.read_event()?;

        if let Event::Tick = event {
            continue;
        }

        if let Event::Key(key) = event {
            let result = mode.handle_key(&key);

            match result {
                HandleKeyResult::Complete(action) => {
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
                            if let Some(cursor) = layout
                                .active_buffer_view()
                                .with_buffer_mut(|buffer| buffer.undo())
                                .flatten()
                            {
                                layout.active_buffer_view_mut().set_cursor_synced(cursor);
                                layout.tab_group_mut().record_cursor_position();
                            }
                        }
                        Some(ActionKind::Redo) => {
                            if let Some(cursor) = layout
                                .active_buffer_view()
                                .with_buffer_mut(|buffer| buffer.redo())
                                .flatten()
                            {
                                layout.active_buffer_view_mut().set_cursor_synced(cursor);
                                layout.tab_group_mut().record_cursor_position();
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
                                    let repeat_text = switch_mode(&mut layout, &mut mode, to_mode);
                                    terminal.set_cursor_style(mode.cursor_style())?;
                                    if let Some(repeat_text) =
                                        repeat_text.filter(|text| !text.is_empty())
                                        && let Some(mut repeat_state) = globals::get_last_repeat()
                                    {
                                        repeat_state.insert_text = Some(repeat_text);
                                        globals::set_last_repeat(repeat_state);
                                    }
                                }
                            } else {
                                let action_result = layout.process_action(&dispatch_action);

                                if action_result == ActionResult::NotHandled {
                                    // Fall back to app-level handling
                                    match dispatch_action {
                                        _ if matches!(
                                            dispatch_action.kind.as_ref(),
                                            Some(ActionKind::SaveBuffer(_))
                                        ) =>
                                        {
                                            let target = match dispatch_action.kind.as_ref() {
                                                Some(ActionKind::SaveBuffer(target)) => *target,
                                                _ => None,
                                            };
                                            let buffer_id = target.unwrap_or_else(|| {
                                                layout.active_buffer_view().buffer_id()
                                            });
                                            let save_result = globals::with_buffer_pool(|pool| {
                                                pool.save_buffer(buffer_id)
                                            });
                                            match save_result {
                                                Ok(()) => {}
                                                Err(error)
                                                    if error.kind()
                                                        == io::ErrorKind::InvalidInput =>
                                                {
                                                    tracing::info!(
                                                        "Skipping save for unnamed buffer {:?}",
                                                        buffer_id
                                                    );
                                                }
                                                Err(error) => {
                                                    tracing::warn!(
                                                        "Failed to save buffer {:?}: {}",
                                                        buffer_id,
                                                        error
                                                    );
                                                }
                                            }
                                            handled = true;
                                        }
                                        _ if matches!(
                                            dispatch_action.kind.as_ref(),
                                            Some(ActionKind::Quit)
                                        ) =>
                                        {
                                            globals::shutdown_job_manager();
                                            break;
                                        }
                                        _ if dispatch_action.kind.is_none() => {
                                            handled = true;
                                        }
                                        _ => { /* Should have been handled by window */ }
                                    }
                                } else if action_result == ActionResult::Handled {
                                    let pending_repeat_suffix = layout.take_pending_repeat_suffix();
                                    if let Some(suffix) = pending_repeat_suffix.as_deref() {
                                        mode.append_repeat_text(suffix);
                                    }
                                    handled = true;
                                }

                                if handled && let Some(to_mode) = dispatch_action.to_mode {
                                    let repeat_text = switch_mode(&mut layout, &mut mode, to_mode);
                                    terminal.set_cursor_style(mode.cursor_style())?;
                                    if let Some(repeat_text) =
                                        repeat_text.filter(|text| !text.is_empty())
                                        && let Some(mut repeat_state) = globals::get_last_repeat()
                                    {
                                        repeat_state.insert_text = Some(repeat_text);
                                        globals::set_last_repeat(repeat_state);
                                    }
                                }
                            }

                            if handled {
                                // Snapshot after the edit so undo can restore the pre-change state.
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
                                        .active_buffer_view()
                                        .with_buffer_mut(|buffer| buffer.update_cursor(cursor))
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
                            }
                        }
                    }

                    layout.set_mode_kind(mode.kind());
                }
                HandleKeyResult::WaitForMore => {
                    // Continue waiting for more keys, no action taken
                }
                HandleKeyResult::InvalidSequence => {
                    // Ignore invalid sequences
                }
            }
        }

        if let Event::Resize(new_rows, new_cols) = event {
            rows = new_rows;
            cols = new_cols;
            handle_resize(&mut terminal, &mut screen, rows, cols)?;
        }
    }

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

fn switch_mode(layout: &mut Layout, mode: &mut Box<dyn Mode>, to_mode: ModeKind) -> Option<String> {
    let repeat_text = if to_mode == ModeKind::Normal {
        mode.take_repeat_text()
    } else {
        None
    };

    if mode.kind().is_visual() && to_mode != mode.kind() {
        layout.active_buffer_view_mut().clear_visual_selection();
    }

    match to_mode {
        ModeKind::Normal => {
            *mode = Box::new(NormalMode::new());
        }
        ModeKind::Insert => {
            *mode = Box::new(InsertMode::new());
        }
        ModeKind::Visual => {
            layout
                .active_buffer_view_mut()
                .begin_visual_selection(VisualSelectionKind::Character);
            *mode = Box::new(VisualMode::new());
        }
        ModeKind::VisualLine => {
            layout
                .active_buffer_view_mut()
                .begin_visual_selection(VisualSelectionKind::Line);
            *mode = Box::new(VisualLineMode::new());
        }
    }

    repeat_text
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
            _ => layout.process_action(&structural_action) == ActionResult::Handled,
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
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex, OnceLock};
    use urvim::buffer::Buffer;
    use urvim::editor::ModeKind;
    use urvim::tab_group::TabGroup;

    struct TestBackend {
        input: Arc<Mutex<VecDeque<u8>>>,
        output: Arc<Mutex<Vec<u8>>>,
    }

    fn repeat_state_lock() -> std::sync::MutexGuard<'static, ()> {
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
    fn select_active_theme_defaults_to_friday_night() {
        let registry = ThemeRegistry::load_builtin().expect("builtins should load");

        let theme = select_active_theme(&registry, None).expect("default theme should exist");

        assert_eq!(theme.name(), "Friday Night");
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
        let mut layout = Layout::new(
            TabGroup::from_buffers(vec![Buffer::from_str("line1\nline2\nline3")]),
            ModeKind::Normal,
        );
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
        let mut layout = Layout::new(
            TabGroup::from_buffers(vec![Buffer::from_str("world")]),
            ModeKind::Normal,
        );
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
        let mut layout = Layout::new(
            TabGroup::from_buffers(vec![Buffer::from_str("hello")]),
            ModeKind::Visual,
        );
        layout
            .active_buffer_view_mut()
            .begin_visual_selection(VisualSelectionKind::Character);
        let mut mode: Box<dyn Mode> = Box::new(VisualMode::new());

        let repeat_text = switch_mode(&mut layout, &mut mode, ModeKind::Normal);

        assert_eq!(mode.kind(), ModeKind::Normal);
        assert!(repeat_text.is_none());
        assert!(layout.active_buffer_view().visual_selection().is_none());
    }

    #[test]
    fn switch_mode_restarts_visual_selection_when_entering_visual() {
        let mut layout = Layout::new(
            TabGroup::from_buffers(vec![Buffer::from_str("hello")]),
            ModeKind::Normal,
        );
        let mut mode: Box<dyn Mode> = Box::new(NormalMode::new());

        let repeat_text = switch_mode(&mut layout, &mut mode, ModeKind::Visual);

        assert_eq!(mode.kind(), ModeKind::Visual);
        assert!(repeat_text.is_none());
        assert!(layout.active_buffer_view().visual_selection().is_some());
    }

    #[test]
    fn switch_mode_starts_linewise_visual_selection_when_entering_visual_line() {
        let mut layout = Layout::new(
            TabGroup::from_buffers(vec![Buffer::from_str("hello")]),
            ModeKind::Normal,
        );
        let mut mode: Box<dyn Mode> = Box::new(NormalMode::new());

        let repeat_text = switch_mode(&mut layout, &mut mode, ModeKind::VisualLine);

        assert_eq!(mode.kind(), ModeKind::VisualLine);
        assert!(repeat_text.is_none());
        let selection = layout
            .active_buffer_view()
            .visual_selection()
            .expect("linewise selection should exist");
        assert_eq!(selection.kind, VisualSelectionKind::Line);
    }
}
