use super::*;
use crate::action::ActionResult;
use crate::editor::Action;
use crate::editor::BoundaryMotion;
use crate::editor::LinewiseMotion;
use crate::editor::Operator;
use crate::editor::OperatorTarget;
use crate::editor::TextObject;
use crate::globals;
use crate::terminal::{Color, Style};
use crate::theme::{SyntaxStyles, Theme, ThemeKind, UiStyles};

fn process_action_and_snapshot(window: &mut Window, action: &Action) {
    assert_eq!(window.process_action(action), ActionResult::Handled);

    if action.is_snapshottable() {
        let cursor = window.buffer_view.cursor();
        window
            .buffer_view
            .with_buffer_mut(|buffer| buffer.push_snapshot(cursor))
            .unwrap_or(());
    }
}

fn buffer_text(view: &BufferView) -> String {
    view.with_buffer(|buffer| buffer.as_str())
        .unwrap_or_default()
}

fn themed_window() -> Theme {
    let default_style = Style::new().fg(Color::ansi(15)).bg(Color::ansi(30));
    let ui_styles = UiStyles::new(
        Style::new().fg(Color::ansi(1)).bg(Color::ansi(2)),
        Style::new().fg(Color::ansi(3)).bg(Color::ansi(4)),
        Style::new().fg(Color::ansi(5)).bg(Color::ansi(6)),
        Style::new().fg(Color::ansi(7)).bg(Color::ansi(8)),
        Style::new().fg(Color::ansi(9)).bg(Color::ansi(10)),
        Style::new().fg(Color::ansi(11)).bg(Color::ansi(12)),
    );
    let syntax_styles = SyntaxStyles::new(
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
        Style::new(),
    );

    Theme::new(
        "demo",
        ThemeKind::Ansi256,
        default_style,
        ui_styles,
        syntax_styles,
    )
}

#[test]
fn test_position_default() {
    let pos = Position::default();
    assert_eq!(pos.row, 0);
    assert_eq!(pos.col, 0);
}

#[test]
fn test_position_new() {
    let pos = Position::new(5, 10);
    assert_eq!(pos.row, 5);
    assert_eq!(pos.col, 10);
}

#[test]
fn test_size_default() {
    let size = Size::default();
    assert_eq!(size.rows, 0);
    assert_eq!(size.cols, 0);
}

#[test]
fn test_size_new() {
    let size = Size::new(24, 80);
    assert_eq!(size.rows, 24);
    assert_eq!(size.cols, 80);
}

