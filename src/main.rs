mod logger;
mod terminal;

use std::io;

fn main() -> io::Result<()> {
    let _guard = logger::init("debug.log");
    if !is_terminal::is_terminal(std::io::stdin()) {
        eprintln!("Error: Must be run from a terminal");
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "stdin is not a terminal",
        ));
    }

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let mut terminal = terminal::Terminal::new(stdin, stdout)?;

    let mut count = 0;
    while count < 10000 {
        let event = terminal.read_event()?;

        if let terminal::Event::Key(key) = event {
            if key.code == terminal::KeyCode::Char('q') {
                break;
            }
            count += 1;
        }
    }

    terminal.reset_style()?;

    Ok(())
}
