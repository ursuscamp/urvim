use clap::Parser;
use std::io;
use urvim::terminal::Modifiers;

use urvim::buffer::{Buffer, Cursor};
use urvim::screen::Screen;
use urvim::terminal::{Event, KeyCode, Terminal, size::get_terminal_size};
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

    loop {
        screen.clear();
        window.render(&mut screen, Position::new(0, 0), Size::new(rows, cols));
        screen.render(&mut terminal)?;

        if let Some(cursor_pos) = window.visual_cursor() {
            terminal.set_cursor_position(cursor_pos.row + 1, cursor_pos.col + 1)?;
        }

        let event = terminal.read_event()?;

        if let Event::Key(key) = event {
            if key.code == KeyCode::Char('q') && key.modifiers == Modifiers::CTRL {
                break;
            }

            let cursor = window.buffer_view().cursor();
            let buffer = window.buffer_view_mut().buffer_mut();

            let new_cursor = match key.code {
                KeyCode::Char(c) => {
                    buffer.insert_char(cursor, c);
                    Some(Cursor::new(cursor.line, cursor.col + c.len_utf8()))
                }
                KeyCode::Enter => {
                    buffer.insert_char(cursor, '\n');
                    Some(Cursor::new(cursor.line + 1, 0))
                }
                KeyCode::Left => buffer.cursor_left(cursor),
                KeyCode::Right => buffer.cursor_right(cursor),
                KeyCode::Up => {
                    let visual_col = buffer.visual_col_at(cursor);
                    buffer.cursor_up(cursor, visual_col)
                }
                KeyCode::Down => {
                    let visual_col = buffer.visual_col_at(cursor);
                    buffer.cursor_down(cursor, visual_col)
                }
                KeyCode::PageUp => {
                    let visual_col = buffer.visual_col_at(cursor);
                    let mut new_cursor = cursor;
                    let scroll_amount = (rows as usize).saturating_sub(1);
                    for _ in 0..scroll_amount {
                        if let Some(c) = buffer.cursor_up(new_cursor, visual_col) {
                            new_cursor = c;
                        } else {
                            break;
                        }
                    }
                    Some(new_cursor)
                }
                KeyCode::PageDown => {
                    let visual_col = buffer.visual_col_at(cursor);
                    let mut new_cursor = cursor;
                    let scroll_amount = (rows as usize).saturating_sub(1);
                    for _ in 0..scroll_amount {
                        if let Some(c) = buffer.cursor_down(new_cursor, visual_col) {
                            new_cursor = c;
                        } else {
                            break;
                        }
                    }
                    Some(new_cursor)
                }
                KeyCode::Home if key.modifiers.has_ctrl() => Some(Cursor::new(0, 0)),
                KeyCode::End if key.modifiers.has_ctrl() => {
                    let last_line = buffer.line_count().saturating_sub(1);
                    let last_col = buffer.line_at(last_line).map(|l| l.len()).unwrap_or(0);
                    Some(Cursor::new(last_line, last_col))
                }
                KeyCode::Home => Some(Cursor::new(cursor.line, 0)),
                KeyCode::End => {
                    let line_len = buffer.line_at(cursor.line).map(|l| l.len()).unwrap_or(0);
                    Some(Cursor::new(cursor.line, line_len))
                }
                _ => None,
            };

            if let Some(new_cursor) = new_cursor {
                window.set_cursor(new_cursor);
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
