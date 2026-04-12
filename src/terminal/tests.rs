use super::*;
use crate::terminal::test_backend::TestBackend;

fn create_terminal(data: Vec<u8>) -> Terminal<TestBackend, TestBackend> {
    let backend = TestBackend::new(data);
    let output_backend = TestBackend::new(Vec::new());
    let mut terminal = Terminal::new_for_testing(backend, output_backend);

    if let Some((rows, cols)) = get_terminal_size() {
        terminal.last_rows = rows;
        terminal.last_cols = cols;
    }

    terminal
}

fn get_terminal_output(terminal: &Terminal<TestBackend, TestBackend>) -> Vec<u8> {
    terminal.output.get_output()
}

#[test]
fn test_char_keys() {
    let mut terminal = create_terminal(b"a".to_vec());
    let event = terminal.read_event().unwrap();
    assert_eq!(event, Event::Key(Key::new(KeyCode::Char('a'))));
}

#[test]
fn test_bracketed_paste() {
    let mut terminal = create_terminal(b"\x1b[200~hello\x1b[201~".to_vec());
    let event = terminal.read_event().unwrap();
    assert_eq!(event, Event::Paste("hello".to_string()));
}

#[test]
fn test_reverse_tab() {
    let mut terminal = create_terminal(b"\x1b[Z".to_vec());
    let event = terminal.read_event().unwrap();
    assert_eq!(
        event,
        Event::Key(Key::with_modifiers(KeyCode::Tab, Modifiers::SHIFT))
    );
}

#[test]
fn test_set_style() {
    let backend = TestBackend::new(Vec::new());
    let output_backend = TestBackend::new(Vec::new());
    let mut terminal = Terminal::new_for_testing(backend, output_backend);
    let style = Style::new().bold().fg(Color::ansi(196));
    terminal.set_style(&style).unwrap();
    assert_eq!(get_terminal_output(&terminal), b"\x1b[1;38;5;196m");
}

#[test]
fn test_cursor_style_name() {
    assert_eq!(CursorStyle::SteadyBar.name(), "Steady Bar");
}
