use std::io;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use urvim::terminal::{CURSOR_STYLES, Event, KeyCode};

fn init_logger() -> WorkerGuard {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("demo.log")
        .expect("Failed to open log file");
    let (non_blocking, guard) = tracing_appender::non_blocking(file);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    guard
}

fn main() -> io::Result<()> {
    let _guard = init_logger();
    if !is_terminal::is_terminal(std::io::stdin()) {
        eprintln!("Error: Must be run from a terminal");
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "stdin is not a terminal",
        ));
    }

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let mut terminal = urvim::terminal::Terminal::new(stdin, stdout)?;

    let mut cursor_visible = true;
    let mut cursor_style_index = 0;

    loop {
        let pos = match terminal.get_cursor_position() {
            Ok(p) => p,
            Err(e) => {
                terminal.set_cursor_position(3, 1)?;
                terminal.write_text(&format!("Error getting position: {:?}", e))?;
                continue;
            }
        };
        terminal.clear_screen().ok();
        terminal.set_cursor_position(1, 1)?;
        terminal.write_text("Use arrow keys to move. Press 'h' to toggle cursor, 'c' to change style. Press 'q' to quit.")?;

        terminal.set_cursor_position(2, 1)?;
        terminal.write_text(&format!("Cursor position: {};{}  ", pos.0, pos.1))?;
        terminal.set_cursor_position(3, 1)?;
        if cursor_visible {
            terminal.show_cursor()?;
            terminal.write_text("Cursor: visible  ")?;
        } else {
            terminal.hide_cursor()?;
            terminal.write_text("Cursor: hidden   ")?;
        }
        terminal.set_cursor_position(4, 1)?;
        let style = CURSOR_STYLES[cursor_style_index];
        terminal.set_cursor_style(style)?;
        terminal.write_text(&format!("Cursor style: {}  ", style.name()))?;

        // Reset cursor to original position before reading key
        terminal.set_cursor_position(pos.0, pos.1)?;

        let event = terminal.read_event()?;

        match event {
            Event::Key(key) => {
                if key.code == KeyCode::Char('q') {
                    break;
                }
                if key.code == KeyCode::Char('h') {
                    cursor_visible = !cursor_visible;
                }
                if key.code == KeyCode::Char('c') {
                    cursor_style_index = (cursor_style_index + 1) % CURSOR_STYLES.len();
                }
                // Calculate new position based on original pos (not where we wrote the display)
                let mut row = pos.0;
                let mut col = pos.1;

                match key.code {
                    KeyCode::Up => {
                        if row > 1 {
                            row -= 1;
                        }
                    }
                    KeyCode::Down => row += 1,
                    KeyCode::Left => {
                        if col > 1 {
                            col -= 1;
                        }
                    }
                    KeyCode::Right => col += 1,
                    _ => {}
                }

                terminal.set_cursor_position(row, col)?;
            }
            Event::Resize(_, _) => {
                terminal.set_cursor_position(1, 1)?;
                terminal.clear_screen()?;
                terminal.write_text("Use arrow keys to move. Press 'h' to toggle cursor, 'c' to change style. Press 'q' to quit.")?;
            }
            _ => {}
        }
    }

    terminal.set_cursor_position(1, 1)?;
    terminal.clear_screen()?;
    terminal.set_style(&urvim::terminal::Style::new().reverse())?;
    terminal.write_text("Goodbye!")?;
    terminal.reset_style()?;
    terminal.flush()?;

    Ok(())
}
