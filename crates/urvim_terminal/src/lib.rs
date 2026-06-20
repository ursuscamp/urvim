//! Terminal I/O handling.
//!
//! This module provides the core `Terminal` type for terminal input and output,
//! including:
//! - Raw terminal mode setup and restoration
//! - Escape sequence parsing for key events
//! - Terminal resize detection
//! - Timer ticks for editor wakeups
//! - Bracketed paste mode support
//! - Text styling and cursor control
//! - Clipboard operations (OSC 52)
//!
//! # Usage
//!
//! ```ignore
//! use urvim_terminal::{Terminal, Event, KeyCode};
//! use std::io::{stdin, stdout};
//!
//! let mut terminal = Terminal::new(stdin(), stdout()).unwrap();
//!
//! loop {
//!     match terminal.read_event().unwrap() {
//!         Event::Key(key) => {
//!             if key.code == KeyCode::Char('q') {
//!                 break;
//!             }
//!         }
//!         Event::Resize(rows, cols) => {
//!             println!("Terminal resized to {}x{}", rows, cols);
//!         }
//!         Event::Tick => {
//!             // No input was available; the editor loop woke up for background work.
//!         }
//!         Event::Paste(text) => {
//!             println!("Pasted: {}", text);
//! }
//!     }
//! }
//!
//! terminal.restore().unwrap();
//! ```

#[allow(dead_code)]
pub mod buffer;
pub mod escape;
mod input;
pub mod keys;
mod lifecycle;
mod output;
pub mod size;
pub mod sizing;
#[allow(dead_code)]
pub mod style;
pub mod utils;

#[cfg(test)]
mod test_backend;

#[cfg(test)]
mod tests;

use crate::utils::write_decimal;
use buffer::ByteBuffer;
pub use keys::{Event, Key, KeyCode, Modifiers};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fd::AsFd;
use rustix::termios::{OptionalActions, Termios, tcgetattr, tcsetattr};
use size::{get_terminal_size, query_cursor_position};
pub use sizing::{HorizontalAlign, TextSizing, TextSizingSupport, VerticalAlign};
use std::io::{self, Read, Write};
pub use style::{Color, Rgb, Style, UnderlineStyle};
#[allow(dead_code)]
pub use utils::osc52_copy_to_clipboard;

/// Escape sequence to enter the alternate screen buffer.
const ENTER_ALTERNATIVE_SCREEN: &str = "\x1b[?1049h";
/// Escape sequence to clear the screen and move cursor to home position.
const CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";
/// Escape sequence to exit the alternate screen buffer.
const EXIT_ALTERNATIVE_SCREEN: &str = "\x1b[?1049l";
/// Escape sequence to enable CSI-u mode (Kitty keyboard protocol).
const ENABLE_CSI_U: &str = "\x1b[>1u";
/// Escape sequence to disable CSI-u mode.
const DISABLE_CSI_U: &str = "\x1b[<u";
/// Escape sequence to enable bracketed paste mode.
const ENABLE_BRACKETED_PASTE: &str = "\x1b[?2004h";
/// Escape sequence to disable bracketed paste mode.
const DISABLE_BRACKETED_PASTE: &str = "\x1b[?2004l";
/// Escape sequence to hide the cursor.
const HIDE_CURSOR: &str = "\x1b[?25l";
/// Escape sequence to show the cursor.
const SHOW_CURSOR: &str = "\x1b[?25h";

/// Maximum size for paste data in bytes. Pastes exceeding this size
/// will be discarded and re-read as individual key events.
const MAX_PASTE_SIZE: usize = 1024 * 1024;

/// Poll timeout in milliseconds for TTY mode.
/// This balances responsiveness with CPU usage:
/// - Shorter timeout = more responsive to input, but more CPU overhead from frequent polls
/// - Longer timeout = less CPU overhead, but higher input latency
/// - 50ms is a good balance: responsive (< 1 frame at 60fps) while not hammering the CPU
const POLL_TIMEOUT_MS: i32 = 50;

/// Terminal cursor style.
///
/// These styles control the appearance of the blinking cursor.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    /// Blinking block cursor.
    BlinkingBlock,
    /// Steady (non-blinking) block cursor.
    SteadyBlock,
    /// Blinking underline cursor.
    BlinkingUnderline,
    /// Steady underline cursor.
    SteadyUnderline,
    /// Blinking vertical bar cursor.
    BlinkingBar,
    /// Steady vertical bar cursor.
    SteadyBar,
}

#[allow(dead_code)]
impl CursorStyle {
    /// Returns the ANSI escape sequence for this cursor style.
    ///
    /// Format: `\x1b[N q` where N is 0-6
    pub fn as_str(&self) -> &'static str {
        match self {
            CursorStyle::BlinkingBlock => "\x1b[0 q",
            CursorStyle::SteadyBlock => "\x1b[2 q",
            CursorStyle::BlinkingUnderline => "\x1b[3 q",
            CursorStyle::SteadyUnderline => "\x1b[4 q",
            CursorStyle::BlinkingBar => "\x1b[5 q",
            CursorStyle::SteadyBar => "\x1b[6 q",
        }
    }

    /// Returns a human-readable name for this cursor style.
    pub fn name(&self) -> &'static str {
        match self {
            CursorStyle::BlinkingBlock => "Blinking Block",
            CursorStyle::SteadyBlock => "Steady Block",
            CursorStyle::BlinkingUnderline => "Blinking Underline",
            CursorStyle::SteadyUnderline => "Steady Underline",
            CursorStyle::BlinkingBar => "Blinking Bar",
            CursorStyle::SteadyBar => "Steady Bar",
        }
    }
}

/// Array of all available cursor styles.
#[allow(dead_code)]
pub const CURSOR_STYLES: &[CursorStyle] = &[
    CursorStyle::BlinkingBlock,
    CursorStyle::SteadyBlock,
    CursorStyle::BlinkingUnderline,
    CursorStyle::SteadyUnderline,
    CursorStyle::BlinkingBar,
    CursorStyle::SteadyBar,
];

/// Terminal instance for reading input and writing output.
///
/// This struct provides a complete terminal interface, handling:
/// - Raw mode terminal configuration
/// - Input event parsing (keys, resize, paste)
/// - Output formatting (cursor movement, styles, text)
///
/// The terminal operates in two modes:
/// 1. **Real TTY mode**: Uses polling for input, detects resize events
/// 2. **Testing mode**: No polling, direct input reading
///
/// Type parameters:
/// - `I`: Input source implementing `Read` and `AsFd`
/// - `O`: Output destination implementing `Write` and `AsFd`
pub struct Terminal<I: Read + AsFd, O: Write + AsFd> {
    input: I,
    output: O,
    /// Original terminal attributes for restoration
    original: Option<Termios>,
    /// Buffer for accumulating input bytes
    buffer: ByteBuffer,
    /// Whether a bracketed paste is in progress
    paste_active: bool,
    /// Last known terminal row count
    last_rows: u16,
    /// Last known terminal column count
    last_cols: u16,
    /// Whether stdin is a real TTY
    is_tty: bool,
    /// Whether to flush output after each write
    flush: bool,
}

impl<I: Read + AsFd, O: Write + AsFd> Drop for Terminal<I, O> {
    fn drop(&mut self) {
        self.restore().ok();
    }
}
