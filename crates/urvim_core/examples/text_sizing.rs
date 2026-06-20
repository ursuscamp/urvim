use std::io;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use urvim_terminal::{TextSizing, VerticalAlign};

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

    let mut terminal = urvim_terminal::Terminal::new(stdin, stdout)?;

    let support = terminal.detect_text_sizing_support()?;

    println!("Text sizing support: {:?}", support);

    terminal.batch(|terminal| {
        terminal.set_cursor_position(3, 1)?;
        terminal.write_text("Text Sizing Examples:")?;

        let mut row = 5;

        terminal.set_cursor_position(row, 1)?;
        terminal.reset_style()?;
        terminal.write_text("Scale 2 (double):")?;
        row += 1;

        terminal.set_cursor_position(row, 1)?;
        terminal.write_styled_text(None, Some(&TextSizing::new().scale(2)), "Double sized text")?;
        row += 3;

        terminal.set_cursor_position(row, 1)?;
        terminal.reset_style()?;
        terminal.write_text("Scale 3 (triple):")?;
        row += 1;

        terminal.set_cursor_position(row, 1)?;
        terminal.write_styled_text(None, Some(&TextSizing::new().scale(3)), "Triple sized text")?;
        row += 4;

        terminal.set_cursor_position(row, 1)?;
        terminal.reset_style()?;
        terminal.write_text("Half size (n=1:d=2):")?;
        row += 1;

        terminal.set_cursor_position(row, 1)?;
        terminal.write_styled_text(
            None,
            Some(&TextSizing::new().numerator(1).denominator(2)),
            "Half sized",
        )?;
        row += 2;

        terminal.set_cursor_position(row, 1)?;
        terminal.reset_style()?;
        terminal.write_text("Superscript (n=1:d=2:v=2):")?;
        row += 1;

        terminal.set_cursor_position(row, 1)?;
        terminal.write_text("E=mc")?;
        terminal.write_styled_text(
            None,
            Some(
                &TextSizing::new()
                    .numerator(1)
                    .denominator(2)
                    .width(2)
                    .vertical(VerticalAlign::Top),
            ),
            "2",
        )?;
        row += 2;

        terminal.set_cursor_position(row, 1)?;
        terminal.reset_style()?;
        terminal.write_text("Combined with SGR style:")?;
        row += 1;

        terminal.set_cursor_position(row, 1)?;
        let style = urvim_terminal::Style::new()
            .bold()
            .fg(urvim_terminal::Color::ansi(196));
        terminal.write_styled_text(
            Some(&style),
            Some(&TextSizing::new().scale(2)),
            "Bold Red Double",
        )?;

        terminal.reset_style()?;

        Ok(())
    })?;

    terminal.set_cursor_position(1, 1)?;

    loop {
        let event = terminal.read_event()?;

        if let urvim_terminal::Event::Key(key) = event
            && key.code == urvim_terminal::KeyCode::Char('q')
        {
            break;
        }
    }

    terminal.reset_style()?;

    Ok(())
}
