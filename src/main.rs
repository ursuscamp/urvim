use clap::Parser;
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
                            if let Some(cursor) =
                                layout.active_buffer_view_mut().buffer_mut().undo()
                            {
                                layout.active_buffer_view_mut().set_cursor(cursor);
                            }
                        }
                        Action::Redo => {
                            if let Some(cursor) =
                                layout.active_buffer_view_mut().buffer_mut().redo()
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
                                        .active_buffer_view_mut()
                                        .buffer_mut()
                                        .push_snapshot(cursor);
                                }

                                if action.updates_snapshot_cursor() {
                                    let cursor = layout.active_buffer_view().cursor();
                                    layout
                                        .active_buffer_view_mut()
                                        .buffer_mut()
                                        .update_cursor(cursor);
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
            screen.resize(rows, cols);
        }
    }

    terminal.reset_style()?;

    Ok(())
}