#[test]
fn test_buffer_view_new() {
    let buffer = Buffer::from_str("test");
    let view = BufferView::new(buffer);

    assert_eq!(view.scroll_offset(), Position::default());
    assert_eq!(view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_buffer_view_cursor() {
    let buffer = Buffer::from_str("test");
    let mut view = BufferView::new(buffer);

    view.set_cursor(Cursor::new(0, 2));
    assert_eq!(view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_buffer_view_scroll_offset() {
    let buffer = Buffer::from_str("test");
    let mut view = BufferView::new(buffer);

    view.set_scroll_offset(Position::new(5, 10));
    assert_eq!(view.scroll_offset(), Position::new(5, 10));
}

#[test]
fn test_window_new() {
    let buffer = Buffer::from_str("test");
    let window = Window::new(buffer);

    assert_eq!(window.buffer_view().cursor(), Cursor::new(0, 0));
}

#[test]
fn test_window_render() {
    let buffer = Buffer::from_str("line1\nline2\nline3");
    let mut window = Window::new(buffer);

    let mut screen = crate::screen::Screen::new(3, 80);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 80));

    // With gutter (3 columns for 3 lines: digits(3) + 2 = 3), buffer starts at col 3
    // Check gutter background is rendered
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    // Check buffer content starts after gutter
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "l");
    assert_eq!(screen.get_cell_mut(1, 3).unwrap().text, "l");
}

#[test]
fn test_window_render_uses_theme_styles() {
    let buffer = Buffer::from_str("line1");
    let mut window = Window::new(buffer);
    let theme = themed_window();
    let expected_gutter_style = theme.ui.gutter;
    let expected_default_style = theme.default_style();
    let _theme_guard = globals::set_test_active_theme(theme);

    let mut screen = crate::screen::Screen::new(1, 12);
    window.render(&mut screen, Position::new(0, 0), Size::new(1, 12));

    assert_eq!(
        screen.get_cell_mut(0, 0).unwrap().style,
        expected_gutter_style
    );
    assert_eq!(
        screen.get_cell_mut(0, 3).unwrap().style,
        expected_default_style
    );
    assert_eq!(
        screen.get_cell_mut(0, 8).unwrap().style,
        expected_default_style
    );
}

#[test]
fn test_window_render_fills_empty_content_rows_with_theme_default() {
    let buffer = Buffer::from_str("line1");
    let mut window = Window::new(buffer);
    let theme = themed_window();
    let expected_gutter_style = theme.ui.gutter;
    let expected_default_style = theme.default_style();
    let _theme_guard = globals::set_test_active_theme(theme);

    let mut screen = crate::screen::Screen::new(3, 12);
    window.render(&mut screen, Position::new(0, 0), Size::new(3, 12));

    assert_eq!(
        screen.get_cell_mut(1, 0).unwrap().style,
        expected_gutter_style
    );
    assert_eq!(
        screen.get_cell_mut(1, 3).unwrap().style,
        expected_default_style
    );
    assert_eq!(
        screen.get_cell_mut(2, 3).unwrap().style,
        expected_default_style
    );
}

#[test]
fn test_render_chunk_new() {
    let chunk = RenderChunk::new("test", crate::terminal::Style::default());
    assert_eq!(chunk.text, "test");
}

#[test]
fn test_render_chunk_default_text() {
    let chunk = RenderChunk::default_text("test");
    assert_eq!(chunk.text, "test");
    assert_eq!(chunk.style, crate::terminal::Style::default());
}

#[test]
fn test_render_data_new() {
    let data = RenderData::new(10);
    assert_eq!(data.line_count(), 0);
    assert_eq!(data.visible_rows(), 10);
}

#[test]
fn test_render_data_get_line() {
    let buffer = Buffer::from_str("line1\nline2\nline3");
    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(3, 80));

    let line = render_data.get_line(0).unwrap();
    assert!(!line.is_empty());
    assert_eq!(line[0].text, "line1");
}

#[test]
fn test_render_data_out_of_bounds() {
    let buffer = Buffer::from_str("line1\nline2\nline3");
    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(Size::new(3, 80));

    assert!(render_data.get_line(10).is_none());
}

// Gutter tests
#[test]
fn test_gutter_width_calculation() {
    // 1-9 lines: 1 digit + 2 padding = 3 columns
    let gutter = Gutter::new(0, 10, 9);
    assert_eq!(gutter.calculate_width(), 3);

    // 1-99 lines: 2 digits + 2 padding = 4 columns
    let gutter = Gutter::new(0, 10, 99);
    assert_eq!(gutter.calculate_width(), 4);

    // 1-999 lines: 3 digits + 2 padding = 5 columns
    let gutter = Gutter::new(0, 10, 999);
    assert_eq!(gutter.calculate_width(), 5);

    // Empty buffer: minimum 3 columns
    let gutter = Gutter::new(0, 10, 0);
    assert_eq!(gutter.calculate_width(), 3);
}

#[test]
fn test_gutter_digit_count() {
    assert_eq!(Gutter::digit_count(0), 1);
    assert_eq!(Gutter::digit_count(9), 1);
    assert_eq!(Gutter::digit_count(10), 2);
    assert_eq!(Gutter::digit_count(99), 2);
    assert_eq!(Gutter::digit_count(100), 3);
    assert_eq!(Gutter::digit_count(999), 3);
    assert_eq!(Gutter::digit_count(1000), 4);
}

#[test]
fn test_gutter_render_background() {
    // Use 10 lines so gutter width is 4 (digits(10) + 2 = 4)
    let mut gutter = Gutter::new(0, 5, 10);
    let mut screen = crate::screen::Screen::new(5, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    let gutter_width = gutter.calculate_width();
    assert_eq!(gutter_width, 4); // Verify expected width

    // Check background is rendered for all visible rows in gutter area
    for row in 0..5 {
        for col in 0..gutter_width {
            let _cell = screen.get_cell_mut(row, col).unwrap();
            // Most cells should be spaces (background or padding)
            // Only specific columns should have line numbers
        }
    }

    // Specifically check that gutter cells have spaces (not line numbers)
    // Column 0 should always be space (left padding)
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, " ");
}

