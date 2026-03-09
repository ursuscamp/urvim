//! Terminal I/O handling.
//!
//! This module provides the core `Terminal` type for terminal input and output,
//! including:
//! - Raw terminal mode setup and restoration
//! - Escape sequence parsing for key events
//! - Terminal resize detection
//! - Bracketed paste mode support
//! - Text styling and cursor control
//! - Clipboard operations (OSC 52)
//!
//! # Usage
//!
//! ```ignore
//! use urvim::terminal::{Terminal, Event, KeyCode};
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
pub mod keys;
pub mod size;
pub mod sizing;
#[allow(dead_code)]
pub mod style;
pub mod utils;

use crate::terminal::utils::write_decimal;
use buffer::ByteBuffer;
use escape::parse_event_with_buffer;
pub use keys::{Event, Key, KeyCode, Modifiers};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fd::AsFd;
use rustix::termios::{OptionalActions, Termios, tcgetattr, tcsetattr};
use size::{get_terminal_size, poll_input, query_cursor_position};
pub use sizing::{HorizontalAlign, TextSizing, TextSizingSupport, VerticalAlign};
use std::io::{self, Read, Write};
pub use style::{Color, Rgb, Style, UnderlineStyle};
use tracing::{debug, trace};
#[allow(dead_code)]
pub use utils::osc52_copy_to_clipboard;

/// Escape sequence to enter the alternate screen buffer.
const ENTER_ALTERNATIVE_SCREEN: &str = "\x1b[?1049h";
/// Escape sequence to clear the screen and move cursor to home position.
const CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";
/// Escape sequence to exit the alternate screen buffer.
const EXIT_ALTERNATIVE_SCREEN: &str = "\x1b[?1049l";
/// Escape sequence to enable CSI-u mode (Kitty keyboard protocol).
const ENABLE_CSI_U: &str = "\x1b[=1u";
/// Escape sequence to disable CSI-u mode.
const DISABLE_CSI_U: &str = "\x1b[=0u";
/// Escape sequence to enable bracketed paste mode.
const ENABLE_BRACKETED_PASTE: &str = "\x1b[?2004h";
/// Escape sequence to disable bracketed paste mode.
const DISABLE_BRACKETED_PASTE: &str = "\x1b[?2004l";
/// Escape sequence to hide the cursor.
const HIDE_CURSOR: &str = "\x1b[?25l";
/// Escape sequence to show the cursor.
const SHOW_CURSOR: &str = "\x1b[?25h";

/// Timeout in milliseconds for waiting for escape sequence data.
/// This is the maximum time to wait after receiving an escape character
/// to determine if more bytes are coming (forming a complete sequence).
const ESC_TIMEOUT_MS: i32 = 50;
/// Maximum size for paste data in bytes. Pastes exceeding this size
/// will be discarded and re-read as individual key events.
const MAX_PASTE_SIZE: usize = 1024 * 1024;

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

#[allow(dead_code)]
impl<I: Read + AsFd, O: Write + AsFd> Terminal<I, O> {
    /// Creates a new terminal instance.
    ///
    /// This puts the terminal into raw mode, enters the alternate screen,
    /// enables bracketed paste mode, and enables the Kitty keyboard protocol.
    ///
    /// # Arguments
    ///
    /// * `input` - The input stream (usually stdin)
    /// * `output` - The output stream (usually stdout)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Getting terminal attributes fails
    /// - Setting raw mode fails
    /// - Writing initial escape sequences fails
    pub fn new(input: I, output: O) -> io::Result<Self> {
        // Save original terminal attributes
        let original = tcgetattr(&input).map_err(io::Error::from)?;

        // Configure raw mode (no echo, no canonical mode, etc.)
        let mut termios = original.clone();
        termios.make_raw();
        tcsetattr(&input, OptionalActions::Now, &termios).map_err(io::Error::from)?;

        // Initialize terminal with required modes
        let mut output = output;
        output.write_all(ENTER_ALTERNATIVE_SCREEN.as_bytes())?;
        output.write_all(CLEAR_SCREEN.as_bytes())?;
        output.write_all(ENABLE_CSI_U.as_bytes())?;
        output.write_all(ENABLE_BRACKETED_PASTE.as_bytes())?;
        output.flush()?;

        // Get initial terminal size
        let (rows, cols) = get_terminal_size().unwrap_or((24, 80));

        // Check if we're connected to a real TTY
        let is_tty = is_terminal::is_terminal(std::io::stdin());

        Ok(Self {
            input,
            output,
            original: Some(original),
            buffer: ByteBuffer::new(),
            paste_active: false,
            last_rows: rows,
            last_cols: cols,
            is_tty,
            flush: true,
        })
    }

    /// Creates a new terminal instance for testing without TTY features.
    ///
    /// This bypasses terminal attribute manipulation and doesn't attempt
    /// polling for input. Useful for unit tests.
    pub fn new_for_testing(input: I, output: O) -> Self {
        Self {
            input,
            output,
            original: None,
            buffer: ByteBuffer::new(),
            paste_active: false,
            last_rows: 24,
            last_cols: 80,
            is_tty: false,
            flush: true,
        }
    }

