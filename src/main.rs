use clap::Parser;
use std::io;

use urvim::action::ActionResult;
use urvim::buffer::Buffer;
use urvim::editor::{Action, HandleKeyResult, InsertMode, Mode, NormalMode};
use urvim::screen::Screen;
use urvim::terminal::{size::get_terminal_size, Event, Terminal};
use urvim::widget::Widget;
use urvim::window::{Position, Size, Window};

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

    let buffer = if let Some(first_file) = cli.files.first() {
        match Buffer::load_from_file(first_file) {
            Ok(buf) => {
                tracing::info!("Opened file: {:?}", first_file);
                buf
            }
            Err(e) => {
                tracing::warn!("Failed to open file {:?}: {}", first_file, e);
                Buffer::new()
            }
        }
    } else {
        Buffer::new()
    };

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let mut terminal = Terminal::new(stdin, stdout)?;

    let (mut rows, mut cols) = get_terminal_size().unwrap_or((24, 80));
    let mut screen = Screen::new(rows, cols);

    let mut window = Window::new(buffer);

    // Initialize with Normal mode and set cursor style
    let mut mode: Box<dyn Mode> = Box::new(NormalMode::new());
    terminal.set_cursor_style(mode.cursor_style())?;

    loop {
        screen.clear();
        window.render(&mut screen, Position::new(0, 0), Size::new(rows, cols));
        screen.render(&mut terminal)?;

        if let Some(cursor_pos) = window.visual_cursor() {
            terminal.set_cursor_position(cursor_pos.row + 1, cursor_pos.col + 1)?;
        }

        let event = terminal.read_event()?;

        if let Event::Key(key) = event {
            let result = mode.handle_key(&key);

            match result {
                HandleKeyResult::Complete(action) => {
                    // First, try to process action through the widget (window)
                    if window.process_action(&action) == ActionResult::NotHandled {
                        // Fall back to app-level handling
                        match action {
                            Action::SwitchToNormal => {
                                mode = Box::new(NormalMode::new());
                                terminal.set_cursor_style(mode.cursor_style())?;
                            }
                            Action::SwitchToInsert => {
                                mode = Box::new(InsertMode::new());
                                terminal.set_cursor_style(mode.cursor_style())?;
                            }
                            Action::Quit => break,
                            Action::None => { /* Ignore */ }
                            _ => { /* Should have been handled by window */ }
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

        if let Event::Resize(new_rows, new_cols) = event {
            rows = new_rows;
            cols = new_cols;
            screen.resize(rows, cols);
        }
    }

    terminal.reset_style()?;

    Ok(())
}