#[test]
fn test_gutter_render_line_numbers() {
    // For 10 lines: digits(10) + 2 = 4 columns
    // Layout: col0=left_pad, col1=empty/1st_digit, col2=2nd_digit/last_digit, col3=right_pad
    let mut gutter = Gutter::new(0, 3, 10);
    let mut screen = crate::screen::Screen::new(3, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    // Width is digits(10) + 2 = 4
    // Line "1": col0=space, col1=space, col2="1", col3=space
    let cell_left_pad = screen.get_cell_mut(0, 0).unwrap();
    assert_eq!(cell_left_pad.text, " "); // left padding
    let cell_empty = screen.get_cell_mut(0, 1).unwrap();
    assert_eq!(cell_empty.text, " "); // empty for 1-digit
    let cell_num = screen.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_num.text, "1"); // line number right-aligned
    let cell_right_pad = screen.get_cell_mut(0, 3).unwrap();
    assert_eq!(cell_right_pad.text, " "); // right padding

    // Line "2": col0=space, col1=space, col2="2", col3=space
    assert_eq!(screen.get_cell_mut(1, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(1, 1).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(1, 2).unwrap().text, "2");
    assert_eq!(screen.get_cell_mut(1, 3).unwrap().text, " ");

    // Line "3": col0=space, col1=space, col2="3", col3=space
    assert_eq!(screen.get_cell_mut(2, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(2, 1).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(2, 2).unwrap().text, "3");
    assert_eq!(screen.get_cell_mut(2, 3).unwrap().text, " ");
}

#[test]
fn test_gutter_wrap_detection() {
    // Simulate scrolling where same buffer line appears in multiple screen rows
    // start_line=5, visible_rows=2 would show buffer lines 5 and 6
    // With 10 lines: width = 4
    // Row 0: buffer line 5 -> "6" at column 2, right padding at column 3
    // Row 1: buffer line 6 -> "7" at column 2, right padding at column 3
    let mut gutter = Gutter::new(5, 2, 10);
    let mut screen = crate::screen::Screen::new(2, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    // Row 0: buffer line 5 -> "6" (1-indexed)
    // Line "6" at column 2 (right-aligned for 1-digit)
    let cell_0 = screen.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_0.text, "6");

    // Row 1: buffer line 6 -> "7" (1-indexed)
    let cell_1 = screen.get_cell_mut(1, 2).unwrap();
    assert_eq!(cell_1.text, "7");
}

#[test]
fn test_gutter_scroll_offset() {
    // Test gutter with scroll offset
    // With 20 total lines: digits(20) + 2 = 4 columns
    // start_line=10 means first visible is buffer line 10 (display 11, 2 digits)
    let mut gutter = Gutter::new(10, 5, 20);
    let mut screen = crate::screen::Screen::new(5, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    // Verify gutter width
    assert_eq!(gutter.calculate_width(), 4);

    // First visible line is buffer line 10 (1-indexed: 11, 2 digits)
    // Layout: col0=left_pad, col1="1", col2="1", col3=right_pad
    let cell_left_pad = screen.get_cell_mut(0, 0).unwrap();
    assert_eq!(cell_left_pad.text, " "); // left padding
    let cell_digit1 = screen.get_cell_mut(0, 1).unwrap();
    assert_eq!(cell_digit1.text, "1"); // first digit of "11"
    let cell_digit2 = screen.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_digit2.text, "1"); // second digit of "11"
    let cell_right_pad = screen.get_cell_mut(0, 3).unwrap();
    assert_eq!(cell_right_pad.text, " "); // right padding
}

#[test]
fn test_window_visual_cursor_with_gutter() {
    let buffer = Buffer::from_str("line1\nline2\nline3");
    let mut window = Window::new(buffer);

    // Set cursor to line 0, column 2 (within "line1")
    window.buffer_view_mut().set_cursor(Cursor::new(0, 2));

    // Need to call render to build render_data first
    let size = Size::new(3, 80);
    let mut screen = crate::screen::Screen::new(3, 80);
    window.render(&mut screen, Position::new(0, 0), size);

    // Get visual cursor position
    let cursor_pos = window.visual_cursor();

    assert!(cursor_pos.is_some());
    let pos = cursor_pos.unwrap();

    // Cursor should be offset by gutter width (3 columns for 3 lines)
    // The cursor is at column 2 in the content, plus 3 for gutter = column 5
    let gutter_width = 3; // digits(3) + 2 = 3
    assert_eq!(pos.col, 2 + gutter_width);
}

#[test]
fn test_gutter_scroll_and_rerender() {
    // Simulate scrolling and re-rendering
    // First render at start_line=0
    let mut gutter = Gutter::new(0, 5, 20);
    let mut screen = crate::screen::Screen::new(5, 80);

    gutter.render(&mut screen, Position::new(0, 0));

    // Verify initial render - line 1 should have gutter style
    // For 20 lines, width = digits(20) + 2 = 2 + 2 = 4
    // Line "1" (digit 1): col0=space, col1=space, col2="1", col3=space
    let cell_line1 = screen.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_line1.text, "1");

    // Now simulate scrolling - create new gutter at start_line=3
    let mut gutter2 = Gutter::new(3, 5, 20);
    let mut screen2 = crate::screen::Screen::new(5, 80);

    gutter2.render(&mut screen2, Position::new(0, 0));

    // After scrolling to line 3, row 0 should show line 4 (buffer line 3 + 1)
    // Line "4": col0=space, col1=space, col2="4", col3=space
    let cell_scrolled = screen2.get_cell_mut(0, 2).unwrap();
    assert_eq!(cell_scrolled.text, "4");

    // Verify gutter background is rendered for ALL rows including empty ones
    // Row 4 would be buffer line 7 which doesn't exist in 20 lines, but background should still be there
    let cell_empty_row = screen2.get_cell_mut(4, 0).unwrap();
    assert_eq!(cell_empty_row.text, " ");
}

#[test]
fn test_gutter_then_buffer_render() {
    // Test that buffer content doesn't overwrite gutter
    // This simulates what happens in Window::render
    let gutter_width = 4; // digits(20) + 2 = 4

    // First render gutter
    let mut gutter = Gutter::new(0, 5, 20);
    let mut screen = crate::screen::Screen::new(5, 80);
    gutter.render(&mut screen, Position::new(0, 0));

    // Verify gutter cells have correct content
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");

    // Now simulate buffer content rendering at offset
    let content_origin = Position::new(0, gutter_width);
    let content_size = Size::new(5, 80 - gutter_width);

    // Create some buffer content to render
    let buffer = crate::buffer::Buffer::from_str("line1\nline2\nline3");
    let view = BufferView::new(buffer);
    let render_data = view.build_render_data(content_size);
    render_data.render(&mut screen, content_origin);

    // After buffer rendering, gutter cells should STILL have correct gutter content
    // Gutter is at columns 0-3, buffer is at column 4+
    // Column 0 should still be gutter left padding
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    // Column 2 should still have line number "1" (not overwritten by buffer)
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
    // Column 3 should still be gutter right padding
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");

    // But buffer content should be at column 4+
    assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, "l"); // "line1"
}

