use std::io;
use urvim::terminal::{Color, Style, UnderlineStyle};

fn main() -> io::Result<()> {
    if !is_terminal::is_terminal(&std::io::stdin()) {
        eprintln!("Error: Must be run from a terminal");
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "stdin is not a terminal",
        ));
    }

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    let mut terminal = urvim::terminal::Terminal::new(stdin, stdout)?;

    write_styled_demo(&mut terminal)?;

    terminal.set_cursor_position(1, 1)?;

    loop {
        let event = terminal.read_event()?;

        if let urvim::terminal::Event::Key(key) = event {
            if key.code == urvim::terminal::KeyCode::Char('q') {
                break;
            }
        }
    }

    terminal.reset_style()?;

    Ok(())
}

fn write_styled_demo(
    terminal: &mut urvim::terminal::Terminal<
        impl io::Read + std::os::unix::io::AsFd,
        impl io::Write + std::os::unix::io::AsFd,
    >,
) -> io::Result<()> {
    terminal.batch(|terminal| {
        let mut r = 1u16;
        let left_col = 1u16;
        let right_col = 45u16;

        terminal.set_cursor_position(r, left_col)?;
        terminal.write_text("Decorations:")?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().bold())?;
        terminal.write_text("Bold")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().faint())?;
        terminal.write_text("Faint")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().italic())?;
        terminal.write_text("Italic")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().underline())?;
        terminal.write_text("Underline")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().blink())?;
        terminal.write_text("Blink")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().reverse())?;
        terminal.write_text("Reverse")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().hidden())?;
        terminal.write_text("Hidden")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().strikethrough())?;
        terminal.write_text("Strikethrough")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().overline())?;
        terminal.write_text("Overline")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().double_underline())?;
        terminal.write_text("Double Underline")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().bold().italic().underline())?;
        terminal.write_text("Bold+Italic+Underline")?;
        terminal.reset_style()?;
        r += 2;

        terminal.set_cursor_position(r, left_col)?;
        terminal.write_text("Foreground ANSI:")?;
        r += 1;

        for i in 0..8u8 {
            terminal.set_cursor_position(r, left_col)?;
            terminal.reset_style()?;
            let style = Style::new().fg(Color::ansi(i));
            terminal.set_style(&style)?;
            terminal.write_text(&format!("{:3}", i))?;
            terminal.reset_style()?;
            terminal.write_text(" ")?;
            r += 1;
        }
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.write_text("Background ANSI:")?;
        r += 1;

        for i in 0..8u8 {
            terminal.set_cursor_position(r, left_col)?;
            terminal.reset_style()?;
            let style = Style::new().bg(Color::ansi(i)).fg(Color::ansi(7));
            terminal.set_style(&style)?;
            terminal.write_text(&format!("{:3}", i))?;
            terminal.reset_style()?;
            terminal.write_text(" ")?;
            r += 1;
        }
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.write_text("Foreground RGB:")?;
        r += 1;

        let rgb_fg = [
            (255, 0, 0, "Red"),
            (0, 255, 0, "Green"),
            (0, 0, 255, "Blue"),
            (255, 255, 0, "Yellow"),
        ];

        for (r_val, g, b, name) in rgb_fg {
            terminal.set_cursor_position(r, left_col)?;
            terminal.reset_style()?;
            let style = Style::new().fg(Color::rgb(r_val, g, b));
            terminal.set_style(&style)?;
            terminal.write_text(name)?;
            terminal.reset_style()?;
            r += 1;
        }
        r += 1;

        terminal.set_cursor_position(r, left_col)?;
        terminal.write_text("Background RGB:")?;
        r += 1;

        let rgb_bg = [
            (255, 0, 0, "Red"),
            (0, 255, 0, "Green"),
            (0, 0, 255, "Blue"),
            (255, 255, 0, "Yellow"),
        ];

        for (r_val, g, b, name) in rgb_bg {
            terminal.set_cursor_position(r, left_col)?;
            terminal.reset_style()?;
            let style = Style::new().bg(Color::rgb(r_val, g, b)).fg(Color::ansi(7));
            terminal.set_style(&style)?;
            terminal.write_text(name)?;
            terminal.reset_style()?;
            r += 1;
        }

        r = 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.write_text("Kitty Underline Styles:")?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().underline_style(UnderlineStyle::Straight))?;
        terminal.write_text("Straight")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().underline_style(UnderlineStyle::Double))?;
        terminal.write_text("Double")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().underline_style(UnderlineStyle::Curly))?;
        terminal.write_text("Curly")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().underline_style(UnderlineStyle::Dotted))?;
        terminal.write_text("Dotted")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().underline_style(UnderlineStyle::Dashed))?;
        terminal.write_text("Dashed")?;
        terminal.reset_style()?;
        r += 2;

        terminal.set_cursor_position(r, right_col)?;
        terminal.write_text("Kitty Underline Colors:")?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().underline().underline_color(Color::ansi(196)))?;
        terminal.write_text("Red underline")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().underline().underline_color(Color::ansi(46)))?;
        terminal.write_text("Green underline")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().underline().underline_color(Color::ansi(21)))?;
        terminal.write_text("Blue underline")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(
            &Style::new()
                .underline()
                .underline_color(Color::rgb(255, 0, 255)),
        )?;
        terminal.write_text("RGB magenta")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(
            &Style::new()
                .underline()
                .underline_color(Color::rgb(255, 255, 0)),
        )?;
        terminal.write_text("RGB yellow")?;
        terminal.reset_style()?;
        r += 2;

        terminal.set_cursor_position(r, right_col)?;
        terminal.write_text("Combined:")?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(
            &Style::new()
                .underline_style(UnderlineStyle::Double)
                .underline_color(Color::ansi(196))
                .fg(Color::ansi(15))
                .bg(Color::ansi(21))
                .bold(),
        )?;
        terminal.write_text("Double+Color+BG")?;
        terminal.reset_style()?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(
            &Style::new()
                .underline_style(UnderlineStyle::Curly)
                .underline_color(Color::rgb(0, 255, 0)),
        )?;
        terminal.write_text("Curly+RGB green")?;
        terminal.reset_style()?;
        r += 2;

        terminal.set_cursor_position(r, right_col)?;
        terminal.write_text("Reset Test:")?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.reset_style()?;
        terminal.set_style(&Style::new().bold().fg(Color::ansi(196)))?;
        terminal.write_text("Bold Red")?;
        terminal.reset_style()?;
        terminal.write_text(" <- should be normal")?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.set_style(&Style::new().italic().fg(Color::ansi(46)))?;
        terminal.write_text("Italic Green")?;
        terminal.reset_style()?;
        terminal.write_text(" <- should be normal")?;
        r += 1;

        terminal.set_cursor_position(r, right_col)?;
        terminal.set_style(
            &Style::new()
                .underline_style(UnderlineStyle::Double)
                .fg(Color::ansi(21)),
        )?;
        terminal.write_text("Double Blue")?;
        terminal.reset_style()?;
        terminal.write_text(" <- should be normal")?;

        Ok(())
    })
}
