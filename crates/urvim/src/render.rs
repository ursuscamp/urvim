use std::io;

use rustix::fd::AsFd;

use urvim_core::globals;
use urvim_core::screen::Screen;
use urvim_core::window::{Position, Size};
use urvim_terminal::Terminal;

pub(super) fn handle_resize<I: io::Read + AsFd, O: io::Write + AsFd>(
    terminal: &mut Terminal<I, O>,
    screen: &mut Screen,
    rows: u16,
    cols: u16,
) -> io::Result<()> {
    screen.resize(rows, cols);
    terminal.clear_screen()
}

pub(super) fn render_frame_if_needed<I: io::Read + AsFd, O: io::Write + AsFd>(
    needs_redraw: bool,
    layout: &mut urvim_core::Layout,
    screen: &mut Screen,
    terminal: &mut Terminal<I, O>,
    rows: u16,
    cols: u16,
) -> io::Result<bool> {
    if !needs_redraw {
        return Ok(false);
    }

    render_frame(layout, screen, terminal, rows, cols)?;
    Ok(true)
}

fn render_frame<I: io::Read + AsFd, O: io::Write + AsFd>(
    layout: &mut urvim_core::Layout,
    screen: &mut Screen,
    terminal: &mut Terminal<I, O>,
    rows: u16,
    cols: u16,
) -> io::Result<()> {
    globals::set_active_buffer_id(layout.active_buffer_view().buffer_id());
    screen.clear();
    layout.render(screen, Position::new(0, 0), Size::new(rows, cols));
    screen.render(terminal)?;

    if let Some(cursor_pos) = layout.visual_cursor() {
        terminal.set_cursor_position(cursor_pos.row + 1, cursor_pos.col + 1)?;
    }

    Ok(())
}