#[test]
fn test_gutter_width_change() {
    // Test gutter when width changes (e.g., file grows from 99 to 100 lines)
    // Old gutter width = 4 (digits(99) + 2 = 2 + 2)
    // New gutter width = 5 (digits(100) + 2 = 3 + 2)

    // Simulate first render with width=4
    let mut screen = crate::screen::Screen::new(3, 80);
    let mut gutter = Gutter::new(0, 3, 99);
    gutter.render(&mut screen, Position::new(0, 0));

    // With width=4 and line "1":
    // col0=space, col1=space, col2="1", col3=space
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, " ");

    // Now simulate re-render with width=5 (simulating file grew)
    // The screen still has old content, but we re-render with new width
    let mut gutter2 = Gutter::new(0, 3, 100);
    gutter2.render(&mut screen, Position::new(0, 0));

    // With width=5 and line "1" (1 digit):
    // col0=space, col1=space, col2=space, col3="1", col4=space
    // Because: right_padding at col4, line at col4-1=3
    assert_eq!(screen.get_cell_mut(0, 0).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 1).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, " ");
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, " ");

    // Also verify multi-digit line number
    // Line "11" would be at columns 2-3
    let mut gutter3 = Gutter::new(9, 3, 100); // start at line 9, showing 10, 11
    gutter3.render(&mut screen, Position::new(0, 0));

    // Line "10" at row 0: col2="1", col3="0", col4=space
    assert_eq!(screen.get_cell_mut(0, 2).unwrap().text, "1");
    assert_eq!(screen.get_cell_mut(0, 3).unwrap().text, "0");
    assert_eq!(screen.get_cell_mut(0, 4).unwrap().text, " ");
}

