use std::io;

fn main() -> io::Result<()> {
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

    terminal.set_cursor_position(1, 1)?;
    terminal.write_text("OSC-52 Clipboard Demo")?;

    terminal.set_cursor_position(3, 1)?;
    terminal.write_text("Press 'c' to copy a message to clipboard")?;
    terminal.set_cursor_position(4, 1)?;
    terminal.write_text("Press 'q' to quit")?;

    terminal.set_cursor_position(6, 1)?;
    terminal.write_text("Copied messages will appear below:")?;

    let mut row = 8u16;

    loop {
        let event = terminal.read_event()?;

        if let urvim::terminal::Event::Key(key) = event {
            if key.code == urvim::terminal::KeyCode::Char('q') {
                break;
            }

            if let urvim::terminal::KeyCode::Char('c') = key.code {
                let message = "Hello from urvim! OSC52 clipboard copy successful.";
                terminal.copy_to_clipboard(message)?;
                terminal.set_cursor_position(row, 1)?;
                terminal.write_text(&format!("Copied: {}", message))?;
                row += 1;
            }
        }
    }

    terminal.reset_style()?;

    Ok(())
}