    /// Flushes the terminal output buffer.
    ///
    /// This is a no-op when the terminal is not in flush-mode.
    pub fn flush(&mut self) -> io::Result<()> {
        if self.flush {
            self.output.flush()?;
        }
        Ok(())
    }

    /// Clears the entire screen.
    pub fn clear_screen(&mut self) -> io::Result<()> {
        self.output.write_all(CLEAR_SCREEN.as_bytes())?;
        self.flush()
    }

    /// Enables or disables automatic output flushing.
    ///
    /// When disabled, output is buffered until explicitly flushed.
    /// Use `flush()` or `batch()` to force output.
    pub fn set_flush(&mut self, enabled: bool) {
        self.flush = enabled;
    }

    /// Executes a function with output flushing disabled.
    ///
    /// This is useful for batching multiple operations to reduce
    /// the number of system calls.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use urvim::terminal::{Terminal, Event, KeyCode};
    /// use std::io::{stdin, stdout};
    ///
    /// let mut terminal = Terminal::new(stdin(), stdout()).unwrap();
    /// terminal.batch(|t| {
    ///     t.set_cursor_position(0, 0)?;
    ///     t.write_text("Hello")?;
    ///     Ok(())
    /// }).unwrap();
    /// ```
    pub fn batch<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Terminal<I, O>) -> io::Result<()>,
    {
        let prev_flush = self.flush;
        self.flush = false;
        f(self)?;
        self.flush = prev_flush;
        self.output.flush()
    }

    /// Restores the terminal to its original state.
    ///
    /// This performs cleanup:
    /// 1. Disables CSI-u mode
    /// 2. Disables bracketed paste mode
    /// 3. Exits the alternate screen
    /// 4. Restores original terminal attributes
    ///
    /// This is called automatically when the terminal is dropped.
    pub fn restore(&mut self) -> io::Result<()> {
        self.output.write_all(DISABLE_CSI_U.as_bytes())?;
        self.output.flush()?;
        self.output.write_all(DISABLE_BRACKETED_PASTE.as_bytes())?;
        self.output.flush()?;
        self.output.write_all(EXIT_ALTERNATIVE_SCREEN.as_bytes())?;
        self.output.flush()?;
        if let Some(original) = &self.original {
            tcsetattr(&self.input, OptionalActions::Now, original).map_err(io::Error::from)
        } else {
            Ok(())
        }
    }

    /// Sets the cursor position.
    ///
    /// Coordinates are 1-indexed (1,1 is the top-left corner).
    ///
    /// # Arguments
    ///
    /// * `row` - The row (1-indexed)
    /// * `col` - The column (1-indexed)
    pub fn set_cursor_position(&mut self, row: u16, col: u16) -> io::Result<()> {
        let mut buf = [0u8; 16];
        let mut i = 0;
        // CSI row;colH - Cursor Position
        buf[i] = b'\x1b';
        i += 1;
        buf[i] = b'[';
        i += 1;

        i = write_decimal(row, &mut buf, i);
        buf[i] = b';';
        i += 1;
        i = write_decimal(col, &mut buf, i);
        buf[i] = b'H';
        i += 1;

        self.output.write_all(&buf[..i])?;
        self.flush()?;
        Ok(())
    }

    /// Queries and returns the current cursor position.
    ///
    /// This sends a Device Status Report (DSR) request to the terminal
    /// and waits for the response. The response format is `\x1b[rows;colsR`.
    ///
    /// Returns `(row, col)` as 1-indexed coordinates.
    pub fn get_cursor_position(&mut self) -> io::Result<(u16, u16)> {
        self.output.write_all(b"\x1b[6n")?;
        self.flush()?;
        query_cursor_position(&mut self.input, &mut self.output, false)
            .ok_or_else(|| io::Error::other("failed to get cursor position"))
    }

    /// Shows the cursor (makes it visible).
    pub fn show_cursor(&mut self) -> io::Result<()> {
        self.output.write_all(SHOW_CURSOR.as_bytes())?;
        self.flush()?;
        Ok(())
    }

    /// Hides the cursor (makes it invisible).
    pub fn hide_cursor(&mut self) -> io::Result<()> {
        self.output.write_all(HIDE_CURSOR.as_bytes())?;
        self.flush()?;
        Ok(())
    }

    /// Sets the cursor style.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use urvim::terminal::{Terminal, CursorStyle};
    /// use std::io::{stdin, stdout};
    ///
    /// let mut terminal = Terminal::new(stdin(), stdout()).unwrap();
    /// terminal.set_cursor_style(CursorStyle::BlinkingBlock)?;
    /// ```
    pub fn set_cursor_style(&mut self, style: CursorStyle) -> io::Result<()> {
        self.output.write_all(style.as_str().as_bytes())?;
        self.flush()?;
        Ok(())
    }

    /// Applies a style to subsequent text output.
    ///
    /// The style affects all text written after this call until
    /// `reset_style()` is called or a new style is applied.
    pub fn set_style(&mut self, style: &Style) -> io::Result<()> {
        style.write_escape_code(&mut self.output)?;
        self.flush()?;
        Ok(())
    }

    /// Resets all styling to default (clears all attributes).
    pub fn reset_style(&mut self) -> io::Result<()> {
        self.output.write_all(b"\x1b[0m")?;
        self.flush()?;
        Ok(())
    }

    /// Writes plain text to the terminal.
    ///
    /// This does not apply any styling; use `set_style()` first
    /// or use `write_styled_text()` for combined operation.
    pub fn write_text(&mut self, text: &str) -> io::Result<()> {
        self.output.write_all(text.as_bytes())?;
        self.flush()?;
        Ok(())
    }

    /// Copies text to the system clipboard.
    ///
    /// Uses the OSC 52 protocol, which is supported by most modern terminals.
    /// Note: Some terminals require specific configuration to enable this.
    pub fn copy_to_clipboard(&mut self, text: &str) -> io::Result<()> {
        let seq = osc52_copy_to_clipboard(text);
        self.output.write_all(&seq)?;
        self.flush()?;
        Ok(())
    }

    /// Detects the terminal's text sizing support.
    ///
    /// This probes the terminal by attempting to use text sizing
    /// and checking if the cursor position changes.
    ///
    /// Returns `TextSizingSupport::None` if unsupported,
    /// `TextSizingSupport::WidthOnly` if only width works,
    /// or `TextSizingSupport::Full` if full sizing is supported.
    pub fn detect_text_sizing_support(&mut self) -> io::Result<TextSizingSupport> {
        use sizing::TextSizingSupport::*;

        let old_pos = self.get_cursor_position()?;

        // Step 1: Get initial position (pos1)
        let pos1 = self.get_cursor_position()?;

        // Step 2: Send text sizing with width=2, then get position (pos2)
        self.output.write_all(b"\x1b]66;w=2; \x07")?;
        self.flush()?;
        let pos2 = self.get_cursor_position()?;

        // Step 3: Send text sizing with scale=2, then get position (pos3)
        self.output.write_all(b"\x1b]66;s=2; \x07")?;
        self.flush()?;
        let pos3 = self.get_cursor_position()?;

        self.set_cursor_position(old_pos.0, old_pos.1)?;

        if pos2.1 == pos1.1 && pos3.1 == pos1.1 {
            return Ok(None);
        }
        if pos3.1 > pos2.1 {
            return Ok(Full);
        }
        Ok(WidthOnly)
    }

    /// Writes styled and/or sized text to the terminal.
    ///
    /// This is a convenience method that combines style application,
    /// optional text sizing, and text output in one call.
    ///
    /// # Arguments
    ///
    /// * `style` - Optional style to apply
    /// * `sizing` - Optional text sizing (Kitty protocol)
    /// * `text` - The text to write
    pub fn write_styled_text<S: AsRef<str>>(
        &mut self,
        style: Option<&Style>,
        sizing: Option<&TextSizing>,
        text: S,
    ) -> io::Result<()> {
        if let Some(s) = style {
            s.write_escape_code(&mut self.output)?;
        }
        if let Some(s) = sizing {
            s.write_escape_code(&mut self.output)?;
        }
        self.output.write_all(text.as_ref().as_bytes())?;
        if sizing.is_some() {
            self.output.write_all(b"\x07")?;
        }
        self.flush()?;
        Ok(())
    }

    /// Reads the next input event from the terminal.
    ///
    /// This is the main input loop. It handles:
    /// - Key press events (including escape sequences for special keys)
    /// - Terminal resize events
    /// - Bracketed paste events
    /// - UTF-8 multi-byte character sequences
    ///
    /// The function blocks until an event is available.
    ///
    /// # Implementation Details
    ///
    /// For TTY terminals:
    /// 1. Poll for input availability with timeout
    /// 2. Check for resize events
    /// 3. Read available bytes
    /// 4. Parse escape sequences if escape character detected
    /// 5. Handle bracketed paste mode
    ///
    /// For non-TTY (pipe/file) input:
    /// - No polling; reads directly
    /// - Still parses escape sequences and handles paste
    pub fn read_event(&mut self) -> io::Result<Event> {
        // In TTY mode, use polling to detect input availability
        if self.is_tty {
            let input_fd = self.input.as_fd();

            loop {
                let pollfd = PollFd::new(&input_fd, PollFlags::IN);
                let mut fds = [pollfd];
                let poll_result = poll(&mut fds, 50);
                if let Err(e) = poll_result {
                    if e.kind() != io::ErrorKind::Interrupted {
                        return Err(e.into());
                    }
                    continue;
                }

                // Check for terminal resize after polling
                if let Some((rows, cols)) = get_terminal_size()
                    && (rows != self.last_rows || cols != self.last_cols)
                {
                    debug!(
                        "resize: {}x{} -> {}x{}",
                        self.last_rows, self.last_cols, rows, cols
                    );
                    self.last_rows = rows;
                    self.last_cols = cols;
                    return Ok(Event::Resize(rows, cols));
                }

                // No input available, continue polling
                if fds[0].revents().is_empty() {
                    continue;
                }

                break;
            }
        } else {
            // Non-TTY mode: check for resize without polling
            if let Some((rows, cols)) = get_terminal_size()
                && (rows != self.last_rows || cols != self.last_cols)
            {
                debug!(
                    "resize: {}x{} -> {}x{}",
                    self.last_rows, self.last_cols, rows, cols
                );
                self.last_rows = rows;
                self.last_cols = cols;
                return Ok(Event::Resize(rows, cols));
            }
        }

        // Check for leftover data from previous parse (e.g., incomplete sequence)
        if self.buffer.filled_len() > 0 {
            let event = parse_event_with_buffer(&mut self.buffer);
            trace!("buffered -> event: {:?}", event);
            return Ok(event);
        }

        // Check if we're in the middle of reading a bracketed paste
        if self.paste_active {
            return self.read_paste_event();
        }

        // Read the first byte to determine event type
        let mut first_byte = [0u8; 1];
        loop {
            match self.input.read(&mut first_byte) {
                Ok(0) => return Ok(KeyCode::Null.event()),
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        // Handle escape character - start of escape sequence
        if first_byte[0] == b'\x1b' {
            self.buffer.clear();
            self.buffer.push(first_byte[0]);

            // Determine if we should wait for more bytes (escape sequence)
            // or treat this as a plain Escape key
            let should_poll = self.is_tty && poll_input(&self.input, ESC_TIMEOUT_MS);
            let should_read = !self.is_tty || should_poll;

            if should_read {
                let mut temp_buf = [0u8; 64];
                loop {
                    let n = match self.input.read(&mut temp_buf) {
                        Ok(n) => n,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    };
                    if n == 0 {
                        break;
                    }
                    for &b in &temp_buf[..n] {
                        self.buffer.push(b);
                    }

                    // Limit buffer size to prevent runaway reads
                    if self.buffer.len() >= 6 {
                        break;
                    }
                    // Check if this looks like a partial CSI sequence
                    if self.buffer.len() >= 2 {
                        let seq = self.buffer.get_range(0, self.buffer.len());
                        // Must be \x1b[ followed by digits and semicolons only
                        if !seq.starts_with(b"\x1b[")
                            || !seq[3..].iter().all(|&b| b.is_ascii_digit() || b == b';')
                        {
                            break;
                        }
                    }
                }
            }

            trace!(
                "ESC read: buf.len={}, buf={:02x?}",
                self.buffer.len(),
                self.buffer.get_range(0, self.buffer.len())
            );

            // Check for bracketed paste start sequence: \x1b[200~
            if self.buffer.len() >= 6 {
                let paste_start = self.buffer.get_range(0, 6);
                if paste_start.starts_with(b"\x1b[200~") {
                    let remaining = self.buffer.get_range(6, self.buffer.len()).to_vec();
                    self.buffer.clear();
                    self.buffer.extend(&remaining);
                    self.paste_active = true;
                    return self.read_paste_event();
                }
            }

            // Parse as escape sequence
            if self.buffer.len() > 1 {
                let event = parse_event_with_buffer(&mut self.buffer);
                trace!("raw: {:02x?} -> event: {:?}", first_byte, event);
                return Ok(event);
            } else {
                // Single escape character - plain Escape key
                trace!("raw: {:02x?} -> event: Esc", first_byte);
                return Ok(KeyCode::Esc.event());
            }
        }

        // Handle high bytes (128+) - UTF-8 multi-byte sequences
        self.buffer.clear();
        self.buffer.push(first_byte[0]);

        if first_byte[0] >= 0x80 {
            let mut temp_buf = [0u8; 64];
            loop {
                // Poll for more bytes with timeout
                if !self.is_tty || poll_input(&self.input, ESC_TIMEOUT_MS) {
                    let n = match self.input.read(&mut temp_buf) {
                        Ok(n) => n,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(e) => {
                            debug!("read error: {:?}", e);
                            break;
                        }
                    };
                    if n == 0 {
                        break;
                    }
                    for &b in &temp_buf[..n] {
                        self.buffer.push(b);
                    }

                    // Check if we have a complete UTF-8 character
                    let data = self.buffer.get_range(0, self.buffer.len());
                    if let Ok(s) = std::str::from_utf8(data)
                        && s.chars().next().map(|c| c.len_utf8()) == Some(self.buffer.len())
                    {
                        break;
                    }
                    // Limit to prevent runaway reads
                    if self.buffer.len() >= 4 {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        let event = parse_event_with_buffer(&mut self.buffer);

        trace!("raw: {:02x?} -> event: {:?}", first_byte, event);

        Ok(event)
    }

    /// Reads a bracketed paste event.
    ///
    /// This is called internally when paste mode is active (`paste_active` is true).
    /// It reads bytes until the paste end marker (`\x1b[201~`) is found.
    ///
    /// The function:
    /// 1. Reads bytes into the buffer
    /// 2. Searches for the end marker
    /// 3. Extracts the paste content
    /// 4. Resets paste state and returns the content
    ///
    /// If the paste exceeds `MAX_PASTE_SIZE`, it is discarded and the
    /// function restarts normal event reading.
    fn read_paste_event(&mut self) -> io::Result<Event> {
        let paste_end_marker = b"\x1b[201~";
        let mut temp_buf = [0u8; 256];

        loop {
            // Safety check: limit paste size to prevent memory issues
            if self.buffer.len() > MAX_PASTE_SIZE {
                debug!(
                    "paste exceeded max size {} bytes, discarding",
                    MAX_PASTE_SIZE
                );
                self.buffer.clear();
                self.paste_active = false;
                return self.read_event();
            }

            // Check if end marker is in buffer
            if self.buffer.len() >= 6 {
                let data = self.buffer.get_range(0, self.buffer.len());
                if let Some(pos) = data.windows(6).position(|w| w == paste_end_marker) {
                    let content_start = 0;
                    let content_end = pos;
                    let content_bytes = self.buffer.get_range(content_start, content_end);

                    let paste_content = String::from_utf8_lossy(content_bytes).into_owned();

                    // Keep any remaining data for next read
                    let remaining_start = pos + 6;
                    let remaining = self
                        .buffer
                        .get_range(remaining_start, self.buffer.len())
                        .to_vec();
                    self.buffer.clear();
                    self.buffer.extend(&remaining);
                    self.paste_active = false;

                    trace!("paste: {} bytes", paste_content.len());
                    return Ok(Event::Paste(paste_content));
                }
            }

            // Read more data
            match self.input.read(&mut temp_buf) {
                Ok(0) => {
                    // EOF - cancel paste and restart
                    self.buffer.clear();
                    self.paste_active = false;
                    return self.read_event();
                }
                Ok(n) => {
                    for &b in &temp_buf[..n] {
                        self.buffer.push(b);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    // Read error - cancel paste
                    debug!("paste read error: {:?}", e);
                    self.buffer.clear();
                    self.paste_active = false;
                    return self.read_event();
                }
            }
        }
    }
}

impl<I: Read + AsFd, O: Write + AsFd> Drop for Terminal<I, O> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(test)]
mod test_helpers {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    pub struct TestBackend {
        input: Arc<Mutex<VecDeque<u8>>>,
        pub output: Arc<Mutex<Vec<u8>>>,
    }

    impl TestBackend {
        pub fn new(data: Vec<u8>) -> Self {
            Self {
                input: Arc::new(Mutex::new(VecDeque::from(data))),
                output: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub fn get_output(&self) -> Vec<u8> {
            self.output.lock().unwrap().clone()
        }
    }

    impl Read for TestBackend {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut input = self.input.lock().unwrap();
            if input.is_empty() {
                return Ok(0);
            }
            let mut i = 0;
            while i < buf.len() {
                match input.pop_front() {
                    Some(b) => {
                        buf[i] = b;
                        i += 1;
                    }
                    None => break,
                }
            }
            Ok(i)
        }
    }

    impl Write for TestBackend {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut output = self.output.lock().unwrap();
            output.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl AsFd for TestBackend {
        fn as_fd(&self) -> rustix::fd::BorrowedFd {
            panic!("TestBackend does not have a valid file descriptor")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_helpers::TestBackend;

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

        let mut terminal = create_terminal(b"Z".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('Z'))));

        let mut terminal = create_terminal(b" ".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char(' '))));
    }

    #[test]
    fn test_enter_key() {
        let mut terminal = create_terminal(b"\n".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Enter.event());

        let mut terminal = create_terminal(b"\r".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Enter.event());
    }

    #[test]
    fn test_tab_key() {
        let mut terminal = create_terminal(b"\t".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Tab.event());
    }

    #[test]
    fn test_backspace() {
        let mut terminal = create_terminal(b"\x7f".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Backspace.event());
    }

    #[test]
    fn test_null_byte() {
        let mut terminal = create_terminal(vec![0]);
        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Null.event());
    }

    #[test]
    fn test_escape_key() {
        let mut terminal = create_terminal(b"\x1b".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Esc.event());
    }

    #[test]
    fn test_ctrl_keys() {
        let mut terminal = create_terminal(b"\x01".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL))
        );

        let mut terminal = create_terminal(b"\x03".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Char('c'), Modifiers::CTRL))
        );

        let mut terminal = create_terminal(b"\x1a".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Char('z'), Modifiers::CTRL))
        );
    }

    #[test]
    fn test_arrow_keys() {
        let mut terminal = create_terminal(b"\x1b[A".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Up)));

        let mut terminal = create_terminal(b"\x1b[B".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Down)));

        let mut terminal = create_terminal(b"\x1b[C".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Right)));

        let mut terminal = create_terminal(b"\x1b[D".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Left)));
    }

    #[test]
    fn test_home_end_keys() {
        let mut terminal = create_terminal(b"\x1b[H".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Home)));

        let mut terminal = create_terminal(b"\x1b[F".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::End)));
    }

    #[test]
    fn test_function_keys_ss3() {
        let mut terminal = create_terminal(b"\x1bOP".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F1)));

        let mut terminal = create_terminal(b"\x1bOQ".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F2)));

        let mut terminal = create_terminal(b"\x1bOR".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F3)));

        let mut terminal = create_terminal(b"\x1bOS".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F4)));
    }

    #[test]
    fn test_function_keys_csi() {
        let mut terminal = create_terminal(b"\x1b[P".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F1)));

        let mut terminal = create_terminal(b"\x1b[Q".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F2)));

        let mut terminal = create_terminal(b"\x1b[R".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F3)));

        let mut terminal = create_terminal(b"\x1b[S".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F4)));
    }

    #[test]
    fn test_tilde_sequence_keys() {
        let mut terminal = create_terminal(b"\x1b[1~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Home)));

        let mut terminal = create_terminal(b"\x1b[3~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Delete)));

        let mut terminal = create_terminal(b"\x1b[4~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::End)));

        let mut terminal = create_terminal(b"\x1b[5~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::PageUp)));

        let mut terminal = create_terminal(b"\x1b[6~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::PageDown)));

        let mut terminal = create_terminal(b"\x1b[7~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Home)));

        let mut terminal = create_terminal(b"\x1b[8~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::End)));
    }

    #[test]
    fn test_unknown_escape_sequence() {
        let mut terminal = create_terminal(b"\x1b[Z".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Esc.event());

        let mut terminal = create_terminal(b"\x1bX".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Esc.event());
    }

    #[test]
    fn test_multiple_events() {
        let data = b"abc\x1b[A\n";
        let mut terminal = create_terminal(data.to_vec());

        assert_eq!(
            terminal.read_event().unwrap(),
            Event::Key(Key::new(KeyCode::Char('a')))
        );
        assert_eq!(
            terminal.read_event().unwrap(),
            Event::Key(Key::new(KeyCode::Char('b')))
        );
        assert_eq!(
            terminal.read_event().unwrap(),
            Event::Key(Key::new(KeyCode::Char('c')))
        );
        assert_eq!(
            terminal.read_event().unwrap(),
            Event::Key(Key::new(KeyCode::Up))
        );
        assert_eq!(terminal.read_event().unwrap(), KeyCode::Enter.event());
    }

    #[test]
    fn test_insert_key() {
        let mut terminal = create_terminal(b"\x1b[2~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Insert)));
    }

    #[test]
    fn test_function_keys_f5_f12() {
        let mut terminal = create_terminal(b"\x1b[15~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F5)));

        let mut terminal = create_terminal(b"\x1b[17~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F6)));

        let mut terminal = create_terminal(b"\x1b[18~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F7)));

        let mut terminal = create_terminal(b"\x1b[19~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F8)));

        let mut terminal = create_terminal(b"\x1b[20~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F9)));

        let mut terminal = create_terminal(b"\x1b[21~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F11)));

        let mut terminal = create_terminal(b"\x1b[23~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F12)));

        let mut terminal = create_terminal(b"\x1b[24~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F12)));
    }

    #[test]
    fn test_bracketed_paste() {
        let mut terminal = create_terminal(b"\x1b[200~hello\x1b[201~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Paste("hello".to_string()));
    }

    #[test]
    fn test_bracketed_paste_multiline() {
        let mut terminal = create_terminal(b"\x1b[200~line1\nline2\nline3\x1b[201~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Paste("line1\nline2\nline3".to_string()));
    }

    #[test]
    fn test_bracketed_paste_empty() {
        let mut terminal = create_terminal(b"\x1b[200~\x1b[201~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Paste("".to_string()));
    }

    #[test]
    fn test_key_after_paste() {
        let mut terminal = create_terminal(b"\x1b[200~paste\x1b[201~a".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Paste("paste".to_string()));

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('a'))));
    }

    #[test]
    fn test_escape_before_paste_start() {
        let mut terminal = create_terminal(b"\x1b[200~test\x1b[201~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Paste("test".to_string()));
    }

    #[test]
    fn test_multibyte_utf8_two_byte() {
        let mut terminal = create_terminal("é".as_bytes().to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('é'))));
    }

    #[test]
    fn test_multibyte_utf8_three_byte() {
        let mut terminal = create_terminal("日本語".as_bytes().to_vec());

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('日'))));

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('本'))));

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('語'))));
    }

    #[test]
    fn test_multibyte_utf8_four_byte_emoji() {
        let mut terminal = create_terminal("😀".as_bytes().to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('😀'))));
    }

    #[test]
    fn test_multibyte_utf8_emoji_after_ascii() {
        let mut terminal = create_terminal("a😀b".as_bytes().to_vec());

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('a'))));

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('😀'))));

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('b'))));
    }

    #[test]
    fn test_invalid_utf8_sequence_returns_null() {
        let mut terminal = create_terminal(vec![0x80]);
        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Null.event());
    }

    #[test]
    fn test_invalid_utf8_sequence_continues_input() {
        let mut terminal = create_terminal(vec![0x80, b'a']);

        let event = terminal.read_event().unwrap();
        assert_eq!(event, KeyCode::Null.event());

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('a'))));
    }

    #[test]
    fn test_resize_event() {
        let mut terminal = create_terminal(b"\x1b[24;80R".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Resize(24, 80));
    }

    #[test]
    fn test_resize_event_single_digit() {
        let mut terminal = create_terminal(b"\x1b[10;20R".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Resize(10, 20));
    }

    #[test]
    fn test_resize_event_minimum() {
        let mut terminal = create_terminal(b"\x1b[1;1R".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Resize(1, 1));
    }

    #[test]
    fn test_resize_event_large() {
        let mut terminal = create_terminal(b"\x1b[200;400R".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Resize(200, 400));
    }

    #[test]
    fn test_resize_after_key() {
        let mut terminal = create_terminal(b"a\x1b[24;80R".to_vec());

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::Char('a'))));

        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Resize(24, 80));
    }

    #[test]
    fn test_dsr_response_different_from_f3() {
        let mut terminal = create_terminal(b"\x1b[3R".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(event, Event::Key(Key::new(KeyCode::F3)));
    }

    #[test]
    fn test_modifiers_shift() {
        let mut terminal = create_terminal(b"\x1b[1;2A".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Up, Modifiers::SHIFT))
        );
    }

    #[test]
    fn test_modifiers_alt() {
        let mut terminal = create_terminal(b"\x1b[1;3A".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Up, Modifiers::ALT))
        );
    }

    #[test]
    fn test_modifiers_ctrl() {
        let mut terminal = create_terminal(b"\x1b[1;5A".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Up, Modifiers::CTRL))
        );
    }

    #[test]
    fn test_modifiers_ctrl_shift() {
        let mut terminal = create_terminal(b"\x1b[1;6A".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(
                KeyCode::Up,
                Modifiers::CTRL | Modifiers::SHIFT
            ))
        );
    }

    #[test]
    fn test_kitty_csi_u_shift_char() {
        let mut terminal = create_terminal(b"\x1b[97;2u".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Char('a'), Modifiers::SHIFT))
        );
    }

    #[test]
    fn test_kitty_csi_u_ctrl_char() {
        let mut terminal = create_terminal(b"\x1b[97;5u".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL))
        );
    }

    #[test]
    fn test_kitty_csi_u_alt_char() {
        let mut terminal = create_terminal(b"\x1b[97;3u".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Char('a'), Modifiers::ALT))
        );
    }

    #[test]
    fn test_kitty_csi_u_ctrl_shift_char() {
        let mut terminal = create_terminal(b"\x1b[97;6u".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(
                KeyCode::Char('a'),
                Modifiers::CTRL | Modifiers::SHIFT
            ))
        );
    }

    #[test]
    fn test_kitty_csi_u_shift_function_key() {
        let mut terminal = create_terminal(b"\x1b[11;2u".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::F1, Modifiers::SHIFT))
        );
    }

    #[test]
    fn test_kitty_csi_u_alt_function_key() {
        let mut terminal = create_terminal(b"\x1b[11;3u".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::F1, Modifiers::ALT))
        );
    }

    #[test]
    fn test_kitty_csi_u_escape_key() {
        let mut terminal = create_terminal(b"\x1b[1;2u".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Esc, Modifiers::SHIFT))
        );
    }

    #[test]
    fn test_kitty_csi_u_arrow_keys() {
        let mut terminal = create_terminal(b"\x1b[27;2u".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Left, Modifiers::SHIFT))
        );
    }

    #[test]
    fn test_modifiers_from_kitty_encoding() {
        assert_eq!(Modifiers::from_kitty_encoding(0), Modifiers::default());
        assert_eq!(Modifiers::from_kitty_encoding(2), Modifiers::SHIFT);
        assert_eq!(Modifiers::from_kitty_encoding(3), Modifiers::ALT);
        assert_eq!(Modifiers::from_kitty_encoding(5), Modifiers::CTRL);
        assert_eq!(
            Modifiers::from_kitty_encoding(6),
            Modifiers::CTRL | Modifiers::SHIFT
        );
        assert_eq!(
            Modifiers::from_kitty_encoding(7),
            Modifiers::CTRL | Modifiers::ALT
        );
    }

    #[test]
    fn test_modifiers_super() {
        let mut terminal = create_terminal(b"\x1b[1;9A".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Up, Modifiers::SUPER))
        );
    }

    #[test]
    fn test_csi_tilde_with_modifiers() {
        let mut terminal = create_terminal(b"\x1b[5;2~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::PageUp, Modifiers::SHIFT))
        );
    }

    #[test]
    fn test_csi_tilde_with_ctrl_modifier() {
        let mut terminal = create_terminal(b"\x1b[3;5~".to_vec());
        let event = terminal.read_event().unwrap();
        assert_eq!(
            event,
            Event::Key(Key::with_modifiers(KeyCode::Delete, Modifiers::CTRL))
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
    fn test_set_style_empty() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        let style = Style::new();
        terminal.set_style(&style).unwrap();

        assert_eq!(get_terminal_output(&terminal), b"");
    }

    #[test]
    fn test_reset_style() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal.reset_style().unwrap();

        assert_eq!(get_terminal_output(&terminal), b"\x1b[0m");
    }

    #[test]
    fn test_osc52_copy_to_clipboard_simple() {
        let seq = osc52_copy_to_clipboard("hello");
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"hello");
        let expected = format!("\x1b]52;c;{}\x07", encoded);
        assert_eq!(seq, expected.as_bytes());
    }

    #[test]
    fn test_osc52_copy_to_clipboard_empty() {
        let seq = osc52_copy_to_clipboard("");
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"");
        let expected = format!("\x1b]52;c;{}\x07", encoded);
        assert_eq!(seq, expected.as_bytes());
    }

    #[test]
    fn test_osc52_copy_to_clipboard_unicode() {
        let seq = osc52_copy_to_clipboard("Hello 世界");
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode("Hello 世界".as_bytes());
        let expected = format!("\x1b]52;c;{}\x07", encoded);
        assert_eq!(seq, expected.as_bytes());
    }

    #[test]
    fn test_osc52_copy_to_clipboard_special_chars() {
        let seq = osc52_copy_to_clipboard("Line1\nLine2\tTab");
        use base64::Engine;
        let encoded =
            base64::engine::general_purpose::STANDARD.encode("Line1\nLine2\tTab".as_bytes());
        let expected = format!("\x1b]52;c;{}\x07", encoded);
        assert_eq!(seq, expected.as_bytes());
    }

    #[test]
    fn test_terminal_copy_to_clipboard() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal.copy_to_clipboard("test").unwrap();

        let output = get_terminal_output(&terminal);
        assert!(output.starts_with(b"\x1b]52;c;"));
        assert!(output.ends_with(b"\x07"));
    }

    #[test]
    fn test_show_cursor() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal.show_cursor().unwrap();

        let output = get_terminal_output(&terminal);
        assert_eq!(output, b"\x1b[?25h");
    }

    #[test]
    fn test_hide_cursor() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal.hide_cursor().unwrap();

        let output = get_terminal_output(&terminal);
        assert_eq!(output, b"\x1b[?25l");
    }

    #[test]
    fn test_set_cursor_style_blinking_block() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal
            .set_cursor_style(CursorStyle::BlinkingBlock)
            .unwrap();

        let output = get_terminal_output(&terminal);
        assert_eq!(output, b"\x1b[0 q");
    }

    #[test]
    fn test_set_cursor_style_steady_block() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal.set_cursor_style(CursorStyle::SteadyBlock).unwrap();

        let output = get_terminal_output(&terminal);
        assert_eq!(output, b"\x1b[2 q");
    }

    #[test]
    fn test_set_cursor_style_blinking_underline() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal
            .set_cursor_style(CursorStyle::BlinkingUnderline)
            .unwrap();

        let output = get_terminal_output(&terminal);
        assert_eq!(output, b"\x1b[3 q");
    }

    #[test]
    fn test_set_cursor_style_steady_underline() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal
            .set_cursor_style(CursorStyle::SteadyUnderline)
            .unwrap();

        let output = get_terminal_output(&terminal);
        assert_eq!(output, b"\x1b[4 q");
    }

    #[test]
    fn test_set_cursor_style_blinking_bar() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal.set_cursor_style(CursorStyle::BlinkingBar).unwrap();

        let output = get_terminal_output(&terminal);
        assert_eq!(output, b"\x1b[5 q");
    }

    #[test]
    fn test_set_cursor_style_steady_bar() {
        let backend = TestBackend::new(Vec::new());
        let output_backend = TestBackend::new(Vec::new());
        let mut terminal = Terminal::new_for_testing(backend, output_backend);

        terminal.set_cursor_style(CursorStyle::SteadyBar).unwrap();

        let output = get_terminal_output(&terminal);
        assert_eq!(output, b"\x1b[6 q");
    }

    #[test]
    fn test_cursor_style_name() {
        assert_eq!(CursorStyle::BlinkingBlock.name(), "Blinking Block");
        assert_eq!(CursorStyle::SteadyBlock.name(), "Steady Block");
        assert_eq!(CursorStyle::BlinkingUnderline.name(), "Blinking Underline");
        assert_eq!(CursorStyle::SteadyUnderline.name(), "Steady Underline");
        assert_eq!(CursorStyle::BlinkingBar.name(), "Blinking Bar");
        assert_eq!(CursorStyle::SteadyBar.name(), "Steady Bar");
    }
}