// Column preservation tests

#[test]
fn test_column_preservation_first_vertical_move() {
    // First vertical move should use current column and remember it
    let buffer = Buffer::from_str("abcdefgh\nij");
    let mut window = Window::new(buffer);

    // Position at column 5 on first line
    window.buffer_view.set_cursor(Cursor::new(0, 5));

    // First move down via Window - should use current column (5), remember it
    window.process_action(&Action::MoveDown);
    assert_eq!(window.buffer_view.cursor().line, 1);
    // Line 2 is "ij" (length 2), so column 5 should clamp to 2
    assert_eq!(window.buffer_view.cursor().col, 2);
}

#[test]
fn test_column_preservation_consecutive_vertical_moves() {
    // Consecutive vertical moves should preserve remembered column
    let buffer = Buffer::from_str("abcdefgh\nabcdefgh\nabcdefgh");
    let mut window = Window::new(buffer);

    // Position at column 5 on first line
    window.buffer_view.set_cursor(Cursor::new(0, 5));

    // Move down - remembers column 5
    window.process_action(&Action::MoveDown);
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 5));

    // Move down again - should use remembered column 5
    window.process_action(&Action::MoveDown);
    assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 5));

    // Move up - should use remembered column 5
    window.process_action(&Action::MoveUp);
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 5));
}

#[test]
fn test_column_preservation_horizontal_resets() {
    // Horizontal movement should reset remembered column
    use crate::editor::Action;

    let buffer = Buffer::from_str("abcdefgh\nabcdefgh\nabcdefgh");
    let mut window = Window::new(buffer);

    // Position at column 5 on first line
    window.buffer_view.set_cursor(Cursor::new(0, 5));

    // Move down - remembers column 5
    window.process_action(&Action::MoveDown);
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 5));

    // Move right - should reset remembered column to current (now at column 6)
    window.process_action(&Action::MoveRight);
    // Now at column 6 on line 1

    // Move down again - should use new column 6 and go to line 2
    window.process_action(&Action::MoveDown);
    assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 6));
}

#[test]
fn test_column_preservation_clamp_on_short_line() {
    // Moving to shorter line should clamp to end of line
    let buffer = Buffer::from_str("abcdefgh\nij\nabcdefgh");
    let mut window = Window::new(buffer);

    // Position at column 5 on first line
    window.buffer_view.set_cursor(Cursor::new(0, 5));

    // Move down to shorter line "ij" (length 2)
    window.process_action(&Action::MoveDown);
    // Should clamp to column 2 (end of "ij")
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 2));

    // Move down to longer line - should use remembered column 5
    window.process_action(&Action::MoveDown);
    assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 5));
}

#[test]
fn test_action_resets_remembered_column() {
    use crate::buffer::Boundary;
    use crate::editor::Action;

    // Horizontal movements should reset
    assert!(Action::MoveLeft.resets_remembered_column());
    assert!(Action::MoveRight.resets_remembered_column());
    assert!(Action::ForwardTo(Boundary::Word).resets_remembered_column());
    assert!(Action::BackTo(Boundary::Word).resets_remembered_column());
    assert!(Action::MoveToLineEnd.resets_remembered_column());
    assert!(Action::MoveToLineStart.resets_remembered_column());
    assert!(Action::MoveToLineContentStart.resets_remembered_column());

    // Vertical movements should NOT reset
    assert!(!Action::MoveUp.resets_remembered_column());
    assert!(!Action::MoveDown.resets_remembered_column());

    // Other actions should not reset
    assert!(!Action::SwitchToInsert.resets_remembered_column());
    assert!(Action::InsertChar('a').resets_remembered_column());
    assert!(Action::DeleteBackward.resets_remembered_column());
    assert!(Action::DeleteForward.resets_remembered_column());
}

