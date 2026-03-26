use clap::Parser;
use rustix::fd::AsFd;
use std::io;

use urvim::Layout;
use urvim::action::ActionResult;
use urvim::editor::{Action, HandleKeyResult, InsertMode, Mode, NormalMode};
use urvim::screen::Screen;
use urvim::terminal::{Event, Terminal, size::get_terminal_size};
use urvim::widget::Widget;
use urvim::window::{Position, Size};

#[derive(Parser)]
#[command(name = "urvim")]
#[command(version = "0.1.0")]
#[command(about = "A terminal-based text editor", long_about = None)]
struct Cli {
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
                            let action_result = layout.process_action(&action);

                            if action_result == ActionResult::NotHandled {
                                // Fall back to app-level handling
                                match action {
                                    Action::SwitchToNormal => {
                                        mode = Box::new(NormalMode::new());
                                        terminal.set_cursor_style(mode.cursor_style())?;
                                        handled = true;
                                    }
                                    Action::SwitchToInsert => {
                                        mode = Box::new(InsertMode::new());
                                        terminal.set_cursor_style(mode.cursor_style())?;
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
                                if action.switches_to_insert_mode() {
                                    mode = Box::new(InsertMode::new());
                                    terminal.set_cursor_style(mode.cursor_style())?;
                                }
                                handled = true;
                            }

                            if handled {
                                // Snapshot after the edit so undo can restore the pre-change state.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    struct TestBackend {
        input: Arc<Mutex<VecDeque<u8>>>,
        output: Arc<Mutex<Vec<u8>>>,
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
}
