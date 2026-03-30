use clap::Parser;
use rustix::fd::AsFd;
use std::io;

use urvim::Layout;
use urvim::action::ActionResult;
use urvim::buffer::Cursor;
use urvim::config::Config;
use urvim::editor::{Action, HandleKeyResult, InsertMode, Mode, NormalMode, RepeatReplay};
use urvim::globals;
use urvim::screen::Screen;
use urvim::terminal::{Event, Terminal, size::get_terminal_size};
use urvim::theme::ThemeRegistry;
use urvim::widget::Widget;
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

    let mut terminal = Terminal::new(stdin, stdout)?;

    let (mut rows, mut cols) = get_terminal_size().unwrap_or((24, 80));
    let mut screen = Screen::new(rows, cols);

    let mut layout = Layout::from_paths(&cli.files);

    // Initialize with Normal mode and set cursor style
    let mut mode: Box<dyn Mode> = Box::new(NormalMode::new());
    terminal.set_cursor_style(mode.cursor_style())?;
    layout.set_mode_kind(mode.kind());

    loop {
        screen.clear();
        layout.render(&mut screen, Position::new(0, 0), Size::new(rows, cols));
        screen.render(&mut terminal)?;

        if let Some(cursor_pos) = layout.visual_cursor() {
            terminal.set_cursor_position(cursor_pos.row + 1, cursor_pos.col + 1)?;
        }

        let event = terminal.read_event()?;

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
                                Action::None
                            } else {
                                action.clone()
                            }
                        });

                    match action {
                        Action::Undo => {
                            if let Some(cursor) = layout
                                .active_buffer_view()
                                .with_buffer_mut(|buffer| buffer.undo())
                                .flatten()
                            {
                                layout.active_buffer_view_mut().set_cursor(cursor);
                            }
                        }
                        Action::Redo => {
                            if let Some(cursor) = layout
                                .active_buffer_view()
                                .with_buffer_mut(|buffer| buffer.redo())
                                .flatten()
                            {
                                layout.active_buffer_view_mut().set_cursor(cursor);
                            }
                        }
                        _ => {
                            let mut handled = false;
                            if let Some(replay) = repeat_replay.as_ref() {
                                handled = replay_repeat_action(&mut layout, replay);
                            } else {
                                let action_result = layout.process_action(&dispatch_action);

                                if action_result == ActionResult::NotHandled {
                                    // Fall back to app-level handling
                                    match dispatch_action {
                                        Action::SwitchToNormal => {
                                            let repeat_text = mode.take_repeat_text();
                                            mode = Box::new(NormalMode::new());
                                            terminal.set_cursor_style(mode.cursor_style())?;
                                            if let Some(repeat_text) =
                                                repeat_text.filter(|text| !text.is_empty())
                                                && let Some(mut repeat_state) =
                                                    globals::get_last_repeat()
                                            {
                                                repeat_state.insert_text = Some(repeat_text);
                                                globals::set_last_repeat(repeat_state);
                                            }
                                            handled = true;
                                        }
                                        Action::SwitchToInsert => {
                                            mode = Box::new(InsertMode::new());
                                            terminal.set_cursor_style(mode.cursor_style())?;
                                            handled = true;
                                        }
                                        Action::SaveBuffer(target) => {
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
                                        Action::Quit => break,
                                        Action::None => {
                                            handled = true;
                                        }
                                        _ => { /* Should have been handled by window */ }
                                    }
                                } else if action_result == ActionResult::Handled {
                                    // Check if this action switches to insert mode (handles Count actions recursively)
                                    if dispatch_action.switches_to_insert_mode() {
                                        mode = Box::new(InsertMode::new());
                                        terminal.set_cursor_style(mode.cursor_style())?;
                                    }
                                    handled = true;
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

fn replay_repeat_action(layout: &mut Layout, replay: &RepeatReplay) -> bool {
    if matches!(replay.action, Action::SwitchToInsert)
        && replay.insert_text.as_deref().is_none_or(str::is_empty)
    {
        return false;
    }

    let structural_action = if replay.structural_count > 1 {
        Action::Count(replay.structural_count, Box::new(replay.action.clone()))
    } else {
        replay.action.clone()
    };

    for _ in 0..replay.repeat_count {
        let handled = match replay.action {
            Action::SwitchToInsert => true,
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
        let _guard = repeat_state_lock();
        globals::set_last_repeat(globals::RepeatState {
            action: Action::DeleteLine,
            count: 3,
            insert_text: Some("hello".to_string()),
        });

        let replay = Action::RepeatLastChange
            .resolve_dot_repeat()
            .expect("repeat should resolve");
        assert!(matches!(replay.action, Action::DeleteLine));
        assert_eq!(replay.structural_count, 3);
        assert_eq!(replay.repeat_count, 1);
        assert_eq!(replay.insert_text.as_deref(), Some("hello"));
    }

    #[test]
    fn resolve_repeat_action_overrides_the_stored_count() {
        let _guard = repeat_state_lock();
        globals::set_last_repeat(globals::RepeatState {
            action: Action::DeleteLine,
            count: 3,
            insert_text: None,
        });

        let replay = Action::Count(2, Box::new(Action::RepeatLastChange))
            .resolve_dot_repeat()
            .expect("repeat should resolve");
        assert!(matches!(replay.action, Action::DeleteLine));
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
            action: Action::ChangeLine,
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
            action: Action::SwitchToInsert,
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
}