#[test]
fn test_action_uses_remembered_column() {
    use crate::editor::Action;

    // Vertical movements should use remembered column
    assert!(Action::MoveUp.uses_remembered_column());
    assert!(Action::MoveDown.uses_remembered_column());

    // Other movements should NOT
    assert!(!Action::MoveLeft.uses_remembered_column());
    assert!(!Action::MoveRight.uses_remembered_column());
}

// Character Scan Motion Tests

#[test]
fn test_find_forward_moves_to_char() {
    // "hello world" - cursor at 'h', find 'o'
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.process_action(&Action::FindForward('o'));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 4)); // 'o' is at column 4
}

#[test]
fn test_find_forward_finds_third_occurrence() {
    // "x x x" - find 3rd 'x'
    let buffer = Buffer::from_str("x x x");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.process_action(&Action::Count(3, Box::new(Action::FindForward('x'))));
    // Third 'x' is at column 4
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 4));
}

#[test]
fn test_find_forward_not_found_stays_in_place() {
    // "hello" - find 'z' (doesn't exist)
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 2)); // at 'l'
    window.process_action(&Action::FindForward('z'));
    // Cursor should stay at column 2
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_find_backward_moves_to_char() {
    // "hello world" - cursor at 'd', find 'o' (first when going backward from cursor)
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 10)); // at 'd'
    window.process_action(&Action::FindBackward('o'));
    // First 'o' when going backward from position 10 is at column 7
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 7));
}

#[test]
fn test_find_backward_not_found_stays_in_place() {
    // "hello" - cursor at 'h', find 'x' (doesn't exist before)
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.process_action(&Action::FindBackward('x'));
    // Cursor should stay at column 0
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_till_forward_lands_before_char() {
    // "hello" - cursor at 'h', till 'o' should land on 'l' (column 3)
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.process_action(&Action::TillForward('o'));
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 3)); // 'l' is at column 3
}

#[test]
fn test_till_forward_clamp_at_line_start() {
    // "hello" - cursor at 'h', till 'h' should clamp to column 0
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.process_action(&Action::TillForward('h'));
    // Till lands one before 'h', which would be column -1, clamped to 0
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_till_backward_lands_after_char() {
    // "hello" - cursor at 'l', till 'e' should land on 'e' (column 1)
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 4)); // at 'o'
    window.process_action(&Action::TillBackward('h'));
    // Till backward 'h' from 'o': 'h' is at 0, +1 = column 1
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_till_backward_clamp_at_line_end() {
    // "hello" - cursor at 'o', till 'o' - no previous 'o' to find, so stays
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 4)); // at 'o'
    window.process_action(&Action::TillBackward('o'));
    // Till backward 'o' from 'o': there's no 'o' before position 4, so cursor stays
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 4));
}

#[test]
fn test_find_forward_with_count() {
    // "hello world" - 2fx finds 2nd 'o'
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    // Use Count wrapper for the action
    window.process_action(&Action::Count(2, Box::new(Action::FindForward('o'))));
    // 'o' appears at column 4 and 7
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 7)); // second 'o'
}

#[test]
fn test_find_backward_with_count() {
    // "hello world" - 2Fl finds 2nd 'l' when going backward from 'd'
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 10)); // at 'd'
    window.process_action(&Action::Count(2, Box::new(Action::FindBackward('l'))));
    // 'l' appears at columns 2, 3, and 9
    // Going backward from 'd' at 10: 1st 'l' is at 9, 2nd 'l' is at 3
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 3));
}

#[test]
fn test_find_backward_skips_current_char_on_duplicate() {
    // "helllo" - cursor on 3rd 'l', Fl should find 2nd 'l'
    let buffer = Buffer::from_str("helllo");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 3)); // at 3rd 'l'
    window.process_action(&Action::FindBackward('l'));
    // Should find 2nd 'l' at column 2, not 3rd 'l' at column 3
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_count_diw_deletes_multiple_words() {
    let buffer = Buffer::from_str("one two three four");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.process_action(&Action::Count(
        3,
        Box::new(Action::Operation(
            Operator::Delete,
            OperatorTarget::TextObject(TextObject::InnerWord),
        )),
    ));

    assert_eq!(
        window
            .buffer_view
            .with_buffer(|buffer| buffer.line_at(0).map(|line| line.to_string()))
            .flatten(),
        Some(" four".to_string())
    );
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dw_deletes_through_next_word_start() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.process_action(&Action::Operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_cw_changes_through_next_word_start() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    let result = window.process_action(&Action::Operation(
        Operator::Change,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
    assert!(
        Action::Operation(
            Operator::Change,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward)
        )
        .switches_to_insert_mode()
    );
}

#[test]
fn test_cw_at_end_of_line_is_noop() {
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 5));
    let result = window.process_action(&Action::Operation(
        Operator::Change,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
    ));

    assert_eq!(result, ActionResult::NotHandled);
    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 5));
}

#[test]
fn test_delete_forward_undo_and_redo() {
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    process_action_and_snapshot(&mut window, &Action::DeleteForward);

    assert_eq!(buffer_text(window.buffer_view()), "ello");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    if let Some(cursor) = window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.undo())
        .flatten()
    {
        window.buffer_view.set_cursor(cursor);
    }
    assert_eq!(buffer_text(window.buffer_view()), "hello");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    if let Some(cursor) = window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.redo())
        .flatten()
    {
        window.buffer_view.set_cursor(cursor);
    }
    assert_eq!(buffer_text(window.buffer_view()), "ello");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dw_undo_and_redo() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    process_action_and_snapshot(
        &mut window,
        &Action::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        ),
    );

    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    if let Some(cursor) = window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.undo())
        .flatten()
    {
        window.buffer_view.set_cursor(cursor);
    }
    assert_eq!(buffer_text(window.buffer_view()), "hello world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    if let Some(cursor) = window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.redo())
        .flatten()
    {
        window.buffer_view.set_cursor(cursor);
    }
    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_cg_changes_to_first_line() {
    let buffer = Buffer::from_str("one\ntwo\nthree");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(1, 0));
    let result = window.process_action(&Action::Operation(
        Operator::Change,
        OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
    ));

    assert_eq!(result, ActionResult::Handled);
    assert_eq!(buffer_text(window.buffer_view()), "one");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_counted_dw_undo_restores_original_text() {
    let buffer = Buffer::from_str("one two three four");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    process_action_and_snapshot(
        &mut window,
        &Action::Count(
            2,
            Box::new(Action::Operation(
                Operator::Delete,
                OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
            )),
        ),
    );

    assert_eq!(buffer_text(window.buffer_view()), "three four");

    if let Some(cursor) = window
        .buffer_view
        .with_buffer_mut(|buffer| buffer.undo())
        .flatten()
    {
        window.buffer_view.set_cursor(cursor);
    }
    assert_eq!(buffer_text(window.buffer_view()), "one two three four");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dollar_deletes_to_line_end() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 6));
    window.process_action(&Action::Operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::LineEnd),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "hello ");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 6));
}

#[test]
fn test_d0_deletes_to_line_start() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 6));
    window.process_action(&Action::Operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::LineStart),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dcaret_deletes_to_line_content_start() {
    let buffer = Buffer::from_str("    hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 10));
    window.process_action(&Action::Operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::LineContentStart),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "    world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 4));
}

#[test]
fn test_db_deletes_back_to_previous_word_start() {
    let buffer = Buffer::from_str("hello world");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 6));
    window.process_action(&Action::Operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::WordBackward),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "world");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dgg_deletes_to_first_line_linewise() {
    let buffer = Buffer::from_str("one\ntwo\nthree\nfour\nfive");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(3, 1));
    window.process_action(&Action::Operation(
        Operator::Delete,
        OperatorTarget::LinewiseMotion(LinewiseMotion::FirstLine),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "five");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_d_g_deletes_to_last_line_linewise() {
    let buffer = Buffer::from_str("one\ntwo\nthree\nfour\nfive");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(2, 0));
    window.process_action(&Action::Operation(
        Operator::Delete,
        OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "one\ntwo");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 0));
}

#[test]
fn test_counted_d_g_deletes_to_destination_line() {
    let buffer = Buffer::from_str("one\ntwo\nthree\nfour\nfive\nsix");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(2, 0));
    window.process_action(&Action::Count(
        5,
        Box::new(Action::Operation(
            Operator::Delete,
            OperatorTarget::LinewiseMotion(LinewiseMotion::LastLine),
        )),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "one\ntwo\nsix");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(2, 0));
}

#[test]
fn test_dw_with_count_deletes_multiple_words() {
    let buffer = Buffer::from_str("one two three four");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.process_action(&Action::Count(
        2,
        Box::new(Action::Operation(
            Operator::Delete,
            OperatorTarget::BoundaryMotion(BoundaryMotion::WordForward),
        )),
    ));

    assert_eq!(buffer_text(window.buffer_view()), "three four");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));
}

#[test]
fn test_dbigword_forward_and_backward() {
    let buffer = Buffer::from_str("alpha --- beta");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.process_action(&Action::Operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordForward),
    ));
    assert_eq!(buffer_text(window.buffer_view()), "--- beta");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 0));

    let buffer = Buffer::from_str("alpha --- beta");
    let mut window = Window::new(buffer);
    window.buffer_view.set_cursor(Cursor::new(0, 10));
    window.process_action(&Action::Operation(
        Operator::Delete,
        OperatorTarget::BoundaryMotion(BoundaryMotion::BigWordBackward),
    ));
    assert_eq!(buffer_text(window.buffer_view()), "alpha beta");
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 6));
}

#[test]
fn test_till_forward_repeated_finds_next_occurrence() {
    // "hello" - tl repeated should find subsequent 'l's
    let buffer = Buffer::from_str("hello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0)); // at 'h'
    window.process_action(&Action::TillForward('l'));
    // First 'l' at column 2, land before it at column 1
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));

    window.process_action(&Action::TillForward('l'));
    // Second 'l' at column 3, land before it at column 2
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));
}

#[test]
fn test_till_backward_repeated_finds_previous_occurrence() {
    // "hhello" - Th repeated should find previous 'h's
    let buffer = Buffer::from_str("hhello");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 5)); // at 'o'
    window.process_action(&Action::TillBackward('h'));
    // First 'h' at column 1, land after it at column 2
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 2));

    window.process_action(&Action::TillBackward('h'));
    // Second 'h' at column 0, land after it at column 1
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));

    window.process_action(&Action::TillBackward('h'));
    // No more 'h' before column 0, cursor stays at column 1
    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_till_forward_preserves_grapheme_boundaries() {
    let buffer = Buffer::from_str("a😀b");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 0));
    window.process_action(&Action::TillForward('b'));

    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_till_backward_preserves_grapheme_boundaries() {
    let buffer = Buffer::from_str("a😀b");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 5));
    window.process_action(&Action::TillBackward('a'));

    assert_eq!(window.buffer_view.cursor(), Cursor::new(0, 1));
}

#[test]
fn test_move_to_last_line_preserves_visual_column() {
    let buffer = Buffer::from_str("ab\na😀b");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 2));
    window.process_action(&Action::MoveToLastLine);

    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 1));
}

#[test]
fn test_count_screen_top_preserves_visual_column() {
    let buffer = Buffer::from_str("ab\na😀b\ncd");
    let mut window = Window::new(buffer);

    window.size = Size::new(2, 10);
    window.buffer_view.set_scroll_offset(Position::new(1, 0));
    window.buffer_view.set_cursor(Cursor::new(0, 2));
    window.process_action(&Action::Count(1, Box::new(Action::MoveToScreenTop)));

    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 1));
}

#[test]
fn test_next_paragraph_clamps_visual_column_on_blank_line() {
    let buffer = Buffer::from_str("ab\n\ncd");
    let mut window = Window::new(buffer);

    window.buffer_view.set_cursor(Cursor::new(0, 2));
    window.process_action(&Action::MoveToNextParagraph);

    assert_eq!(window.buffer_view.cursor(), Cursor::new(1, 0));
}
